use rbx_types::EnumItem;

pub trait EnumItemFromNameAndValueName {
    fn from_name_and_value_name(enum_name: &str, enum_value_name: &str) -> Option<EnumItem> {
        let enum_descriptor = rbx_reflection_database::get().ok()?.enums.get(enum_name)?;
        let enum_value = enum_descriptor.items.get(enum_value_name)?;

        Some(EnumItem {
            ty: enum_name.to_string(),
            value: *enum_value,
        })
    }
}

impl EnumItemFromNameAndValueName for EnumItem {}
