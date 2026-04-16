use palette::{IntoColor, Oklab, Oklch, Srgb};
use rbx_types::{Color3, EnumItem, Variant};

use super::variants::EnumItemFromNameAndValueName;

#[derive(Clone, Debug)]
pub enum Datatype {
    Variant(Variant),
    TupleData(Vec<Datatype>),
    IncompleteEnumShorthand(String),
    Oklab(Oklab),
    Oklch(Oklch),
    None,
}

impl Datatype {
    pub fn coerce_to_variant(self, key: Option<&str>) -> Option<Variant> {
        match self {
            Datatype::Variant(variant) => Some(variant),

            Datatype::TupleData(tuple_data) => {
                if !tuple_data.is_empty() {
                    tuple_data[0].to_owned().coerce_to_variant(key)
                } else {
                    None
                }
            }

            Datatype::IncompleteEnumShorthand(value) => {
                let key = key?;
                let enum_item = EnumItem::from_name_and_value_name(key, &value)?;
                Some(Variant::EnumItem(enum_item))
            }

            Datatype::Oklab(color) => {
                let color: Srgb<f32> = color.into_color();
                Some(Variant::Color3(Color3::new(color.red, color.green, color.blue)))
            }
            Datatype::Oklch(color) => {
                let color: Srgb<f32> = color.into_color();
                Some(Variant::Color3(Color3::new(color.red, color.green, color.blue)))
            }

            Datatype::None => None,
        }
    }

    pub fn coerce_to_static(self, key: Option<&str>) -> Option<Datatype> {
        match self {
            Datatype::None => None,
            Datatype::IncompleteEnumShorthand(value) => {
                let key = key?;
                let enum_item = EnumItem::from_name_and_value_name(key, &value)?;
                Some(Datatype::Variant(Variant::EnumItem(enum_item)))
            }
            d => Some(d),
        }
    }
}
