use rbx_types::{Font, FontStyle, FontWeight, Variant};

use crate::datatype::Datatype;

pub fn font_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    let font_name = if let Some(component) = datatypes.get(0) {
        match component {
            Datatype::Variant(Variant::String(font_str)) => {
                if font_str.starts_with("rbxasset://") || font_str.starts_with("rbxassetid://") {
                    font_str.clone()
                } else {
                    format!("rbxasset://fonts/families/{}.json", font_str)
                }
            }
            Datatype::Variant(Variant::Float32(num)) => format!("rbxassetid://{}", num),
            _ => "rbxasset://fonts/families/SourceSansPro.json".to_string(),
        }
    } else {
        "rbxasset://fonts/families/SourceSansPro.json".to_string()
    };

    let font_weight = if let Some(component) = datatypes.get(1) {
        match component {
            Datatype::Variant(Variant::String(weight_string))
            | Datatype::IncompleteEnumShorthand(weight_string) => match weight_string.as_str() {
                "Thin" | "Enum.FontWeight.Thin" => FontWeight::Thin,
                "ExtraLight" | "Enum.FontWeight.ExtraLight" => FontWeight::ExtraLight,
                "Light" | "Enum.FontWeight.Light" => FontWeight::Light,
                "Regular" | "Enum.FontWeight.Regular" => FontWeight::Regular,
                "Medium" | "Enum.FontWeight.Medium" => FontWeight::Medium,
                "SemiBold" | "Enum.FontWeight.SemiBold" => FontWeight::SemiBold,
                "Bold" | "Enum.FontWeight.Bold" => FontWeight::Bold,
                "ExtraBold" | "Enum.FontWeight.ExtraBold" => FontWeight::ExtraBold,
                "Heavy" | "Enum.FontWeight.Heavy" => FontWeight::Heavy,
                _ => FontWeight::Regular,
            },
            Datatype::Variant(Variant::Float32(float32)) => match *float32 {
                x if x == 100.0 => FontWeight::Thin,
                x if x == 200.0 => FontWeight::ExtraLight,
                x if x == 300.0 => FontWeight::Light,
                x if x == 400.0 => FontWeight::Regular,
                x if x == 500.0 => FontWeight::Medium,
                x if x == 600.0 => FontWeight::SemiBold,
                x if x == 700.0 => FontWeight::Bold,
                x if x == 800.0 => FontWeight::ExtraBold,
                x if x == 900.0 => FontWeight::Heavy,
                _ => FontWeight::Regular,
            },
            _ => FontWeight::Regular,
        }
    } else {
        FontWeight::Regular
    };

    let font_style = if let Some(component) = datatypes.get(2) {
        match component {
            Datatype::Variant(Variant::String(style_str))
            | Datatype::IncompleteEnumShorthand(style_str) => match style_str.as_str() {
                "Italic" | "Enum.FontStyle.Italic" => FontStyle::Italic,
                _ => FontStyle::Normal,
            },
            Datatype::Variant(Variant::Float32(float32)) => match *float32 {
                x if x == 1.0 => FontStyle::Italic,
                _ => FontStyle::Normal,
            },
            _ => FontStyle::Normal,
        }
    } else {
        FontStyle::Normal
    };

    Datatype::Variant(Variant::Font(Font::new(&font_name, font_weight, font_style)))
}
