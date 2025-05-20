use phf_macros::phf_map;

use crate::Token;

#[derive(PartialEq, Debug, Clone)]
pub enum Operator {
    Pow,
    Div,
    FloorDiv,
    Mod,
    Mult,
    Add,
    Sub
}

const TOKEN_TO_OPERATOR_MAP: phf::Map<usize, Operator> = phf_map! {
    0usize => Operator::Pow,
    1usize => Operator::Div,
    2usize => Operator::FloorDiv,
    3usize => Operator::Mod,
    4usize => Operator::Mod,
    5usize => Operator::Mult,
    6usize => Operator::Add,
    7usize => Operator::Sub
};

impl Operator {
    pub fn can_merge_with(&self, right: &Operator) -> bool {
        let left_is_add_or_sub = matches!(self, Operator::Add | Operator::Sub);
        let right_is_add_or_sub = matches!(right, Operator::Add | Operator::Sub);

        if (left_is_add_or_sub && !right_is_add_or_sub) || 
            (right_is_add_or_sub && !left_is_add_or_sub)
        { false } else { true }
    }

    pub fn merge_with(&self, right: &Operator) -> Operator {
        match (&self, &right) {
            (Operator::Sub, Operator::Add) => Operator::Sub,
            (Operator::Sub, Operator::Sub) => Operator::Add,
            _ => right.clone()
        }
    }

    pub fn from_token<'a>(token: Token) -> Option<&'a Operator> {
        TOKEN_TO_OPERATOR_MAP.get(&(token as usize))
    }
}