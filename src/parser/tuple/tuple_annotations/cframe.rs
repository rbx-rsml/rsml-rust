use rbx_types::{CFrame, Matrix3, Variant, Vector3};

use crate::parser::Datatype;

use super::coerce_datatype_to_f32;

fn coerce_datatype_to_vec3(datatype: Option<&Datatype>, default: Vector3) -> Vector3 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Vector3(vector3)) => *vector3,
            Datatype::Variant(Variant::Vector3int16(vector3i16)) => {
                Vector3::new(vector3i16.x as f32, vector3i16.y as f32, vector3i16.z as f32)
            },
            _ => default
        }
    }
    default
}

pub fn cframe_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let datatypes_0 = datatypes.get(0);

    if let Some(Datatype::Variant(Variant::Float32(pos_x_component))) = datatypes_0 {
        let pos_y_component = coerce_datatype_to_f32(datatypes.get(1), *pos_x_component);
        let pos_z_component = coerce_datatype_to_f32(datatypes.get(2), pos_y_component);

        let orien_x_x_component = coerce_datatype_to_f32(datatypes.get(3), 0.0);
        let orien_x_y_component = coerce_datatype_to_f32(datatypes.get(4), orien_x_x_component);
        let orien_x_z_component = coerce_datatype_to_f32(datatypes.get(5), orien_x_y_component);

        let orien_y_x_component = coerce_datatype_to_f32(datatypes.get(6), 0.0);
        let orien_y_y_component = coerce_datatype_to_f32(datatypes.get(7), orien_y_x_component);
        let orien_y_z_component = coerce_datatype_to_f32(datatypes.get(8), orien_y_y_component);

        let orien_z_x_component = coerce_datatype_to_f32(datatypes.get(9), 0.0);
        let orien_z_y_component = coerce_datatype_to_f32(datatypes.get(10), orien_z_x_component);
        let orien_z_z_component = coerce_datatype_to_f32(datatypes.get(11), orien_z_y_component);

        return Datatype::Variant(Variant::CFrame(CFrame::new(
            Vector3::new(*pos_x_component, pos_y_component, pos_z_component),
            Matrix3::new(
                Vector3::new(orien_x_x_component, orien_x_y_component, orien_x_z_component),
                Vector3::new(orien_y_x_component, orien_y_y_component, orien_y_z_component),
                Vector3::new(orien_z_x_component, orien_z_y_component, orien_z_z_component),
            )
        )))
    } else {
        let pos_component = coerce_datatype_to_vec3(
            datatypes.get(0),
            Vector3::new(0.0, 0.0, 0.0)
        );
        let orien_x_component = coerce_datatype_to_vec3(
            datatypes.get(1),
            Vector3::new(0.0, 0.0, 0.0)
        );
        let orien_y_component = coerce_datatype_to_vec3(
            datatypes.get(2),
            orien_x_component
        );
        let orien_z_component = coerce_datatype_to_vec3(
            datatypes.get(3),
            orien_y_component
        );
    
        return Datatype::Variant(Variant::CFrame(CFrame::new(
            pos_component,
            Matrix3::new(orien_x_component, orien_y_component, orien_z_component)
        )))
    }
}