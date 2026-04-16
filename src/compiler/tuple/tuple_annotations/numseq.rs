use crate::compiler::datatype::Datatype;
use rbx_types::{NumberSequence, NumberSequenceKeypoint, Variant};
use std::cmp::Ordering;

use super::{coerce_datatype_to_f32, extract_datatype_f32};

fn numseq_get_number_time_and_envelope(datatype: &Datatype) -> (f32, Option<f32>, f32) {
    match datatype {
        Datatype::TupleData(tuple_data) => {
            let time = extract_datatype_f32(tuple_data.get(0));
            let number = coerce_datatype_to_f32(tuple_data.get(1), 0.0);
            let envelope = coerce_datatype_to_f32(tuple_data.get(2), 0.0);

            (number, time, envelope)
        }

        Datatype::Variant(Variant::Float32(float32)) => (*float32, None, 0.0),

        _ => (0.0, None, 0.0),
    }
}

#[derive(Debug)]
enum Number {
    NumberSequenceKeypoint(NumberSequenceKeypoint),
    Empty,
}

impl Number {
    fn coerce_to_keypoint(&self) -> NumberSequenceKeypoint {
        match self {
            Number::NumberSequenceKeypoint(keypoint) => *keypoint,
            _ => unreachable!(),
        }
    }
}

fn numseq_get_start_time(current_idx: usize, numbers: &Vec<Number>) -> (f32, f32) {
    if current_idx != 0 {
        for idx in (0..current_idx).rev() {
            let float32 = &numbers[idx];

            if let Number::NumberSequenceKeypoint(keypoint) = float32 {
                return (idx as f32, keypoint.time);
            }
        }
    }

    (0.0, 0.0)
}

fn numseq_get_end_time(
    current_idx: usize,
    numbers: &Vec<Number>,
    numbers_len: usize,
) -> (f32, f32) {
    if current_idx != numbers_len {
        for idx in (current_idx + 1)..numbers_len {
            let color = &numbers[idx];

            if let Number::NumberSequenceKeypoint(keypoint) = color {
                return (idx as f32, keypoint.time);
            }
        }
    }

    ((numbers_len - 1) as f32, 1.0)
}

pub fn numseq_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    if datatypes.len() == 1 {
        let (number, _, envelope) = numseq_get_number_time_and_envelope(&datatypes[0]);
        return Datatype::Variant(Variant::NumberSequence(NumberSequence {
            keypoints: vec![
                NumberSequenceKeypoint::new(0.0, number, envelope),
                NumberSequenceKeypoint::new(1.0, number, envelope),
            ],
        }));
    }

    let mut numbers: Vec<Number> = vec![];
    let mut untimed_numbers: Vec<(usize, f32, f32)> = vec![];

    for (idx, datatype) in datatypes.iter().enumerate() {
        let (number, time, envelope) = numseq_get_number_time_and_envelope(datatype);

        if let Some(time) = time {
            numbers.push(Number::NumberSequenceKeypoint(
                NumberSequenceKeypoint::new(time, number, envelope),
            ));
        } else {
            untimed_numbers.push((idx, number, envelope));
        }
    }

    numbers.sort_unstable_by(|a, b| {
        a.coerce_to_keypoint()
            .time
            .partial_cmp(&b.coerce_to_keypoint().time)
            .unwrap_or(Ordering::Less)
    });

    for (idx, _, _) in &untimed_numbers {
        numbers.insert(*idx, Number::Empty);
    }

    let numbers_len = numbers.len();
    for (idx, number, envelope) in &untimed_numbers {
        let idx = *idx;
        let (start_idx, start_time) = numseq_get_start_time(idx, &numbers);
        let (end_idx, end_time) = numseq_get_end_time(idx, &numbers, numbers_len);

        let time = start_time
            + (end_time - start_time) * (((idx as f32) - start_idx) / (end_idx - start_idx));

        numbers[idx] = Number::NumberSequenceKeypoint(NumberSequenceKeypoint::new(
            time, *number, *envelope,
        ));
    }

    let mut numseq_keypoints: Vec<NumberSequenceKeypoint> = numbers
        .into_iter()
        .map(|item| item.coerce_to_keypoint())
        .collect();

    let first_keypoint = numseq_keypoints[0];
    if first_keypoint.time != 0.0 {
        numseq_keypoints.insert(
            0,
            NumberSequenceKeypoint::new(0.0, first_keypoint.value, first_keypoint.envelope),
        );
    }

    let last_keypoint = numseq_keypoints[numbers_len - 1];
    if last_keypoint.time != 1.0 {
        numseq_keypoints.push(NumberSequenceKeypoint::new(
            1.0,
            last_keypoint.value,
            last_keypoint.envelope,
        ));
    }

    Datatype::Variant(Variant::NumberSequence(NumberSequence {
        keypoints: numseq_keypoints,
    }))
}
