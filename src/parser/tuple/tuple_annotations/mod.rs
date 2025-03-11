use phf_macros::phf_map;
use crate::parser::Datatype;
use rbx_types::{Color3, Font, FontStyle, FontWeight, Rect, UDim, UDim2, Variant, Vector2, Vector3};

mod colorseq;
use colorseq::colorseq_annotation;

mod numseq;
use numseq::numseq_annotation;

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

fn udim_annotation(datatypes: &Vec<Datatype>) -> Variant {
    let scale = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let offset = coerce_datatype_to_f32(datatypes.get(1), scale * 100.0);

    return Variant::UDim(UDim::new(scale, offset as i32));
}

fn udim2_annotation(datatypes: &Vec<Datatype>) -> Variant {
    if datatypes.len() <= 2 {
        let x_component = coerce_datatype_to_udim(datatypes.get(0), UDim::new(0.0, 0));
        let y_component = coerce_datatype_to_udim(datatypes.get(1), x_component);
        return Variant::UDim2(UDim2::new(x_component, y_component))

    } else {
        let x_scale = coerce_datatype_to_f32(datatypes.get(0), 0.0);
        let x_offset = coerce_datatype_to_f32(datatypes.get(1), 0.0) as i32;
        let y_scale = coerce_datatype_to_f32(datatypes.get(3), 0.0);
        let y_offset = coerce_datatype_to_f32(datatypes.get(4), 0.0) as i32;

        return Variant::UDim2(UDim2::new(
            UDim::new(x_scale, x_offset), UDim::new(y_scale, y_offset)
        ))
    }
}

fn vec2_annotation(datatypes: &Vec<Datatype>) -> Variant {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);

    return Variant::Vector2(Vector2::new(x_component, y_component))
}

fn vec3_annotation(datatypes: &Vec<Datatype>) -> Variant {
    let x_component = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let y_component = coerce_datatype_to_f32(datatypes.get(1), x_component);
    let z_component = coerce_datatype_to_f32(datatypes.get(2), y_component);

    return Variant::Vector3(Vector3::new(x_component, y_component, z_component))
}

// TODO: add support for 2 vector 2's.
fn rect_annotation(datatypes: &Vec<Datatype>) -> Variant {
    let min_x = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let min_y = coerce_datatype_to_f32(datatypes.get(1), min_x);
    let max_x = coerce_datatype_to_f32(datatypes.get(3), min_x);
    let max_y = coerce_datatype_to_f32(datatypes.get(4), max_x);

    return Variant::Rect(Rect::new(
        Vector2::new(min_x, max_x), Vector2::new(min_y, max_y)
    ))
}

fn color3_annotation(datatypes: &Vec<Datatype>) -> Variant {
    let red = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let green = coerce_datatype_to_f32(datatypes.get(1), red);
    let blue = coerce_datatype_to_f32(datatypes.get(3), green);

    return Variant::Color3(Color3::new(red, green, blue))
}

fn rgb_annotation(datatypes: &Vec<Datatype>) -> Variant {
    let red = coerce_datatype_to_f32(datatypes.get(0), 0.0);
    let green = coerce_datatype_to_f32(datatypes.get(1), red);
    let blue = coerce_datatype_to_f32(datatypes.get(3), green);

    return Variant::Color3(Color3::new(red / 255.0, green / 255.0, blue / 255.0))
}

fn font_annotation(datatypes: &Vec<Datatype>) -> Variant {
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
            Datatype::Variant(Variant::String(weight_str)) => match weight_str.as_str() {
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
            Datatype::Variant(Variant::String(style_str)) => match style_str.as_str() {
                "Italic" => FontStyle::Italic,
                _ => FontStyle::Normal
            },
            _ => FontStyle::Normal
        }
    } else { FontStyle::Normal };

    Variant::Font(Font::new(font_name, font_weight, font_style))
}


fn extract_datatype_f32(datatype: Option<&Datatype>) -> Option<f32> {
    match datatype {
        Some(Datatype::Variant(Variant::Float32(float32))) => Some(*float32),
        _ => None
    }
}

pub static TUPLE_ANNOTATIONS: phf::Map<&'static str, fn(&Vec<Datatype>) -> Variant> = phf_map! {
    "udim" => udim_annotation,
    "udim2" => udim2_annotation,
    "vec2" => vec2_annotation,
    "vec3" => vec3_annotation,
    "rect" => rect_annotation,
    "color3" => color3_annotation,
    "rgb" => rgb_annotation,
    "font" => font_annotation,
    "colorseq" => colorseq_annotation,
    "numseq" => numseq_annotation
};