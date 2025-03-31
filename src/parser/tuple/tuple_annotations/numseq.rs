use crate::parser::Datatype;
use rbx_types::{NumberSequence, NumberSequenceKeypoint, Variant};
use std::cmp::Ordering;

use super::extract_datatype_f32;

fn coerce_datatype_to_f32(datatype: Option<&Datatype>, default: f32) -> f32 {
    if let Some(datatype) = datatype {
        return match datatype {
            Datatype::Variant(Variant::Float32(float32)) => *float32,
            _ => default
        }
    }
    default
}

fn numseq_get_number_time_and_envelope(datatype: &Datatype) -> (f32, Option<f32>, f32) {
    match datatype {
        Datatype::TupleData(tuple_data) => {
            let time = extract_datatype_f32(tuple_data.get(0));
            let number = coerce_datatype_to_f32(
                tuple_data.get(1),
                0.0
            );
            let envelope = coerce_datatype_to_f32(
                tuple_data.get(2),
                0.0
            );

            (number, time, envelope)
        },

        Datatype::Variant(Variant::Float32(float32)) => (*float32, None, 0.0),

        _ => (0.0, None, 0.0)
    }
}

#[derive(Debug)]
enum Number {
    NumberSequenceKeypoint(NumberSequenceKeypoint),

    // Used to specify that a number sequence keypoint
    // will be calculated later on.
    Empty
}

impl Number {
    fn coerce_to_keypoint(&self) -> NumberSequenceKeypoint {
        match self {
            Number::NumberSequenceKeypoint(keypoint) => *keypoint,
            _ => unreachable!()
        }
    }
}

fn numseq_get_start_time(current_idx: usize, numbers: &Vec<Number>) -> (f32, f32) {
    if current_idx != 0 {
        for idx in (current_idx - 1)..=0 {
            let float32 = &numbers[idx];
    
            if let Number::NumberSequenceKeypoint(keypoint) = float32 {
                return (idx as f32, keypoint.time)
            }
        };
    }

    return (0.0, 0.0)
}

fn numseq_get_end_time(current_idx: usize, numbers: &Vec<Number>, numbers_len: usize) -> (f32, f32) {
    if current_idx != numbers_len {
        for idx in (current_idx + 1)..numbers_len {
            let color = &numbers[idx];
    
            if let Number::NumberSequenceKeypoint(keypoint) = color {
                return (idx as f32, keypoint.time)
            }
        };
    }

    return ((numbers_len - 1) as f32, 1.0)
}

pub fn numseq_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    // If the data only contains one color then we only
    // need to return a color sequence with that color.
    if datatypes.len() == 1 {
        let (number, _, envelope) = numseq_get_number_time_and_envelope(&datatypes[0]);
        return Datatype::Variant(Variant::NumberSequence(NumberSequence {
            keypoints: vec![
                NumberSequenceKeypoint::new(0.0, number, envelope),
                NumberSequenceKeypoint::new(1.0, number, envelope)
            ]
        }))
    };

    let mut numbers: Vec<Number> = vec![];
    let mut untimed_numbers: Vec<(usize, f32, f32)> = vec![];

    // Separates the colors based on if their time is explicitly stated.
    for (idx, datatype) in datatypes.into_iter().enumerate() {
        let (color, time, envelope) = numseq_get_number_time_and_envelope(datatype);

        if let Some(time) = time {
            numbers.push(Number::NumberSequenceKeypoint(NumberSequenceKeypoint::new(time, color, envelope)));
        } else {
            untimed_numbers.push((idx, color, envelope));
        }
    };

    numbers.sort_unstable_by(|a, b| {
        a.coerce_to_keypoint().time
            .partial_cmp(&b.coerce_to_keypoint().time)
            .unwrap_or(Ordering::Less)
    });

    // We need to insert the colors now to ensure color 
    // times are calculated properly in the next step.
    for (idx, _, _) in &untimed_numbers {
        numbers.insert(*idx, Number::Empty)
    };

    let numbers_len = numbers.len();
    for (idx, number, envelope) in &untimed_numbers {
        let idx = *idx;
        let (start_idx, start_time) = numseq_get_start_time(idx, &numbers);
        let (end_idx, end_time) = numseq_get_end_time(idx, &numbers, numbers_len);

        let time = start_time + (end_time - start_time) * (((idx as f32) - start_idx) / (end_idx - start_idx));

        numbers[idx] = Number::NumberSequenceKeypoint(NumberSequenceKeypoint::new(time, *number, *envelope))
    };

    // Coerces all of the colors to be keypoints.
    let mut numseq_keypoints: Vec<NumberSequenceKeypoint> = numbers.into_iter()
        .map(|item| { item.coerce_to_keypoint() })
        .collect();

    // Ensures that the first keypoint's time is 0.
    let first_keypoint = numseq_keypoints[0];
    if first_keypoint.time != 0.0 {
        numseq_keypoints.insert(0, NumberSequenceKeypoint::new(0.0, first_keypoint.value, first_keypoint.envelope));
    }

    // Ensures that the last keypoint's time is 1.
    let last_keypoint = numseq_keypoints[numbers_len - 1];
    if last_keypoint.time != 1.0 {
        numseq_keypoints.push(NumberSequenceKeypoint::new(1.0, last_keypoint.value, last_keypoint.envelope))
    }

    return Datatype::Variant(Variant::NumberSequence(NumberSequence {
        keypoints: numseq_keypoints
    }))
}