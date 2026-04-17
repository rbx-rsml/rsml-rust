use rbx_types::Variant;

use crate::datatype::{Datatype, StaticLookup, evaluate_construct};
use crate::lexer::Token;
use crate::parser::ParsedRsml;
use crate::parser::types::{Construct, Delimited, Node, SelectorNode};

pub mod tree_node;
mod selector;

use selector::build_selector_string;
use tree_node::*;

pub struct Compiler<'a> {
    pub parsed: ParsedRsml<'a>,
}

pub struct CompilerData<'a> {
    pub compiler: Compiler<'a>,
    pub tree_nodes: TreeNodeGroup,
}

impl<'a> Compiler<'a> {
    pub fn new(parsed: ParsedRsml<'a>) -> CompilerData<'a> {
        let compiler = Self { parsed };
        let mut tree_nodes = TreeNodeGroup::new();
        let mut current_idx = TreeNodeType::Root;

        for construct in &compiler.parsed.ast {
            compile_construct(construct, &mut tree_nodes, &mut current_idx);
        }

        CompilerData {
            compiler,
            tree_nodes,
        }
    }
}

struct CompilerLookup<'a> {
    tree_nodes: &'a TreeNodeGroup,
    idx: TreeNodeType,
}

impl<'a> StaticLookup for CompilerLookup<'a> {
    fn resolve_static(&self, name: &str) -> Datatype {
        resolve_static_attribute(name, self.tree_nodes, self.idx)
    }

    fn resolve_dynamic(&self, name: &str) -> Datatype {
        Datatype::Variant(Variant::String(format!("${}", name)))
    }
}

fn compile_construct(
    construct: &Construct,
    tree_nodes: &mut TreeNodeGroup,
    current_idx: &mut TreeNodeType,
) {
    match construct {
        Construct::Rule { selectors, body } => {
            compile_rule(selectors, body, tree_nodes, current_idx);
        }

        Construct::Assignment { left, right, .. } => {
            compile_assignment(left, right.as_deref(), tree_nodes, current_idx);
        }

        Construct::Priority { body, .. } => {
            if let TreeNodeType::Node(node_idx) = *current_idx {
                if let Some(body) = body {
                    let idx = *current_idx;
                    let lookup = CompilerLookup { tree_nodes, idx };
                    if let Some(Datatype::Variant(Variant::Float32(value))) =
                        evaluate_construct(body, None, &lookup)
                    {
                        if let Some(node) = tree_nodes[node_idx].as_mut() {
                            node.priority = Some(value as i32);
                        }
                    }
                }
            }
        }

        Construct::Tween { name, body, .. } => {
            if let TreeNodeType::Node(node_idx) = *current_idx {
                if let Some(name_node) = name {
                    if let Token::Identifier(tween_name) = name_node.token.value() {
                        if let Some(body) = body {
                            let idx = *current_idx;
                            let lookup = CompilerLookup { tree_nodes, idx };
                            if let Some(datatype) = evaluate_construct(body, None, &lookup) {
                                if let Some(node) = tree_nodes[node_idx].as_mut() {
                                    node.tweens.insert(tween_name.to_string(), datatype);
                                }
                            }
                        }
                    }
                }
            }
        }

        Construct::Derive { .. }
        | Construct::Macro { .. }
        | Construct::MacroCall { .. } => {}

        _ => {}
    }
}

fn compile_rule(
    selectors: &Option<Vec<SelectorNode>>,
    body: &Option<Delimited>,
    tree_nodes: &mut TreeNodeGroup,
    current_idx: &mut TreeNodeType,
) {
    let selector_string = selectors.as_ref().map(|s| build_selector_string(s));

    let new_node_idx = tree_nodes.nodes_len();
    let new_node_idx_type = TreeNodeType::Node(new_node_idx);

    match tree_nodes.get_node_mut(*current_idx) {
        AnyTreeNodeMut::Root(node) => node.unwrap().child_rules.push(new_node_idx),
        AnyTreeNodeMut::Node(node) => node.unwrap().child_rules.push(new_node_idx),
    }

    let new_node = TreeNode::new(*current_idx, selector_string);
    tree_nodes.add_node(new_node);

    if let Some(body) = body {
        if let Some(constructs) = &body.content {
            let saved_idx = *current_idx;
            *current_idx = new_node_idx_type;

            for construct in constructs {
                compile_construct(construct, tree_nodes, current_idx);
            }

            *current_idx = saved_idx;
        }
    }
}

fn compile_assignment(
    left: &Node,
    right: Option<&Construct>,
    tree_nodes: &mut TreeNodeGroup,
    current_idx: &mut TreeNodeType,
) {
    let Some(right) = right else { return };
    let idx = *current_idx;
    let lookup = CompilerLookup { tree_nodes, idx };

    match left.token.value() {
        Token::Identifier(prop_name) => {
            if let TreeNodeType::Node(node_idx) = idx {
                let datatype = evaluate_construct(right, Some(prop_name), &lookup);
                let variant = datatype.and_then(|d| d.coerce_to_variant(Some(prop_name)));

                if let Some(variant) = variant {
                    if let Some(node) = tree_nodes[node_idx].as_mut() {
                        node.properties.insert(prop_name.to_string(), variant);
                    }
                }
            }
        }

        Token::TokenIdentifier(attr_name) => {
            let datatype = evaluate_construct(right, Some(attr_name), &lookup);
            let variant = datatype.and_then(|d| d.coerce_to_variant(Some(attr_name)));

            if let Some(variant) = variant {
                match tree_nodes.get_node_mut(idx) {
                    AnyTreeNodeMut::Root(node) => {
                        node.unwrap()
                            .attributes
                            .insert(attr_name.to_string(), variant);
                    }
                    AnyTreeNodeMut::Node(node) => {
                        node.unwrap()
                            .attributes
                            .insert(attr_name.to_string(), variant);
                    }
                }
            }
        }

        Token::StaticTokenIdentifier(static_name) => {
            let datatype = evaluate_construct(right, Some(static_name), &lookup);
            let static_val = datatype.and_then(|d| d.coerce_to_static(Some(static_name)));

            if let Some(static_val) = static_val {
                match tree_nodes.get_node_mut(idx) {
                    AnyTreeNodeMut::Root(node) => {
                        node.unwrap()
                            .static_attributes
                            .insert(static_name.to_string(), static_val);
                    }
                    AnyTreeNodeMut::Node(node) => {
                        node.unwrap()
                            .static_attributes
                            .insert(static_name.to_string(), static_val);
                    }
                }
            }
        }

        _ => {}
    }
}

fn resolve_static_attribute(
    name: &str,
    tree_nodes: &TreeNodeGroup,
    idx: TreeNodeType,
) -> Datatype {
    match tree_nodes.get(idx) {
        AnyTreeNode::Root(node) => node
            .and_then(|n| n.static_attributes.get(name))
            .map(|d| d.clone())
            .unwrap_or(Datatype::None),

        AnyTreeNode::Node(node) => {
            if let Some(node) = node {
                if let Some(val) = node.static_attributes.get(name) {
                    return val.clone();
                }
                resolve_static_attribute(name, tree_nodes, node.parent)
            } else {
                Datatype::None
            }
        }
    }
}
