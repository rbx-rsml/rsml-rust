use phf_macros::phf_map;
use crate::parser::Datatype;
use rbx_types::{BrickColor, CFrame, Color3, Content, Font, FontStyle, FontWeight, Matrix3, NumberRange, Rect, UDim, UDim2, Variant, Vector2, Vector2int16, Vector3, Vector3int16};

mod colorseq;
use colorseq::colorseq_annotation;

mod numseq;
use numseq::numseq_annotation;

mod lerp;
use lerp::lerp_annotation;

mod rounding;
use rounding::{ceil_annotation, floor_annotation, round_annotation};

fn extract_datatype_f32(datatype: Option<&Datatype>) -> Option<f32> {
    match datatype {
        Some(Datatype::Variant(Variant::Float32(float32))) => Some(*float32),
        _ => None
    }
}

fn coerce_datatype_to_f32(datatype: Option<&Datatype>, default: f32) -> f32 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Float32(float32)) => *float32,
            _ => default
        }
    }
    default
}

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

fn udim_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let scale = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let offset = coerce_datatype_to_f32(datatypes.get(1), scale * 100.0);

    Datatype::Variant(Variant::UDim(UDim::new(scale, offset as i32)))
}

fn udim2_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    if datatypes.len() <= 2 {
        let x_component = coerce_datatype_to_udim(datatypes.get(0), UDim::new(0.0, 0));
        let y_component = coerce_datatype_to_udim(datatypes.get(1), x_component);
        return Datatype::Variant(Variant::UDim2(UDim2::new(x_component, y_component)))

    } else {
        let x_scale = coerce_datatype_to_f32(datatypes.get(0), 0.0);
        let x_offset = coerce_datatype_to_f32(datatypes.get(1), 0.0) as i32;
        let y_scale = coerce_datatype_to_f32(datatypes.get(3), 0.0);
        let y_offset = coerce_datatype_to_f32(datatypes.get(4), 0.0) as i32;

        return Datatype::Variant(Variant::UDim2(UDim2::new(
            UDim::new(x_scale, x_offset), UDim::new(y_scale, y_offset)
        )))
    }
}

fn vec2_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);

    Datatype::Variant(Variant::Vector2(Vector2::new(x_component, y_component)))
}

fn vec2i16_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);

    Datatype::Variant(Variant::Vector2int16(Vector2int16::new(x_component as i16, y_component as i16)))
}

fn vec3_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);
    let z_component = coerce_datatype_to_f32(datatypes.get(2), y_component);

    Datatype::Variant(Variant::Vector3(Vector3::new(x_component, y_component, z_component)))
}

fn vec3i16_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);
    let z_component = coerce_datatype_to_f32(datatypes.get(2), y_component);

    Datatype::Variant(Variant::Vector3int16(Vector3int16::new(x_component as i16, y_component as i16, z_component as i16)))
}

fn cframe_annotation(datatypes: &Vec<Datatype>) -> Datatype {
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

// TODO: fix rect annotation in luau version.
fn rect_annotation(datatypes: &Vec<Datatype>) -> Datatype {
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

fn color3_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let first = datatypes.get(0);

    if let Some(Datatype::Variant(Variant::BrickColor(brick_color))) = first {
        Datatype::Variant(Variant::Color3(brick_color.to_color3uint8().into()))

    } else {
        let red = coerce_datatype_to_f32(first, 0.0);
        let green = coerce_datatype_to_f32(datatypes.get(1), red);
        let blue = coerce_datatype_to_f32(datatypes.get(3), green);

        Datatype::Variant(Variant::Color3(Color3::new(red, green, blue)))
    }
}

fn rgb_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let red = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let green = coerce_datatype_to_f32(datatypes.get(1), red);
    let blue = coerce_datatype_to_f32(datatypes.get(3), green);

    return Datatype::Variant(Variant::Color3(Color3::new(red / 255.0, green / 255.0, blue / 255.0)))
}

fn font_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let font_name = if let Some(component) = datatypes.get(0) {
        match component {
            Datatype::Variant(Variant::String(font_str)) => match font_str.starts_with("rbxasset://") {
                true => font_str,
                false => &format!("rbxasset://fonts/families/{}.json", font_str)
            },
            Datatype::Variant(Variant::Float32(num)) => &format!("rbxassetid://{}", num),
            _ => "rbxasset://fonts/families/SourceSansPro.json"
        }
    } else { "rbxasset://fonts/families/SourceSansPro.json" };

    let font_weight = if let Some(component) = datatypes.get(1) {
        match component {
            Datatype::Variant(Variant::String(weight_string)) | 
            Datatype::IncompleteEnumShorthand(weight_string) => match weight_string.as_str() {
                "Thin" => FontWeight::Thin,
                "ExtraLight" => FontWeight::ExtraLight,
                "Light" => FontWeight::Light,
                "Medium" => FontWeight::Medium,
                "SemiBold" => FontWeight::SemiBold,
                "Bold" => FontWeight::Bold,
                "ExtraBold" => FontWeight::ExtraBold,
                "Heavy" => FontWeight::Heavy,
                _ => FontWeight::Regular
            },

            _ => FontWeight::Regular
        }
    } else { FontWeight::Regular };

    let font_style = if let Some(component) = datatypes.get(2) {
        match component {
            Datatype::Variant(Variant::String(style_str)) | 
            Datatype::IncompleteEnumShorthand(style_str) => match style_str.as_str() {
                "Italic" => FontStyle::Italic,
                _ => FontStyle::Normal
            },
            _ => FontStyle::Normal
        }
    } else { FontStyle::Normal };

    Datatype::Variant(Variant::Font(Font::new(font_name, font_weight, font_style)))
}

fn numrange_annotation(datatype: &Vec<Datatype>) -> Datatype {
    let min = coerce_datatype_to_f32(datatype.get(0), 0.0);
    let max = coerce_datatype_to_f32(datatype.get(1), min);
    
    return Datatype::Variant(Variant::NumberRange(NumberRange::new(min, max)))
}

fn content_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let content = match datatypes.get(0) {
        Some(Datatype::Variant(Variant::String(string))) => Content::from(string.to_string()),
        Some(Datatype::Variant(Variant::Float32(float32))) => Content::from(format!("rbxassetid://{float32}")),
        _ => Content::default()
    };

    return Datatype::Variant(Variant::Content(content))
}

fn brickcolor_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    if let Some(Datatype::Variant(Variant::String(string))) = datatypes.get(0) {
        return Datatype::Variant(Variant::BrickColor(BrickColor::from_name(string).unwrap_or(BrickColor::MediumStoneGrey)))
    }

    Datatype::Variant(Variant::BrickColor(BrickColor::MediumStoneGrey))
}


pub static TUPLE_ANNOTATIONS: phf::Map<&'static str, fn(&Vec<Datatype>) -> Datatype> = phf_map! {
    "udim" => udim_annotation,
    "udim2" => udim2_annotation,
    "rect" => rect_annotation,
    "vec2" => vec2_annotation,
    "vec2i16" => vec2i16_annotation,
    "vec3" => vec3_annotation,
    "vec3i16" => vec3i16_annotation,
    "cframe" => cframe_annotation,
    "color3" => color3_annotation,
    "rgb" => rgb_annotation,
    "brickcolor" => brickcolor_annotation,
    "font" => font_annotation,
    "colorseq" => colorseq_annotation,
    "numseq" => numseq_annotation,
    "numrange" => numrange_annotation,
    "content" => content_annotation,

    "lerp" => lerp_annotation,
    "floor" => floor_annotation,
    "ceil" => ceil_annotation,
    "round" => round_annotation
};