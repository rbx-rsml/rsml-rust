use rbx_types::{UDim, UDim2, Variant};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

fn coerce_datatype_to_udim(datatype: Option<&Datatype>, default: UDim) -> UDim {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Float32(float32)) => UDim::new(*float32, 0),
            Datatype::Variant(Variant::UDim(udim)) => *udim,
            _ => default
        }
    }
    default
}

pub fn udim_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let scale = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let offset = coerce_datatype_to_f32(datatypes.get(1), scale * 100.0);

    Datatype::Variant(Variant::UDim(UDim::new(scale, offset as i32)))
}

pub fn udim2_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    if datatypes.len() <= 2 {
        let x_component = coerce_datatype_to_udim(datatypes.get(0), UDim::new(0.0, 0));
        let y_component = coerce_datatype_to_udim(datatypes.get(1), x_component);
        return Datatype::Variant(Variant::UDim2(UDim2::new(x_component, y_component)))

    } else {
        let x_scale = coerce_datatype_to_f32(datatypes.get(0), 0.0);
        let x_offset = coerce_datatype_to_f32(datatypes.get(1), x_scale * 100.0) as i32;
        let y_scale = coerce_datatype_to_f32(datatypes.get(3), x_scale);
        let y_offset = coerce_datatype_to_f32(datatypes.get(4), y_scale * 100.0) as i32;

        return Datatype::Variant(Variant::UDim2(UDim2::new(
            UDim::new(x_scale, x_offset), UDim::new(y_scale, y_offset)
        )))
    }
}