use crate::{node_token_matches, token_kind_list};
use crate::lexer::{SpannedToken, Token, TokenKind, TOKEN_KIND_CONSTRUCT_DELIMITERS};
use crate::list::{Stringified, TokenKindList};
use crate::parser::parse_error::{ParseError, ParseErrorMessage};
use crate::parser::types::*;
use crate::parser::Parser;

impl<'a> Parser<'a> {
    pub(crate) fn parse_property_assignment_or_rule_scope(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, Identifier(_)) { return Parsed (Some(node), None) };

        let middle_node = match self.advance_until(
            &token_kind_list!(
                "property assignment, selector part or rule body", [
                    Equals, ScopeOpen, Identifier, NameSelector,
                    TagSelectorOrEnumPart, StateSelectorOrEnumPart,
                    PseudoSelector, Comma, ChildrenSelector, DescendantsSelector,
                    MacroCallIdentifier
                ]
            ),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => return Parsed (Some(node), None),
            None => return Parsed (None, None)
        };

        let middle_token_value = middle_node.token.value();

        // We switch to parsing a selector if the token is not an equals sign.
        if !matches!(middle_token_value, Token::Equals) {
            return match middle_token_value {
                Token::ScopeOpen => self.parse_rule_scope_body(middle_node, Some(vec![SelectorNode::Token(node)])),

                Token::Comma => {
                    let token = middle_node.token.clone();
                    self.parse_rule_scope_selector(token, vec![SelectorNode::Token(node), SelectorNode::Token(middle_node)], false)
                },

                Token::MacroCallIdentifier(_) => {
                    let (next_node, selector_node) = self.parse_macro_call_in_selector(middle_node);
                    let mut selectors = vec![SelectorNode::Token(node), selector_node];

                    match next_node {
                        Some(next) => {
                            let token = next.token.clone();
                            if node_token_matches!(next, ScopeOpen) {
                                self.parse_rule_scope_body(next, Some(selectors))
                            } else {
                                selectors.push(SelectorNode::Token(next));
                                self.parse_rule_scope_selector(token, selectors, true)
                            }
                        },
                        None => Parsed (None, Some(Construct::Rule { selectors: Some(selectors), body: None }))
                    }
                },

                _ => {
                    let token = middle_node.token.clone();
                    self.parse_rule_scope_selector(token, vec![SelectorNode::Token(node), SelectorNode::Token(middle_node)], true)
                }
            }
        }

        let left_node = node;

        let node = self.advance_without_flags();
        self.did_advance = true;

        let (node_status, body_nodes) =
            self.parse_datatype(node, TOKEN_KIND_CONSTRUCT_DELIMITERS);
        let body_nodes = body_nodes.map(|x| Box::new(x));

        let terminator = match node_status {
            NodeStatus::Exists => match self.advance_until(token_kind_list![ SemiColon ], &TOKEN_KIND_CONSTRUCT_DELIMITERS) {
                Some(Ok(node)) => node,
                Some(Err(node)) => return Parsed (Some(node), Some(Construct::Assignment {
                    left: left_node, middle: Some(middle_node), right: body_nodes, terminator: None
                })),
                None => return Parsed (None, Some(Construct::Assignment {
                    left: left_node, middle: Some(middle_node), right: body_nodes, terminator: None
                })),
            },

            NodeStatus::Err(node) => {
                if node_token_matches!(node, SemiColon) {
                    node

                } else {
                    let construct = Construct::Assignment {
                        left: left_node, middle: Some(middle_node), right: body_nodes, terminator: None
                    };

                    self.ast_errors.push(
                        ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::SemiColon.name())) },
                        self.range_from_span(clamp_span_to_end(construct.end()))
                    );

