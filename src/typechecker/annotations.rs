use phf_macros::phf_map;
use rbx_types::Variant;

use crate::{
    datatype::Datatype,
    lexer::{SpannedToken, Token},
    parser::{AstErrors, Construct, Node},
};

use super::{PushTypeError, TokenKey, Typechecker, TypecheckerLookup, type_error::*};
use crate::datatype::StaticLookup;

#[derive(Clone, Copy)]
pub enum AnnotationArgType {
    Number,
    Scale,
    Measurement,
    String,
    Color,
    Asset,
    Any,
    Vector2,
    Vector3,
    Tuple(&'static [&'static [AnnotationArgType]]),
    Enum(&'static str),
}

pub struct AnnotationSignature {
    /// Fixed-position args. Each inner slice is a union of accepted types at that position.
    pub head: &'static [&'static [AnnotationArgType]],

    /// When set, the signature accepts any number of extra args after `head`,
    /// each matching this union.
    pub tail: Option<&'static [AnnotationArgType]>,
}

pub struct AnnotationSpec {
    pub signatures: &'static [AnnotationSignature],
}

use AnnotationArgType as Arg;

const COLOR_OR_NUM_COLOR_TUPLE: &[Arg] =
    &[Arg::Color, Arg::Tuple(&[&[Arg::Number], &[Arg::Color]])];

const NUM_OR_NUM3_TUPLE: &[Arg] =
    &[Arg::Number, Arg::Tuple(&[&[Arg::Number], &[Arg::Number], &[Arg::Number]])];

const SCALE_OR_NUMBER: &[Arg] = &[Arg::Scale, Arg::Number];

static ANNOTATION_SPECS: phf::Map<&'static str, AnnotationSpec> = phf_map! {
    "udim" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Measurement]], tail: None },
            AnnotationSignature { head: &[&[Arg::Number], &[Arg::Number]], tail: None },
        ],
    },
    "udim2" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Measurement]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Measurement], &[Arg::Measurement]],
                tail: None,
            },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number], &[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "rect" => AnnotationSpec {
        signatures: &[
            AnnotationSignature {
                head: &[&[Arg::Vector2], &[Arg::Vector2]],
                tail: None,
            },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number], &[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "vec2" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Number]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "vec2i16" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Number]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "vec3" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Number]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number]],
                tail: None,
            },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "vec3i16" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Number]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number]],
                tail: None,
            },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "cframe" => AnnotationSpec {
        signatures: &[
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number], &[Arg::Number]],
                tail: None,
            },
            AnnotationSignature {
                head: &[&[Arg::Vector3], &[Arg::Vector3], &[Arg::Vector3], &[Arg::Vector3]],
                tail: None,
            },
            AnnotationSignature {
                head: &[
                    &[Arg::Number], &[Arg::Number], &[Arg::Number],
                    &[Arg::Number], &[Arg::Number], &[Arg::Number],
                    &[Arg::Number], &[Arg::Number], &[Arg::Number],
                    &[Arg::Number], &[Arg::Number], &[Arg::Number],
                ],
                tail: None,
            },
        ],
    },
    "color3" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Color]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "rgb" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Color]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "oklab" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Color]], tail: None },
            AnnotationSignature {
                head: &[SCALE_OR_NUMBER, SCALE_OR_NUMBER, SCALE_OR_NUMBER],
                tail: None,
            },
        ],
    },
    "oklch" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Color]], tail: None },
            AnnotationSignature {
                head: &[SCALE_OR_NUMBER, SCALE_OR_NUMBER, &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "brickcolor" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::String]], tail: None },
        ],
    },
    "colorseq" => AnnotationSpec {
        signatures: &[
            AnnotationSignature {
                head: &[COLOR_OR_NUM_COLOR_TUPLE],
                tail: Some(COLOR_OR_NUM_COLOR_TUPLE),
            },
        ],
    },
    "numseq" => AnnotationSpec {
        signatures: &[
            AnnotationSignature {
                head: &[NUM_OR_NUM3_TUPLE],
                tail: Some(NUM_OR_NUM3_TUPLE),
            },
        ],
    },
    "numrange" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Number]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Number], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "font" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Asset]], tail: None },
            AnnotationSignature {
                head: &[&[Arg::Asset], &[Arg::Enum("FontWeight")]],
                tail: None,
            },
            AnnotationSignature {
                head: &[
                    &[Arg::Asset],
                    &[Arg::Enum("FontWeight")],
                    &[Arg::Enum("FontStyle")],
                ],
                tail: None,
            },
        ],
    },
    "content" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Asset]], tail: None },
        ],
    },
    "lerp" => AnnotationSpec {
        signatures: &[
            AnnotationSignature {
                head: &[&[Arg::Any], &[Arg::Any]],
                tail: None,
            },
            AnnotationSignature {
                head: &[&[Arg::Any], &[Arg::Any], &[Arg::Number]],
                tail: None,
            },
        ],
    },
    "floor" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Any]], tail: None },
        ],
    },
    "ceil" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Any]], tail: None },
        ],
    },
    "round" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Any]], tail: None },
        ],
    },
    "abs" => AnnotationSpec {
        signatures: &[
            AnnotationSignature { head: &[&[Arg::Any]], tail: None },
        ],
    },
};

