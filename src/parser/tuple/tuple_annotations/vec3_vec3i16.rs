use rbx_types::{Variant, Vector3, Vector3int16};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

pub fn vec3_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);
    let z_component = coerce_datatype_to_f32(datatypes.get(2), y_component);

    Datatype::Variant(Variant::Vector3(Vector3::new(x_component, y_component, z_component)))
}

pub fn vec3i16_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);
    let z_component = coerce_datatype_to_f32(datatypes.get(2), y_component);

    Datatype::Variant(Variant::Vector3int16(Vector3int16::new(x_component as i16, y_component as i16, z_component as i16)))
}