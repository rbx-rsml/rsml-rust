use crate::{
    lexer::{SpannedToken, Token},
    parser::{AstErrors, Construct, Delimited, Node},
};

use super::{DefinitionKind, PushTypeError, Typechecker, type_error::*};

fn is_number(construct: &Construct) -> bool {
    matches!(
        construct,
        Construct::Node {
            node: Node {
                token: SpannedToken(_, Token::Number(_), _),
                ..
            },
        }
    )
}

fn is_enum(construct: &Construct, expected_name: &str) -> bool {
    match construct {
        Construct::Enum {
            name:
                Some(Node {
                    token:
                        SpannedToken(
                            _,
                            Token::StateSelectorOrEnumPart(Some(name))
                            | Token::TagSelectorOrEnumPart(Some(name)),
                            _,
                        ),
                    ..
                }),
            ..
        } => *name == expected_name,

        // Enum shorthand like `:InOut`
        Construct::Node {
            node: Node {
                token: SpannedToken(_, Token::StateSelectorOrEnumPart(Some(_)), _),
                ..
            },
        } => true,

        _ => false,
    }
}

fn get_enum_variant<'a>(construct: &'a Construct) -> Option<&'a str> {
    match construct {
        Construct::Enum {
            variant:
                Some(Node {
                    token:
                        SpannedToken(
                            _,
                            Token::StateSelectorOrEnumPart(Some(variant))
                            | Token::TagSelectorOrEnumPart(Some(variant)),
                            _,
                        ),
                    ..
                }),
            ..
        } => Some(variant),

        // Enum shorthand like `:InOut`
        Construct::Node {
            node: Node {
                token: SpannedToken(_, Token::StateSelectorOrEnumPart(Some(variant)), _),
                ..
            },
        } => Some(variant),

        _ => None,
    }
}

fn validate_enum_variant(variant: &str, enum_name: &str) -> bool {
    let Ok(db) = rbx_reflection_database::get() else {
        return true;
    };
    let Some(enum_desc) = db.enums.get(enum_name) else {
        return true;
    };
    enum_desc.items.contains_key(variant)
}

/// Registers definitions for a tween enum arg so completions work correctly:
/// - Shorthand (`:Linear`): `EnumVariant` for the full arg span
/// - Full enum (`Enum.EasingStyle.Linear`): `FilteredEnumName` for the name
///   portion (shows only the correct enum name), then `EnumVariant` for the
///   variant portion (only when the name token has a value)
fn register_enum_arg_definitions(
    arg: &Construct,
    enum_name: &str,
    slot_end: usize,
    definitions: &mut super::Definitions,
) {
    match arg {
        Construct::Enum {
            keyword,
            name,
            variant,
        } => {
            let name_range_start = keyword.token.end();
            let name_range_end = name
                .as_ref()
                .map(|node| node.token.end())
                .unwrap_or(slot_end);

            definitions.insert(
                name_range_start..=name_range_end,
                DefinitionKind::FilteredEnumName {
                    enum_name: enum_name.to_string(),
                },
            );

            if let Some(name_node) = name {
                let has_name = matches!(
                    name_node.token.value(),
                    Token::TagSelectorOrEnumPart(Some(_))
                    | Token::StateSelectorOrEnumPart(Some(_))
                );

                if has_name {
                    let variant_range_start = name_node.token.end();
                    let variant_range_end = variant
                        .as_ref()
                        .map(|node| node.token.end())
                        .unwrap_or(slot_end);

                    definitions.insert(
                        variant_range_start..=variant_range_end,
                        DefinitionKind::EnumVariant {
                            enum_name: enum_name.to_string(),
                        },
                    );
                }
            }
        }

        _ => {
            let arg_span = arg.span();
            definitions.insert(
                arg_span.0..=arg_span.1,
                DefinitionKind::EnumVariant {
                    enum_name: enum_name.to_string(),
                },
            );
        }
    }
}

fn is_comma(construct: &Construct) -> bool {
    matches!(
        construct,
        Construct::Node {
            node: Node {
                token: SpannedToken(_, Token::Comma, _),
                ..
            },
        }
    )
}

impl<'a> Typechecker<'a> {
    pub(super) fn typecheck_tween(
        &self,
        body: &Construct<'a>,
        ast_errors: &mut AstErrors,
        definitions: &mut super::Definitions,
    ) {
        match body {
            // Case 1: bare number — `@tween Prop .5;`
            construct if is_number(construct) => (),

            // Case 2: tuple — `@tween Prop (.5, :InOut, :In);`
            Construct::Table {
                body: Delimited { content: Some(items), .. },
            } => {
                let args: Vec<&Construct<'a>> = items.iter().filter(|item| !is_comma(item)).collect();

                if args.is_empty() {
                    ast_errors.push(
                        TypeError::InvalidType { expected: Some(Datatype::Tween) },
                        self.parsed.range_from_span(body.span()),
                    );
                    return;
                }

                // Arg 0: must be a number
                if !is_number(args[0]) {
                    ast_errors.push(
                        TypeError::InvalidTweenArg { expected: "number" },
                        self.parsed.range_from_span(args[0].span()),
                    );
                }

                let tuple_end = body.span().1;

                // Arg 1: optional, must be Enum.EasingStyle
                if let Some(arg) = args.get(1) {
                    let slot_end = args.get(2).map(|a| a.span().0).unwrap_or(tuple_end);
                    register_enum_arg_definitions(arg, "EasingStyle", slot_end, definitions);
                    if !is_enum(arg, "EasingStyle") {
                        ast_errors.push(
                            TypeError::InvalidTweenArg { expected: "Enum.EasingStyle" },
                            self.parsed.range_from_span(arg.span()),
                        );
                    } else if let Some(variant) = get_enum_variant(arg) {
                        if !validate_enum_variant(variant, "EasingStyle") {
                            ast_errors.push(
                                TypeError::InvalidTweenArg { expected: "a valid Enum.EasingStyle variant" },
                                self.parsed.range_from_span(arg.span()),
                            );
                        }
                    }
                }

                // Arg 2: optional, must be Enum.EasingDirection
                if let Some(arg) = args.get(2) {
                    register_enum_arg_definitions(arg, "EasingDirection", tuple_end, definitions);
                    if !is_enum(arg, "EasingDirection") {
                        ast_errors.push(
                            TypeError::InvalidTweenArg { expected: "Enum.EasingDirection" },
                            self.parsed.range_from_span(arg.span()),
                        );
                    } else if let Some(variant) = get_enum_variant(arg) {
                        if !validate_enum_variant(variant, "EasingDirection") {
                            ast_errors.push(
                                TypeError::InvalidTweenArg { expected: "a valid Enum.EasingDirection variant" },
                                self.parsed.range_from_span(arg.span()),
                            );
                        }
                    }
                }

                // Too many args
                for arg in args.iter().skip(3) {
                    ast_errors.push(
                        TypeError::InvalidType { expected: Some(Datatype::Tween) },
                        self.parsed.range_from_span(arg.span()),
                    );
                }
            }

            // Anything else is invalid
            _ => {
                ast_errors.push(
                    TypeError::InvalidType { expected: Some(Datatype::Tween) },
                    self.parsed.range_from_span(body.span()),
                );
            }
        }
    }
}
