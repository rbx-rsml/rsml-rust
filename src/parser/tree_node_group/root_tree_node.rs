use std::collections::HashMap;
use rbx_types::Attributes;

use crate::Datatype;

#[derive(Debug)]
pub struct RootTreeNode {
    pub attributes: Attributes,
    pub static_attributes: HashMap<String, Datatype>,
    pub child_rules: Vec<usize>,
}

impl RootTreeNode {
    pub fn new() -> Self {
        Self {
            attributes: Attributes::new(),
            static_attributes: HashMap::new(),
            child_rules: vec![]
        }
    }
}