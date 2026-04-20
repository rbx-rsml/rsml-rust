use rbx_reflection::{DataType, PropertyDescriptor, PropertyKind, ReflectionDatabase, Scriptability};
use rbx_types::{Variant, VariantType};

use crate::datatype::variant_type_name;

pub(crate) fn lookup_property<'db>(
    db: &'db ReflectionDatabase<'db>,
    class_name: &str,
    property_name: &str,
) -> Option<&'db PropertyDescriptor<'db>> {
    let class_desc = db.classes.get(class_name)?;

    for ancestor in db.superclasses_iter(class_desc) {
        let Some(prop_desc) = ancestor.properties.get(property_name) else {
            continue;
        };

        if matches!(prop_desc.kind, PropertyKind::Alias { .. }) {
            continue;
        }

        if matches!(prop_desc.scriptability, Scriptability::None) {
            continue;
        }

        return Some(prop_desc);
    }

    None
}

pub(crate) fn expected_type_label(desc: &PropertyDescriptor) -> String {
    match &desc.data_type {
        DataType::Value(variant_type) => variant_type_name(*variant_type).to_string(),
        DataType::Enum(enum_name) => format!("Enum.{}", enum_name),
        _ => String::from("unknown"),
    }
}

pub(crate) fn variant_matches(desc: &PropertyDescriptor, value: &Variant) -> bool {
    match &desc.data_type {
        DataType::Value(expected) => variant_assignable(*expected, value.ty()),

        DataType::Enum(expected_enum) => match value {
            Variant::EnumItem(enum_item) => enum_item.ty == expected_enum.as_ref(),
            _ => false,
        },

        _ => true,
    }
}

/// All numeric `VariantType`s share the displayed name `"number"` — assignment
/// should be accepted across them rather than requiring the user to match the
/// exact backing width.
fn variant_assignable(expected: VariantType, got: VariantType) -> bool {
    if expected == got {
        return true;
    }

    matches!(
        (expected, got),
        (
            VariantType::Float32
            | VariantType::Float64
            | VariantType::Int32
            | VariantType::Int64,
            VariantType::Float32
            | VariantType::Float64
            | VariantType::Int32
            | VariantType::Int64,
        ) | (VariantType::Color3, VariantType::Color3uint8)
          | (VariantType::Color3uint8, VariantType::Color3)
          | (VariantType::Content, VariantType::ContentId)
          | (VariantType::ContentId, VariantType::Content)
    )
}
