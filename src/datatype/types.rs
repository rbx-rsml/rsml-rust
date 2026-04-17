use palette::{IntoColor, Oklab, Oklch, Srgb};
use rbx_types::{Color3, EnumItem, Variant, VariantType};

use super::variants::EnumItemFromNameAndValueName;

#[derive(Clone, Debug, PartialEq)]
pub enum Datatype {
    Variant(Variant),
    TupleData(Vec<Datatype>),
    IncompleteEnumShorthand(String),
    Oklab(Oklab),
    Oklch(Oklch),
    None,
}

// Datatype contains floats (via rbx_types::Variant, Oklab, Oklch) that do not
// legitimately produce NaN from parsed RSML input, so PartialEq is effectively
// reflexive in practice. Eq is required for rangemap coalescing of Definitions.
impl Eq for Datatype {}

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

    pub fn type_name(&self) -> String {
        match self {
            Datatype::Variant(variant) => variant_type_name(variant.ty()).to_string(),
            Datatype::TupleData(items) => {
                let inner: Vec<String> = items.iter().map(Datatype::type_name).collect();
                format!("({})", inner.join(", "))
            }
            Datatype::IncompleteEnumShorthand(_) => "EnumItem".to_string(),
            Datatype::Oklab(_) => "Oklab".to_string(),
            Datatype::Oklch(_) => "Oklch".to_string(),
            Datatype::None => "unknown".to_string(),
        }
    }
}

fn variant_type_name(ty: VariantType) -> &'static str {
    match ty {
        VariantType::Axes => "Axes",
        VariantType::BinaryString => "BinaryString",
        VariantType::Bool => "boolean",
        VariantType::BrickColor => "BrickColor",
        VariantType::CFrame => "CFrame",
        VariantType::Color3 => "Color3",
        VariantType::Color3uint8 => "Color3uint8",
        VariantType::ColorSequence => "ColorSequence",
        VariantType::ContentId => "ContentId",
        VariantType::Enum => "Enum",
        VariantType::Faces => "Faces",
        VariantType::Float32 => "number",
        VariantType::Float64 => "number",
        VariantType::Int32 => "number",
        VariantType::Int64 => "number",
        VariantType::NumberRange => "NumberRange",
        VariantType::NumberSequence => "NumberSequence",
        VariantType::PhysicalProperties => "PhysicalProperties",
        VariantType::Ray => "Ray",
        VariantType::Rect => "Rect",
        VariantType::Ref => "Ref",
        VariantType::Region3 => "Region3",
        VariantType::Region3int16 => "Region3int16",
        VariantType::SharedString => "SharedString",
        VariantType::String => "string",
        VariantType::UDim => "UDim",
        VariantType::UDim2 => "UDim2",
        VariantType::Vector2 => "Vector2",
        VariantType::Vector2int16 => "Vector2int16",
        VariantType::Vector3 => "Vector3",
        VariantType::Vector3int16 => "Vector3int16",
        VariantType::OptionalCFrame => "CFrame?",
        VariantType::Tags => "Tags",
        VariantType::Attributes => "Attributes",
        VariantType::Font => "Font",
        VariantType::UniqueId => "UniqueId",
        VariantType::MaterialColors => "MaterialColors",
        VariantType::SecurityCapabilities => "SecurityCapabilities",
        VariantType::EnumItem => "EnumItem",
        VariantType::Content => "Content",
        _ => "unknown",
    }
}
