use std::collections::HashMap;
use std::mem;
use std::ops::{Index, IndexMut};

use rbx_types::Attributes;

use super::datatype::Datatype;

#[derive(Clone, PartialEq, Copy, Eq, Debug, Hash)]
pub enum TreeNodeType {
    Root,
    Node(usize),
}

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
            child_rules: vec![],
        }
    }
}

#[derive(Debug)]
pub struct TreeNode {
    pub selector: Option<String>,
    pub priority: Option<i32>,
    pub tweens: HashMap<String, Datatype>,
    pub attributes: Attributes,
    pub static_attributes: HashMap<String, Datatype>,
    pub properties: Attributes,
    pub child_rules: Vec<usize>,
    pub parent: TreeNodeType,
}

impl TreeNode {
    pub fn new(parent: TreeNodeType, selector: Option<String>) -> Self {
        Self {
            attributes: Attributes::new(),
            static_attributes: HashMap::new(),
            properties: Attributes::new(),
            child_rules: vec![],
            priority: None,
            tweens: HashMap::new(),
            selector,
            parent,
        }
    }
}

pub enum AnyTreeNode<'a> {
    Node(Option<&'a TreeNode>),
    Root(Option<&'a RootTreeNode>),
}

pub enum AnyTreeNodeMut<'a> {
    Node(Option<&'a mut TreeNode>),
    Root(Option<&'a mut RootTreeNode>),
}

#[derive(Debug)]
pub struct TreeNodeGroup {
    root: Option<RootTreeNode>,
    nodes: Vec<Option<TreeNode>>,
}

impl TreeNodeGroup {
    pub fn new() -> Self {
        Self {
            root: Some(RootTreeNode::new()),
            nodes: vec![],
        }
    }

    pub fn get(&self, idx: TreeNodeType) -> AnyTreeNode<'_> {
        match idx {
            TreeNodeType::Node(idx) => AnyTreeNode::Node(self.nodes[idx].as_ref()),
            TreeNodeType::Root => AnyTreeNode::Root(self.root.as_ref()),
        }
    }

    pub fn nodes_len(&self) -> usize {
        self.nodes.len()
    }

    pub fn get_root(&self) -> Option<&RootTreeNode> {
        self.root.as_ref()
    }

    pub fn get_node_mut(&mut self, idx: TreeNodeType) -> AnyTreeNodeMut<'_> {
        match idx {
            TreeNodeType::Node(idx) => AnyTreeNodeMut::Node(self.nodes[idx].as_mut()),
            TreeNodeType::Root => AnyTreeNodeMut::Root(self.root.as_mut()),
        }
    }

    pub fn get_root_mut(&mut self) -> Option<&mut RootTreeNode> {
        self.root.as_mut()
    }

    pub fn add_node(&mut self, tree_node: TreeNode) {
        self.nodes.push(Some(tree_node));
    }

    pub fn take_node(&mut self, idx: usize) -> Option<TreeNode> {
        mem::replace(&mut self.nodes[idx], None)
    }

    pub fn take_root(&mut self) -> Option<RootTreeNode> {
        mem::replace(&mut self.root, None)
    }
}

impl Index<usize> for TreeNodeGroup {
    type Output = Option<TreeNode>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.nodes[index]
    }
}

impl IndexMut<usize> for TreeNodeGroup {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.nodes[index]
    }
}
