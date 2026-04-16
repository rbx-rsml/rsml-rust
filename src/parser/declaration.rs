use crate::lexer::{
    SpannedToken, TOKEN_KIND_CONSTRUCT_DELIMITERS, TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS,
    TOKEN_KIND_MACRO_CALL_DELIMITERS, Token, TokenKind,
};
use crate::list::{Stringified, TokenKindList};
use crate::parser::Parser;
use crate::parser::parse_error::{ParseError, ParseErrorMessage};
use crate::parser::types::*;
use crate::{node_token_matches, token_kind_list};

use phf_macros::phf_set;

static MACRO_RETURN_TYPES: phf::Set<&str> = phf_set! {
    "Construct",
    "Assignment",
    "Selector",
};

static MACRO_RETURN_TYPE_NAMES: [&str; 3] = ["Construct", "Assignment", "Selector"];

impl<'a> Parser<'a> {
    /// Many declarations in rsml just have a datatype after them.
    /// So we can use the same function to parse them.
    pub(crate) fn parse_declaration_with_datatype(
        &mut self,
        node: Node<'a>,
        declaration_token_kind: TokenKind,
        constructor: fn(
            declaration: Node<'a>,
            body: Option<Box<Construct<'a>>>,
            terminator: Option<Node<'a>>,
        ) -> Construct<'a>,
    ) -> Parsed<'a> {
        if node.token.value().kind() != declaration_token_kind {
            return Parsed(Some(node), None);
        }
        let declaration_node = node;

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
                        Some(constructor(declaration_node, body_nodes, None)),
                    );
                }
                None => return Parsed(None, Some(constructor(declaration_node, body_nodes, None))),
            },

            NodeStatus::Err(node) => {
                if node_token_matches!(node, SemiColon) {
                    node
                } else {
                    let construct = constructor(declaration_node, body_nodes, None);

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
                let construct = constructor(declaration_node, body_nodes, None);

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
            Some(constructor(declaration_node, body_nodes, Some(terminator))),
        )
    }

    pub(crate) fn parse_derive(&mut self, node: Node<'a>) -> Parsed<'a> {
        self.parse_declaration_with_datatype(
            node,
            TokenKind::DeriveDeclaration,
            |declaration, body, terminator| Construct::Derive {
                declaration,
                body,
                terminator,
            },
        )
    }

    pub(crate) fn parse_priority(&mut self, node: Node<'a>) -> Parsed<'a> {
        self.parse_declaration_with_datatype(
            node,
            TokenKind::PriorityDeclaration,
            |declaration, body, terminator| Construct::Priority {
                declaration,
                body,
                terminator,
            },
        )
    }

    pub(crate) fn parse_name(&mut self, node: Node<'a>) -> Parsed<'a> {
        self.parse_declaration_with_datatype(
            node,
            TokenKind::NameDeclaration,
            |declaration, body, terminator| Construct::Name {
                declaration,
                body,
                terminator,
            },
        )
    }

    pub(crate) fn parse_tween(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, TweenDeclaration) {
            return Parsed(Some(node), None);
        }

        let declaration_node = node;

        let name_node = match self.advance_until(
            token_kind_list!("tween name", [Identifier]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => Some(node),
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::Tween {
                        declaration: declaration_node,
                        name: None,
                        body: None,
                        terminator: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Tween {
                        declaration: declaration_node,
                        name: None,
                        body: None,
                        terminator: None,
                    }),
                );
            }
        };

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
                        Some(Construct::Tween {
                            declaration: declaration_node,
                            name: name_node,
                            body: body_nodes,
                            terminator: None,
                        }),
                    );
                }
                None => {
                    return Parsed(
                        None,
                        Some(Construct::Tween {
                            declaration: declaration_node,
                            name: name_node,
                            body: body_nodes,
                            terminator: None,
                        }),
                    );
                }
            },

            NodeStatus::Err(node) => {
                if node_token_matches!(node, SemiColon) {
                    node
                } else {
                    let construct = Construct::Tween {
                        declaration: declaration_node,
                        name: name_node,
                        body: body_nodes,
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
                let construct = Construct::Tween {
                    declaration: declaration_node,
                    name: name_node,
                    body: body_nodes,
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
            Some(Construct::Tween {
                declaration: declaration_node,
                name: name_node,
                body: body_nodes,
                terminator: Some(terminator),
            }),
        )
    }

    // TODO: properly implement macros.
    pub(crate) fn parse_macro_call(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, MacroCallIdentifier(_)) {
            return Parsed(Some(node), None);
        }

        let Parsed(next_node, construct) = self.parse_macro_call_body(node);

        // If a `{` or `,` follows the macro call, parse it as a Rule with the macro call as a selector.
        if let Some(ref next) = next_node
            && (node_token_matches!(next, ScopeOpen) || node_token_matches!(next, Comma))
            && let Some(Construct::MacroCall { name, body, .. }) = construct
        {
            let is_scope_open = node_token_matches!(next, ScopeOpen);
            let selector_node = SelectorNode::MacroCall { name, body };
            let selectors = vec![selector_node];

            if is_scope_open {
                return self.parse_rule_scope_body(next_node.unwrap(), Some(selectors));
            } else {
                let comma_node = next_node.unwrap();
                let token = comma_node.token.clone();
                let mut selectors = selectors;
                selectors.push(SelectorNode::Token(comma_node));
                return self.parse_rule_scope_selector(token, selectors, false);
            }
        }

        Parsed(next_node, construct)
    }

    pub(crate) fn parse_macro_call_body(&mut self, name_node: Node<'a>) -> Parsed<'a> {
        let open_node = match self.advance_until(
            token_kind_list![ParensOpen],
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::MacroCall {
                        name: name_node,
                        body: None,
                        terminator: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::MacroCall {
                        name: name_node,
                        body: None,
                        terminator: None,
                    }),
                );
            }
        };

        let mut body_content: Vec<Construct<'a>> = vec![];

        let mut parens_nestedness: usize = 0;

        let close_node = loop {
            let node = self.advance();

            match node {
                Some(Node {
                    token: SpannedToken(_, Token::ParensOpen, _),
                    ..
                }) => parens_nestedness += 1,

                Some(
                    node @ Node {
                        token: SpannedToken(_, Token::ParensClose, _),
                        ..
                    },
                ) => {
                    if parens_nestedness == 0 {
                        break node;
                    } else {
                        parens_nestedness -= 1
                    }
                }

                Some(node) => body_content.push(Construct::Node { node }),

                None => {
                    let construct = Construct::MacroCall {
                        name: name_node,
                        body: Some(Delimited::new(open_node, Some(body_content), None)),
                        terminator: None,
                    };

                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())),
                        },
                        self.range_from_span(construct.span()),
                    );

                    return Parsed(None, Some(construct));
                }
            }
        };

        let terminator_node = match self.advance_until(
            token_kind_list![SemiColon, ScopeOpen, Comma],
            &TOKEN_KIND_MACRO_CALL_DELIMITERS,
        ) {
            Some(Ok(node)) if !matches!(node.token.value(), Token::SemiColon) => {
                return Parsed(
                    Some(node),
                    Some(Construct::MacroCall {
                        name: name_node,
                        body: Some(Delimited::new(
                            open_node,
                            Some(body_content),
                            Some(close_node),
                        )),
                        terminator: None,
                    }),
                );
            }
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::MacroCall {
                        name: name_node,
                        body: Some(Delimited::new(
                            open_node,
                            Some(body_content),
                            Some(close_node),
                        )),
                        terminator: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::MacroCall {
                        name: name_node,
                        body: Some(Delimited::new(
                            open_node,
                            Some(body_content),
                            Some(close_node),
                        )),
                        terminator: None,
                    }),
                );
            }
        };

        Parsed(
            self.advance(),
            Some(Construct::MacroCall {
                name: name_node,
                body: Some(Delimited::new(
                    open_node,
                    Some(body_content),
                    Some(close_node),
                )),
                terminator: Some(terminator_node),
            }),
        )
    }

    /// Parses a macro call in selector context (no semicolon terminator required).
    /// Returns the `SelectorNode::MacroCall` and the next node after the closing paren.
    pub(crate) fn parse_macro_call_in_selector(
        &mut self,
        name_node: Node<'a>,
    ) -> (Option<Node<'a>>, SelectorNode<'a>) {
        let open_node = match self.advance_until(
            token_kind_list![ParensOpen],
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                return (
                    Some(node),
                    SelectorNode::MacroCall {
                        name: name_node,
                        body: None,
                    },
                );
            }
            None => {
                return (
                    None,
                    SelectorNode::MacroCall {
                        name: name_node,
                        body: None,
                    },
                );
            }
        };

        let mut body_content: Vec<Construct<'a>> = vec![];
        let mut parens_nestedness: usize = 0;

        let close_node = loop {
            let node = self.advance();

            match node {
                Some(Node {
                    token: SpannedToken(_, Token::ParensOpen, _),
                    ..
                }) => parens_nestedness += 1,

                Some(
                    node @ Node {
                        token: SpannedToken(_, Token::ParensClose, _),
                        ..
                    },
                ) => {
                    if parens_nestedness == 0 {
                        break node;
                    } else {
                        parens_nestedness -= 1
                    }
                }

                Some(node) => body_content.push(Construct::Node { node }),

                None => {
                    let selector_node = SelectorNode::MacroCall {
                        name: name_node,
                        body: Some(Delimited::new(open_node, Some(body_content), None)),
                    };

                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())),
                        },
                        self.range_from_span((selector_node.start(), selector_node.end())),
                    );

                    return (None, selector_node);
                }
            }
        };

        (
            self.advance(),
            SelectorNode::MacroCall {
                name: name_node,
                body: Some(Delimited::new(
                    open_node,
                    Some(body_content),
                    Some(close_node),
                )),
            },
        )
    }

    pub(crate) fn parse_macro(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, MacroDeclaration) {
            return Parsed(Some(node), None);
        }

        let declaration_node = node;

        let name_node = match self.advance_until(
            token_kind_list!("macro name", [Identifier]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => Some(node),
            Some(Err(node)) => {
                let construct = Construct::Macro {
                    declaration: declaration_node,
                    name: None,
                    args: None,
                    return_type: None,
                    body: None,
                };
                return Parsed(Some(node), Some(construct));
            }
            None => {
                let construct = Construct::Macro {
                    declaration: declaration_node,
                    name: None,
                    args: None,
                    return_type: None,
                    body: None,
                };
                return Parsed(self.advance(), Some(construct));
            }
        };

        let args_or_body_node = match self.advance_until(
            token_kind_list!(
                "macro arguments, return type or body",
                [ScopeOpen, ParensOpen, ReturnArrow]
            ),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::Macro {
                        declaration: declaration_node,
                        name: name_node,
                        args: None,
                        return_type: None,
                        body: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Macro {
                        declaration: declaration_node,
                        name: name_node,
                        args: None,
                        return_type: None,
                        body: None,
                    }),
                );
            }
        };

        if matches!(args_or_body_node.token.value(), Token::ParensOpen) {
            self.parse_macro_args(args_or_body_node, declaration_node, name_node)
        } else if matches!(args_or_body_node.token.value(), Token::ReturnArrow) {
            let (return_type, return_type_str) = self.parse_macro_return_type(args_or_body_node);

            let body_node = match self.advance_until(
                token_kind_list![ScopeOpen],
                &TOKEN_KIND_CONSTRUCT_DELIMITERS,
            ) {
                Some(Ok(node)) => node,
                Some(Err(node)) => {
                    return Parsed(
                        Some(node),
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args: None,
                            return_type: Some(return_type),
                            body: None,
                        }),
                    );
                }
                None => {
                    return Parsed(
                        None,
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args: None,
                            return_type: Some(return_type),
                            body: None,
                        }),
                    );
                }
            };

            self.parse_macro_body(
                body_node,
                declaration_node,
                name_node,
                None,
                Some(return_type),
                return_type_str,
            )
        } else {
            self.parse_macro_body(
                args_or_body_node,
                declaration_node,
                name_node,
                None,
                None,
                None,
            )
        }
    }

    /// Parses the identifier after `->` and validates it against the allowed return types.
    /// Returns the (arrow_node, ident_node) pair and the identifier string if valid.
    fn parse_macro_return_type(
        &mut self,
        arrow_node: Node<'a>,
    ) -> ((Node<'a>, Option<Node<'a>>), Option<&'a str>) {
        let ident_node = match self.advance_until(
            token_kind_list!("macro return type", [Identifier]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(_)) | None => return ((arrow_node, None), None),
        };

        let return_type_str = if let Token::Identifier(name) = ident_node.token.value() {
            if MACRO_RETURN_TYPES.contains(*name) {
                Some(*name)
            } else {
                self.ast_errors.push(
                    ParseError::UnexpectedTokens {
                        msg: Some(ParseErrorMessage::correction(
                            Some(name.to_string()),
                            self.range_from_span(ident_node.token.span()),
                            &MACRO_RETURN_TYPE_NAMES,
                        )),
                    },
                    self.range_from_span(ident_node.token.span()),
                );
                None
            }
        } else {
            None
        };

        ((arrow_node, Some(ident_node)), return_type_str)
    }

    fn parse_macro_args(
        &mut self,
        args_open_node: Node<'a>,
        declaration_node: Node<'a>,
        name_node: Option<Node<'a>>,
    ) -> Parsed<'a> {
        let mut node = match self.advance_until(
            token_kind_list![MacroArgIdentifier, Comma, ParensClose],
            &TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::Macro {
                        declaration: declaration_node,
                        name: name_node,
                        args: Some(Delimited {
                            left: args_open_node,
                            content: None,
                            right: None,
                        }),
                        return_type: None,
                        body: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Macro {
                        declaration: declaration_node,
                        name: name_node,
                        args: Some(Delimited {
                            left: args_open_node,
                            content: None,
                            right: None,
                        }),
                        return_type: None,
                        body: None,
                    }),
                );
            }
        };

        let mut last_token_value = node.token.value().clone();
        let mut last_token_span = node.token.span();

        if matches!(last_token_value, Token::ParensClose) {
            return self.parse_macro_body_open(
                declaration_node,
                name_node,
                args_open_node,
                None,
                Some(node),
            );
        }

        let mut args = vec![Construct::Node { node }];

        loop {
            let advance_until_result = match last_token_value {
                Token::Comma => self.advance_until(
                    token_kind_list![MacroArgIdentifier, ParensClose],
                    &TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS,
                ),

                _ => self.advance_until(
                    token_kind_list![MacroArgIdentifier, Comma, ParensClose],
                    &TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS,
                ),
            };

            node = match advance_until_result {
                Some(Ok(node)) => node,
                Some(Err(node)) => {
                    return Parsed(
                        Some(node),
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args: Some(Delimited::new(args_open_node, Some(args), None)),
                            return_type: None,
                            body: None,
                        }),
                    );
                }
                None => {
                    return Parsed(
                        None,
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args: Some(Delimited::new(args_open_node, Some(args), None)),
                            return_type: None,
                            body: None,
                        }),
                    );
                }
            };

            let token_span = node.token.span();
            let token_value = node.token.value().clone();

            if matches!(token_value, Token::ParensClose) {
                return self.parse_macro_body_open(
                    declaration_node,
                    name_node,
                    args_open_node,
                    Some(args),
                    Some(node),
                );
            };

            args.push(Construct::Node { node });

            if matches!(
                (&last_token_value, &token_value),
                (Token::MacroArgIdentifier(_), Token::MacroArgIdentifier(_))
            ) {
                self.ast_errors.push(
                    ParseError::MissingToken {
                        msg: Some(ParseErrorMessage::Expected(TokenKind::Comma.name())),
                    },
                    self.range_from_span((last_token_span.1 - 1, last_token_span.1)),
                );
            }

            last_token_value = token_value;
            last_token_span = token_span;
        }
    }

    fn parse_macro_body_open(
        &mut self,
        declaration_node: Node<'a>,
        name_node: Option<Node<'a>>,
        args_open_node: Node<'a>,
        args_content_node: Option<Vec<Construct<'a>>>,
        args_close_node: Option<Node<'a>>,
    ) -> Parsed<'a> {
        let body_or_arrow_node = match self.advance_until(
            token_kind_list![ScopeOpen, ReturnArrow],
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::Macro {
                        declaration: declaration_node,
                        name: name_node,
                        args: Some(Delimited {
                            left: args_open_node,
                            content: args_content_node,
                            right: args_close_node,
                        }),
                        return_type: None,
                        body: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Macro {
                        declaration: declaration_node,
                        name: name_node,
                        args: Some(Delimited {
                            left: args_open_node,
                            content: args_content_node,
                            right: args_close_node,
                        }),
                        return_type: None,
                        body: None,
                    }),
                );
            }
        };

        let args = Some(Delimited::new(
            args_open_node,
            args_content_node,
            args_close_node,
        ));

        if matches!(body_or_arrow_node.token.value(), Token::ReturnArrow) {
            let (return_type, return_type_str) = self.parse_macro_return_type(body_or_arrow_node);

            let body_node = match self.advance_until(
                token_kind_list![ScopeOpen],
                &TOKEN_KIND_CONSTRUCT_DELIMITERS,
            ) {
                Some(Ok(node)) => node,
                Some(Err(node)) => {
                    return Parsed(
                        Some(node),
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args,
                            return_type: Some(return_type),
                            body: None,
                        }),
                    );
                }
                None => {
                    return Parsed(
                        None,
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args,
                            return_type: Some(return_type),
                            body: None,
                        }),
                    );
                }
            };

            self.parse_macro_body(
                body_node,
                declaration_node,
                name_node,
                args,
                Some(return_type),
                return_type_str,
            )
        } else {
            self.parse_macro_body(
                body_or_arrow_node,
                declaration_node,
                name_node,
                args,
                None,
                None,
            )
        }
    }

    pub(crate) fn parse_macro_body(
        &mut self,
        body_open_node: Node<'a>,
        declaration_node: Node<'a>,
        name_node: Option<Node<'a>>,
        args_node: Option<Delimited<'a>>,
        return_type: Option<(Node<'a>, Option<Node<'a>>)>,
        return_type_str: Option<&str>,
    ) -> Parsed<'a> {
        match return_type_str {
            Some("Assignment") => self.parse_macro_body_assignment(
                body_open_node,
                declaration_node,
                name_node,
                args_node,
                return_type,
            ),
            Some("Selector") => self.parse_macro_body_selector(
                body_open_node,
                declaration_node,
                name_node,
                args_node,
                return_type,
            ),
            _ => self.parse_macro_body_construct(
                body_open_node,
                declaration_node,
                name_node,
                args_node,
                return_type,
            ),
        }
    }

    fn parse_macro_body_construct(
        &mut self,
        body_open_node: Node<'a>,
        declaration_node: Node<'a>,
        name_node: Option<Node<'a>>,
        args_node: Option<Delimited<'a>>,
        return_type: Option<(Node<'a>, Option<Node<'a>>)>,
    ) -> Parsed<'a> {
        let Some(node) = self.advance() else {
            self.ast_errors.push(
                ParseError::MissingToken {
                    msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                },
                self.range_from_span(clamp_span_to_end(body_open_node.token.end())),
            );
            return Parsed(
                None,
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Construct(None),
                        close: None,
                    }),
                }),
            );
        };

        if node_token_matches!(node, ScopeClose) {
            return Parsed(
                self.advance(),
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Construct(None),
                        close: Some(node),
                    }),
                }),
            );
        }

        let mut body_content: Vec<Construct<'a>> = vec![];

        let (node, parse_ended_reason) = self.parse_loop_inner(node, |parser, mut node| {
            node = parser
                .parse_macro(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_macro_call(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_derive(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_priority(node)
                .handle_construct(&mut body_content)?;
            node = parser
                .parse_name(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_tween(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_static_token_assignment(node)
                .handle_construct(&mut body_content)?;
            node = parser
                .parse_token_assignment(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_property_assignment_or_rule_scope(node)
                .handle_construct(&mut body_content)?;
            node = parser
                .parse_rule_scope_selector_begin(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_none(node)
                .handle_construct(&mut body_content)?;

            let end_parsing = node_token_matches!(node, ScopeClose);
            Some((node, end_parsing))
        });

        if matches!(parse_ended_reason, ParseEndedReason::Manual) {
            return Parsed(
                self.advance(),
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Construct(Some(body_content)),
                        close: node,
                    }),
                }),
            );
        } else {
            let construct = Construct::Macro {
                declaration: declaration_node,
                name: name_node,
                args: args_node,
                return_type,
                body: Some(MacroBody {
                    open: body_open_node,
                    content: MacroBodyContent::Construct(Some(body_content)),
                    close: None,
                }),
            };

            self.ast_errors.push(
                ParseError::MissingToken {
                    msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                },
                self.range_from_span(clamp_span_to_end(construct.end())),
            );

            Parsed(self.advance(), Some(construct))
        }
    }

    fn parse_macro_body_assignment(
        &mut self,
        body_open_node: Node<'a>,
        declaration_node: Node<'a>,
        name_node: Option<Node<'a>>,
        args_node: Option<Delimited<'a>>,
        return_type: Option<(Node<'a>, Option<Node<'a>>)>,
    ) -> Parsed<'a> {
        let Some(node) = self.advance() else {
            self.ast_errors.push(
                ParseError::MissingToken {
                    msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                },
                self.range_from_span(clamp_span_to_end(body_open_node.token.end())),
            );
            return Parsed(
                None,
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Assignment(None),
                        close: None,
                    }),
                }),
            );
        };

        if node_token_matches!(node, ScopeClose) {
            return Parsed(
                self.advance(),
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Assignment(None),
                        close: Some(node),
                    }),
                }),
            );
        }

        let (node_status, datatype) =
            self.parse_datatype(Some(node), TOKEN_KIND_CONSTRUCT_DELIMITERS);

        let close_node = match node_status {
            NodeStatus::Exists => match self.advance_until(
                token_kind_list![ScopeClose],
                &TOKEN_KIND_CONSTRUCT_DELIMITERS,
            ) {
                Some(Ok(node)) => Some(node),
                Some(Err(node)) => {
                    return Parsed(
                        Some(node),
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args: args_node,
                            return_type,
                            body: Some(MacroBody {
                                open: body_open_node,
                                content: MacroBodyContent::Assignment(datatype.map(Box::new)),
                                close: None,
                            }),
                        }),
                    );
                }
                None => None,
            },

            NodeStatus::Err(node) => {
                if node_token_matches!(node, ScopeClose) {
                    Some(node)
                } else {
                    return Parsed(
                        Some(node),
                        Some(Construct::Macro {
                            declaration: declaration_node,
                            name: name_node,
                            args: args_node,
                            return_type,
                            body: Some(MacroBody {
                                open: body_open_node,
                                content: MacroBodyContent::Assignment(datatype.map(Box::new)),
                                close: None,
                            }),
                        }),
                    );
                }
            }

            NodeStatus::None => None,
        };

        if close_node.is_none() {
            let construct = Construct::Macro {
                declaration: declaration_node,
                name: name_node,
                args: args_node,
                return_type,
                body: Some(MacroBody {
                    open: body_open_node,
                    content: MacroBodyContent::Assignment(datatype.map(Box::new)),
                    close: None,
                }),
            };

            self.ast_errors.push(
                ParseError::MissingToken {
                    msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                },
                self.range_from_span(clamp_span_to_end(construct.end())),
            );

            return Parsed(self.advance(), Some(construct));
        }

        Parsed(
            self.advance(),
            Some(Construct::Macro {
                declaration: declaration_node,
                name: name_node,
                args: args_node,
                return_type,
                body: Some(MacroBody {
                    open: body_open_node,
                    content: MacroBodyContent::Assignment(datatype.map(Box::new)),
                    close: close_node,
                }),
            }),
        )
    }

    fn parse_macro_body_selector(
        &mut self,
        body_open_node: Node<'a>,
        declaration_node: Node<'a>,
        name_node: Option<Node<'a>>,
        args_node: Option<Delimited<'a>>,
        return_type: Option<(Node<'a>, Option<Node<'a>>)>,
    ) -> Parsed<'a> {
        let Some(node) = self.advance() else {
            self.ast_errors.push(
                ParseError::MissingToken {
                    msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                },
                self.range_from_span(clamp_span_to_end(body_open_node.token.end())),
            );
            return Parsed(
                None,
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Selector(None),
                        close: None,
                    }),
                }),
            );
        };

        if node_token_matches!(node, ScopeClose) {
            return Parsed(
                self.advance(),
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Selector(None),
                        close: Some(node),
                    }),
                }),
            );
        }

        let node = match self.node_is_kind_else_advance_until(
            node,
            &token_kind_list!(
                "selector part",
                [
                    Identifier,
                    NameSelector,
                    TagSelectorOrEnumPart,
                    StateSelectorOrEnumPart,
                    PseudoSelector,
                    QuerySelector,
                    ChildrenSelector,
                    DescendantsSelector,
                    MacroCallIdentifier,
                    ScopeClose
                ]
            ),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                let has_close = node_token_matches!(node, ScopeClose);
                let close = if has_close { Some(node) } else { None };
                let construct = Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Selector(None),
                        close,
                    }),
                };
                if !has_close {
                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                        },
                        self.range_from_span(clamp_span_to_end(construct.end())),
                    );
                }
                return Parsed(self.advance(), Some(construct));
            }
            None => {
                let construct = Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Selector(None),
                        close: None,
                    }),
                };
                self.ast_errors.push(
                    ParseError::MissingToken {
                        msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                    },
                    self.range_from_span(clamp_span_to_end(construct.end())),
                );
                return Parsed(None, Some(construct));
            }
        };

        if node_token_matches!(node, ScopeClose) {
            return Parsed(
                self.advance(),
                Some(Construct::Macro {
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Selector(None),
                        close: Some(node),
                    }),
                }),
            );
        }

        let first_token = node.token.clone();
        let selectors = vec![SelectorNode::Token(node)];

        let (terminator, selectors) = self.parse_selector_tokens(first_token, selectors, true);

        let content = if selectors.is_empty() {
            None
        } else {
            Some(selectors)
        };

        if let Some(close_node) = terminator {
            if node_token_matches!(close_node, ScopeClose) {
                return Parsed(
                    self.advance(),
                    Some(Construct::Macro {
                        declaration: declaration_node,
                        name: name_node,
                        args: args_node,
                        return_type,
                        body: Some(MacroBody {
                            open: body_open_node,
                            content: MacroBodyContent::Selector(content),
                            close: Some(close_node),
                        }),
                    }),
                );
            }
        }

        let construct = Construct::Macro {
            declaration: declaration_node,
            name: name_node,
            args: args_node,
            return_type,
            body: Some(MacroBody {
                open: body_open_node,
                content: MacroBodyContent::Selector(content),
                close: None,
            }),
        };

        self.ast_errors.push(
            ParseError::MissingToken {
                msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
            },
            self.range_from_span(clamp_span_to_end(construct.end())),
        );

        Parsed(self.advance(), Some(construct))
    }
}
