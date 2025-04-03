use rbx_types::{NumberRange, Variant};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

pub fn numrange_annotation(datatype: &Vec<Datatype>) -> Datatype {
    let min = coerce_datatype_to_f32(datatype.get(0), 0.0);
    let max = coerce_datatype_to_f32(datatype.get(1), min);
    
    return Datatype::Variant(Variant::NumberRange(NumberRange::new(min, max)))
}