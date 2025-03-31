use guarded::guarded_unwrap;
use num_traits::Float;
use rbx_types::{CFrame, Color3, Matrix3, Rect, UDim, UDim2, Variant, Vector2, Vector3};

use crate::parser::datatype::Datatype;

macro_rules! implement_method_for_datatypes {
    ($trait_name:ident, $method_name:ident) => {
        impl $trait_name for f32 {
            fn $method_name(&self) -> Self {
                Float::$method_name(*self)
            }
        }

        impl $trait_name for UDim {
            fn $method_name(&self) -> Self {
                UDim::new(
                    self.scale.$method_name(),
                    self.offset
                )
            }
        }
        
        impl $trait_name for UDim2 {
            fn $method_name(&self) -> Self {
                UDim2::new(
                    self.x.$method_name(),
                    self.y.$method_name()
                )
            }
        }
        
        impl $trait_name for Rect {
            fn $method_name(&self) -> Self {
                Rect::new(
                    self.min.$method_name(),
                    self.max.$method_name(),
                )
            }
        }
        
        impl $trait_name for Vector2 {
            fn $method_name(&self) -> Self {
                Vector2::new(
                    self.x.$method_name(),
                    self.y.$method_name()
                )
            }
        }
        
        impl $trait_name for Vector3 {
            fn $method_name(&self) -> Self {
                Vector3::new(
                    self.x.$method_name(),
                    self.y.$method_name(),
                    self.z.$method_name()
                )
            }
        }
        
        impl $trait_name for Matrix3 {
            fn $method_name(&self) -> Self {
                Matrix3::new(
                    self.x.$method_name(),
                    self.y.$method_name(),
                    self.z.$method_name()
                )
            }
        }
        
        impl $trait_name for CFrame {
            fn $method_name(&self) -> Self {
                CFrame::new(
                    self.position.$method_name(),
                    self.orientation.$method_name()
                )
            }
        }
        
        impl $trait_name for Color3 {
            fn $method_name(&self) -> Self {
                Color3::new(
                    self.r.$method_name(),
                    self.g.$method_name(),
                    self.b.$method_name(),
                )
            }
        }
    };
}

trait Ceil {
    fn ceil(&self) -> Self;
}
implement_method_for_datatypes!(Ceil, ceil);

trait Floor {
    fn floor(&self) -> Self;
}
implement_method_for_datatypes!(Floor, floor);

trait Round {
    fn round(&self) -> Self;
}
implement_method_for_datatypes!(Round, round);


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