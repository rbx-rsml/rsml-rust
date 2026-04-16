use std::{collections::HashSet, sync::LazyLock};

use crate::{node_token_matches, token_kind_list};
use crate::lexer::{MultilineString, SpannedToken, Token, TokenKind, TOKEN_KIND_ADD_SUB_PRECEDENCE, TOKEN_KIND_CONSTRUCT_DELIMITERS, TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS, TOKEN_KIND_OPERATOR_PRECEDENCE};
use crate::list::{Stringified, TokenKindList};
use crate::parser::parse_error::{ParseError, ParseErrorMessage};
use crate::parser::types::*;
use crate::parser::Parser;

type SymResult<T> = Result<T, T>;

impl<'a> Parser<'a> {
    pub(crate) fn parse_datatype(
        &mut self,
        node: Option<Node<'a>>,
        construct_delimiters: LazyLock<HashSet<TokenKind>>
    ) -> (NodeStatus<'a>, Option<Construct<'a>>) {
        let (node_status, construct) = self.parse_datatype_part(node, &construct_delimiters);

        if let Some(construct) = construct {
            let middle_node = match node_status {
                NodeStatus::Exists => self.advance(),
                NodeStatus::None | NodeStatus::Err(_) => return (node_status, Some(construct)),
            };

            if let Some(some_middle_node) = middle_node {
                if let Some(precedence) = TOKEN_KIND_OPERATOR_PRECEDENCE.get(&some_middle_node.token.value().kind()) {
                    let (right_node, operators) =
                        self.parse_datatype_operators(some_middle_node, *precedence);

                    if node_token_matches!(right_node, Some(SemiColon)) {
                        self.ast_errors.push(
                            ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected("a datatype")) },
                            self.range_from_span(clamp_span_to_end(operators.last().unwrap().token.end()))
                        );

                        return (right_node.to_status(), Some(Construct::MathOperation {
                            left: Box::new(construct), operators: operators, right: None
                        }))
                    }

                    self.parse_datatype_operation(
                        right_node, construct,
                        *precedence, operators,
                        &construct_delimiters
                    )

                } else { (some_middle_node.to_status(), Some(construct)) }

            } else { (middle_node.to_status(), Some(construct)) }

        } else { (node_status, None) }
    }

    fn parse_datatype_part(
        &mut self, node: Option<Node<'a>>,
        construct_delimiters: &LazyLock<HashSet<TokenKind>>
    ) -> (NodeStatus<'a>, Option<Construct<'a>>) {
        let node = match self.optional_node_is_kind_else_advance_until(
            node, token_kind_list!("a datatype", [
                Identifier, ParensOpen,
                StringMulti, StringSingle,
                Number, NumberScale, NumberOffset,
                Boolean, Nil,
                StaticTokenIdentifier, TokenIdentifier,
                ColorHex, ColorTailwind, ColorCss, ColorBrick, ColorSkin,
                RbxAsset, RbxContent,
                EnumKeyword, StateSelectorOrEnumPart,
                MacroCallIdentifier, MacroArgIdentifier,
                OpSub
            ]),
            construct_delimiters
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => return (NodeStatus::Err(node), None),
            None => return (NodeStatus::None, None),
        };

        let token = &node.token;

        match token.value() {
            Token::Identifier(_) => self.parse_annotated_table_datatype(node),

            Token::ParensOpen => self.parse_table_datatype(node),

            Token::EnumKeyword => self.parse_enum_datatype(node),

            Token::MacroCallIdentifier(_) => {
                let Parsed (node, construct) = self.parse_macro_call_body(node);
                (node.to_status(), construct)
            },

            Token::StringMulti(MultilineString { nestedness: Err(expected_nestedness), .. }) => {
                self.handle_multiline_string_error(&token, *expected_nestedness);

                (NodeStatus::Exists, Some(Construct::Node { node }))
            },

            Token::OpSub => {
                let next_node = self.advance();
                let (operand_status, operand) = self.parse_datatype_part(next_node, construct_delimiters);
                match operand {
                    Some(operand) => (operand_status, Some(Construct::UnaryMinus {
                        operator: node,
                        operand: Box::new(operand),
                    })),
                    None => (operand_status, Some(Construct::Node { node }))
                }
            },

            _ => (NodeStatus::Exists, Some(Construct::Node { node }))
        }
    }

    fn parse_datatype_operators(
        &mut self, some_middle_node: Node<'a>, precedence: usize
    ) -> (Option<Node<'a>>, Vec<Node<'a>>) {
        let mut operators = vec![some_middle_node];
        let right_node = if precedence == TOKEN_KIND_ADD_SUB_PRECEDENCE {

            // Chains consecuative Add and Sub operators.
            loop {
                let middle_node = self.advance();

                if let Some(some_middle_node) = middle_node {
                    if let Some(precedence) = TOKEN_KIND_OPERATOR_PRECEDENCE.get(&some_middle_node.token.value().kind()) {
                        if *precedence == TOKEN_KIND_ADD_SUB_PRECEDENCE {
                            operators.push(some_middle_node);
                        } else {
                            self.ast_errors.push(
                                ParseError::UnexpectedTokens { msg: None },
                                self.range_from_span(some_middle_node.token.span())
                            );
                        }
                    } else { break Some(some_middle_node) }
                } else { break middle_node }
            }

        } else { self.advance() };

        (right_node, operators)
    }

    fn parse_datatype_operation(
        &mut self,
        node: Option<Node<'a>>,
        last_datatype: Construct<'a>,
        last_precedence: usize,
        last_operators: Vec<Node<'a>>,
        construct_delimiters: &LazyLock<HashSet<TokenKind>>
    ) -> (NodeStatus<'a>, Option<Construct<'a>>) {
        let (node_status, construct) = self.parse_datatype_part(node, construct_delimiters);

        if let Some(construct) = construct {
            let middle_node = match node_status {
                NodeStatus::Exists => self.advance(),
                NodeStatus::None | NodeStatus::Err(_) => return (node_status, Some(Construct::MathOperation {
                    left: Box::new(last_datatype), operators: last_operators, right: Some(Box::new(construct))
                })),
            };

            if let Some(some_middle_node) = middle_node {
                if let Some(precedence) = TOKEN_KIND_OPERATOR_PRECEDENCE.get(&some_middle_node.token.value().kind()) {
                    let (right_node, operators) =
                        self.parse_datatype_operators(some_middle_node, *precedence);

                    if node_token_matches!(right_node, Some(SemiColon)) {
                        self.ast_errors.push(
                            ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected("a datatype")) },
                            self.range_from_span(clamp_span_to_end(operators.last().unwrap().token.end()))
                        );

                        return if *precedence > last_precedence {
                            (right_node.to_status(), Some(Construct::MathOperation {
                                left: Box::new(last_datatype),
                                operators: last_operators,
                                right: Some(Box::new(Construct::MathOperation {
                                    left: Box::new(construct),
                                    operators,
                                    right: None
                                }))
                            }))

                        } else {
                            (right_node.to_status(), Some(Construct::MathOperation {
                                left: Box::new(Construct::MathOperation {
                                    left: Box::new(last_datatype),
                                    operators: last_operators,
                                    right: Some(Box::new(construct))
                                }),
                                operators,
                                right: None
                            }))
                        }
                    }

                    if *precedence > last_precedence {
                        let (node_status, construct) = self.parse_datatype_operation(
                            right_node, construct,
                            *precedence, operators,
                            construct_delimiters
                        );

                        return (node_status, Some(Construct::MathOperation {
                            left: Box::new(last_datatype),
                            operators: last_operators,
                            right: construct.map(Box::new)
                        }))

                    } else {
                        return self.parse_datatype_operation(
                            right_node,
                            Construct::MathOperation {
                                left: Box::new(last_datatype),
                                operators: last_operators,
                                right: Some(Box::new(construct))
                            },
                            *precedence, operators,
                            construct_delimiters
                        )
                    }

                } else {
                    (some_middle_node.to_status(), Some(Construct::MathOperation {
                        left: Box::new(last_datatype), operators: last_operators, right: Some(Box::new(construct))
                    }))
                }

            } else {
                (middle_node.to_status(), Some(Construct::MathOperation {
                    left: Box::new(last_datatype), operators: last_operators, right: Some(Box::new(construct))
                }))
            }

        } else {
            (node_status, Some(Construct::MathOperation {
                left: Box::new(last_datatype), operators: last_operators, right: construct.map(Box::new)
            }))
        }
    }

    fn parse_table_datatype_arg_main(
        &mut self,
        this_node: Option<Node<'a>>,
        datatype_group: Construct<'a>,
        datatype_groups: &mut Vec<Construct<'a>>
    ) -> Option<SymResult<Node<'a>>> {
        let Some(this_node) = this_node else {
            datatype_groups.push(datatype_group);
            return None;
        };

        match this_node.token.value() {
            Token::Comma => {
                let next_node = self.advance();

                if let Some(next_node) = next_node {
                    let next_token_value = next_node.token.value();

                    if matches!(next_token_value, Token::ParensClose) {
                        datatype_groups.push(datatype_group);

                        self.ast_errors.push(
                            ParseError::UnexpectedTokens { msg: None },
                            self.range_from_span(this_node.token.span())
                        );

                        Some(Err(next_node))

                    } else if TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS.contains(&next_token_value.kind()) {
                        Some(Err(next_node))

                    } else {
                        datatype_groups.reserve(2);
                        datatype_groups.push(datatype_group);
                        datatype_groups.push(Construct::Node { node: this_node });

                        Some(Ok(next_node))
                    }

                } else {
                    datatype_groups.reserve(2);
                    datatype_groups.push(datatype_group);
                    datatype_groups.push(Construct::Node { node: this_node });

                    None
                }
            },

            Token::ParensClose => {
                datatype_groups.push(datatype_group);

                Some(Err(this_node))
            },

            token => {
                if TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS.contains(&token.kind()) {
                    datatype_groups.push(datatype_group);

                    Some(Err(this_node))

                } else {
                    self.ast_errors.push(
                        ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::Comma.name())) },
                        self.range_from_span(clamp_span_to_end(datatype_group.end()))
                    );

                    datatype_groups.push(datatype_group);

                    Some(Ok(this_node))
                }
            }
        }
    }

    fn parse_table_datatype_args(&mut self, mut node: Option<Node<'a>>) -> (Option<Node<'a>>, Option<Vec<Construct<'a>>>) {
        let (this_node_status, datatype_group) =
            self.parse_datatype(node, TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS);

        if let Some(datatype_group) = datatype_group {
            let mut datatype_groups = vec![];

            let this_node = this_node_status.consume_err_or_advance(self);
            node = match self.parse_table_datatype_arg_main(this_node, datatype_group, &mut datatype_groups) {
                Some(Ok(node)) => Some(node),
                Some(Err(node)) => return (Some(node), Some(datatype_groups)),
                None => return (None, Some(datatype_groups))
            };

            loop {
                let (this_node_status, datatype_group) =
                    self.parse_datatype(node, TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS);

                let this_node = this_node_status.consume_err_or_advance(self);
                node = if let Some(datatype_group) = datatype_group {
                    match self.parse_table_datatype_arg_main(this_node, datatype_group, &mut datatype_groups) {
                        Some(Ok(node)) => Some(node),
                        Some(Err(node)) => return (Some(node), Some(datatype_groups)),
                        None => return (None, Some(datatype_groups))
                    }
                } else { break (None, None) }
            }
        } else { (None, None) }
    }

    fn parse_table_datatype(&mut self, table_open_node: Node<'a>) -> (NodeStatus<'a>, Option<Construct<'a>>) {
        let node = if let Some(node) = self.advance() {
            let token_value = node.token.value();

            if matches!(token_value, Token::ParensClose) {
                return (NodeStatus::Exists, Some(Construct::Table {
                    body: Delimited::new(table_open_node, None, Some(node))
                }))

            } else if TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS.contains(&token_value.kind()) {
                self.ast_errors.push(
                    ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())) },
                    self.range_from_span(clamp_span_to_end(table_open_node.token.end()))
                );

                return (NodeStatus::Err(node), Some(Construct::Table {
                    body: Delimited::new(table_open_node, None, None)
                }))

            } else { node }

        } else {
            self.ast_errors.push(
                ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())) },
                self.range_from_span(clamp_span_to_end(table_open_node.token.end()))
            );

            return (NodeStatus::None, Some(Construct::Table {
                body: Delimited::new(table_open_node, None, None)
            }))
        };

        let (node, datatype_groups) = self.parse_table_datatype_args(Some(node));

        if !node_token_matches!(node, Some(ParensClose)) {
            let construct = Construct::Table {
                body: Delimited::new(table_open_node, datatype_groups, None)
            };

            self.ast_errors.push(
                ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())) },
                self.range_from_span(clamp_span_to_end(construct.end()))
            );

            return (node.to_status(), Some(construct))
        }

        (NodeStatus::Exists, Some(Construct::Table {
            body: Delimited::new(table_open_node, datatype_groups, node)
        }))
    }

    fn parse_annotated_table_datatype(&mut self, annotation_node: Node<'a>) -> (NodeStatus<'a>, Option<Construct<'a>>) {
        let table_open_node = match self.advance() {
            Some(node @ Node { token: SpannedToken(_, Token::ParensOpen, _), .. }) => node,

            Some(node) => {
                let error_span =
                    if node_token_matches!(node, SemiColon) { annotation_node.token.span() }
                    else { (annotation_node.token.start(), node.token.end()) };

                self.ast_errors.push(
                    ParseError::UnexpectedTokens { msg: Some(ParseErrorMessage::Expected("a datatype")) },
                    self.range_from_span(error_span)
                );

                return (NodeStatus::Err(node), None);
            },

            None => {
                self.ast_errors.push(
                    ParseError::UnexpectedTokens { msg: Some(ParseErrorMessage::Expected("a datatype")) },
                    self.range_from_span(annotation_node.token.span())
                );

                return (NodeStatus::None, None)
            }
        };

        let node = if let Some(node) = self.advance() {
            let token_value = node.token.value();

            if matches!(token_value, Token::ParensClose) {
                return (NodeStatus::Exists, Some(Construct::AnnotatedTable {
                    annotation: annotation_node,
                    body: Some(Delimited::new(table_open_node, None, Some(node)))
                }))

            } else if TOKEN_KIND_INSIDE_PARENS_CONSTRUCT_DELIMITERS.contains(&token_value.kind()) {
                self.ast_errors.push(
                    ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())) },
                    self.range_from_span(clamp_span_to_end(table_open_node.token.end()))
                );

                return (NodeStatus::Err(node), Some(Construct::AnnotatedTable {
                    annotation: annotation_node,
                    body: Some(Delimited::new(table_open_node, None, None))
                }))

            } else { node }

        } else {
            self.ast_errors.push(
                ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())) },
                self.range_from_span(clamp_span_to_end(table_open_node.token.end()))
            );

            return (NodeStatus::None, Some(Construct::AnnotatedTable {
                annotation: annotation_node,
                body: Some(Delimited::new(table_open_node, None, None))
            }))
        };

        let (node, datatype_groups) = self.parse_table_datatype_args(Some(node));

        if !node_token_matches!(node, Some(ParensClose)) {
            let construct = Construct::AnnotatedTable {
                annotation: annotation_node,
                body: Some(Delimited::new(table_open_node, datatype_groups, None))
            };

            self.ast_errors.push(
                ParseError::MissingToken { msg: Some(ParseErrorMessage::Expected(TokenKind::ParensClose.name())) },
                self.range_from_span(clamp_span_to_end(construct.end()))
            );

            return (node.to_status(), Some(construct))
        }

        (NodeStatus::Exists, Some(Construct::AnnotatedTable {
            annotation: annotation_node,
            body: Some(Delimited::new(table_open_node, datatype_groups, node))
        }))
    }

    fn parse_enum_datatype(&mut self, keyword_node: Node<'a>) -> (NodeStatus<'a>, Option<Construct<'a>>) {
        let name_node = match self.advance_until(
            token_kind_list!("enum part", [ StateSelectorOrEnumPart, TagSelectorOrEnumPart ]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => return (NodeStatus::Err(node), Some(Construct::Enum {
                keyword: keyword_node, name: None, variant: None
            })),
            None => return (NodeStatus::None, Some(Construct::Enum {
                keyword: keyword_node, name: None, variant: None
            })),
        };

        let variant_node = match self.advance_until(
            token_kind_list!("enum part", [ StateSelectorOrEnumPart, TagSelectorOrEnumPart ]),
            &TOKEN_KIND_CONSTRUCT_DELIMITERS
        ) {
            Some(Ok(node)) => node,
            Some(Err(node)) => return (NodeStatus::Err(node), Some(Construct::Enum {
                keyword: keyword_node, name: Some(name_node), variant: None
            })),
            None => return (NodeStatus::None, Some(Construct::Enum {
                keyword: keyword_node, name: Some(name_node), variant: None
            })),
        };

        (self.advance().to_status(), Some(Construct::Enum {
            keyword: keyword_node, name: Some(name_node), variant: Some(variant_node)
        }))
    }
}
