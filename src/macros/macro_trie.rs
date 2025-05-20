use std::{collections::HashMap, mem};
use fxhash::FxBuildHasher;

type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

#[derive(Default, Debug)]
pub struct TrieNode {
    pub is_end: bool,
    children: FxHashMap<char, TrieNode>,
}

impl TrieNode {
    pub fn get(&self, char: char) -> Option<&TrieNode> {
        self.children.get(&char)
    }

    pub fn get_mut(&mut self, char: char) -> Option<&mut TrieNode> {
        self.children.get_mut(&char)
    }

    pub fn remove(&mut self, char: char) -> Option<TrieNode> {
        self.children.remove(&char)
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}


#[derive(Default, Debug)]
pub struct Trie {
    pub root: TrieNode,
}

impl<'a> Trie {
    pub fn new() -> Self {
        Trie {
            root: TrieNode::default(),
        }
    }

    pub fn insert(&mut self, word: &str) {
        let mut current_node = &mut self.root;

        for char in word.chars() {
            current_node = current_node.children.entry(char).or_default();
        }
        current_node.is_end = true;
    }

    pub fn contains(&self, word: &str) -> bool {
        let mut current_node = &self.root;

        for char in word.chars() {
            match current_node.children.get(&char) {
                Some(node) => current_node = node,
                None => return false,
            }
        }

        current_node.is_end
    }

    pub fn get(&self, char: char) -> Option<&TrieNode> {
        self.root.get(char)
    }

    pub fn scan<I>(&'a self, chars: I) -> TrieScan<'a, I> 
    where I: Iterator<Item = char>
    {
        TrieScan::<'a>::new(&self.root, chars)
    }
}

pub struct TrieScan<'a, I> where I: Iterator<Item = char> {
    root_node: &'a TrieNode,
    current_node: &'a TrieNode,
    current_match: String,
    current_char_idx: usize,
    chars: I
}

impl<'a, I> TrieScan<'a, I> where I: Iterator<Item = char> {
    fn new(root_node: &'a TrieNode, chars: I) -> Self
    {
        Self {
            root_node,
            current_node: root_node,
            current_match: String::new(),
            current_char_idx: 0,
            chars
        }
    }
}

impl<'a, I> Iterator for TrieScan<'a, I> where I: Iterator<Item = char> {
    type Item = (String, usize);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.chars.next() {
                Some(char) => {
                    self.current_char_idx += 1;
                    if char == '[' { break }
                },
                None => return None
            }
        }

        loop {
            match self.chars.next() {
                Some(char) => {
                    let current_char_idx = self.current_char_idx;
                    self.current_char_idx += 1;

                    if self.current_node.is_end && char == ']' {
                        self.current_node = self.root_node;
                        return Some((mem::take(&mut self.current_match), current_char_idx));
                    }

                    match self.current_node.get(char) {
                        Some(child_node) => {
                            self.current_match.push(char);
                            self.current_node = child_node;
                        },
                        None => {
                            self.current_match.clear();
                            self.current_node = self.root_node;
                        }
                    }
                },
                None => {
                    if self.current_node.is_end {
                        self.current_node = self.root_node;
                        return Some((mem::take(&mut self.current_match), self.current_char_idx));
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}