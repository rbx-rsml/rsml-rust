use std::collections::HashMap;

use crate::lexer::Token;
use crate::parser::{self, Delimited};

#[derive(Debug, Clone)]
pub struct SchemaField<'a> {
    pub name: &'a str,
    pub type_name: &'a str,
}

#[derive(Debug, Clone)]
pub struct SchemaDefinition<'a> {
    pub fields: Vec<SchemaField<'a>>,
    pub is_public: bool,
}

pub type SchemaRegistry<'a> = HashMap<&'a str, SchemaDefinition<'a>>;

pub fn collect_schema_fields<'a>(
    body: &Option<Delimited<'a, parser::SchemaField<'a>>>,
) -> Vec<SchemaField<'a>> {
    let Some(body) = body else { return Vec::new() };
    let Some(content) = &body.content else {
        return Vec::new();
    };

    content
        .iter()
        .filter_map(|field| {
            let Token::TokenIdentifier(name) = field.name.token.value() else {
                return None;
            };

            let type_name = type_name_of_field(field)?;

            Some(SchemaField {
                name: *name,
                type_name,
            })
        })
        .collect()
}

fn type_name_of_field<'a>(field: &parser::SchemaField<'a>) -> Option<&'a str> {
    if let Some(colon) = &field.colon {
        if let Token::StateSelectorOrEnumPart(Some(inline)) = colon.token.value() {
            return Some(*inline);
        }
    }

    let type_node = field.type_name.as_ref()?;
    if let Token::Identifier(name) = type_node.token.value() {
        Some(*name)
    } else {
        None
    }
}
