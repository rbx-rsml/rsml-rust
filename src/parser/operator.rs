#[derive(PartialEq, Debug, Clone)]
pub enum Operator {
    Pow,
    Div,
    Mod,
    Mult,
    Add,
    Sub
}

impl Operator {
    pub fn combine_with<'a>(&self, right: &Operator) -> Operator {
        match (&self, &right) {
            (Operator::Sub, Operator::Add) => Operator::Sub,
            (Operator::Sub, Operator::Sub) => Operator::Add,
            _ => right.clone()
        }
    }
}