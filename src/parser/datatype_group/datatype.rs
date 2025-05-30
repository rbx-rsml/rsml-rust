use crate::parser::EnumItemFromNameAndValueName;

use super::Operator;

use palette::{IntoColor, Oklab, Oklch, Srgb};
use guarded::guarded_unwrap;
use rbx_types::{Color3, EnumItem, Variant};

#[derive(Clone, Debug)]
pub enum Datatype {
    Variant(Variant),

    TupleData(Vec<Datatype>),

    IncompleteEnumShorthand(String),

    Oklab(Oklab),
    Oklch(Oklch),

    Operator(Operator),

    // We can't use `Option::None` to represent the lack of a datatype
    // as it is used to represent the end of a datatype group.
    None
}

impl Datatype {
    pub fn coerce_to_variant(self, key: Option<&str>) -> Option<Variant> {
        match self {
            Datatype::Variant(variant) => Some(variant),

            Datatype::TupleData(tuple_data) => {
                if tuple_data.len() != 0 {
                    tuple_data[0].to_owned().coerce_to_variant(key)

                } else { None }
            },

            Datatype::IncompleteEnumShorthand(value) => {
                let key = guarded_unwrap!(key, return None);

                let enum_item = guarded_unwrap!(EnumItem::from_name_and_value_name(key, &value), return None);
                Some(Variant::EnumItem(enum_item))
            },

            Datatype::Oklab(color) => {
                let color: Srgb<f32> = color.into_color();
                Some(Variant::Color3(Color3::new(color.red, color.green, color.blue)))
            },
            Datatype::Oklch(color) => {
                let color: Srgb<f32> = color.into_color();
                Some(Variant::Color3(Color3::new(color.red, color.green, color.blue)))
            }

            Datatype::None | Datatype::Operator(_) => None,
        }
    }

    pub fn coerce_to_static(self, key: Option<&str>) -> Option<Datatype> {
        match self {
            Datatype::None | Datatype::Operator(_) => None,
            Datatype::IncompleteEnumShorthand(value) => {
                let key = guarded_unwrap!(key, return None);

                let enum_item = guarded_unwrap!(EnumItem::from_name_and_value_name(key, &value), return None);
                Some(Datatype::Variant(Variant::EnumItem(enum_item)))
            },
            d => Some(d)
        }
    }
}
