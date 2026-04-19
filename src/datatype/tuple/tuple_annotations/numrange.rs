use rbx_types::{NumberRange, Variant};

use crate::datatype::Datatype;

use crate::datatype::tuple::tuple_annotations::coerce_datatype_to_f32;

pub fn numrange_annotation(datatype: &Vec<Datatype>) -> Datatype {
    let min = coerce_datatype_to_f32(datatype.get(0), 0.0);
    let max = coerce_datatype_to_f32(datatype.get(1), min);

    Datatype::Variant(Variant::NumberRange(NumberRange::new(min, max)))
}
