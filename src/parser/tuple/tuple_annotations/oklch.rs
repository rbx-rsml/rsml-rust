use palette::{convert::IntoColor, Oklch, Srgb};
use rbx_types::Variant;

use crate::parser::Datatype;
use super::Remap;

pub fn oklch_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first_datatype = datatypes.get(0);

    if let Some(Datatype::Oklab(color)) = first_datatype {
        Datatype::Oklch((*color).into_color())

    } else if let Some(Datatype::Variant(Variant::Color3uint8(color))) = first_datatype {
        Datatype::Oklch(Srgb::new(color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0).into_color())

    } else if let Some(Datatype::Variant(Variant::Color3(color))) = first_datatype {
        Datatype::Oklch(Srgb::new(color.r, color.g, color.b).into_color())

    } else {
        let l_component = match first_datatype {
            Some(Datatype::Variant(Variant::UDim(udim))) => udim.scale,
            Some(Datatype::Variant(Variant::Float32(float32))) => *float32,
            _ => 0.0
        };
    
        let chroma_component = match datatypes.get(1) {
            Some(Datatype::Variant(Variant::UDim(udim))) => udim.scale.remap((-1.0, 1.0), (-0.4, 0.4)),
            Some(Datatype::Variant(Variant::Float32(float32))) => *float32,
            _ => 0.0
        };
    
        let hue_component = match datatypes.get(2) {
            Some(Datatype::Variant(Variant::Float32(float32))) => *float32,
            _ => 0.0
        };

        Datatype::Oklch(Oklch::new(l_component, chroma_component, hue_component))
    }
}