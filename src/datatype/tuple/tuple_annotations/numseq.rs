use crate::datatype::Datatype;
use rbx_types::{NumberSequence, NumberSequenceKeypoint, Variant};
use std::cmp::Ordering;

use crate::datatype::tuple::tuple_annotations::{coerce_datatype_to_f64, extract_datatype_f64};

struct NumberTimeAndEnvelope {
    number: f64,
    time: Option<f64>,
    envelope: f64,
}

struct UntimedNumber {
    idx: usize,
    number: f64,
    envelope: f64,
}

struct TimePoint {
    idx: f64,
    time: f64,
}

fn numseq_get_number_time_and_envelope(datatype: &Datatype) -> NumberTimeAndEnvelope {
    match datatype {
        Datatype::TupleData(tuple_data) => {
            let time = extract_datatype_f64(tuple_data.get(0));
            let number = coerce_datatype_to_f64(tuple_data.get(1), 0.0);
            let envelope = coerce_datatype_to_f64(tuple_data.get(2), 0.0);

            NumberTimeAndEnvelope { number, time, envelope }
        }

        Datatype::Variant(Variant::Float64(float64)) => NumberTimeAndEnvelope {
            number: *float64,
            time: None,
            envelope: 0.0,
        },

        _ => NumberTimeAndEnvelope { number: 0.0, time: None, envelope: 0.0 },
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

fn numseq_get_start_time(current_idx: usize, numbers: &Vec<Number>) -> TimePoint {
    if current_idx != 0 {
        for idx in (0..current_idx).rev() {
            let entry = &numbers[idx];

            if let Number::NumberSequenceKeypoint(keypoint) = entry {
                return TimePoint { idx: idx as f64, time: keypoint.time as f64 };
            }
        }
    }

    TimePoint { idx: 0.0, time: 0.0 }
}

fn numseq_get_end_time(
    current_idx: usize,
    numbers: &Vec<Number>,
    numbers_len: usize,
) -> TimePoint {
    if current_idx != numbers_len {
        for idx in (current_idx + 1)..numbers_len {
            let entry = &numbers[idx];

            if let Number::NumberSequenceKeypoint(keypoint) = entry {
                return TimePoint { idx: idx as f64, time: keypoint.time as f64 };
            }
        }
    }

    TimePoint { idx: (numbers_len - 1) as f64, time: 1.0 }
}

pub fn numseq_annotation(datatypes: &Vec<Datatype>) -> Datatype {
    if datatypes.len() == 1 {
        let NumberTimeAndEnvelope { number, envelope, .. } =
            numseq_get_number_time_and_envelope(&datatypes[0]);
        return Datatype::Variant(Variant::NumberSequence(NumberSequence {
            keypoints: vec![
                NumberSequenceKeypoint::new(0.0, number as f32, envelope as f32),
                NumberSequenceKeypoint::new(1.0, number as f32, envelope as f32),
            ],
        }));
    }

    let mut numbers: Vec<Number> = vec![];
    let mut untimed_numbers: Vec<UntimedNumber> = vec![];

    for (idx, datatype) in datatypes.iter().enumerate() {
        let NumberTimeAndEnvelope { number, time, envelope } =
            numseq_get_number_time_and_envelope(datatype);

        if let Some(time) = time {
            numbers.push(Number::NumberSequenceKeypoint(
                NumberSequenceKeypoint::new(time as f32, number as f32, envelope as f32),
            ));
        } else {
            untimed_numbers.push(UntimedNumber { idx, number, envelope });
        }
    }

    numbers.sort_unstable_by(|a, b| {
        a.coerce_to_keypoint()
            .time
            .partial_cmp(&b.coerce_to_keypoint().time)
            .unwrap_or(Ordering::Less)
    });

    for untimed in &untimed_numbers {
        numbers.insert(untimed.idx, Number::Empty);
    }

    let numbers_len = numbers.len();
    for untimed in &untimed_numbers {
        let idx = untimed.idx;
        let start = numseq_get_start_time(idx, &numbers);
        let end = numseq_get_end_time(idx, &numbers, numbers_len);

        let time = start.time
            + (end.time - start.time) * ((idx as f64 - start.idx) / (end.idx - start.idx));

        numbers[idx] = Number::NumberSequenceKeypoint(NumberSequenceKeypoint::new(
            time as f32,
            untimed.number as f32,
            untimed.envelope as f32,
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
