use super::Operator;
use rbx_types::Variant;

#[derive(Clone, Debug)]
pub enum Datatype {
    Operator(Operator),
    Variant(Variant),
    TupleData(Vec<Datatype>),

    // What empty tuples resolve to since `None` is
    // used to identify the end of a datatype group.
    Empty
}

impl Datatype {
    pub fn coerce_to_variant(self) -> Option<Variant> {
        match self {
            Datatype::Variant(variant) => Some(variant),
            Datatype::TupleData(tuple_data) => {
                if tuple_data.len() != 0 {
                    tuple_data[0].to_owned().coerce_to_variant()

                } else { None }
            },
            _ => None
        }
    }
}