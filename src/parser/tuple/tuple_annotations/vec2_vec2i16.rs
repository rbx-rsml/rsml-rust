use rbx_types::{Variant, Vector2, Vector2int16};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

pub fn vec2_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);

    Datatype::Variant(Variant::Vector2(Vector2::new(x_component, y_component)))
}

pub fn vec2i16_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);

    Datatype::Variant(Variant::Vector2int16(Vector2int16::new(x_component as i16, y_component as i16)))
}