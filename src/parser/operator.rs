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

impl Operator {
    pub fn can_merge_with(&self, right: &Operator) -> bool {
        let left_is_add_or_sub = matches!(self, Operator::Add | Operator::Sub);
        let right_is_add_or_sub = matches!(right, Operator::Add | Operator::Sub);

        if (left_is_add_or_sub && !right_is_add_or_sub) || 
            (right_is_add_or_sub && !left_is_add_or_sub)
        { false } else { true }
    }

    pub fn merge_with<'a>(&self, right: &Operator) -> Operator {
        match (&self, &right) {
            (Operator::Sub, Operator::Add) => Operator::Sub,
            (Operator::Sub, Operator::Sub) => Operator::Add,
            _ => right.clone()
        }
    }
}