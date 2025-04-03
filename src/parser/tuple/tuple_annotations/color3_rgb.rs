use rbx_types::{Color3, Variant};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

pub fn color3_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first = datatypes.get(0);

    if let Some(Datatype::Variant(Variant::BrickColor(brick_color))) = first {
        Datatype::Variant(Variant::Color3(brick_color.to_color3uint8().into()))

    } else {
        let red = coerce_datatype_to_f32(first, 0.0);
        let green = coerce_datatype_to_f32(datatypes.get(1), red);
        let blue = coerce_datatype_to_f32(datatypes.get(3), green);

        Datatype::Variant(Variant::Color3(Color3::new(red, green, blue)))
    }
}

pub fn rgb_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let red = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let green = coerce_datatype_to_f32(datatypes.get(1), red);
    let blue = coerce_datatype_to_f32(datatypes.get(3), green);

    return Datatype::Variant(Variant::Color3(Color3::new(red / 255.0, green / 255.0, blue / 255.0)))
}