use std::mem;

mod tree_node;
pub use tree_node::TreeNode;

#[derive(Debug)]
pub struct TreeNodeGroup(Vec<Option<TreeNode>>);

impl TreeNodeGroup {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn get(&self, idx: usize) -> Option<&TreeNode> {
        self.0[idx].as_ref()
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut TreeNode> {
        self.0[idx].as_mut()
    }

    pub fn push(&mut self, tree_node: TreeNode) {
        self.0.push(Some(tree_node));
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn take(&mut self, idx: usize) -> Option<TreeNode> {
        mem::replace(&mut self.0[idx], None)
    }

    pub fn take_root(&mut self) -> TreeNode {
        mem::replace(&mut self.0[0], None).unwrap()
    }
}