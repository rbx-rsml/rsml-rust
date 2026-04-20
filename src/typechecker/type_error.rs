use std::path::PathBuf;

use crate::types::Severity;
use crate::typechecker::normalize_path::NormalizePath;

/// Joins `items` into an Oxford-comma list with the given `conjunction`
/// (e.g. `"or"`, `"and"`). Callers pre-format each item (adding backticks,
/// wrapping in a unit, etc.) so this helper stays punctuation-only.
fn oxford_join(items: &[String], conjunction: &str) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} {} {}", items[0], conjunction, items[1]),
        _ => {
            let (last, leading) = items.split_last().unwrap();
            format!("{}, {} {}", leading.join(", "), conjunction, last)
        }
    }
}

pub enum ExpectedDatatype {
    String,
    Number,
    Tween
}

impl ToString for ExpectedDatatype {
    fn to_string(&self) -> String {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Tween => "number or (number, EasingStyle?, EasingDirection?)"
        }.into()
    }
}

pub enum CyclicKind<'a> {
    Internal,
    External(&'a str)
}

pub enum TypeError<'a> {
    UnknownDerive { path: Option<&'a str> },
    CyclicDerive { kind: CyclicKind<'a> },
    InvalidType { expected: Option<ExpectedDatatype> },
    InvalidTweenArg { expected: &'a str },
    InvalidSelector { msg: Option<&'a str> },
    InvalidMacroArg { msg: &'a str },
    UndefinedMacro { name: &'a str },
    WrongMacroArgCount { name: &'a str, expected: Vec<usize>, got: usize },
    WrongMacroContext { name: &'a str, expected: &'a str, got: &'a str },
    DuplicateMacro { name: &'a str, arg_count: usize },
    RecursiveMacroCall,
    NotAllowedInContext { name: &'a str, context: &'a str },
    UnknownAnnotation { name: &'a str },
    WrongAnnotationArgCount { name: &'a str, expected: Vec<usize>, got: usize },
    WrongAnnotationArgType { arg_index: usize, expected: &'a str },
    UndefinedToken { name: &'a str, is_static: bool },
    UnknownEnum { name: String },
    UnknownEnumVariant { enum_name: String, variant: String },
    UnknownProperty { name: String, missing: Vec<String>, present: Vec<String> },
    PropertyTypeMismatch { name: String, expected: String, got: String },
}

impl<'a> TypeError<'a> {
    pub fn severity(&self) -> Severity {
        match self {
            Self::UnknownDerive { .. } |
            Self::CyclicDerive { .. } |
            Self::InvalidType { .. } |
            Self::InvalidTweenArg { .. } |
            Self::InvalidSelector { .. } |
            Self::InvalidMacroArg { .. } |
            Self::UndefinedMacro { .. } |
            Self::WrongMacroArgCount { .. } |
            Self::WrongMacroContext { .. } |
            Self::DuplicateMacro { .. } |
            Self::RecursiveMacroCall |
            Self::NotAllowedInContext { .. } |
            Self::UnknownAnnotation { .. } |
            Self::WrongAnnotationArgCount { .. } |
            Self::WrongAnnotationArgType { .. } |
            Self::UndefinedToken { .. } |
            Self::UnknownEnum { .. } |
            Self::UnknownEnumVariant { .. } |
            Self::UnknownProperty { .. } |
            Self::PropertyTypeMismatch { .. } => Severity::Error
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::UnknownDerive { path } => match path {
                Some(path) => format!(
                    "Type Error (Unknown Derive): {:#?}",
                    std::path::absolute(path)
                        .unwrap_or(PathBuf::from(path))
                        .normalize()
                ),
                None => String::from("Type Error (Unknown Derive)")
            },

            Self::CyclicDerive { kind } => match kind {
                    CyclicKind::Internal => String::from("Type Error (Cyclic Derive): Cannot derive the current Style Sheet."),
                    CyclicKind::External(ancestry_chain) => format!(
                        "Type Error (Cyclic Derive): {}",
                        ancestry_chain
                    ),
                },

            Self::InvalidType { expected } => match expected {
                Some(expected) => format!("Type Error (Invalid Type): Expected type `{}`.", expected.to_string()),
                None => String::from("Type Error (Invalid Type)")
            },

            Self::InvalidTweenArg { expected } =>
                format!("Type Error (Invalid Tween Argument): Expected `{}`.", expected),

            Self::InvalidSelector { msg } => match msg {
                Some(msg) => format!("Type Error (Invalid Selector): {}", msg),
                None => String::from("Type Error (Invalid Selector)")
            },

            Self::InvalidMacroArg { msg } =>
                format!("Type Error (Invalid Macro Argument): {}", msg),

            Self::UndefinedMacro { name } =>
                format!("Type Error (Undefined Macro): No macro named `{}` has been defined.", name),

            Self::WrongMacroArgCount { name, expected, got } => {
                let mut sorted = expected.clone();
                sorted.sort();

                let expected_str = match sorted.len() {
                    0 => String::from("no arguments"),
                    1 => format!("{} argument{}", sorted[0], if sorted[0] == 1 { "" } else { "s" }),
                    _ => {
                        let parts: Vec<String> = sorted.iter().map(|count| count.to_string()).collect();
                        format!("{} arguments", oxford_join(&parts, "or"))
                    }
                };

                format!(
                    "Type Error (Wrong Macro Argument Count): Macro `{}` expects {}, but {} {} provided.",
                    name, expected_str, got, if *got == 1 { "was" } else { "were" }
                )
            }

            Self::WrongMacroContext { name, expected, got } =>
                format!(
                    "Type Error (Wrong Macro Context): Macro `{}` returns {}, but is used in a {} context.",
                    name, expected, got
                ),

            Self::DuplicateMacro { name, arg_count } =>
                format!(
                    "Type Error (Duplicate Macro): Macro `{}` with {} argument{} has already been defined.",
                    name, arg_count, if *arg_count == 1 { "" } else { "s" }
                ),

            Self::RecursiveMacroCall =>
                String::from("Type Error (Recursive Macro Call): Infinite recursion cycle detected."),

            Self::NotAllowedInContext { name, context } =>
                format!("{} are not allowed in {}.", name, context),

            Self::UnknownAnnotation { name } =>
                format!("Type Error (Unknown Annotation): No annotation named `{}` exists.", name),

            Self::WrongAnnotationArgCount { name, expected, got } => {
                let expected_str = match expected.len() {
                    0 => String::from("no arguments"),
                    1 => format!("{} argument{}", expected[0], if expected[0] == 1 { "" } else { "s" }),
                    _ => {
                        let mut sorted = expected.clone();
                        sorted.sort();
                        let parts: Vec<String> = sorted.iter().map(|n| n.to_string()).collect();
                        format!("{} arguments", parts.join(" or "))
                    }
                };
                format!(
                    "Type Error (Wrong Annotation Argument Count): Annotation `{}` expects {}, but {} {} provided.",
                    name, expected_str, got, if *got == 1 { "was" } else { "were" }
                )
            }

            Self::WrongAnnotationArgType { arg_index, expected } =>
                format!(
                    "Type Error (Wrong Annotation Argument Type): Argument {} must be of type `{}`.",
                    arg_index + 1, expected
                ),

            Self::UndefinedToken { name, is_static } => {
                let sigil = if *is_static { "$!" } else { "$" };
                format!(
                    "Type Error (Undefined Token): Token `{}{}` is not defined.",
                    sigil, name
                )
            }

            Self::UnknownEnum { name } =>
                format!("Type Error (Unknown Enum): No enum named `{}` exists.", name),

            Self::UnknownEnumVariant { enum_name, variant } =>
                format!(
                    "Type Error (Unknown Enum Variant): Enum `{}` has no variant `{}`.",
                    enum_name, variant
                ),

            Self::UnknownProperty { name, missing, present } => {
                let missing_parts: Vec<String> = missing
                    .iter()
                    .map(|class_name| format!("`{}`", class_name))
                    .collect();
                let missing_list = oxford_join(&missing_parts, "or");

                if present.is_empty() {
                    format!(
                        "Type Error (Unknown Property): Property `{}` does not exist on {}.",
                        name, missing_list
                    )
                } else {
                    let present_parts: Vec<String> = present
                        .iter()
                        .map(|class_name| format!("`{}`", class_name))
                        .collect();
                    let present_list = oxford_join(&present_parts, "or");

                    format!(
                        "Type Error (Unknown Property): Property `{}` does not exist on {} but it does exist on {}.",
                        name, missing_list, present_list
                    )
                }
            }

            Self::PropertyTypeMismatch { name, expected, got } =>
                format!(
                    "Type Error (Property Type Mismatch): Property `{}` expects type `{}`, got `{}`.",
                    name, expected, got
                ),
        }
    }

    pub fn data(&self) -> Option<serde_json::Value> {
        None
    }
}

impl<'a> ToString for TypeError<'a> {
    fn to_string(&self) -> String {
        format!("TYPE_ERROR({})", match self {
            Self::UnknownDerive { .. } => "UNKNOWN_DERIVE",
            Self::CyclicDerive { .. } => "CYCLIC_DERIVE",
            Self::InvalidType { .. } => "INVALID_TYPE",
            Self::InvalidTweenArg { .. } => "INVALID_TWEEN_ARG",
            Self::InvalidSelector { .. } => "INVALID_SELECTOR",
            Self::InvalidMacroArg { .. } => "INVALID_MACRO_ARG",
            Self::UndefinedMacro { .. } => "UNDEFINED_MACRO",
            Self::WrongMacroArgCount { .. } => "WRONG_MACRO_ARG_COUNT",
            Self::WrongMacroContext { .. } => "WRONG_MACRO_CONTEXT",
            Self::DuplicateMacro { .. } => "DUPLICATE_MACRO",
            Self::RecursiveMacroCall => "RECURSIVE_MACRO_CALL",
            Self::NotAllowedInContext { .. } => "NOT_ALLOWED_IN_CONTEXT",
            Self::UnknownAnnotation { .. } => "UNKNOWN_ANNOTATION",
            Self::WrongAnnotationArgCount { .. } => "WRONG_ANNOTATION_ARG_COUNT",
            Self::WrongAnnotationArgType { .. } => "WRONG_ANNOTATION_ARG_TYPE",
            Self::UndefinedToken { .. } => "UNDEFINED_TOKEN",
            Self::UnknownEnum { .. } => "UNKNOWN_ENUM",
            Self::UnknownEnumVariant { .. } => "UNKNOWN_ENUM_VARIANT",
            Self::UnknownProperty { .. } => "UNKNOWN_PROPERTY",
            Self::PropertyTypeMismatch { .. } => "PROPERTY_TYPE_MISMATCH",
        })
    }
}