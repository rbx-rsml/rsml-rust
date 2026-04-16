use rbx_types_ops::{Abs, Ceil, Floor, Round};

use crate::compiler::datatype::Datatype;

pub fn floor_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let Some(datatype) = datatypes.get(0) else {
        return Datatype::None;
    };

    match datatype {
        Datatype::Variant(variant) => Datatype::Variant(variant.clone().floor()),
        _ => datatype.clone(),
    }
}

pub fn ceil_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let Some(datatype) = datatypes.get(0) else {
        return Datatype::None;
    };

    match datatype {
        Datatype::Variant(variant) => Datatype::Variant(variant.clone().ceil()),
        _ => datatype.clone(),
    }
}

pub fn round_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let Some(datatype) = datatypes.get(0) else {
        return Datatype::None;
    };

    match datatype {
        Datatype::Variant(variant) => Datatype::Variant(variant.clone().round()),
        _ => datatype.clone(),
    }
}

pub fn abs_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let Some(datatype) = datatypes.get(0) else {
        return Datatype::None;
    };

    match datatype {
        Datatype::Variant(variant) => Datatype::Variant(variant.clone().abs()),
        _ => datatype.clone(),
    }
}
