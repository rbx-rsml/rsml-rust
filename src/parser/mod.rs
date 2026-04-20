use crate::types::Range;

use crate::lexer::{RsmlLexer, TOKEN_KIND_CONSTRUCT_DELIMITERS, Token, TokenKind};
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

pub struct RsmlParser<'a> {
    pub lexer: RsmlLexer<'a>,
    pub(crate) last_token_end: usize,

    pub ast: Vec<Construct<'a>>,
    pub ast_errors: AstErrors,

    pub did_advance: bool,

    pub directives: Directives,
    pub(crate) pending_node: Option<Node<'a>>,
    pub(crate) directives_phase_done: bool,
}

impl<'a> RsmlParser<'a> {
    pub fn new(lexer: RsmlLexer<'a>) -> ParsedRsml<'a> {
        let mut parser = Self {
            lexer,
            last_token_end: 0,

            ast: Vec::new(),
            ast_errors: AstErrors::new(),

            did_advance: false,

            directives: Directives::default(),
            pending_node: None,
            directives_phase_done: false,
        };

        parser.parse_directives();

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

            node = parser.parse_none(node).handle_construct(&mut parser.ast)?;

            Some(node)
        });

        ParsedRsml {
            ast: parser.ast,
            ast_errors: parser.ast_errors,
            directives: parser.directives,
            rope: parser.lexer.rope,
        }
    }

    pub fn from_source(source: &'a str) -> ParsedRsml<'a> {
        Self::new(RsmlLexer::new(source))
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
        let lexer = crate::lexer::RsmlLexer::new(source);
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
    use crate::parser::*;
    use crate::compiler::RsmlCompiler;

    macro_rules! parser_test {
        ($name:ident, $source:expr) => {
            #[test]
            fn $name() {
                let parsed = RsmlParser::parse_source($source);
                insta::assert_debug_snapshot!(parsed.ast);
            }

            paste::paste! {
                #[test]
                fn [<compiler_ $name>]() {
                    let parsed = RsmlParser::parse_source($source);
                    let compiled = RsmlCompiler::new(parsed);
                    insta::assert_debug_snapshot!(compiled);
                }
            }
        };
    }

    #[test]
    fn unary_minus_in_udim2_expression() {
        let source = r#"$Size = udim2(-20px + 100%, -20px + 100%);"#;
        let parsed = RsmlParser::parse_source(source);

        assert!(parsed.ast_errors.0.is_empty(), "Expected no parse errors, got: {:?}", parsed.ast_errors.0);
        insta::assert_debug_snapshot!(parsed.ast);
    }

    parser_test!(derive_string, r#"@derive "some-module";"#);
    parser_test!(derive_missing_semicolon, r#"@derive "module""#);
    parser_test!(derive_missing_body, r#"@derive"#);
    parser_test!(priority_number, r#"@priority 10;"#);
    parser_test!(priority_missing_semicolon, r#"@priority 10"#);
    parser_test!(tween_simple, r#"@tween MyTween 0.5;"#);
    parser_test!(tween_string_value, r#"@tween Slide "ease-in";"#);
    parser_test!(tween_missing_name, r#"@tween ;"#);
    parser_test!(tween_missing_semicolon, r#"@tween MyTween 0.5"#);

    parser_test!(assign_property_string, r#"Text = "hello";"#);
    parser_test!(assign_property_number, r#"Size = 42;"#);
    parser_test!(assign_property_boolean, r#"Visible = true;"#);
    parser_test!(assign_property_nil, r#"Parent = nil;"#);
    parser_test!(assign_property_missing_value, r#"Text = ;"#);
    parser_test!(assign_property_missing_semicolon, r#"Text = "hello""#);
    parser_test!(assign_token, r#"$Size = 100;"#);
    parser_test!(assign_token_annotated_table, r#"$Size = udim2(1, 0, 1, 0);"#);
    parser_test!(assign_token_missing_semicolon, r#"$Size = 100"#);
    parser_test!(assign_static_token, r#"$!Padding = 10px;"#);
    parser_test!(assign_static_token_missing_value, r#"$!Padding = ;"#);

    parser_test!(value_number_scale, r#"Size = 100%;"#);
    parser_test!(value_number_offset, r#"Size = 20px;"#);
    parser_test!(value_string_double, r#"Text = "hello world";"#);
    parser_test!(value_string_single, r#"Text = 'hello world';"#);
    parser_test!(value_string_multi, r#"Text = [[multi line]];"#);
    parser_test!(value_color_hex, r#"Color = #ff00ff;"#);
    parser_test!(value_color_tailwind, r#"Color = tw:red:500;"#);
    parser_test!(value_color_css, r#"Color = css:tomato;"#);
    parser_test!(value_color_brick, r#"Color = bc:red;"#);
    parser_test!(value_rbx_asset, r#"Image = rbxassetid://12345;"#);
    parser_test!(value_rbx_content, r#"Image = contentid://12345;"#);
    parser_test!(value_enum, r#"SortOrder = Enum.SortOrder.LayoutOrder;"#);
    parser_test!(value_enum_missing_variant, r#"SortOrder = Enum.SortOrder;"#);

    parser_test!(annotated_table_udim2, r#"$Size = udim2(1, 0, 1, 0);"#);
    parser_test!(annotated_table_no_args, r#"$Val = empty();"#);
    parser_test!(annotated_table_nested, r#"$Val = outer(inner(1, 2));"#);
    parser_test!(annotated_table_missing_close, r#"$Val = udim2(1, 0;"#);
    parser_test!(table_bare, r#"$Val = (1, 2, 3);"#);
    parser_test!(table_empty, r#"$Val = ();"#);
    parser_test!(table_nested, r#"$Val = ((1, 2), (3, 4));"#);
    parser_test!(table_missing_close, r#"$Val = (1, 2"#);

    parser_test!(math_add, r#"$Val = 10 + 20;"#);
    parser_test!(math_sub, r#"$Val = 10 - 5;"#);
    parser_test!(math_mult, r#"$Val = 10 * 5;"#);
    parser_test!(math_div, r#"$Val = 10 / 5;"#);
    parser_test!(math_floor_div, r#"$Val = 10 // 3;"#);
    parser_test!(math_mod, r#"$Val = 10 % 3;"#);
    parser_test!(math_pow, r#"$Val = 2 ^ 8;"#);
    parser_test!(math_precedence_add_mult, r#"$Val = 1 + 2 * 3;"#);
    parser_test!(math_precedence_mult_add, r#"$Val = 1 * 2 + 3;"#);
    parser_test!(math_chained_add_sub, r#"$Val = 1 + 2 - 3 + 4;"#);
    parser_test!(unary_minus_simple, r#"$Val = -10;"#);
    parser_test!(unary_minus_in_expression, r#"$Val = -10 + 20;"#);
    parser_test!(math_udim_mixed, r#"$Size = 50% + 10px;"#);
    parser_test!(math_missing_right_operand, r#"$Val = 10 +;"#);

    parser_test!(rule_identifier, r#"Frame { }"#);
    parser_test!(rule_name_selector, r#"#MyFrame { }"#);
    parser_test!(rule_tag_selector, r#".tagged { }"#);
    parser_test!(rule_state_selector, r#":hover { }"#);
    parser_test!(rule_pseudo_selector, r#"::UIPadding { }"#);
    parser_test!(rule_children_selector, r#"Frame > TextLabel { }"#);
    parser_test!(rule_descendants_selector, r#"Frame >> TextLabel { }"#);
    parser_test!(rule_comma_selectors, r#"Frame, TextLabel { }"#);
    parser_test!(rule_comma_three, r#"Frame, TextLabel, ImageLabel { }"#);
    parser_test!(rule_compound, r#"Frame .tag :hover { }"#);
    parser_test!(rule_with_assignment, r#"Frame { Size = 100; }"#);
    parser_test!(rule_with_multiple_assignments, "Frame {\n    Size = 100;\n    Visible = true;\n}");
    parser_test!(rule_nested, r#"Frame { TextLabel { Text = "hi"; } }"#);
    parser_test!(rule_deeply_nested, r#".tag { :hover { Color = #f00; } }"#);
    parser_test!(rule_missing_close_brace, r#"Frame { Size = 100;"#);
    parser_test!(rule_children_missing_part, r#"Frame > { }"#);
    parser_test!(rule_macro_call_selector, r#"sel!(10px) { Size = 100; }"#);
    parser_test!(rule_macro_call_comma, r#"sel!(1), Frame { }"#);

    parser_test!(macro_construct_return, r#"@macro MyMacro -> Construct { Size = 100; }"#);
    parser_test!(macro_args_construct_return, r#"@macro MyMacro(&v) -> Construct { Size = &v; }"#);
    parser_test!(macro_assignment_return, r#"@macro MyColor -> Assignment { #ff0000 }"#);
    parser_test!(macro_assignment_return_args, r#"@macro Scale(&x) -> Assignment { &x }"#);
    parser_test!(macro_selector_return, r#"@macro MySel -> Selector { Frame .tag }"#);
    parser_test!(macro_selector_return_args, r#"@macro MySel(&c) -> Selector { Frame .tag :hover }"#);
    parser_test!(macro_nested_rule, r#"@macro Theme(&c) -> Construct { Frame { Color = &c; } }"#);
    parser_test!(macro_empty_body, r#"@macro MyMacro -> Construct { }"#);
    parser_test!(macro_missing_name, r#"@macro { }"#);
    parser_test!(macro_missing_body, r#"@macro MyMacro"#);
    parser_test!(macro_missing_close_brace, r#"@macro M -> Construct { Size = 1;"#);
    parser_test!(macro_invalid_return_type, r#"@macro M -> Invalid { }"#);
    parser_test!(macro_args_missing_comma, r#"@macro M(&a &b) -> Construct { }"#);

    parser_test!(macro_call_no_args, r#"MyMacro!();"#);
    parser_test!(macro_call_with_args, r#"MyMacro!(10px, 20px);"#);
    parser_test!(macro_call_complex_args, r#"Apply!(#ff0000, 10px, "hello");"#);
    parser_test!(macro_call_missing_semicolon, r#"MyMacro!()"#);
    parser_test!(macro_call_missing_close_paren, r#"MyMacro!(10px;"#);

    parser_test!(builtin_padding_one_arg, r#"Frame { Padding!(10px); }"#);
    parser_test!(builtin_padding_two_args, r#"Frame { Padding!(10px, 20px); }"#);
    parser_test!(builtin_padding_three_args, r#"Frame { Padding!(10px, 20px, 30px); }"#);
    parser_test!(builtin_padding_four_args, r#"Frame { Padding!(10px, 20px, 30px, 40px); }"#);
    parser_test!(builtin_corner_radius, r#"Frame { CornerRadius!(8px); }"#);
    parser_test!(builtin_scale, r#"Frame { Scale!(1.5); }"#);
    parser_test!(macro_call_math_arg, r#"Frame { Padding!(0% + .5); }"#);

    parser_test!(comment_before_assign, "-- comment\nSize = 100;");
    parser_test!(comment_multi_before_assign, r#"--[[comment]] Size = 100;"#);
    parser_test!(comment_multi_nested, r#"--[==[comment]==] Size = 100;"#);
    parser_test!(comment_leading_trivia, "-- a\n-- b\nSize = 100;");

    parser_test!(directive_nobuiltins_alone, "--!nobuiltins");
    parser_test!(directive_nobuiltins_then_code, "--!nobuiltins\nSize = 100;");
    parser_test!(directive_nobuiltins_blocks_builtin_expansion, "--!nobuiltins\nFrame { Padding!(10px); }");
    parser_test!(directive_strict_alone, "--!strict");
    parser_test!(directive_nonstrict_alone, "--!nonstrict");
    parser_test!(directive_after_comment, "-- preface\n--!nobuiltins\nSize = 100;");
    parser_test!(directive_unknown, "--!foo\nSize = 100;");
    parser_test!(directive_empty, "--!\nSize = 100;");
    parser_test!(directive_after_code, "Size = 1;\n--!nobuiltins\nSize = 2;");

    #[test]
    fn directive_sets_nobuiltins_flag() {
        let parsed = RsmlParser::parse_source("--!nobuiltins\nSize = 100;");
        assert!(parsed.directives.nobuiltins);
    }

    #[test]
    fn no_directive_leaves_flag_unset() {
        let parsed = RsmlParser::parse_source("Size = 100;");
        assert!(!parsed.directives.nobuiltins);
    }

    #[test]
    fn directive_sets_strict_language_mode() {
        use crate::types::LanguageMode;
        let parsed = RsmlParser::parse_source("--!strict\nSize = 100;");
        assert_eq!(parsed.directives.language_mode, Some(LanguageMode::Strict));
    }

    #[test]
    fn directive_sets_nonstrict_language_mode() {
        use crate::types::LanguageMode;
        let parsed = RsmlParser::parse_source("--!nonstrict\nSize = 100;");
        assert_eq!(parsed.directives.language_mode, Some(LanguageMode::Nonstrict));
    }

    #[test]
    fn no_directive_leaves_language_mode_unset() {
        let parsed = RsmlParser::parse_source("Size = 100;");
        assert_eq!(parsed.directives.language_mode, None);
    }

    #[test]
    fn directive_after_code_emits_error() {
        let parsed = RsmlParser::parse_source("Size = 1;\n--!nobuiltins");
        assert!(!parsed.directives.nobuiltins);
        assert!(parsed.ast_errors.0.iter().any(|d| d.code == "DIRECTIVE_NOT_AT_TOP"));
    }

    #[test]
    fn unknown_directive_emits_error() {
        let parsed = RsmlParser::parse_source("--!foo\nSize = 1;");
        assert!(parsed.ast_errors.0.iter().any(|d| d.code == "UNKNOWN_DIRECTIVE"));
    }

    #[test]
    fn empty_directive_emits_error() {
        let parsed = RsmlParser::parse_source("--!\nSize = 1;");
        assert!(parsed.ast_errors.0.iter().any(|d| d.code == "EMPTY_DIRECTIVE"));
    }

    parser_test!(query_selector, r#"@media { }"#);
    parser_test!(query_selector_unknown, r#"@foobar { }"#);

    parser_test!(empty_source, r#""#);
    parser_test!(multiple_top_level, "@priority 5;\nFrame { Size = 100; }");
    parser_test!(full_stylesheet, r#"
@derive "base";
@priority 5;
@tween Fade 0.3;

$!PrimaryColor = #3498db;
$Padding = 10px;

@macro Highlight(&color) -> Construct {
    BackgroundColor3 = &color;
}

Frame {
    BackgroundColor3 = $!PrimaryColor;
    Size = udim2(1, -$Padding * 2, 1, -$Padding * 2);

    TextLabel {
        Text = "Hello";
        TextColor3 = css:white;
    }

    :hover {
        BackgroundColor3 = tw:blue:600;
    }
}

#Sidebar, .panel {
    Size = udim2(0, 200px, 1, 0);
}
"#);
    parser_test!(macro_def_and_call, "@macro P(&v) -> Construct { $!P = &v; }\nP!(10px);");
    parser_test!(
        macro_user_nested_expansion,
        "@macro Inner(&v) -> Construct { ::UIPadding { PaddingTop = &v; } }\n@macro Outer(&v) -> Construct { Inner!(&v); }\nFrame { Outer!(10px); }"
    );
    parser_test!(
        macro_recursion_guard,
        "@macro Recur() -> Construct { Recur!(); }\nFrame { Recur!(); }"
    );
    parser_test!(
        macro_overload_cross_call_not_blocked,
        "@macro Foo() -> Construct { Foo!(10px); }\n@macro Foo(&v) -> Construct { ::Inner { X = &v; } }\nFrame { Foo!(); }"
    );
    parser_test!(
        macro_overload_by_arg_count,
        "@macro Set(&a) -> Construct { ::Inner { X = &a; } }\n@macro Set(&a, &b) -> Construct { ::Inner { Y = &b; } }\nFrame { Set!(1px); Set!(2px, 3px); }"
    );
    parser_test!(
        macro_selector_expansion,
        "@macro Foo -> Selector { TextButton }\nFoo!(), Frame { }"
    );
    parser_test!(
        macro_selector_overload,
        "@macro Sel -> Selector { A }\n@macro Sel(&x) -> Selector { B }\nSel!() { }\nSel!(1) { }"
    );
    parser_test!(
        macro_selector_recursion_guard,
        "@macro Loop -> Selector { Loop!() }\nLoop!() { }"
    );
    parser_test!(
        macro_selector_overload_cross_call_not_blocked,
        "@macro Sel -> Selector { Sel!(1) }\n@macro Sel(&x) -> Selector { TextButton }\nSel!() { }"
    );
    parser_test!(
        macro_selector_recursion_inline_comma,
        "@macro Foo -> Selector { TextButton, Foo!() }\nFoo!(), Frame { }"
    );
    parser_test!(
        macro_selector_undefined_dropped,
        "Missing!(), Frame { }"
    );
    parser_test!(
        macro_indirect_recursion_typechecker_error,
        "@macro A() -> Construct { B!(); }\n@macro B() -> Construct { A!(); }\nFrame { A!(); }"
    );
}
