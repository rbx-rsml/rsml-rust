use crate::types::Range;

use crate::lexer::{DECLARATION_NAMES, Lexer, TOKEN_KIND_CONSTRUCT_DELIMITERS, Token, TokenKind};
use crate::list::TokenKindList;
use crate::range_from_span::RangeFromSpan;
use crate::{node_token_matches, token_kind_list};

mod advance;
mod datatype;
mod declaration;
mod parse_error;
mod rule;
pub mod types;

use parse_error::{ParseError, ParseErrorMessage};
pub use types::*;

#[macro_export]
macro_rules! node_token_matches {
    ($node:ident, Some($( $name:ident )|*)) => {
        matches!($node, Some($crate::parser::types::Node { token: $crate::lexer::SpannedToken (_, $( $crate::lexer::Token::$name )|*, _), .. }))
    };

    ($node:ident, $( $name:ident )|*) => {
        matches!($node, $crate::parser::types::Node { token: $crate::lexer::SpannedToken (_, $( $crate::lexer::Token::$name )|*, _), .. })
    };

    ($node:ident, Some($( $name:ident($( $args:pat ),*) )|*)) => {
        matches!($node, Some($crate::parser::types::Node { token: $crate::lexer::SpannedToken(_, $( $crate::lexer::Token::$name($( $args ),*) )|*, _), .. }))
    };

    ($node:ident, $( $name:ident($( $args:pat ),*) )|*) => {
        matches!($node, $crate::parser::types::Node { token: $crate::lexer::SpannedToken(_, $( $crate::lexer::Token::$name($( $args ),*) )|*, _), .. })
    };
}

