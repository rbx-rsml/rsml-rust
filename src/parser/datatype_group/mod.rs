use std::ops::{Index, IndexMut};

use rbx_types::Variant;
use rbx_types_ops::BasicOperations;

mod datatype;
pub use datatype::Datatype;

use super::operator::Operator;

pub struct DatatypeGroup(Vec<Datatype>);

impl DatatypeGroup {
    pub fn push(&mut self, value: Datatype) {
        self.0.push(value);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn remove(&mut self, index: usize) -> Datatype {
        self.0.remove(index)
    }

    pub fn new(first_item: Datatype) -> Self {
        Self(vec![first_item])
    }

    pub fn ensure_then_insert(group: Option<DatatypeGroup>, to_insert: Datatype) -> DatatypeGroup {
        if let Some(mut group) = group {
            group.push(to_insert);
            group
        } else {
            DatatypeGroup::new(to_insert)
        }
    }

    pub fn find_operators<'a>(
        &self, operator_group: &'a [OperatorData]
    ) -> Vec<(usize, fn(&Variant, &Variant) -> Option<Variant>)> {
        let mut indexes = vec![];
    
        'datatype_loop: for (idx, datatype) in self.0.iter().enumerate() {
            if let Datatype::Operator(datatype_operator) = datatype  {
                for (operator, operation_fn) in operator_group {
                    if operator == datatype_operator {
                        indexes.push((idx, *operation_fn));
                        continue 'datatype_loop;
                    }
                }
            }
        }
    
        return indexes
    }

    pub fn coerce_to_datatype(&mut self) -> Datatype {
        if self.len() == 1 {
            return self[0].clone()
        }

        return self.solve()
    }

    fn solve(&mut self) -> Datatype {
        // Merges Add and Sub operators with the datatype to the right if
        // the datatype to  the left isn't an operator.
        let occurrences = self.find_operators(&ADD_SUB_OPERATORS);
        let mut occurrence_idx_offset = 0;
        for (mut occurrence_idx, operation_fn) in occurrences {
            occurrence_idx -= occurrence_idx_offset;
    
            let right_idx = occurrence_idx + 1;
            if right_idx >= self.len() { continue; }
    
            let can_merge;
            if occurrence_idx == 0 { can_merge = true }
            else {
                let left = &self[occurrence_idx - 1];
                can_merge = if matches!(left, Datatype::Operator(_)) { true } else { false }
            }
    
            if can_merge {
                let right = self.remove(right_idx);
                occurrence_idx_offset += 1;
    
                let solved_variant = match &right {
                    Datatype::Variant(right) => operation_fn(&Variant::Float32(0.0), right),
                    _ => None
                };
    
                self[occurrence_idx] = match solved_variant {
                    Some(solved_variant) => Datatype::Variant(solved_variant),
                    None => right
                }
            }
        }
    
        for operator_group in ORDERED_OPERATORS {
            let occurrences = self.find_operators(&operator_group);
            let mut occurrence_idx_offset = 0;
    
            for (mut occurrence_idx, operation_fn) in occurrences {
                occurrence_idx -= occurrence_idx_offset;
    
                let right_idx = occurrence_idx + 1;
                if right_idx >= self.len() { continue; }
    
                let right = self.remove(right_idx);
                occurrence_idx_offset += 1;
    
                let (left, left_idx) = {
                    if occurrence_idx == 0 {
                        (Datatype::Variant(Variant::Float32(0.0)), 0)
    
                    } else {
                        let left_idx = occurrence_idx - 1;
                        occurrence_idx_offset += 1;
                        let left = self.remove(left_idx);
    
                        if matches!(left, Datatype::None) {
                            (Datatype::Variant(Variant::Float32(0.0)), left_idx)
                        } else {
                            (left, left_idx)
                        }
                    }
                };
    
                let left = match left {
                    Datatype::Variant(left) => left,
                    _ => { self[left_idx] = left; continue }
                };
    
                let right = match right {
                    Datatype::Variant(right) => right,
                    _ => { self[left_idx] = Datatype::Variant(left); continue }
                };
    
                self[left_idx] = match operation_fn(&left, &right) {
                    Some(solved_variant) => Datatype::Variant(solved_variant),
                    None => Datatype::Variant(left),
                };
            }
        };
    
        return self[0].clone()
    }
}

impl Index<usize> for DatatypeGroup {
    type Output = Datatype;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for DatatypeGroup {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}


fn pow(left: &Variant, right: &Variant) -> Option<Variant> {
    left.pow(right)
}
 
fn div(left: &Variant, right: &Variant) -> Option<Variant> {
    left.div(right)
}

fn floor_div(left: &Variant, right: &Variant) -> Option<Variant> {
    left.floor_div(right)
}

fn modulus(left: &Variant, right: &Variant) -> Option<Variant> {
    left.modulus(right)
}

fn mult(left: &Variant, right: &Variant) -> Option<Variant> {
    left.mult(right)
}

fn add(left: &Variant, right: &Variant) -> Option<Variant> {
    left.add(right)
}

fn sub(left: &Variant, right: &Variant) -> Option<Variant> {
    left.sub(right)
}

type OperatorData = (Operator, fn(&Variant, &Variant) -> Option<Variant>);

static ADD_SUB_OPERATORS: [OperatorData; 2] = [
    (Operator::Add, add),
    (Operator::Sub, sub),
];

static ORDERED_OPERATORS: &[&[OperatorData]] = &[
    &[(Operator::Pow, pow)],

    &[
        (Operator::Div, div),
        (Operator::FloorDiv, floor_div),
        (Operator::Mod, modulus),
        (Operator::Mult, mult),
    ],

    &ADD_SUB_OPERATORS,
];