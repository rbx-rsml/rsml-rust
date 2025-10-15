use palette::{FromColor, Srgb};
use rbx_types::{Color3, Color3uint8, Variant};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

pub fn color3_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first = datatypes.get(0);

    if let Some(Datatype::Variant(Variant::BrickColor(brick_color))) = first {
        Datatype::Variant(Variant::Color3(brick_color.to_color3uint8().into()))

    } else if let Some(Datatype::Variant(Variant::Color3uint8(color))) = first {
        Datatype::Variant(Variant::Color3((*color).into()))

    } else if let Some(Datatype::Oklab(color)) = first {
        let color: Srgb<f32> = Srgb::from_color(*color);
        Datatype::Variant(Variant::Color3(Color3::new(color.red, color.green, color.blue)))

    } else if let Some(Datatype::Oklch(color)) = first {
        let color: Srgb<f32> = Srgb::from_color(*color);
        Datatype::Variant(Variant::Color3(Color3::new(color.red, color.green, color.blue)))

    } else {
        let red = coerce_datatype_to_f32(first, 0.0);
        let green = coerce_datatype_to_f32(datatypes.get(1), red);
        let blue = coerce_datatype_to_f32(datatypes.get(2), green);

        Datatype::Variant(Variant::Color3(Color3::new(red, green, blue)))
    }
}

pub fn rgb_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first = datatypes.get(0);

    if let Some(Datatype::Variant(Variant::BrickColor(brick_color))) = first {
        Datatype::Variant(Variant::Color3uint8(brick_color.to_color3uint8()))

    } else if let Some(Datatype::Variant(Variant::Color3uint8(color))) = first {
        Datatype::Variant(Variant::Color3uint8((*color).into()))

    } else if let Some(Datatype::Oklab(color)) = first {
        let color: Srgb<u8> = Srgb::from_color(*color).into();
        Datatype::Variant(Variant::Color3uint8(Color3uint8::new(color.red, color.green, color.blue)))

    } else if let Some(Datatype::Oklch(color)) = first {
        let color: Srgb<u8> = Srgb::from_color(*color).into();
        Datatype::Variant(Variant::Color3uint8(Color3uint8::new(color.red, color.green, color.blue)))

    } else {
        let red = coerce_datatype_to_f32(datatypes.get(0), 0.0);
        let green = coerce_datatype_to_f32(datatypes.get(1), red);
        let blue = coerce_datatype_to_f32(datatypes.get(2), green);

        Datatype::Variant(Variant::Color3uint8(Color3uint8::new(red as u8, green as u8, blue as u8)))
    }
}