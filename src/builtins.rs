use std::sync::LazyLock;

use crate::lexer::{RsmlLexer, Token};
use crate::parser::{Construct, ParsedRsml, RsmlParser};
use crate::typechecker::{
    MacroDefinition, MacroRegistry, collect_macro_def_arg_names, macro_return_context,
};

const BUILTINS_SOURCE: &str = include_str!("../builtins.rsml");

pub struct BuiltinData {
    pub parsed: &'static ParsedRsml<'static>,
    pub registry: MacroRegistry<'static>,
}

pub static BUILTINS: LazyLock<BuiltinData> = LazyLock::new(|| {
    let parsed: &'static ParsedRsml<'static> =
        Box::leak(Box::new(RsmlParser::new(RsmlLexer::new(BUILTINS_SOURCE))));

    let mut registry = MacroRegistry::new();
    for construct in &parsed.ast {
        if let Construct::Macro {
            name: Some(name_node),
            args,
            body,
            return_type,
            ..
        } = construct
        {
            if let Token::Identifier(name_str) = name_node.token.value() {
                registry
                    .entry(name_str.to_string())
                    .or_insert_with(Vec::new)
                    .push(MacroDefinition {
                        arg_names: collect_macro_def_arg_names(args),
                        body: body.as_ref().map(|b| &b.content),
                        return_context: macro_return_context(return_type),
                    });
            }
        }
    }
    BuiltinData { parsed, registry }
});