fn is_comma(construct: &Construct) -> bool {
    matches!(
        construct,
        Construct::Node {
            node: Node { token: SpannedToken(_, Token::Comma, _), .. },
        }
    )
}

fn annotation_name<'a>(construct: &'a Construct<'a>) -> Option<&'a str> {
    let Construct::AnnotatedTable { annotation, .. } = construct else {
        return None;
    };

    let Token::Identifier(name) = annotation.token.value() else {
        return None;
    };

    Some(name)
}

fn token_matches(construct: &Construct, check: impl Fn(&Token) -> bool) -> bool {
    matches!(
        construct,
        Construct::Node { node: Node { token: SpannedToken(_, token, _), .. } } if check(token)
    )
}

/// Enum names/variants are lexed as either `TagSelectorOrEnumPart` or
/// `StateSelectorOrEnumPart` depending on the preceding token, so both must be accepted.
fn enum_identifier<'a>(token: &Token<'a>) -> Option<&'a str> {
    match token {
        Token::TagSelectorOrEnumPart(Some(name))
        | Token::StateSelectorOrEnumPart(Some(name)) => Some(*name),

        _ => None,
    }
}

fn matches_enum(construct: &Construct, expected_enum: &str) -> bool {
    // Shorthand form like `:Bold` omits the enum name — we have no way to
    // know which enum was intended, so optimistically accept for any expected enum.
    let is_shorthand = matches!(
        construct,
        Construct::Node {
            node: Node {
                token: SpannedToken(_, Token::StateSelectorOrEnumPart(Some(_)), _),
                ..
            },
        }
    );

    if is_shorthand {
        return true;
    }

    let Construct::Enum {
        name: Some(Node { token: SpannedToken(_, name_token, _), .. }),
        variant: Some(Node { token: SpannedToken(_, variant_token, _), .. }),
        ..
    } = construct
    else {
        return false;
    };

    let Some(actual_name) = enum_identifier(name_token) else { return false };

    if actual_name != expected_enum {
        return false;
    }

    let Some(actual_variant) = enum_identifier(variant_token) else { return false };

    validate_enum_variant(actual_variant, expected_enum)
}

