use ropey::Rope;
use crate::types::{Position, Range};

pub trait RangeFromSpan {
    fn from_span(rope: &Rope, location: (usize, usize)) -> Range;
}

impl RangeFromSpan for Range {
    fn from_span(rope: &Rope, location: (usize, usize)) -> Range {
        let (start_byte_idx, end_byte_idx) = location;
        let (start_char_idx, end_char_idx) =
            (rope.byte_to_char(start_byte_idx), rope.byte_to_char(end_byte_idx));

        let start_line_idx = rope.char_to_line(start_char_idx);
        let start_line = rope.line_to_char(start_line_idx);
        let start_col = start_char_idx - start_line;

        let end_line_idx = rope.char_to_line(end_char_idx);
        let end_line = rope.line_to_char(end_line_idx);
        let end_col = end_char_idx - end_line;

        Range {
            start: Position {
                line: start_line_idx as u32,
                character: start_col as u32,
            },
            end: Position {
                line: end_line_idx as u32,
                character: end_col as u32,
            },
        }
    }
}
