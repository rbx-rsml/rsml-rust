use crate::parser::Datatype;
use rbx_types::{Color3, ColorSequence, ColorSequenceKeypoint, Variant};
use std::cmp::Ordering;

use super::extract_datatype_f32;

fn coerce_datatype_to_color3(datatype: Option<&Datatype>, default: Color3) -> Color3 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Color3(color3)) => *color3,
            _ => default
        }
    }
    default
}

fn colorseq_get_color_and_time(datatype: &Datatype) -> (Color3, Option<f32>) {
    match datatype {
        Datatype::TupleData(tuple_data) => {
            let time = extract_datatype_f32(tuple_data.get(0));
            let color = coerce_datatype_to_color3(
                tuple_data.get(1),
                Color3::new(0.0, 0.0, 0.0)
            );

            (color, time)
        },

        Datatype::Variant(Variant::Color3(color3)) => (*color3, None),

        _ => (Color3::new(0.0, 0.0, 0.0), None)
    }
}

#[derive(Debug)]
enum Color {
    ColorSequenceKeypoint(ColorSequenceKeypoint),

    // Used to specify that a color sequence keypoint
    // will be calculated later on.
    Empty
}

impl Color {
    fn coerce_to_keypoint(&self) -> ColorSequenceKeypoint {
        match self {
            Color::ColorSequenceKeypoint(keypoint) => *keypoint,
            _ => unreachable!()
        }
    }
}

fn colorseq_get_start_time(current_idx: usize, colors: &Vec<Color>) -> (f32, f32) {
    if current_idx != 0 {
        for idx in (current_idx - 1)..=0 {
            let color = &colors[idx];
    
            if let Color::ColorSequenceKeypoint(keypoint) = color {
                return (idx as f32, keypoint.time)
            }
        };
    }

    return (0.0, 0.0)
}

fn colorseq_get_end_time(current_idx: usize, colors: &Vec<Color>, colors_len: usize) -> (f32, f32) {
    if current_idx != colors_len {
        for idx in (current_idx + 1)..colors_len {
            let color = &colors[idx];
    
            if let Color::ColorSequenceKeypoint(keypoint) = color {
                return (idx as f32, keypoint.time)
            }
        };
    }

    return ((colors_len - 1) as f32, 1.0)
}

pub fn colorseq_annotation(datatypes: &Vec<Datatype>) -> Variant {
    // If the data only contains one color then we only
    // need to return a color sequence with that color.
    if datatypes.len() == 1 {
        let (color, _) = colorseq_get_color_and_time(&datatypes[0]);
        return Variant::ColorSequence(ColorSequence {
            keypoints: vec![
                ColorSequenceKeypoint::new(0.0, color),
                ColorSequenceKeypoint::new(1.0, color)
            ]
        })
    };

    let mut colors: Vec<Color> = vec![];
    let mut untimed_colors: Vec<(usize, Color3)> = vec![];

    // Separates the colors based on if their time is explicitly stated.
    for (idx, datatype) in datatypes.into_iter().enumerate() {
        let (color, time) = colorseq_get_color_and_time(datatype);

        if let Some(time) = time {
            colors.push(Color::ColorSequenceKeypoint(ColorSequenceKeypoint::new(time, color)));
        } else {
            untimed_colors.push((idx, color));
        }
    };

    colors.sort_unstable_by(|a, b| {
        a.coerce_to_keypoint().time
            .partial_cmp(&b.coerce_to_keypoint().time)
            .unwrap_or(Ordering::Less)
    });

    // We need to insert the colors now to ensure color 
    // times are calculated properly in the next step.
    for (idx, _) in &untimed_colors {
        colors.insert(*idx, Color::Empty)
    };

    let colors_len = colors.len();
    for (idx, color) in &untimed_colors {
        let idx = *idx;
        let (start_idx, start_time) = colorseq_get_start_time(idx, &colors);
        let (end_idx, end_time) = colorseq_get_end_time(idx, &colors, colors_len);

        let time = start_time + (end_time - start_time) * (((idx as f32) - start_idx) / (end_idx - start_idx));

        colors[idx] = Color::ColorSequenceKeypoint(ColorSequenceKeypoint::new(time, *color))
    };

    // Coerces all of the colors to be keypoints.
    let mut colorseq_keypoints: Vec<ColorSequenceKeypoint> = colors.into_iter()
        .map(|item| { item.coerce_to_keypoint() })
        .collect();

    // Ensures that the first keypoint's time is 0.
    let first_keypoint = colorseq_keypoints[0];
    if first_keypoint.time != 0.0 {
        colorseq_keypoints.insert(0, ColorSequenceKeypoint::new(0.0, first_keypoint.color));
    }

    // Ensures that the last keypoint's time is 1.
    let last_keypoint = colorseq_keypoints[colors_len - 1];
    if last_keypoint.time != 1.0 {
        colorseq_keypoints.push(ColorSequenceKeypoint::new(1.0, last_keypoint.color))
    }

    return Variant::ColorSequence(ColorSequence {
        keypoints: colorseq_keypoints
    })
}