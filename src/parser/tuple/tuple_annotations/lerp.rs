use guarded::guarded_unwrap;
use num_traits::Num;
use rbx_types::{CFrame, Color3, Matrix3, Rect, UDim, UDim2, Variant, Vector2, Vector2int16, Vector3, Vector3int16};

use crate::parser::datatype::Datatype;

use super::coerce_datatype_to_f32;

macro_rules! implement_from_to_method_for_datatypes {
    ($trait_name:ident, $method_name:ident) => {
        impl $trait_name for f32 {
            fn $method_name(&self, to: &f32, time: f32) -> Self {
                $method_name(*self, *to, time)
            }
        }
        
        impl $trait_name for i32 {
            fn $method_name(&self, to: &i32, time: f32) -> Self {
                $method_name(*self as f32, *to as f32, time) as i32
            }
        }
        
        impl $trait_name for i16 {
            fn $method_name(&self, to: &i16, time: f32) -> Self {
                $method_name(*self as f32, *to as f32, time) as i16
            }
        }
        
        impl $trait_name for UDim {
            fn $method_name(&self, to: &UDim, time: f32) -> Self {
                UDim::new(
                    self.scale.$method_name(&to.scale, time),
                    self.offset.$method_name(&to.offset, time)
                )
            }
        }
        
        impl $trait_name for UDim2 {
            fn $method_name(&self, to: &UDim2, time: f32) -> Self {
                UDim2::new(
                    self.x.$method_name(&to.x, time),
                    self.y.$method_name(&to.y, time)
                )
            }
        }
        
        impl $trait_name for Rect {
            fn $method_name(&self, to: &Rect, time: f32) -> Self {
                Rect::new(
                    self.min.$method_name(&to.min, time),
                    self.max.$method_name(&to.max, time),
                )
            }
        }
        
        impl $trait_name for Vector2 {
            fn $method_name(&self, to: &Vector2, time: f32) -> Self {
                Vector2::new(
                    self.x.$method_name(&to.x, time),
                    self.y.$method_name(&to.y, time)
                )
            }
        }
        
        impl $trait_name for Vector2int16 {
            fn $method_name(&self, to: &Vector2int16, time: f32) -> Self {
                Vector2int16::new(
                    self.x.$method_name(&to.x, time),
                    self.y.$method_name(&to.y, time)
                )
            }
        }
        
        impl $trait_name for Vector3 {
            fn $method_name(&self, to: &Vector3, time: f32) -> Self {
                Vector3::new(
                    self.x.$method_name(&to.x, time),
                    self.y.$method_name(&to.y, time),
                    self.z.$method_name(&to.z, time)
                )
            }
        }
        
        impl $trait_name for Vector3int16 {
            fn $method_name(&self, to: &Vector3int16, time: f32) -> Self {
                Vector3int16::new(
                    self.x.$method_name(&to.x, time),
                    self.y.$method_name(&to.y, time),
                    self.z.$method_name(&to.z, time)
                )
            }
        }
        
        impl $trait_name for Matrix3 {
            fn $method_name(&self, to: &Matrix3, time: f32) -> Self {
                Matrix3::new(
                    self.x.$method_name(&to.x, time),
                    self.y.$method_name(&to.y, time),
                    self.z.$method_name(&to.z, time)
                )
            }
        }
        
        impl $trait_name for CFrame {
            fn $method_name(&self, to: &CFrame, time: f32) -> Self {
                CFrame::new(
                    self.position.$method_name(&to.position, time),
                    self.orientation.$method_name(&to.orientation, time)
                )
            }
        }
        
        impl $trait_name for Color3 {
            fn $method_name(&self, to: &Color3, time: f32) -> Self {
                Color3::new(
                    self.r.$method_name(&to.r, time),
                    self.g.$method_name(&to.g, time),
                    self.b.$method_name(&to.b, time),
                )
            }
        }
    };
}

fn lerp<N: Num + Copy>(from: N, to: N, time: N) -> N {
    from + (to - from) * time
}

trait Lerp {
    fn lerp(&self, to: &Self, time: f32) -> Self;
}
implement_from_to_method_for_datatypes!(Lerp, lerp);

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