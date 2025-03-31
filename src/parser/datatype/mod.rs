use super::Operator;
use guarded::guarded_unwrap;
use rbx_types::Variant;

#[derive(Clone, Debug)]
pub enum Datatype {
    Operator(Operator),
    Variant(Variant),
    TupleData(Vec<Datatype>),

    IncompleteEnumShorthand(String),

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

                // TODO: convert this to its enum member number value (instead of a string) using an api dump.
                Some(Variant::String(format!("Enum.{}.{}", key, value)))
            },
            Datatype::None | Datatype::Operator(_) => None,
        }
    }
}