use std::{collections::HashSet, mem::discriminant, sync::LazyLock};

use crate::lexer::{MultilineString, SpannedToken, Token, TokenKind};
use crate::list::TokenKindList;
use crate::parser::parse_error::{ParseError, ParseErrorMessage};
use crate::parser::types::*;
use crate::parser::RsmlParser;

type SymResult<T> = Result<T, T>;

impl<'a> RsmlParser<'a> {
    pub(crate) fn next_token(&mut self) -> Option<SpannedToken<'a>> {
        self.lexer.next()
    }

    pub(crate) fn handle_multiline_string_error(
        &mut self,
        token: &SpannedToken,
        expected_nestedness: usize
    ) {
        self.ast_errors.push(
            ParseError::MissingToken {
                msg: Some(ParseErrorMessage::Expected(&format!("\"]{}]\"", "=".repeat(expected_nestedness))))
            },
            self.range_from_span(clamp_span_to_end(token.end()))
        )
    }

    pub(crate) fn next_node(&mut self) -> Option<Node<'a>> {
        let mut token = self.next_token()?;

        match token.value() {
            Token::CommentMulti(MultilineString { nestedness: Err(expected_nestedness), .. }) => {
                self.handle_multiline_string_error(&token, *expected_nestedness)
            },

            Token::CommentSingle(_) | Token::CommentMulti(MultilineString { nestedness: Ok(_), .. }) => (),

            _ => return Some(Node {
                token: token,
                leading_trivia: None
            })
        }

        let mut leading_trivia = vec![ token ];

        loop {
            let Some(next_token) = self.next_token() else {
                return Some(Node {
                    token: SpannedToken::new(self.last_token_end, Token::None, self.last_token_end),
                    leading_trivia: Some(leading_trivia)
                })
            };
            token = next_token;

            match token.value() {
                Token::CommentMulti(MultilineString { nestedness: Err(expected_nestedness), .. }) => {
                    self.handle_multiline_string_error(&token, *expected_nestedness);

                    leading_trivia.push(token);
                },

                Token::CommentSingle(_) | Token::CommentMulti(MultilineString { nestedness: Ok(_), .. }) =>
                    leading_trivia.push(token),

                _ => return Some(Node {
                    token: token,
                    leading_trivia: Some(leading_trivia)
                })
            }
        }
    }

    /// Advances to the next valid node. Does not update the `did_advance` or `last_token_end` flags.
    pub(crate) fn advance_without_flags<'b>(
        &mut self
    ) -> Option<Node<'a>> {
        match self.next_node()? {
            Node { token: SpannedToken (span_start, Token::Error, mut span_end), .. } => loop {
                match self.next_node() {
                    Some(Node { token: SpannedToken (_, Token::Error, next_span_end), .. }) => span_end = next_span_end,

                    node => {
                        self.ast_errors.push(
                            ParseError::UnexpectedTokens { msg: None },
                            self.range_from_span((span_start, span_end))
                        );

                        break node
                    }
                }
            },

            node => Some(node)
        }
    }

    /// Advances to the next valid node.
    pub fn advance(&mut self) -> Option<Node<'a>> {
        let node = self.advance_without_flags()
            .update_last_token_end(self);
        self.did_advance = true;

        node
    }

    pub(crate) fn advance_until_core_loop<const N: usize>(
        &mut self,
        allow_list: &TokenKindList<N>,
        construct_delimiters: &LazyLock<HashSet<TokenKind>>,
        span_start: usize, mut span_end: usize
    ) -> Option<SymResult<Node<'a>>> {
        loop {
            match self.next_node() {
                Some(Node { token: SpannedToken (_, Token::Error, next_span_end), .. }) => span_end = next_span_end,

                Some(node) => {
                    let token = &node.token;
                    let token_kind = &token.value().kind();

                    if allow_list.has_discriminant(&discriminant(token_kind)) {
                        self.ast_errors.push(
                            ParseError::UnexpectedTokens { msg: None },
                            self.range_from_span((span_start, span_end))
                        );

                        self.last_token_end = token.end();

                        break Some(Ok(node))

                    } else if construct_delimiters.contains(token_kind) {
                        self.ast_errors.push(
                            ParseError::UnexpectedTokens {
                                msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                            },
                            self.range_from_span((span_start, span_end))
                        );

                        break Some(Err(node))

                    } else {
                        span_end = token.end()
                    }
                },

                None => {
                    self.ast_errors.push(
                        ParseError::UnexpectedTokens {
                            msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                        },
                        self.range_from_span((span_start, span_end))
                    );

                    break None
                }
            }
        }
    }

    /// Advances to the next valid node which has a token in the allow list. Does not set the `did_advance` flag.
    pub(crate) fn advance_until_without_flag<const N: usize>(
        &mut self,
        allow_list: &TokenKindList<N>,
        construct_delimiters: &LazyLock<HashSet<TokenKind>>
    ) -> Option<SymResult<Node<'a>>> {
        match self.next_node() {
            Some(Node { token: SpannedToken (span_start, Token::Error, span_end), .. }) => {
                self.advance_until_core_loop(allow_list, construct_delimiters, span_start, span_end)
            },

            Some(node) => {
                let token = &node.token;
                let token_kind = &token.value().kind();

                if allow_list.has_discriminant(&discriminant(&token_kind)) {
                    self.last_token_end = token.end();

                    Some(Ok(node))

                } else if construct_delimiters.contains(token_kind) {
                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                        },
                        self.range_from_span(clamp_span_to_end(self.last_token_end))
                    );

                    Some(Err(node))

                } else {
                    self.advance_until_core_loop(allow_list, construct_delimiters, token.start(), token.end())
                }
            },

            None => {
                self.ast_errors.push(
                    ParseError::MissingToken {
                        msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                    },
                    self.range_from_span(clamp_span_to_end(self.last_token_end))
                );

                None
            }
        }
    }

    /// Advances to the next valid node which has a token in the allow list.
    pub(crate) fn advance_until<const N: usize>(
        &mut self,
        allow_list: &TokenKindList<N>,
        construct_delimiters: &LazyLock<HashSet<TokenKind>>
    ) -> Option<SymResult<Node<'a>>> {
        let next = self.advance_until_without_flag(allow_list, construct_delimiters);
        self.did_advance = true;
        next
    }

    pub(crate) fn node_is_kind_else_advance_until<const N: usize>(
        &mut self,
        node: Node<'a>,
        allow_list: &TokenKindList<N>,
        construct_delimiters: &LazyLock<HashSet<TokenKind>>
    ) -> Option<SymResult<Node<'a>>> {
        if allow_list.has_discriminant(&node.token.value().discriminant()) { return Some(Ok(node)) };

        if construct_delimiters.contains(&node.token.value().kind()) {
            self.ast_errors.push(
                ParseError::MissingToken {
                    msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                },
                self.range_from_span(clamp_span_to_end(self.last_token_end))
            );

            return Some(Err(node))
        }

        let last_token = node.token;

        match self.next_node() {
            Some(Node { token: SpannedToken (_, Token::Error, span_end), .. }) => {
                self.advance_until_core_loop(allow_list, construct_delimiters, last_token.start(), span_end)
            },

            Some(node) => {
                let token = &node.token;
                let token_kind = &token.value().kind();

                if allow_list.has_discriminant(&discriminant(&token_kind)) {
                    self.ast_errors.push(
                        ParseError::UnexpectedTokens { msg: None },
                        self.range_from_span(last_token.span())
                    );

                    self.last_token_end = token.end();

                    Some(Ok(node))

                } else if construct_delimiters.contains(token_kind) {
                    self.ast_errors.push(
                        ParseError::UnexpectedTokens {
                            msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                        },
                        self.range_from_span(last_token.span())
                    );

                    Some(Err(node))

                } else {
                    self.advance_until_core_loop(allow_list, construct_delimiters, last_token.start(), token.end())
                }
            },

            None => {
                self.ast_errors.push(
                    ParseError::UnexpectedTokens {
                        msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                    },
                    self.range_from_span(last_token.span())
                );

                None
            }
        }

    }

    pub(crate) fn optional_node_is_kind_else_advance_until<const N: usize>(
        &mut self,
        node: Option<Node<'a>>,
        allow_list: &TokenKindList<N>,
        construct_delimiters: &LazyLock<HashSet<TokenKind>>
    ) -> Option<SymResult<Node<'a>>> {
        match node {
            Some(node) => self.node_is_kind_else_advance_until(node, allow_list, construct_delimiters),

            None => {
                self.ast_errors.push(
                    ParseError::MissingToken {
                        msg: allow_list.to_string().as_deref().map(|x| ParseErrorMessage::Expected(x))
                    },
                    self.range_from_span(clamp_span_to_end(self.last_token_end))
                );

                None
            }
        }
    }
}