fn datatype_matches_arg_type(dt: &Datatype, arg_type: &AnnotationArgType) -> bool {
    match arg_type {
        Arg::Any => !matches!(dt, Datatype::None),

        Arg::Number => matches!(dt, Datatype::Variant(Variant::Float32(_))),

        Arg::Scale => matches!(
            dt,
            Datatype::Variant(Variant::UDim(_)) | Datatype::Variant(Variant::Float32(_))
        ),

        Arg::Measurement => matches!(
            dt,
            Datatype::Variant(Variant::Float32(_)) | Datatype::Variant(Variant::UDim(_))
        ),

        Arg::String => matches!(dt, Datatype::Variant(Variant::String(_))),

        Arg::Color => matches!(
            dt,
            Datatype::Variant(Variant::Color3(_))
                | Datatype::Variant(Variant::BrickColor(_))
                | Datatype::Oklab(_)
                | Datatype::Oklch(_)
        ),

        Arg::Asset => matches!(
            dt,
            Datatype::Variant(Variant::String(_))
                | Datatype::Variant(Variant::Content(_))
                | Datatype::Variant(Variant::Float32(_))
        ),

        Arg::Vector2 => matches!(
            dt,
            Datatype::Variant(Variant::Vector2(_)) | Datatype::Variant(Variant::Vector2int16(_))
        ),

        Arg::Vector3 => matches!(
            dt,
            Datatype::Variant(Variant::Vector3(_)) | Datatype::Variant(Variant::Vector3int16(_))
        ),

        Arg::Enum(name) => match dt {
            Datatype::Variant(Variant::EnumItem(item)) => item.ty == *name,
            Datatype::IncompleteEnumShorthand(_) => true,
            _ => false,
        },

        Arg::Tuple(sig) => match dt {
            Datatype::TupleData(vec) => {
                vec.len() == sig.len()
                    && vec
                        .iter()
                        .zip(sig.iter())
                        .all(|(elem, allowed)| datatype_matches_any_arg_type(elem, allowed))
            }
            _ => false,
        },
    }
}

fn datatype_matches_any_arg_type(dt: &Datatype, allowed: &[AnnotationArgType]) -> bool {
    allowed.iter().any(|t| datatype_matches_arg_type(dt, t))
}

fn validate_enum_variant(variant: &str, enum_name: &str) -> bool {
    // If reflection data is unavailable, fall back to accepting — this matches
    // how `typechecker/tween.rs` handles the same situation.
    let Ok(db) = rbx_reflection_database::get() else {
        return true;
    };

    let Some(enum_descriptor) = db.enums.get(enum_name) else {
        return true;
    };

    enum_descriptor.items.contains_key(variant)
}

fn describe_types(allowed_types: &[AnnotationArgType]) -> String {
    let parts: Vec<String> = allowed_types.iter().map(describe_type).collect();
    parts.join(" | ")
}

fn describe_type(arg_type: &AnnotationArgType) -> String {
    match arg_type {
        Arg::Number => "number".into(),
        Arg::Scale => "scale".into(),
        Arg::Measurement => "measurement".into(),
        Arg::String => "string".into(),
        Arg::Color => "color".into(),
        Arg::Asset => "asset".into(),
        Arg::Any => "any".into(),
        Arg::Vector2 => "Vector2".into(),
        Arg::Vector3 => "Vector3".into(),

        Arg::Tuple(signature) => {
            let parts: Vec<String> =
                signature.iter().map(|&position| describe_types(position)).collect();
            format!("({})", parts.join(", "))
        }

        Arg::Enum(name) => format!("Enum.{}", name),
    }
}

fn signature_accepts_count(signature: &AnnotationSignature, arg_count: usize) -> bool {
    if signature.tail.is_some() {
        arg_count >= signature.head.len()
    } else {
        arg_count == signature.head.len()
    }
}

impl<'a> Typechecker<'a> {
    fn matches_tuple(
        &self,
        construct: &Construct<'a>,
        signature: &[&[AnnotationArgType]],
    ) -> bool {
        let Construct::Table { body } = construct else { return false };
        let Some(content) = &body.content else { return false };

        let inner_args: Vec<&Construct> =
            content.iter().filter(|item| !is_comma(item)).collect();

        if inner_args.len() != signature.len() {
            return false;
        }

        signature
            .iter()
            .zip(inner_args.iter())
            .all(|(allowed, arg)| self.matches_any_type(arg, allowed))
    }

