use std::collections::HashMap;
use rbx_types::{Attributes, Variant};

use crate::Datatype;

#[derive(Debug)]
pub struct TreeNode {
    pub selector: Option<String>,
    pub name: Option<String>,
    pub priority: Option<i32>,
    pub attributes: Attributes,
    pub static_attributes: HashMap<String, Datatype>,
    pub properties: HashMap<String, Variant>,
    pub child_rules: Vec<usize>,
    pub parent: TreeNodeType
}

#[derive(Clone, PartialEq, Copy, Eq, Debug, Hash)]
pub enum TreeNodeType {
    Root,
    Node(usize)
}

impl TreeNode {
    pub fn new(parent: TreeNodeType, selector: Option<String>) -> Self {
        Self {
            attributes: Attributes::new(),
            static_attributes: HashMap::new(),
            properties: HashMap::new(),
            child_rules: vec![],
            priority: None,
            name: None,
            selector,
            parent
        }
    }
}