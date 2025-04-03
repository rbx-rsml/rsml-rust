use guarded::guarded_unwrap;
use rbx_types::Variant;
use rbx_types_ops::{Ceil, Floor, Round};

use crate::parser::Datatype;

macro_rules! op_match_variant {
    ($method_name:ident, $datatype:expr, [$($name:ident),*]) => {
        match ($datatype) {
            $(
                Datatype::Variant(Variant::$name(from)) => {
                    Variant::$name(from.$method_name())
                }
            )*

            // TODO: find a way to avoid cloning here.
            _ => return $datatype.clone()
        }
    };
}

pub fn floor_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let datatype = guarded_unwrap!(datatypes.get(0), return Datatype::None);

    Datatype::Variant(
        op_match_variant!(
            floor, datatype, [
                Float32, UDim, UDim2, Rect, Vector2, Vector3, CFrame, Color3
            ]
        )
    )
}

pub fn ceil_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let datatype = guarded_unwrap!(datatypes.get(0), return Datatype::None);

    Datatype::Variant(
        op_match_variant!(
            ceil, datatype, [
                Float32, UDim, UDim2, Rect, Vector2, Vector3, CFrame, Color3
            ]
        )
    )
}

pub fn round_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let datatype = guarded_unwrap!(datatypes.get(0), return Datatype::None);

    Datatype::Variant(
        op_match_variant!(
            round, datatype, [
                Float32, UDim, UDim2, Rect, Vector2, Vector3, CFrame, Color3
            ]
        )
    )
}