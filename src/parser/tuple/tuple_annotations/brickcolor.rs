use rbx_types::{BrickColor, Variant};

use crate::parser::Datatype;

pub fn brickcolor_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    if let Some(Datatype::Variant(Variant::String(string))) = datatypes.get(0) {
        return Datatype::Variant(Variant::BrickColor(BrickColor::from_name(string).unwrap_or(BrickColor::MediumStoneGrey)))
    }

    Datatype::Variant(Variant::BrickColor(BrickColor::MediumStoneGrey))
}