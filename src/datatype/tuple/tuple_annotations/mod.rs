use phf_macros::phf_map;
use crate::datatype::Datatype;
use rbx_types::Variant;

mod udim_udim2;
use udim_udim2::{udim_annotation, udim2_annotation};

mod rect;
use rect::rect_annotation;

mod vec2_vec2i16;
use vec2_vec2i16::{vec2_annotation, vec2i16_annotation};

mod vec3_vec3i16;
use vec3_vec3i16::{vec3_annotation, vec3i16_annotation};

mod cframe;
use cframe::cframe_annotation;

mod color3_rgb;
use color3_rgb::{color3_annotation, rgb_annotation};

mod oklab;
use oklab::oklab_annotation;

mod oklch;
use oklch::oklch_annotation;

mod brickcolor;
use brickcolor::brickcolor_annotation;

mod colorseq;
use colorseq::colorseq_annotation;

mod numseq;
use numseq::numseq_annotation;

mod numrange;
use numrange::numrange_annotation;

mod font;
use font::font_annotation;

mod content;
use content::content_annotation;

mod lerp;
use lerp::lerp_annotation;

mod floor_ceil_round_abs;
use floor_ceil_round_abs::{abs_annotation, ceil_annotation, floor_annotation, round_annotation};

pub(crate) fn extract_datatype_f64(datatype: Option<&Datatype>) -> Option<f64> {
    match datatype {
        Some(Datatype::Variant(Variant::Float64(float64))) => Some(*float64),
        _ => None,
    }
}

pub(crate) fn coerce_datatype_to_f64(datatype: Option<&Datatype>, default: f64) -> f64 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Float64(float64)) => *float64,
            _ => default,
        };
    }
    default
}

pub(crate) fn coerce_datatype_to_i32(datatype: Option<&Datatype>, default: i32) -> i32 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Float64(float64)) => *float64 as i32,
            _ => default,
        };
    }
    default
}

pub(crate) struct RemapRange {
    pub start: f64,
    pub end: f64,
}

pub(crate) trait Remap {
    fn remap(self, from: RemapRange, to: RemapRange) -> f64;
}

impl Remap for f64 {
    fn remap(self, from: RemapRange, to: RemapRange) -> f64 {
        to.start + ((self - from.start) / (from.end - from.start)) * (to.end - to.start)
    }
}

pub const TUPLE_ANNOTATIONS: phf::Map<&'static str, fn(&Vec<Datatype>) -> Datatype> = phf_map! {
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
    "oklab" => oklab_annotation,
    "oklch" => oklch_annotation,
    "brickcolor" => brickcolor_annotation,
    "colorseq" => colorseq_annotation,
    "numseq" => numseq_annotation,
    "numrange" => numrange_annotation,
    "font" => font_annotation,
    "content" => content_annotation,
    "lerp" => lerp_annotation,
    "floor" => floor_annotation,
    "ceil" => ceil_annotation,
    "round" => round_annotation,
    "abs" => abs_annotation,
};
