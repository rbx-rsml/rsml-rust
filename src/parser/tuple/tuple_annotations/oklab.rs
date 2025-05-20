use palette::{convert::IntoColor, Oklab, Srgb};
use rbx_types::Variant;

use crate::parser::Datatype;
use super::Remap;

pub fn oklab_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first_datatype = datatypes.get(0);

    if let Some(Datatype::Oklch(color)) = first_datatype {
        Datatype::Oklab((*color).into_color())

    } else if let Some(Datatype::Variant(Variant::Color3uint8(color))) = first_datatype {
        Datatype::Oklab(Srgb::new(color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0).into_color())

    } else if let Some(Datatype::Variant(Variant::Color3(color))) = first_datatype {
        Datatype::Oklab(Srgb::new(color.r, color.g, color.b).into_color())

    } else {
        let l_component = match first_datatype {
            Some(Datatype::Variant(Variant::UDim(udim))) => udim.scale,
            Some(Datatype::Variant(Variant::Float32(float32))) => *float32,
            _ => 0.0
        };
    
        let a_component = match datatypes.get(1) {
            Some(Datatype::Variant(Variant::UDim(udim))) => udim.scale.remap((-1.0, 1.0), (-0.4, 0.4)),
            Some(Datatype::Variant(Variant::Float32(float32))) => *float32,
            _ => 0.0
        };
    
        let b_component = match datatypes.get(2) {
            Some(Datatype::Variant(Variant::UDim(udim))) => udim.scale.remap((-1.0, 1.0), (-0.4, 0.4)),
            Some(Datatype::Variant(Variant::Float32(float32))) => *float32,
            _ => 0.0
        };

        Datatype::Oklab(Oklab::new(l_component, a_component, b_component))
    }
}