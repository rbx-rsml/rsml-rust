use std::sync::LazyLock;

use crate::lexer::{RsmlLexer, Token};
use crate::macro_registry::{
    MacroDefinition, MacroKey, MacroRegistry, collect_macro_def_arg_names, macro_return_context,
};
use crate::parser::{Construct, ParsedRsml, RsmlParser};

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
                let arg_names = collect_macro_def_arg_names(args);

                registry.insert(
                    MacroKey {
                        name: *name_str,
                        arity: arg_names.len(),
                    },
                    MacroDefinition {
                        arg_names,
                        body: body.as_ref().map(|b| &b.content),
                        return_context: macro_return_context(return_type),
                    },
                );
            }
        }
    }
    BuiltinData { parsed, registry }
});
