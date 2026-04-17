use crate::{
    lexer::{SpannedToken, Token},
    parser::{AstErrors, Construct, Delimited, Node},
};

use super::{PushTypeError, Typechecker, type_error::*};

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
                        TypeError::InvalidType { expected: Some(ExpectedDatatype::Tween) },
                        self.parsed.range_from_span(body.span()),
                    );
                    return;
                }

                if !is_number(args[0]) {
                    ast_errors.push(
                        TypeError::InvalidTweenArg { expected: "number" },
                        self.parsed.range_from_span(args[0].span()),
                    );
                }

                if let Some(arg) = args.get(1) {
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

                if let Some(arg) = args.get(2) {
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

                for arg in args.iter().skip(3) {
                    ast_errors.push(
                        TypeError::InvalidType { expected: Some(ExpectedDatatype::Tween) },
                        self.parsed.range_from_span(arg.span()),
                    );
                }
            }

            _ => {
                ast_errors.push(
                    TypeError::InvalidType { expected: Some(ExpectedDatatype::Tween) },
                    self.parsed.range_from_span(body.span()),
                );
            }
        }
    }
}