                    return Parsed (Some(node), Some(construct))
                }
            },

            NodeStatus::None => {
                let construct = Construct::Assignment {
                    left: left_node, middle: Some(middle_node), right: body_nodes, terminator: None
                };

                self.ast_errors.push(
                    ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::SemiColon.name())) },
                    self.range_from_span(clamp_span_to_end(construct.end()))
                );

                return Parsed (None, Some(construct))
            }
        };

        Parsed (self.advance(), Some(Construct::Assignment {
            left: left_node, middle: Some(middle_node), right: body_nodes, terminator: Some(terminator)
        }))
    }

    pub(crate) fn parse_rule_scope_selector_begin(&mut self, node: Node<'a>) -> Parsed<'a> {
        match node.token.value() {
            Token::MacroCallIdentifier(_) => {
                let (next_node, selector_node) = self.parse_macro_call_in_selector(node);
                let mut selectors = vec![selector_node];

                match next_node {
                    Some(next) => {
                        let token = next.token.clone();
                        if node_token_matches!(next, ScopeOpen) {
                            self.parse_rule_scope_body(next, Some(selectors))
                        } else {
                            selectors.push(SelectorNode::Token(next));
                            self.parse_rule_scope_selector(token, selectors, true)
                        }
                    },
                    None => Parsed (None, Some(Construct::Rule { selectors: Some(selectors), body: None }))
                }
            },

            Token::NameSelector(_) | Token::TagSelectorOrEnumPart(_) |
            Token::StateSelectorOrEnumPart(_) | Token::PseudoSelector(_) |
            Token::QuerySelector(_) | Token::ChildrenSelector |
            Token::DescendantsSelector => {
                let token = node.token.clone();
                self.parse_rule_scope_selector(token, vec![SelectorNode::Token(node)], true)
            },

            Token::ScopeOpen => self.parse_rule_scope_body(node, None),

            _ => Parsed(Some(node), None)
        }
    }

    /// Parses rule scope selectors. When `comma_allowed` is true, also accepts
    /// comma as a valid token (the "delimited" path). When false, expects only
    /// selector parts and "{".
    pub(crate) fn parse_rule_scope_selector(
        &mut self, last_token: SpannedToken<'a>, selectors: Vec<SelectorNode<'a>>,
        comma_allowed: bool
    ) -> Parsed<'a> {
        let (node, selectors) = self.parse_selector_tokens(last_token, selectors, comma_allowed);

        match node {
            Some(node) if node_token_matches!(node, ScopeOpen) => {
                self.parse_rule_scope_body(node, Some(selectors))
            },
            Some(node) => Parsed (Some(node), Some(Construct::Rule { selectors: Some(selectors), body: None })),
            None => Parsed (None, Some(Construct::Rule { selectors: Some(selectors), body: None })),
        }
    }

    /// Collects selector tokens until a scope delimiter (`{` or `}`) is reached.
    /// Returns the terminator node and collected selectors.
    pub(crate) fn parse_selector_tokens(
        &mut self,
        last_token: SpannedToken<'a>,
        mut selectors: Vec<SelectorNode<'a>>,
        comma_allowed: bool,
    ) -> (Option<Node<'a>>, Vec<SelectorNode<'a>>) {
        let result = if comma_allowed {
            self.advance_until(token_kind_list!("selector part", [
                Identifier, NameSelector, TagSelectorOrEnumPart, StateSelectorOrEnumPart, PseudoSelector,
                QuerySelector, ChildrenSelector, DescendantsSelector, MacroCallIdentifier, ScopeOpen, ScopeClose, Comma
            ]), &TOKEN_KIND_CONSTRUCT_DELIMITERS)
        } else {
            self.advance_until(token_kind_list!("selector part", [
                Identifier, NameSelector, TagSelectorOrEnumPart, StateSelectorOrEnumPart, PseudoSelector,
                QuerySelector, ChildrenSelector, DescendantsSelector, MacroCallIdentifier, ScopeOpen, ScopeClose
            ]), &TOKEN_KIND_CONSTRUCT_DELIMITERS)
        };

        let node = match result {
            Some(Ok(node)) => node,
            Some(Err(node)) => return (Some(node), selectors),
            None => return (None, selectors),
        };

        self.handle_hierarchy_selector_without_part(&last_token, &node.token);

        if node_token_matches!(node, ScopeOpen) || node_token_matches!(node, ScopeClose) {
            if !comma_allowed && matches!(last_token.value(), Token::Comma) {
                self.ast_errors.push(
                    ParseError::UnexpectedTokens { msg: None },
                    self.range_from_span(last_token.span())
                );
            }
            return (Some(node), selectors);
        }

        if node_token_matches!(node, MacroCallIdentifier(_)) {
            let (next_node, selector_node) = self.parse_macro_call_in_selector(node);
            selectors.push(selector_node);

            return match next_node {
                Some(next) => {
                    if node_token_matches!(next, ScopeOpen) || node_token_matches!(next, ScopeClose) {
                        (Some(next), selectors)
                    } else {
                        let token = next.token.clone();
                        selectors.push(SelectorNode::Token(next));
                        if comma_allowed {
                            match token.value() {
                                Token::Comma => self.parse_selector_tokens(token, selectors, false),
                                _ => self.parse_selector_tokens(token, selectors, true)
                            }
                        } else {
                            self.parse_selector_tokens(token, selectors, true)
                        }
                    }
                },
                None => (None, selectors)
            };
        }

        let token = node.token.clone();
        selectors.push(SelectorNode::Token(node));

        if comma_allowed {
            match token.value() {
                Token::Comma => self.parse_selector_tokens(token, selectors, false),
                _ => self.parse_selector_tokens(token, selectors, true)
            }
        } else {
            self.parse_selector_tokens(token, selectors, true)
        }
    }

    fn handle_hierarchy_selector_without_part(&mut self, last_token: &SpannedToken<'a>, token: &SpannedToken<'a>) {
        if !(
            matches!(last_token.value(), Token::DescendantsSelector | Token::ChildrenSelector) &&
            matches!(
                token.value(),
                Token::DescendantsSelector | Token::ChildrenSelector | Token::Comma | Token::ScopeOpen | Token::ScopeClose
            )
        ) { return }

        self.ast_errors.push(
            ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected("selector part")) },
            self.range_from_span(clamp_span_to_end(last_token.end()))
        );
    }

    pub(crate) fn parse_rule_scope_body(&mut self, body_open_node: Node<'a>, selectors: Option<Vec<SelectorNode<'a>>>) -> Parsed<'a> {
        let Some(node) = self.advance() else {
            self.ast_errors.push(
                ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())) },
                self.range_from_span(clamp_span_to_end(body_open_node.token.end()))
            );
            return Parsed (None, Some(Construct::rule(selectors, Delimited::new(body_open_node, None, None))));
        };

        if node_token_matches!(node, ScopeClose) {
            return Parsed (self.advance(), Some(Construct::rule(
                selectors,
                Delimited::new(body_open_node, None, Some(node))
            )))
        }

        let mut body_content: Vec<Construct<'a>> = vec![];

        let (node, parse_ended_reason) =
            self.parse_loop_inner(node,|parser, mut node| {
                node = parser.parse_macro(node).handle_construct(&mut body_content)?;

                node = parser.parse_macro_call(node).handle_construct(&mut body_content)?;

                node = parser.parse_derive(node).handle_construct(&mut body_content)?;

                node = parser.parse_priority(node).handle_construct(&mut body_content)?;
                node = parser.parse_name(node).handle_construct(&mut body_content)?;

                node = parser.parse_tween(node).handle_construct(&mut body_content)?;

                node = parser.parse_static_token_assignment(node).handle_construct(&mut body_content)?;

                node = parser.parse_token_assignment(node).handle_construct(&mut body_content)?;

                node = parser.parse_property_assignment_or_rule_scope(node).handle_construct(&mut body_content)?;
                node = parser.parse_rule_scope_selector_begin(node).handle_construct(&mut body_content)?;

                node = parser.parse_invalid_declaration(node)?;
                node = parser.parse_none(node).handle_construct(&mut body_content)?;

                let end_parsing = node_token_matches!(node, ScopeClose);
                Some((node, end_parsing))
            });

        if matches!(parse_ended_reason, ParseEndedReason::Manual) {
            return Parsed (self.advance(), Some(Construct::rule(
                selectors,
                Delimited::new(body_open_node, Some(body_content), node)
            )))

        } else {
            let construct = Construct::rule(
                selectors,
                Delimited::new(body_open_node, Some(body_content), None)
            );

            self.ast_errors.push(
                ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())) },
                self.range_from_span(clamp_span_to_end(construct.end()))
            );

            Parsed (self.advance(), Some(construct))
        }
    }
}
