use std::{collections::HashMap, rc::Rc};

use guarded::guarded_unwrap;
use rbx_types::{Attributes, EnumItem, Variant};

pub trait EnumItemFromNameAndValueName {
    fn from_name_and_value_name(enum_name: &str, enum_value_name: &str) -> Option<EnumItem> {
        let enum_descriptor = guarded_unwrap!(rbx_reflection_database::get().enums.get(enum_name), return None);

        let enum_value = guarded_unwrap!(enum_descriptor.items.get(enum_value_name), return None);

        return Some(EnumItem { ty: enum_name.to_string(), value: *enum_value })
    }
}

impl EnumItemFromNameAndValueName for EnumItem {}
