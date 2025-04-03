use rbx_types::{Content, Variant};

use crate::parser::Datatype;

pub fn content_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let content = match datatypes.get(0) {
        Some(Datatype::Variant(Variant::String(string))) => Content::from(string.to_string()),
        Some(Datatype::Variant(Variant::Float32(float32))) => Content::from(format!("rbxassetid://{float32}")),
        _ => Content::default()
    };

    return Datatype::Variant(Variant::Content(content))
}