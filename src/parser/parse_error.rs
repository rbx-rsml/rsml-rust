use std::cmp::min;

use levenshtein::levenshtein;
use serde_json::Value;
use crate::types::{Range, Severity};

use crate::collection;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseErrorMessage<'a> {
    Expected(&'a str),
    Correction { closest: Option<&'a str>, range: Range }
}

impl<'a> ParseErrorMessage<'a> {
    pub fn correction<const N: usize>(name: Option<String>, range: Range, allow_list: &[&'static str; N]) -> Self {
        Self::Correction {
            closest:
                if let Some(name) = name { calc_closest(name, allow_list) }
                else { None },
            range
        }
    }
}

impl<'a> ToString for ParseErrorMessage<'a> {
    fn to_string(&self) -> String {
        match self {
            Self::Expected(str) => format!("Expected {str}."),
            Self::Correction { closest, .. } => {
                closest
                    .map(|x| format!("Did you mean {x}?"))
                    .unwrap_or_default()
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError<'a> {
    UnexpectedTokens { msg: Option<ParseErrorMessage<'a>> },
    MissingToken { msg: Option<ParseErrorMessage<'a>> },
}

impl<'a> ParseError<'a> {
    pub fn severity(&self) -> Severity {
        match self {
            Self::UnexpectedTokens { .. } |
            Self::MissingToken { .. } => Severity::Error,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::UnexpectedTokens { msg } => match msg {
                Some(msg) => format!("Unexpected Token(s): {}", msg.to_string()),
                None => String::from("Unexpected Token(s)")
            },

            Self::MissingToken { msg } => match msg {
                Some(msg) => format!("Missing Token: {}", msg.to_string()),
                None => String::from("Missing Token")
            },
        }
    }

    pub fn data(&self) -> Option<Value> {
        match self {
            Self::UnexpectedTokens {
                msg: Some(ParseErrorMessage::Correction { closest, range })
            } | Self::MissingToken {
                msg: Some(ParseErrorMessage::Correction { closest, range })
            } => {
                let (range_start, range_end) = (range.start, range.end);

                closest.as_ref().map(|x| {
                    Value::Object(collection!{
                        "range_start".to_string() => Value::Object(collection!{
                            "line".to_string() => Value::Number((range_start.line).into()),
                            "char".to_string() => Value::Number((range_start.character).into()),
                        }),
                        "range_end".to_string() => Value::Object(collection!{
                            "line".to_string() => Value::Number((range_end.line).into()),
                            "char".to_string() => Value::Number((range_end.character).into()),
                        }),
                        "closest".to_string() => Value::String(x.to_string()),
                    })
                })
            },
            _ => None
        }
    }
}

impl<'a> ToString for ParseError<'a> {
    fn to_string(&self) -> String {
         match self {
            Self::UnexpectedTokens { .. } => "UNEXPECTED_TOKENS",
            Self::MissingToken { .. } => "MISSING_TOKEN",
        }.into()
    }
}

pub fn calc_closest<'a, const N: usize>(name: String, allow_list: &[&'static str; N]) -> Option<&'a str> {
    let name_len = name.len();

    allow_list
        .iter()
        .map(|x| (levenshtein(&name[0..min(name_len, x.len())], x), *x))
        .min_by_key(|x| x.0)
        .map(|x| x.1)
}