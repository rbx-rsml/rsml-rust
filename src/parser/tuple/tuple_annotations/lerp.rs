use guarded::guarded_unwrap;
use rbx_types::Variant;
use rbx_types_ops::Lerp;

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

macro_rules! op_match_between_two_variants {
    ($method_name:ident, $from:expr, $to:expr, $time:expr, [$($name:ident),*]) => {
        match ($from, $to) {
            $(
                (Datatype::Variant(Variant::$name(from)), Datatype::Variant(Variant::$name(to))) => {
                    Variant::$name(from.$method_name(to, $time))
                }
            )*

            // TODO: find a way to avoid cloning here.
            _ => return $from.clone()
        }
    };
}

pub fn lerp_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let from = guarded_unwrap!(datatypes.get(0), return Datatype::None);
    let to = guarded_unwrap!(datatypes.get(1), return from.clone());
    let time = coerce_datatype_to_f32(datatypes.get(2), 0.5);

    Datatype::Variant(
        op_match_between_two_variants!(
            lerp, from, to, time, [ 
                Float32, UDim, UDim2, Rect, Vector2, Vector2int16,
                Vector3, Vector3int16, CFrame, Color3
            ]
        )
    )
}