use crate::lexer::{
    TOKEN_KIND_CONSTRUCT_DELIMITERS, TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS,
    TOKEN_KIND_MACRO_CALL_DELIMITERS, TOKEN_KIND_WITH_LIST_DELIMITERS, Token, TokenKind,
};
use crate::list::{Stringified, TokenKindList};
use crate::parser::RsmlParser;
use crate::parser::parse_error::{ParseError, ParseErrorMessage};
use crate::parser::types::*;
use crate::{node_token_matches, token_kind_list};

use phf_macros::phf_set;

static MACRO_RETURN_TYPES: phf::Set<&str> = phf_set! {
    "Construct",
    "Datatype",
    "Selector",
};

static MACRO_RETURN_TYPE_NAMES: [&str; 3] = ["Construct", "Datatype", "Selector"];

impl<'a> RsmlParser<'a> {
    /// Many declarations in rsml just have a datatype after them, so the same
    /// function can parse all of them.
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
        if !node_token_matches!(node, DeriveDeclaration) {
            return Parsed(Some(node), None);
        }
        let declaration_node = node;

        let body_node_opt = self.advance_until(
            token_kind_list!("derive path string", [StringSingle, StringMulti]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        );

        let body = match body_node_opt {
            Some(Ok(string_node)) => Some(Box::new(Construct::Node { node: string_node })),
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::Derive {
                        declaration: declaration_node,
                        body: None,
                        with_clause: None,
                        terminator: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Derive {
                        declaration: declaration_node,
                        body: None,
                        with_clause: None,
                        terminator: None,
                    }),
                );
            }
        };

        let mut next = self.advance_until(
            token_kind_list!("\";\" or \"@with\"", [SemiColon, WithDeclaration]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        );

        let with_clause = if let Some(Ok(node)) = &next
            && node_token_matches!(node, WithDeclaration)
        {
            let with_keyword = match next.take() {
                Some(Ok(node)) => node,
                _ => unreachable!(),
            };
            let (clause, after) = self.parse_with_clause(with_keyword);
            next = after;
            Some(clause)
        } else {
            None
        };

        let terminator = match next {
            // Accept the `;` whether `advance_until` returned it via the
            // happy path (`Ok`) or via a delimiter bail (`Err`). When the
            // with-list ended early at `;` we don't want a redundant
            // "Expected `;`" error — the `;` is right there.
            Some(Ok(node)) | Some(Err(node)) if node_token_matches!(node, SemiColon) => Some(node),
            Some(Ok(node)) => {
                let construct = Construct::Derive {
                    declaration: declaration_node,
                    body,
                    with_clause,
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
            Some(Err(node)) => {
                // The inner `advance_until` already reported the missing
                // semicolon at the bail position; just pass the construct
                // back without piling on a duplicate diagnostic.
                let construct = Construct::Derive {
                    declaration: declaration_node,
                    body,
                    with_clause,
                    terminator: None,
                };
                return Parsed(Some(node), Some(construct));
            }
            None => {
                let construct = Construct::Derive {
                    declaration: declaration_node,
                    body,
                    with_clause,
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
            Some(Construct::Derive {
                declaration: declaration_node,
                body,
                with_clause,
                terminator,
            }),
        )
    }

    /// Parses the body of a `@with` clause attached to `@derive`. Caller has
    /// already consumed the `@with` keyword. Returns the clause and the next
    /// token's `advance_until` result (a `;` or block delimiter).
    fn parse_with_clause(
        &mut self,
        with_keyword: Node<'a>,
    ) -> (
        DeriveWithClause<'a>,
        Option<Result<Node<'a>, Node<'a>>>,
    ) {
        let star_or_open = self.advance_until(
            token_kind_list!("\"*\" or \"{\"", [OpMult, ScopeOpen]),
            &TOKEN_KIND_WITH_LIST_DELIMITERS,
        );

        match star_or_open {
            Some(Ok(node)) if node_token_matches!(node, OpMult) => {
                let clause = DeriveWithClause {
                    keyword: with_keyword,
                    items: Some(DeriveWithItems::All(node)),
                };
                let after = self.advance_until(
                    token_kind_list![SemiColon],
                    &TOKEN_KIND_WITH_LIST_DELIMITERS,
                );
                (clause, after)
            }

            Some(Ok(node)) => {
                let (list, after) = self.parse_with_list(node);
                (
                    DeriveWithClause {
                        keyword: with_keyword,
                        items: Some(DeriveWithItems::List(list)),
                    },
                    after,
                )
            }

            Some(Err(err_node)) => (
                DeriveWithClause {
                    keyword: with_keyword,
                    items: None,
                },
                Some(Err(err_node)),
            ),

            None => (
                DeriveWithClause {
                    keyword: with_keyword,
                    items: None,
                },
                None,
            ),
        }
    }

    fn parse_with_list(
        &mut self,
        open_node: Node<'a>,
    ) -> (
        Delimited<'a, DeriveWithItem<'a>>,
        Option<Result<Node<'a>, Node<'a>>>,
    ) {
        let mut items: Vec<DeriveWithItem<'a>> = Vec::new();

        loop {
            let next = self.advance_until(
                token_kind_list!(
                    "import name or \"}\"",
                    [Identifier, StaticTokenIdentifier, ScopeClose]
                ),
                &TOKEN_KIND_WITH_LIST_DELIMITERS,
            );

            let name_node = match next {
                Some(Ok(node)) if node_token_matches!(node, ScopeClose) => {
                    let close = node;
                    let delim = Delimited::new(
                        open_node,
                        if items.is_empty() { None } else { Some(items) },
                        Some(close),
                    );
                    let after = self.advance_until(
                        token_kind_list![SemiColon],
                        &TOKEN_KIND_WITH_LIST_DELIMITERS,
                    );
                    return (delim, after);
                }
                Some(Ok(node)) => node,
                Some(Err(err_node)) => {
                    let delim = Delimited::new(
                        open_node,
                        if items.is_empty() { None } else { Some(items) },
                        None,
                    );
                    // `advance_until` already reported a "Expected import
                    // name or `}`" diagnostic at the bail token; don't pile
                    // on a second "Expected `}`" at the same position.
                    return (delim, Some(Err(err_node)));
                }
                None => {
                    let delim = Delimited::new(
                        open_node,
                        if items.is_empty() { None } else { Some(items) },
                        None,
                    );
                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                        },
                        self.range_from_span(clamp_span_to_end(delim.end())),
                    );
                    return (delim, None);
                }
            };

            let after_name = self.advance_until(
                token_kind_list!("\",\" or \"}\"", [Comma, ScopeClose]),
                &TOKEN_KIND_WITH_LIST_DELIMITERS,
            );

            match after_name {
                Some(Ok(sep)) if node_token_matches!(sep, Comma) => {
                    items.push(DeriveWithItem {
                        name: name_node,
                        trailing_comma: Some(sep),
                    });
                    continue;
                }
                Some(Ok(close)) => {
                    items.push(DeriveWithItem {
                        name: name_node,
                        trailing_comma: None,
                    });
                    let delim = Delimited::new(open_node, Some(items), Some(close));
                    let after = self.advance_until(
                        token_kind_list![SemiColon],
                        &TOKEN_KIND_WITH_LIST_DELIMITERS,
                    );
                    return (delim, after);
                }
                Some(Err(err_node)) => {
                    items.push(DeriveWithItem {
                        name: name_node,
                        trailing_comma: None,
                    });
                    let delim = Delimited::new(open_node, Some(items), None);
                    // `advance_until` already reported "Expected `,` or `}`"
                    // at the bail token; don't pile on a second
                    // "Expected `}`" at the same position.
                    return (delim, Some(Err(err_node)));
                }
                None => {
                    items.push(DeriveWithItem {
                        name: name_node,
                        trailing_comma: None,
                    });
                    let delim = Delimited::new(open_node, Some(items), None);
                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                        },
                        self.range_from_span(clamp_span_to_end(delim.end())),
                    );
                    return (delim, None);
                }
            }
        }
    }

    pub(crate) fn parse_pub(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, PubDeclaration) {
            return Parsed(Some(node), None);
        }
        let pub_node = node;

        let next = match self.advance_until(
            token_kind_list!(
                "\"@macro\", \"@schema\", or static token assignment",
                [MacroDeclaration, SchemaDeclaration, StaticTokenIdentifier]
            ),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                self.ast_errors.push(
                    ParseError::UnexpectedTokens { msg: None },
                    self.range_from_span(pub_node.token.span()),
                );
                return Parsed(Some(node), None);
            }
            None => {
                self.ast_errors.push(
                    ParseError::UnexpectedTokens { msg: None },
                    self.range_from_span(pub_node.token.span()),
                );
                return Parsed(None, None);
            }
        };

        self.pending_pub_modifier = Some(pub_node);
        Parsed(Some(next), None)
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

    /// Parses the `(` args `)` portion of a macro call, treating each arg as a
    /// datatype (so math expressions like `0% + .5` become a single
    /// `Construct::MathOperation`). Does NOT consume any trailing terminator —
    /// the caller handles that.
    ///
    /// `body.right.is_some()` ⇔ the close paren was consumed and the parser
    /// cursor is positioned right after it. The returned `Option<Node>` carries
    /// either the unconsumed errored token (when close was missing) or `None`
    /// (on success or EOF).
    fn parse_macro_call_args(
        &mut self,
        open_node: Node<'a>,
    ) -> (Option<Node<'a>>, Delimited<'a>) {
        let first_node = match self.advance() {
            Some(node) => {
                let token_value = node.token.value();

                if matches!(token_value, Token::ParensClose) {
                    return (None, Delimited::new(open_node, None, Some(node)));
                } else if TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS
                    .contains(&token_value.kind())
                {
                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())),
                        },
                        self.range_from_span(clamp_span_to_end(open_node.token.end())),
                    );

                    return (Some(node), Delimited::new(open_node, None, None));
                }

                node
            }

            None => {
                self.ast_errors.push(
                    ParseError::MissingToken {
                        msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())),
                    },
                    self.range_from_span(clamp_span_to_end(open_node.token.end())),
                );

                return (None, Delimited::new(open_node, None, None));
            }
        };

        let (next_node, datatype_groups) = self.parse_table_datatype_args(Some(first_node));

        if !node_token_matches!(next_node, Some(ParensClose)) {
            let delimited = Delimited::new(open_node, datatype_groups, None);
            self.ast_errors.push(
                ParseError::MissingToken {
                    msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())),
                },
                self.range_from_span(clamp_span_to_end(delimited.end())),
            );
            return (next_node, delimited);
        }

        (None, Delimited::new(open_node, datatype_groups, next_node))
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

        let (next_node, body) = self.parse_macro_call_args(open_node);

        if body.right.is_none() {
            return Parsed(
                next_node,
                Some(Construct::MacroCall {
                    name: name_node,
                    body: Some(body),
                    terminator: None,
                }),
            );
        }

        let terminator_node = match self.advance_until(
            token_kind_list![SemiColon, ScopeOpen, Comma],
            &TOKEN_KIND_MACRO_CALL_DELIMITERS,
        ) {
            Some(Ok(node)) if !matches!(node.token.value(), Token::SemiColon) => {
                return Parsed(
                    Some(node),
                    Some(Construct::MacroCall {
                        name: name_node,
                        body: Some(body),
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
                        body: Some(body),
                        terminator: None,
                    }),
                );
            }

            None => {
                return Parsed(
                    None,
                    Some(Construct::MacroCall {
                        name: name_node,
                        body: Some(body),
                        terminator: None,
                    }),
                );
            }
        };

        Parsed(
            self.advance(),
            Some(Construct::MacroCall {
                name: name_node,
                body: Some(body),
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

        let (next_node, body) = self.parse_macro_call_args(open_node);

        if body.right.is_none() {
            return (
                next_node,
                SelectorNode::MacroCall {
                    name: name_node,
                    body: Some(body),
                },
            );
        }

        (
            self.advance(),
            SelectorNode::MacroCall {
                name: name_node,
                body: Some(body),
            },
        )
    }

    pub(crate) fn parse_extends(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, ExtendsDeclaration) {
            return Parsed(Some(node), None);
        }

        let declaration_node = node;

        let name_node = match self.advance_until(
            token_kind_list!("schema name", [Identifier]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => Some(node),
            Some(Err(node)) => {
                if node_token_matches!(node, SemiColon) {
                    return Parsed(
                        self.advance(),
                        Some(Construct::Extends {
                            declaration: declaration_node,
                            name: None,
                            terminator: Some(node),
                        }),
                    );
                }

                return Parsed(
                    Some(node),
                    Some(Construct::Extends {
                        declaration: declaration_node,
                        name: None,
                        terminator: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Extends {
                        declaration: declaration_node,
                        name: None,
                        terminator: None,
                    }),
                );
            }
        };

        let terminator = match self.advance_until(
            token_kind_list![SemiColon],
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => Some(node),
            Some(Err(node)) => {
                let construct = Construct::Extends {
                    declaration: declaration_node,
                    name: name_node,
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
            None => {
                let construct = Construct::Extends {
                    declaration: declaration_node,
                    name: name_node,
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
            Some(Construct::Extends {
                declaration: declaration_node,
                name: name_node,
                terminator,
            }),
        )
    }

    pub(crate) fn parse_schema(&mut self, node: Node<'a>) -> Parsed<'a> {
        if !node_token_matches!(node, SchemaDeclaration) {
            return Parsed(Some(node), None);
        }

        let declaration_node = node;

        let name_node = match self.advance_until(
            token_kind_list!("schema name", [Identifier]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => Some(node),
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::Schema {
                        pub_modifier: self.pending_pub_modifier.take(),
                        declaration: declaration_node,
                        name: None,
                        body: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Schema {
                        pub_modifier: self.pending_pub_modifier.take(),
                        declaration: declaration_node,
                        name: None,
                        body: None,
                    }),
                );
            }
        };

        let body_open_node = match self.advance_until(
            token_kind_list![ScopeOpen],
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => {
                return Parsed(
                    Some(node),
                    Some(Construct::Schema {
                        pub_modifier: self.pending_pub_modifier.take(),
                        declaration: declaration_node,
                        name: name_node,
                        body: None,
                    }),
                );
            }
            None => {
                return Parsed(
                    None,
                    Some(Construct::Schema {
                        pub_modifier: self.pending_pub_modifier.take(),
                        declaration: declaration_node,
                        name: name_node,
                        body: None,
                    }),
                );
            }
        };

        self.parse_schema_body(declaration_node, name_node, body_open_node)
    }

    fn parse_schema_body(
        &mut self,
        declaration_node: Node<'a>,
        name_node: Option<Node<'a>>,
        body_open_node: Node<'a>,
    ) -> Parsed<'a> {
        let mut fields: Vec<SchemaField<'a>> = Vec::new();

        loop {
            let next = match self.advance_until(
                token_kind_list!(
                    "field declaration or `}`",
                    [TokenIdentifier, ScopeClose]
                ),
                &TOKEN_KIND_CONSTRUCT_DELIMITERS,
            ) {
                Some(Ok(node)) => node,
                Some(Err(node)) => {
                    let construct = Construct::Schema {
                        pub_modifier: self.pending_pub_modifier.take(),
                        declaration: declaration_node,
                        name: name_node,
                        body: Some(Delimited::new(
                            body_open_node,
                            if fields.is_empty() { None } else { Some(fields) },
                            None,
                        )),
                    };

                    self.ast_errors.push(
                        ParseError::MissingToken {
                            msg: Some(ParseErrorMessage::Expected(TokenKind::ScopeClose.name())),
                        },
                        self.range_from_span(clamp_span_to_end(construct.end())),
                    );

                    return Parsed(Some(node), Some(construct));
                }
                None => {
                    let construct = Construct::Schema {
                        pub_modifier: self.pending_pub_modifier.take(),
                        declaration: declaration_node,
                        name: name_node,
                        body: Some(Delimited::new(
                            body_open_node,
                            if fields.is_empty() { None } else { Some(fields) },
                            None,
                        )),
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

            if node_token_matches!(next, ScopeClose) {
                return Parsed(
                    self.advance(),
                    Some(Construct::Schema {
                        pub_modifier: self.pending_pub_modifier.take(),
                        declaration: declaration_node,
                        name: name_node,
                        body: Some(Delimited::new(
                            body_open_node,
                            if fields.is_empty() { None } else { Some(fields) },
                            Some(next),
                        )),
                    }),
                );
            }

            let field = self.parse_schema_field(next);
            fields.push(field);
        }
    }

    fn parse_schema_field(&mut self, name_node: Node<'a>) -> SchemaField<'a> {
        let colon_node = match self.advance_until(
            token_kind_list!("\":\"", [StateSelectorOrEnumPart]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => Some(node),
            _ => None,
        };

        let inline_type = colon_node.as_ref().and_then(|n| match n.token.value() {
            Token::StateSelectorOrEnumPart(Some(_)) => Some(()),
            _ => None,
        });

        let type_name_node = if inline_type.is_some() {
            None
        } else {
            match self.advance_until(
                token_kind_list!("type name", [Identifier]),
                &TOKEN_KIND_CONSTRUCT_DELIMITERS,
            ) {
                Some(Ok(node)) => Some(node),
                _ => None,
            }
        };

        let terminator = match self.advance_until(
            token_kind_list![SemiColon],
            &TOKEN_KIND_CONSTRUCT_DELIMITERS,
        ) {
            Some(Ok(node)) => Some(node),
            _ => None,
        };

        SchemaField {
            name: name_node,
            colon: colon_node,
            type_name: type_name_node,
            terminator,
        }
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                        pub_modifier: self.pending_pub_modifier.take(),
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
                        pub_modifier: self.pending_pub_modifier.take(),
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
                            pub_modifier: self.pending_pub_modifier.take(),
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
                            pub_modifier: self.pending_pub_modifier.take(),
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
                        pub_modifier: self.pending_pub_modifier.take(),
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
                        pub_modifier: self.pending_pub_modifier.take(),
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
                            pub_modifier: self.pending_pub_modifier.take(),
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
                            pub_modifier: self.pending_pub_modifier.take(),
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
                        pub_modifier: self.pending_pub_modifier.take(),
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
                        pub_modifier: self.pending_pub_modifier.take(),
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
                            pub_modifier: self.pending_pub_modifier.take(),
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
                            pub_modifier: self.pending_pub_modifier.take(),
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
            Some("Datatype") => self.parse_macro_body_datatype(
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                .parse_tween(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_schema(node)
                .handle_construct(&mut body_content)?;

            node = parser
                .parse_extends(node)
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                pub_modifier: self.pending_pub_modifier.take(),
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

    fn parse_macro_body_datatype(
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
                    pub_modifier: self.pending_pub_modifier.take(),
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Datatype(None),
                        close: None,
                    }),
                }),
            );
        };

        if node_token_matches!(node, ScopeClose) {
            return Parsed(
                self.advance(),
                Some(Construct::Macro {
                    pub_modifier: self.pending_pub_modifier.take(),
                    declaration: declaration_node,
                    name: name_node,
                    args: args_node,
                    return_type,
                    body: Some(MacroBody {
                        open: body_open_node,
                        content: MacroBodyContent::Datatype(None),
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
                            pub_modifier: self.pending_pub_modifier.take(),
                            declaration: declaration_node,
                            name: name_node,
                            args: args_node,
                            return_type,
                            body: Some(MacroBody {
                                open: body_open_node,
                                content: MacroBodyContent::Datatype(datatype.map(Box::new)),
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
                            pub_modifier: self.pending_pub_modifier.take(),
                            declaration: declaration_node,
                            name: name_node,
                            args: args_node,
                            return_type,
                            body: Some(MacroBody {
                                open: body_open_node,
                                content: MacroBodyContent::Datatype(datatype.map(Box::new)),
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
                pub_modifier: self.pending_pub_modifier.take(),
                declaration: declaration_node,
                name: name_node,
                args: args_node,
                return_type,
                body: Some(MacroBody {
                    open: body_open_node,
                    content: MacroBodyContent::Datatype(datatype.map(Box::new)),
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
                pub_modifier: self.pending_pub_modifier.take(),
                declaration: declaration_node,
                name: name_node,
                args: args_node,
                return_type,
                body: Some(MacroBody {
                    open: body_open_node,
                    content: MacroBodyContent::Datatype(datatype.map(Box::new)),
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                    pub_modifier: self.pending_pub_modifier.take(),
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
                    pub_modifier: self.pending_pub_modifier.take(),
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

        let (terminator, selectors) = if node_token_matches!(node, MacroCallIdentifier(_)) {
            let (next_node, selector_node) = self.parse_macro_call_in_selector(node);
            let mut selectors = vec![selector_node];

            match next_node {
                Some(next)
                    if node_token_matches!(next, ScopeOpen)
                        || node_token_matches!(next, ScopeClose) =>
                {
                    (Some(next), selectors)
                }
                Some(next) => {
                    let token = next.token.clone();
                    selectors.push(SelectorNode::Token(next));
                    match token.value() {
                        Token::Comma => self.parse_selector_tokens(token, selectors, false),
                        _ => self.parse_selector_tokens(token, selectors, true),
                    }
                }
                None => (None, selectors),
            }
        } else {
            let first_token = node.token.clone();
            let selectors = vec![SelectorNode::Token(node)];
            self.parse_selector_tokens(first_token, selectors, true)
        };

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
                        pub_modifier: self.pending_pub_modifier.take(),
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
            pub_modifier: self.pending_pub_modifier.take(),
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