    fn matches_type(&self, construct: &Construct<'a>, arg_type: &AnnotationArgType) -> bool {
        if let Construct::Node {
            node: Node { token: SpannedToken(_, Token::StaticTokenIdentifier(name), _), .. },
        } = construct
        {
            let key = TokenKey { name: name.to_string(), is_static: true };
            let declared = self.declared_tokens.iter().rev().any(|frame| frame.contains(&key));
            if !declared {
                return true;
            }
            let lookup = TypecheckerLookup { scopes: &self.static_scopes };
            let resolved = lookup.resolve_static(name);
            return datatype_matches_arg_type(&resolved, arg_type);
        }

        // Math/unary expressions are accepted for numeric arg types since their
        // result type can't be statically determined without full type inference.
        let is_arithmetic =
            matches!(construct, Construct::MathOperation { .. } | Construct::UnaryMinus { .. });

        match arg_type {
            Arg::Any => true,

            Arg::Number => {
                token_matches(construct, |token| matches!(token, Token::Number(_)))
                    || is_arithmetic
            }

            Arg::Scale => {
                token_matches(construct, |token| matches!(token, Token::NumberScale(_)))
            }

            Arg::Measurement => {
                let matches_token = token_matches(construct, |token| {
                    matches!(
                        token,
                        Token::Number(_) | Token::NumberScale(_) | Token::NumberOffset(_)
                    )
                });

                matches_token || is_arithmetic
            }

            Arg::String => token_matches(construct, |token| {
                matches!(token, Token::StringSingle(_) | Token::StringMulti(_))
            }),

            Arg::Color => {
                let matches_token = token_matches(construct, |token| {
                    matches!(
                        token,
                        Token::ColorHex(_)
                            | Token::ColorTailwind(_)
                            | Token::ColorSkin(_)
                            | Token::ColorBrick(_)
                            | Token::ColorCss(_)
                    )
                });

                let matches_annotation = annotation_name(construct).is_some_and(|name| {
                    matches!(
                        name.to_ascii_lowercase().as_str(),
                        "color3" | "rgb" | "oklab" | "oklch" | "brickcolor"
                    )
                });

                matches_token || matches_annotation
            }

            Arg::Asset => token_matches(construct, |token| {
                matches!(
                    token,
                    Token::RbxAsset(_)
                        | Token::RbxContent(_)
                        | Token::Number(_)
                        | Token::StringSingle(_)
                        | Token::StringMulti(_)
                )
            }),

            Arg::Vector2 => annotation_name(construct).is_some_and(|name| {
                matches!(name.to_ascii_lowercase().as_str(), "vec2" | "vec2i16")
            }),

            Arg::Vector3 => annotation_name(construct).is_some_and(|name| {
                matches!(name.to_ascii_lowercase().as_str(), "vec3" | "vec3i16")
            }),

            Arg::Tuple(signature) => self.matches_tuple(construct, signature),

            Arg::Enum(expected_name) => matches_enum(construct, expected_name),
        }
    }

    fn matches_any_type(
        &self,
        construct: &Construct<'a>,
        allowed_types: &[AnnotationArgType],
    ) -> bool {
        allowed_types.iter().any(|arg_type| self.matches_type(construct, arg_type))
    }

