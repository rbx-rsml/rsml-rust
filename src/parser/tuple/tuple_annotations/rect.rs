use rbx_types::{Rect, Variant, Vector2};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

fn coerce_datatype_to_vec2(datatype: Option<&Datatype>, default: Vector2) -> Vector2 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Vector2(vector2)) => *vector2,
            Datatype::Variant(Variant::Vector2int16(vector2i16)) => {
                Vector2::new(vector2i16.x as f32, vector2i16.y as f32)
            },
            _ => default
        }
    }
    default
}

pub fn rect_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first = datatypes.get(0);

    if let Some(Datatype::Variant(Variant::Vector2(vec))) = first {
        let max = coerce_datatype_to_vec2(datatypes.get(1), *vec);

        return Datatype::Variant(Variant::Rect(Rect::new(*vec, max)))

    } else {
        let min_x = coerce_datatype_to_f32(first, 0.0);
        let min_y = coerce_datatype_to_f32(datatypes.get(1), min_x);
        let max_x = coerce_datatype_to_f32(datatypes.get(2), min_x);
        let max_y = coerce_datatype_to_f32(datatypes.get(3), min_y);

        return Datatype::Variant(Variant::Rect(Rect::new(
            Vector2::new(min_x, min_y), Vector2::new(max_x, max_y)
        )))
    }
}