pub struct Parser<'a> {
    pub lexer: Lexer<'a>,
    pub(crate) last_token_end: usize,

    pub ast: Vec<Construct<'a>>,
    pub ast_errors: AstErrors,

    pub did_advance: bool,
}

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> ParsedRsml<'a> {
        let mut parser = Self {
            lexer,
            last_token_end: 0,

            ast: Vec::new(),
            ast_errors: AstErrors::new(),

            did_advance: false,
        };

        parser.parse_loop(|parser, mut node| {
            node = parser.parse_macro(node).handle_construct(&mut parser.ast)?;
            node = parser
                .parse_macro_call(node)
                .handle_construct(&mut parser.ast)?;

            node = parser
                .parse_derive(node)
                .handle_construct(&mut parser.ast)?;

            node = parser
                .parse_priority(node)
                .handle_construct(&mut parser.ast)?;

            node = parser.parse_name(node).handle_construct(&mut parser.ast)?;

            node = parser.parse_tween(node).handle_construct(&mut parser.ast)?;

            node = parser
                .parse_static_token_assignment(node)
                .handle_construct(&mut parser.ast)?;

            node = parser
                .parse_token_assignment(node)
                .handle_construct(&mut parser.ast)?;

            node = parser
                .parse_property_assignment_or_rule_scope(node)
                .handle_construct(&mut parser.ast)?;
            node = parser
                .parse_rule_scope_selector_begin(node)
                .handle_construct(&mut parser.ast)?;

            node = parser.parse_invalid_declaration(node)?;

            node = parser.parse_none(node).handle_construct(&mut parser.ast)?;

            Some(node)
        });

        ParsedRsml {
            ast: parser.ast,
            ast_errors: parser.ast_errors,
            rope: parser.lexer.rope,
        }
    }

    pub fn range_from_span(&self, span: (usize, usize)) -> Range {
        Range::from_span(&self.lexer.rope, span)
    }

    fn parse_assignment(&mut self, node: Node<'a>) -> Parsed<'a> {
        let middle_node =
            match self.advance_until(token_kind_list![Equals], &TOKEN_KIND_CONSTRUCT_DELIMITERS) {
                Some(Ok(node)) => node,
                Some(Err(node)) => return Parsed(Some(node), None),
                None => return Parsed(None, None),
            };

        let left_node = node;

        let node = self.advance_without_flags();
        self.did_advance = true;

        let (node_status, body_nodes) = self.parse_datatype(node, TOKEN_KIND_CONSTRUCT_DELIMITERS);
        let body_nodes = body_nodes.map(|x| Box::new(x));

        let terminator = match node_status {
            NodeStatus::Exists => match self.advance_until(
                token_kind_list![SemiColon],
                &TOKEN_KIND_CONSTRUCT_DELIMITERS,
            ) {
                Some(Ok(node)) => node,
                Some(Err(node)) => {
                    return Parsed(
                        Some(node),
                        Some(Construct::Assignment {
                            left: left_node,
                            middle: Some(middle_node),
                            right: body_nodes,
                            terminator: None,
                        }),
                    );
                }
                None => {
                    return Parsed(
                        None,
                        Some(Construct::Assignment {
                            left: left_node,
                            middle: Some(middle_node),
                            right: body_nodes,
                            terminator: None,
                        }),
                    );
                }
            },

            NodeStatus::Err(node) => {
                if node_token_matches!(node, SemiColon) {
                    node
                } else {
                    let construct = Construct::Assignment {
                        left: left_node,
                        middle: Some(middle_node),
                        right: body_nodes,
                        terminator: None,
                    };

                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::SemiColon.name())),
                        },
                        self.range_from_span(clamp_span_to_end(construct.end())),
                    );

                    return Parsed(Some(node), Some(construct));
                }
            }

            NodeStatus::None => {
                let construct = Construct::Assignment {
                    left: left_node,
                    middle: Some(middle_node),
                    right: body_nodes,
                    terminator: None,
                };

                self.ast_errors.push(
                    ParseError::MissingToken {
                        msg: Some(ParseErrorMessage::Expected(TokenKind::SemiColon.name())),
                    },
                    self.range_from_span(clamp_span_to_end(construct.end())),
                );

                return Parsed(None, Some(construct));
            }
        };

        Parsed(
            self.advance(),
            Some(Construct::Assignment {
                left: left_node,
                middle: Some(middle_node),
                right: body_nodes,
                terminator: Some(terminator),
            }),
        )
    }

    pub(crate) fn parse_static_token_assignment(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, StaticTokenIdentifier(_)) {
            return Parsed(Some(node), None);
        };
        self.parse_assignment(node)
    }

    pub(crate) fn parse_token_assignment(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, TokenIdentifier(_)) {
            return Parsed(Some(node), None);
        };
        self.parse_assignment(node)
    }

    pub(crate) fn parse_invalid_declaration(&mut self, node: Node<'a>) -> Option<Node<'a>> {
        let token = &node.token;

        let name = if let Token::InvalidDeclaration(x) = token.value() {
            x
        } else {
            return Some(node);
        };

        self.ast_errors.push(
            ParseError::UnexpectedTokens {
                msg: Some(ParseErrorMessage::correction(
                    name.as_deref().map(|x| format!("@{x}")),
                    self.range_from_span((token.start(), token.end())),
                    &DECLARATION_NAMES,
                )),
            },
            self.range_from_span((token.start(), token.end())),
        );

        self.advance()
    }

    pub(crate) fn parse_none(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, None) {
            return Parsed(Some(node), None);
        };

        Parsed(self.advance(), Some(Construct::None { node }))
    }

    pub(crate) fn parse_loop<F: Fn(&mut Self, Node<'a>) -> Option<Node<'a>>>(
        &mut self,
        routine: F,
    ) -> Option<Node<'a>> {
        let mut node = self.advance_without_flags().update_last_token_end(self)?;
        let token = &node.token;

        let mut error_span: Option<(usize, usize)> = if matches!(token.value(), Token::Error) {
            Some((token.start(), token.end()))
        } else {
            None
        };

        loop {
            let Some(next_node) = routine(self, node) else {
                break;
            };
            node = next_node;

            if self.did_advance {
                self.did_advance = false;

                if let Some((error_span_start, error_span_end)) = error_span {
                    self.ast_errors.push(
                        ParseError::UnexpectedTokens { msg: None },
                        self.range_from_span((error_span_start, error_span_end)),
                    );
                }
            } else {
                let token = &node.token;

                if let Some((error_span_start, _)) = error_span {
                    error_span = Some((error_span_start, token.end()))
                } else {
                    error_span = Some((token.start(), token.end()))
                }

                let Some(next_node) = self.advance_without_flags().update_last_token_end(self)
                else {
                    break;
                };

                node = next_node;
            }
        }

        if let Some((error_span_start, error_span_end)) = error_span {
            self.ast_errors.push(
                ParseError::UnexpectedTokens { msg: None },
                self.range_from_span((error_span_start, error_span_end)),
            );
        }

        None
    }

    #[cfg(test)]
    pub fn parse_source(source: &'a str) -> ParsedRsml<'a> {
        let lexer = crate::lexer::Lexer::new(source);
        Self::new(lexer)
    }

    pub(crate) fn parse_loop_inner<F: FnMut(&mut Self, Node<'a>) -> Option<(Node<'a>, bool)>>(
        &mut self,
        mut node: Node<'a>,
        mut routine: F,
    ) -> (Option<Node<'a>>, ParseEndedReason) {
        let last_did_advance = self.did_advance;
        self.did_advance = false;

        let mut error_span: Option<(usize, usize)> = None;

        loop {
            let Some(parsed) = routine(self, node) else {
                self.did_advance = last_did_advance;
                return (None, ParseEndedReason::Eof);
            };
            node = parsed.0;

            if self.did_advance {
                self.did_advance = false;

                if let Some((error_span_start, error_span_end)) = error_span {
                    self.ast_errors.push(
                        ParseError::UnexpectedTokens { msg: None },
                        self.range_from_span((error_span_start, error_span_end)),
                    );
                }

                if parsed.1 {
                    return (Some(node), ParseEndedReason::Manual);
                }
            } else {
                if parsed.1 {
                    if let Some((error_span_start, error_span_end)) = error_span {
                        self.ast_errors.push(
                            ParseError::UnexpectedTokens { msg: None },
                            self.range_from_span((error_span_start, error_span_end)),
                        );
                    }

                    return (Some(node), ParseEndedReason::Manual);
                }

                let token = &node.token;
                if let Some((error_span_start, _)) = error_span {
                    error_span = Some((error_span_start, token.end()))
                } else {
                    error_span = Some((token.start(), token.end()))
                }

                let Some(next_node) = self.advance_without_flags().update_last_token_end(self)
                else {
                    break;
                };
                node = next_node;
            }
        }

        if let Some((error_span_start, error_span_end)) = error_span {
            self.ast_errors.push(
                ParseError::UnexpectedTokens { msg: None },
                self.range_from_span((error_span_start, error_span_end)),
            );
        }

        self.did_advance = last_did_advance;

        (Some(node), ParseEndedReason::Eof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unary_minus_in_udim2_expression() {
        let source = r#"$Size = udim2(-20px + 100%, -20px + 100%);"#;
        let parsed = Parser::parse_source(source);

        assert!(parsed.ast_errors.0.is_empty(), "Expected no parse errors, got: {:?}", parsed.ast_errors.0);
        insta::assert_debug_snapshot!(parsed.ast);
    }
}
