use rbx_types::{Font, FontStyle, FontWeight, Variant};

use crate::parser::Datatype;

pub fn font_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let font_name = if let Some(component) = datatypes.get(0) {
        match component {
            Datatype::Variant(Variant::String(font_str)) => match font_str.starts_with("rbxasset://") {
                true => font_str,
                false => &format!("rbxasset://fonts/families/{}.json", font_str)
            },
            Datatype::Variant(Variant::Int64(num)) => &format!("rbxassetid://{}", num),
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
                "Regular" => FontWeight::Regular,
                "Medium" => FontWeight::Medium,
                "SemiBold" => FontWeight::SemiBold,
                "Bold" => FontWeight::Bold,
                "ExtraBold" => FontWeight::ExtraBold,
                "Heavy" => FontWeight::Heavy,
                "Enum.FontWeight.Thin" => FontWeight::Thin,
                "Enum.FontWeight.ExtraLight" => FontWeight::ExtraLight,
                "Enum.FontWeight.Light" => FontWeight::Light,
                "Enum.FontWeight.Regular" => FontWeight::Regular,
                "Enum.FontWeight.Medium" => FontWeight::Medium,
                "Enum.FontWeight.SemiBold" => FontWeight::SemiBold,
                "Enum.FontWeight.Bold" => FontWeight::Bold,
                "Enum.FontWeight.ExtraBold" => FontWeight::ExtraBold,
                "Enum.FontWeight.Heavy" => FontWeight::Heavy,
                _ => FontWeight::Regular
            },
            Datatype::Variant(Variant::Float32(float32)) => match *float32 {
                100.0 => FontWeight::Thin,
                200.0 => FontWeight::ExtraLight,
                300.0 => FontWeight::Light,
                400.0 => FontWeight::Regular,
                500.0 => FontWeight::Medium,
                600.0 => FontWeight::SemiBold,
                700.0 => FontWeight::Bold,
                800.0 => FontWeight::ExtraBold,
                900.0 => FontWeight::Heavy,
                _ => FontWeight::Regular
            }
            _ => FontWeight::Regular
        }
    } else { FontWeight::Regular };

    let font_style = if let Some(component) = datatypes.get(2) {
        match component {
            Datatype::Variant(Variant::String(style_str)) | 
            Datatype::IncompleteEnumShorthand(style_str) => match style_str.as_str() {
                "Italic" => FontStyle::Italic,
                "Enum.FontStyle.Italic" => FontStyle::Italic,
                _ => FontStyle::Normal
            },
            Datatype::Variant(Variant::Float32(float32)) => match *float32 {
                1.0 => FontStyle::Italic,
                _ => FontStyle::Normal
            }
            _ => FontStyle::Normal
        }
    } else { FontStyle::Normal };

    Datatype::Variant(Variant::Font(Font::new(font_name, font_weight, font_style)))
}
