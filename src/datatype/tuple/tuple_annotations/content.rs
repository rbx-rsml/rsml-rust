use rbx_types::{Content, Variant};

use crate::datatype::Datatype;

pub fn content_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let content = match datatypes.get(0) {
        Some(Datatype::Variant(Variant::String(string))) => Content::from(string.to_string()),
        Some(Datatype::Variant(Variant::Float64(float64))) => {
            Content::from(format!("rbxassetid://{float64}"))
        }
        _ => Content::default(),
    };

    Datatype::Variant(Variant::Content(content))
}
