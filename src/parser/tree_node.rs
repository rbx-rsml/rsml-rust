use std::collections::{HashMap, HashSet};
use rbx_types::Variant;

#[derive(Debug)]
pub struct TreeNode {
    pub selector: Option<String>,
    pub name: Option<String>,
    pub derives: HashSet<String>,
    pub priority: Option<i32>,
    pub attributes: HashMap<String, Variant>,
    pub properties: HashMap<String, Variant>,
    pub rules: Vec<usize>,
    pub parent: usize
}

impl TreeNode {
    pub fn new(parent: usize, selector: Option<String>) -> Self {
        Self {
            attributes: HashMap::new(),
            properties: HashMap::new(),
            derives: HashSet::new(),
            rules: vec![],
            priority: None,
            name: None,
            selector,
            parent
        }
    }
}