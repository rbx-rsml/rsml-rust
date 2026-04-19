use palette::{IntoColor, Oklab, Srgb};
use rbx_types::Variant;

use crate::datatype::Datatype;
use crate::datatype::tuple::tuple_annotations::{Remap, RemapRange};

pub fn oklab_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first_datatype = datatypes.get(0);

    if let Some(Datatype::Oklch(color)) = first_datatype {
        Datatype::Oklab((*color).into_color())
    } else if let Some(Datatype::Variant(Variant::Color3uint8(color))) = first_datatype {
        Datatype::Oklab(
            Srgb::new(color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0)
                .into_color(),
        )
    } else if let Some(Datatype::Variant(Variant::Color3(color))) = first_datatype {
        Datatype::Oklab(Srgb::new(color.r, color.g, color.b).into_color())
    } else {
        let l_component: f64 = match first_datatype {
            Some(Datatype::Variant(Variant::UDim(udim))) => udim.scale as f64,
            Some(Datatype::Variant(Variant::Float64(float64))) => *float64,
            _ => 0.0,
        };

        let a_component: f64 = match datatypes.get(1) {
            Some(Datatype::Variant(Variant::UDim(udim))) => (udim.scale as f64).remap(
                RemapRange { start: -1.0, end: 1.0 },
                RemapRange { start: -0.4, end: 0.4 },
            ),
            Some(Datatype::Variant(Variant::Float64(float64))) => *float64,
            _ => 0.0,
        };

        let b_component: f64 = match datatypes.get(2) {
            Some(Datatype::Variant(Variant::UDim(udim))) => (udim.scale as f64).remap(
                RemapRange { start: -1.0, end: 1.0 },
                RemapRange { start: -0.4, end: 0.4 },
            ),
            Some(Datatype::Variant(Variant::Float64(float64))) => *float64,
            _ => 0.0,
        };

        Datatype::Oklab(Oklab::new(
            l_component as f32,
            a_component as f32,
            b_component as f32,
        ))
    }
}
