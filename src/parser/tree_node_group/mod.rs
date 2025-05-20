use std::{mem, ops::{Index, IndexMut}};

mod tree_node;
pub use tree_node::{TreeNode, TreeNodeType};

mod root_tree_node;
pub use root_tree_node::RootTreeNode;

#[derive(Debug)]
pub struct TreeNodeGroup {
    root: Option<RootTreeNode>,
    nodes: Vec<Option<TreeNode>>
}

pub enum AnyTreeNode<'a> {
    Node(Option<&'a TreeNode>),
    Root(Option<&'a RootTreeNode>)
}

pub enum AnyTreeNodeMut<'a> {
    Node(Option<&'a mut TreeNode>),
    Root(Option<&'a mut RootTreeNode>)
}

impl TreeNodeGroup {
    pub fn new() -> Self {
        Self {
            root: Some(RootTreeNode::new()),
            nodes: vec![]
        }
    }

    pub fn get(&self, idx: TreeNodeType) -> AnyTreeNode {
        match idx {
            TreeNodeType::Node(idx) => AnyTreeNode::Node(self.nodes[idx].as_ref()),
            TreeNodeType::Root => AnyTreeNode::Root(self.root.as_ref())
        }
    }

    pub fn get_root(&self) -> Option<&RootTreeNode> {
        self.root.as_ref()
    }

    pub fn get_mut(&mut self, idx: TreeNodeType) -> AnyTreeNodeMut {
        match idx {
            TreeNodeType::Node(idx) => AnyTreeNodeMut::Node(self.nodes[idx].as_mut()),
            TreeNodeType::Root => AnyTreeNodeMut::Root(self.root.as_mut())
        }
    }

    pub fn get_root_mut(&mut self) -> Option<&mut RootTreeNode> {
        self.root.as_mut()
    }

    pub fn push(&mut self, tree_node: TreeNode) {
        self.nodes.push(Some(tree_node));
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn take(&mut self, idx: usize) -> Option<TreeNode> {
        mem::replace(&mut self.nodes[idx], None)
    }

    pub fn take_root(&mut self) -> RootTreeNode {
        mem::replace(&mut self.root, None).unwrap()
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