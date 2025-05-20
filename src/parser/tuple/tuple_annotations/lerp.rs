use guarded::guarded_unwrap;
use palette::{Mix, IntoColor};
use rbx_types::Variant;
use rbx_types_ops::Lerp;

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

macro_rules! op_match_between_two_variants {
    ($method_name:ident, $from:expr, $to:expr, $time:expr, [$($name:ident),*]) => {
        match ($from, $to) {
            (Datatype::Oklab(from), Datatype::Oklab(to)) => Datatype::Oklab(from.mix(*to, $time)),
            (Datatype::Oklab(from), Datatype::Oklch(to)) => Datatype::Oklab(from.mix((*to).into_color(), $time)),
            (Datatype::Oklch(from), Datatype::Oklch(to)) => Datatype::Oklch(from.mix(*to, $time)),
            (Datatype::Oklch(from), Datatype::Oklab(to)) => Datatype::Oklch(from.mix((*to).into_color(), $time)),
            
            // TODO: find a way to avoid cloning here.
            _ => match ($from.clone().coerce_to_variant(None), $to.clone().coerce_to_variant(None)) {
                (Some(from), Some(to)) => match (&from, &to) {
                    $(
                        (Variant::$name(from), Variant::$name(to)) => {
                            Datatype::Variant(Variant::$name(from.$method_name(to, $time)))
                        }
                    )*

                    _ => return Datatype::Variant(from)
                }

                (Some(from), None) => Datatype::Variant(from),

                _ => Datatype::None
            }
        }
    };
}

pub fn lerp_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let from = guarded_unwrap!(datatypes.get(0), return Datatype::None);
    let to = guarded_unwrap!(datatypes.get(1), return from.clone());
    let time = coerce_datatype_to_f32(datatypes.get(2), 0.5);


    op_match_between_two_variants!(
        lerp, from, to, time, [ 
            Float32, UDim, UDim2, Rect, Vector2, Vector2int16,
            Vector3, Vector3int16, CFrame, Color3
        ]
    )
}