    fn signature_fully_matches(
        &self,
        signature: &AnnotationSignature,
        args: &[&Construct<'a>],
    ) -> bool {
        if !signature_accepts_count(signature, args.len()) {
            return false;
        }

        let head_matches = args
            .iter()
            .zip(signature.head.iter())
            .all(|(arg, allowed)| self.matches_any_type(arg, allowed));

        if !head_matches {
            return false;
        }

        let Some(tail_types) = signature.tail else { return true };

        args[signature.head.len()..]
            .iter()
            .all(|arg| self.matches_any_type(arg, tail_types))
    }

    pub(super) fn validate_annotation(
        &self,
        construct: &Construct<'a>,
        ast_errors: &mut AstErrors,
    ) {
        match construct {
            Construct::AnnotatedTable { annotation, body } => {
                let args: Vec<&Construct<'a>> = body
                    .as_ref()
                    .and_then(|body| body.content.as_deref())
                    .map(|content| content.iter().filter(|item| !is_comma(item)).collect())
                    .unwrap_or_default();

                // Validate arguments first so nested annotations still get checked
                // even when the outer annotation is unknown or has a bad arg count.
                for arg in &args {
                    self.report_tokens_in_annotation(arg, ast_errors);
                    self.validate_annotation(arg, ast_errors);
                }

                let Token::Identifier(name) = annotation.token.value() else {
                    return;
                };

                let name_lower = name.to_ascii_lowercase();

                let Some(spec) = ANNOTATION_SPECS.get(name_lower.as_str()) else {
                    ast_errors.push(
                        TypeError::UnknownAnnotation { name },
                        self.parsed.range_from_span(annotation.token.span()),
                    );
                    return;
                };

                self.check_annotation_args(construct, name, spec, &args, ast_errors);
            }

            Construct::Table { body } => {
                let Some(content) = &body.content else { return };

                for item in content {
                    self.validate_annotation(item, ast_errors);
                }
            }

            Construct::MathOperation { left, right, .. } => {
                self.validate_annotation(left, ast_errors);

                if let Some(right) = right {
                    self.validate_annotation(right, ast_errors);
                }
            }

            Construct::UnaryMinus { operand, .. } => {
                self.validate_annotation(operand, ast_errors);
            }

            _ => (),
        }
    }

    fn check_annotation_args(
        &self,
        annotation_construct: &Construct<'a>,
        name: &'a str,
        spec: &AnnotationSpec,
        args: &[&Construct<'a>],
        ast_errors: &mut AstErrors,
    ) {
        // If any signature matches fully, no error.
        if spec.signatures.iter().any(|signature| self.signature_fully_matches(signature, args)) {
            return;
        }

        // Narrow to signatures that accept this arg count; if none do, the count itself is wrong.
        let count_matches: Vec<&AnnotationSignature> = spec
            .signatures
            .iter()
            .filter(|signature| signature_accepts_count(signature, args.len()))
            .collect();

        if count_matches.is_empty() {
            let valid_counts: Vec<usize> =
                spec.signatures.iter().map(|signature| signature.head.len()).collect();

            ast_errors.push(
                TypeError::WrongAnnotationArgCount {
                    name,
                    expected: valid_counts,
                    got: args.len(),
                },
                self.parsed.range_from_span(annotation_construct.span()),
            );
            return;
        }

        // Arg count fits at least one signature but the types don't — report per-arg
        // type errors against the first such signature.
        let signature = count_matches[0];

        for (index, arg) in args.iter().enumerate() {
            let allowed_types = if index < signature.head.len() {
                signature.head[index]
            } else {
                signature.tail.unwrap_or(&[])
            };

            if self.matches_any_type(arg, allowed_types) {
                continue;
            }

            let expected_description = describe_types(allowed_types);
            ast_errors.push(
                TypeError::WrongAnnotationArgType {
                    arg_index: index,
                    expected: &expected_description,
                },
                self.parsed.range_from_span(arg.span()),
            );
        }
    }

    /// Walks an annotation argument and emits an error for every token
    /// reference (`$Token`) it finds. Static tokens (`$!Token`) are allowed
    /// since they resolve at compile time.
    /// Stops at nested `AnnotatedTable` boundaries — those get their own walk
    /// when `validate_annotation` recurses into them.
    fn report_tokens_in_annotation(
        &self,
        construct: &Construct<'a>,
        ast_errors: &mut AstErrors,
    ) {
        match construct {
            Construct::Node { node } => {
                let token = node.token.value();

                if !matches!(token, Token::TokenIdentifier(_)) {
                    return;
                }

                ast_errors.push(
                    TypeError::NotAllowedInContext {
                        name: "Tokens",
                        context: "tuple annotations",
                    },
                    self.parsed.range_from_span(node.token.span()),
                );
            }

            Construct::Table { body } => {
                let Some(content) = &body.content else { return };

                for item in content {
                    self.report_tokens_in_annotation(item, ast_errors);
                }
            }

            Construct::MathOperation { left, right, .. } => {
                self.report_tokens_in_annotation(left, ast_errors);

                if let Some(right) = right {
                    self.report_tokens_in_annotation(right, ast_errors);
                }
            }

            Construct::UnaryMinus { operand, .. } => {
                self.report_tokens_in_annotation(operand, ast_errors);
            }

            _ => (),
        }
    }
}
