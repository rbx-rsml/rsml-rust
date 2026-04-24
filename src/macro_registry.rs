use std::collections::HashMap;

use crate::lexer::Token;
use crate::parser::{Construct, Delimited, MacroBodyContent, Node};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroReturnContext {
    Construct,
    Datatype,
    Selector,
}

impl MacroReturnContext {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Construct => "Construct",
            Self::Datatype => "Datatype",
            Self::Selector => "Selector",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MacroDefinition<'a> {
    pub arg_names: Vec<&'a str>,
    pub body: Option<&'a MacroBodyContent<'a>>,
    pub return_context: MacroReturnContext,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct MacroKey<'a> {
    pub name: &'a str,
    pub arity: usize,
}

pub type MacroRegistry<'a> = HashMap<MacroKey<'a>, MacroDefinition<'a>>;

pub fn collect_macro_def_arg_names<'a>(args: &Option<Delimited<'a>>) -> Vec<&'a str> {
    let Some(args) = args else { return Vec::new() };
    let Some(content) = &args.content else {
        return Vec::new();
    };
    content
        .iter()
        .filter_map(|construct| {
            if let Construct::Node { node } = construct {
                if let Token::MacroArgIdentifier(Some(name)) = node.token.value() {
                    return Some(*name);
                }
            }
            None
        })
        .collect()
}

#[cfg(feature = "typechecker")]
pub(crate) fn count_macro_call_args(body: &Option<Delimited>) -> usize {
    let Some(body) = body else { return 0 };
    let Some(content) = &body.content else {
        return 0;
    };
    if content.is_empty() {
        return 0;
    }
    content
        .iter()
        .filter(|construct| {
            matches!(
                construct,
                Construct::Node { node } if matches!(node.token.value(), Token::Comma)
            )
        })
        .count()
        + 1
}

pub fn macro_return_context(return_type: &Option<(Node, Option<Node>)>) -> MacroReturnContext {
    if let Some((_, Some(ident))) = return_type {
        match ident.token.value() {
            Token::Identifier("Datatype") => MacroReturnContext::Datatype,
            Token::Identifier("Selector") => MacroReturnContext::Selector,
            _ => MacroReturnContext::Construct,
        }
    } else {
        MacroReturnContext::Construct
    }
}
