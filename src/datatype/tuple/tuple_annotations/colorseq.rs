use crate::datatype::Datatype;
use rbx_types::{Color3, ColorSequence, ColorSequenceKeypoint, Variant};
use std::cmp::Ordering;

use crate::datatype::tuple::tuple_annotations::extract_datatype_f64;

struct ColorAndTime {
    color: Color3,
    time: Option<f64>,
}

struct UntimedColor {
    idx: usize,
    color: Color3,
}

struct TimePoint {
    idx: f64,
    time: f64,
}

fn coerce_datatype_to_color3(datatype: Option<&Datatype>, default: Color3) -> Color3 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Color3(color3)) => *color3,
            _ => default,
        };
    }
    default
}

fn colorseq_get_color_and_time(datatype: &Datatype) -> ColorAndTime {
    match datatype {
        Datatype::TupleData(tuple_data) => {
            let time = extract_datatype_f64(tuple_data.get(0));
            let color = coerce_datatype_to_color3(tuple_data.get(1), Color3::new(0.0, 0.0, 0.0));

            ColorAndTime { color, time }
        }

        Datatype::Variant(Variant::Color3(color3)) => ColorAndTime { color: *color3, time: None },

        _ => ColorAndTime { color: Color3::new(0.0, 0.0, 0.0), time: None },
    }
}

#[derive(Debug)]
enum Color {
    ColorSequenceKeypoint(ColorSequenceKeypoint),
    Empty,
}

impl Color {
    fn coerce_to_keypoint(&self) -> ColorSequenceKeypoint {
        match self {
            Color::ColorSequenceKeypoint(keypoint) => *keypoint,
            _ => unreachable!(),
        }
    }
}

fn colorseq_get_start_time(current_idx: usize, colors: &Vec<Color>) -> TimePoint {
    if current_idx != 0 {
        for idx in (0..current_idx).rev() {
            let color = &colors[idx];

            if let Color::ColorSequenceKeypoint(keypoint) = color {
                return TimePoint { idx: idx as f64, time: keypoint.time as f64 };
            }
        }
    }

    TimePoint { idx: 0.0, time: 0.0 }
}

fn colorseq_get_end_time(
    current_idx: usize,
    colors: &Vec<Color>,
    colors_len: usize,
) -> TimePoint {
    if current_idx != colors_len {
        for idx in (current_idx + 1)..colors_len {
            let color = &colors[idx];

            if let Color::ColorSequenceKeypoint(keypoint) = color {
                return TimePoint { idx: idx as f64, time: keypoint.time as f64 };
            }
        }
    }

    TimePoint { idx: (colors_len - 1) as f64, time: 1.0 }
}

pub fn colorseq_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    if datatypes.is_empty() {
        return Datatype::None;
    }

    if datatypes.len() == 1 {
        let ColorAndTime { color, .. } = colorseq_get_color_and_time(&datatypes[0]);
        return Datatype::Variant(Variant::ColorSequence(ColorSequence {
            keypoints: vec![
                ColorSequenceKeypoint::new(0.0, color),
                ColorSequenceKeypoint::new(1.0, color),
            ],
        }));
    }

    let mut colors: Vec<Color> = vec![];
    let mut untimed_colors: Vec<UntimedColor> = vec![];

    for (idx, datatype) in datatypes.iter().enumerate() {
        let ColorAndTime { color, time } = colorseq_get_color_and_time(datatype);

        if let Some(time) = time {
            colors.push(Color::ColorSequenceKeypoint(ColorSequenceKeypoint::new(
                time as f32,
                color,
            )));
        } else {
            untimed_colors.push(UntimedColor { idx, color });
        }
    }

    colors.sort_unstable_by(|a, b| {
        a.coerce_to_keypoint()
            .time
            .partial_cmp(&b.coerce_to_keypoint().time)
            .unwrap_or(Ordering::Less)
    });

    for untimed in &untimed_colors {
        colors.insert(untimed.idx, Color::Empty);
    }

    let colors_len = colors.len();
    for untimed in &untimed_colors {
        let idx = untimed.idx;
        let start = colorseq_get_start_time(idx, &colors);
        let end = colorseq_get_end_time(idx, &colors, colors_len);

        let time = start.time
            + (end.time - start.time) * ((idx as f64 - start.idx) / (end.idx - start.idx));

        colors[idx] = Color::ColorSequenceKeypoint(ColorSequenceKeypoint::new(
            time as f32,
            untimed.color,
        ));
    }

    let mut colorseq_keypoints: Vec<ColorSequenceKeypoint> =
        colors.into_iter().map(|item| item.coerce_to_keypoint()).collect();

    let first_keypoint = colorseq_keypoints[0];
    if first_keypoint.time != 0.0 {
        colorseq_keypoints.insert(0, ColorSequenceKeypoint::new(0.0, first_keypoint.color));
    }

    let last_keypoint = colorseq_keypoints[colors_len - 1];
    if last_keypoint.time != 1.0 {
        colorseq_keypoints.push(ColorSequenceKeypoint::new(1.0, last_keypoint.color));
    }

    Datatype::Variant(Variant::ColorSequence(ColorSequence {
        keypoints: colorseq_keypoints,
    }))
}
