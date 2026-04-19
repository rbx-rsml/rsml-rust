use std::collections::HashMap;

use rbx_types::Variant;

use crate::datatype::{Datatype, StaticLookup, evaluate_construct};
use crate::lexer::Token;
use crate::parser::ParsedRsml;
use crate::parser::types::{Construct, Delimited, MacroBodyContent, Node, SelectorNode};
use crate::typechecker::{
    MacroDefinition, MacroRegistry, collect_macro_def_arg_names, macro_return_context,
};

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

#[derive(Clone, Copy)]
pub struct BoundArg<'a> {
    pub construct: &'a Construct<'a>,
    pub scope_depth: usize,
}

pub type BindingFrame<'a> = HashMap<String, BoundArg<'a>>;

pub struct MacroContext<'a> {
    pub local: MacroRegistry<'a>,
    pub bindings: Vec<BindingFrame<'a>>,
    pub expansion_stack: Vec<String>,
}

impl<'a> Compiler<'a> {
    pub fn new(parsed: ParsedRsml<'a>) -> CompilerData<'a> {
        let compiler = Self { parsed };
        let mut tree_nodes = TreeNodeGroup::new();
        let mut current_idx = TreeNodeType::Root;

        let local = collect_user_macros(&compiler.parsed.ast);
        let mut macro_ctx = MacroContext {
            local,
            bindings: vec![HashMap::new()],
            expansion_stack: Vec::new(),
        };

        for construct in &compiler.parsed.ast {
            compile_construct(construct, &mut tree_nodes, &mut current_idx, &mut macro_ctx);
        }

        CompilerData {
            compiler,
            tree_nodes,
        }
    }
}

fn collect_user_macros<'a>(ast: &'a [Construct<'a>]) -> MacroRegistry<'a> {
    let mut registry = MacroRegistry::new();
    for construct in ast {
        if let Construct::Macro {
            name: Some(name_node),
            args,
            body,
            return_type,
            ..
        } = construct
        {
            if let Token::Identifier(name_str) = name_node.token.value() {
                registry
                    .entry(name_str.to_string())
                    .or_insert_with(Vec::new)
                    .push(MacroDefinition {
                        arg_names: collect_macro_def_arg_names(args),
                        body: body.as_ref().map(|b| &b.content),
                        return_context: macro_return_context(return_type),
                    });
            }
        }
    }
    registry
}

struct CompilerLookup<'a> {
    tree_nodes: &'a TreeNodeGroup,
    idx: TreeNodeType,
    macro_ctx: Option<&'a MacroContext<'a>>,
    active_scope_depth: usize,
}

impl<'a> StaticLookup for CompilerLookup<'a> {
    fn resolve_static(&self, name: &str) -> Datatype {
        resolve_static_attribute(name, self.tree_nodes, self.idx)
    }

    fn resolve_dynamic(&self, name: &str) -> Datatype {
        Datatype::Variant(Variant::String(format!("${}", name)))
    }

    fn resolve_macro_arg(&self, name: &str, key: Option<&str>) -> Option<Datatype> {
        let ctx = self.macro_ctx?;
        let frame = ctx.bindings.get(self.active_scope_depth)?;
        let bound = *frame.get(name)?;

        let inner_lookup = CompilerLookup {
            tree_nodes: self.tree_nodes,
            idx: self.idx,
            macro_ctx: self.macro_ctx,
            active_scope_depth: bound.scope_depth,
        };
        evaluate_construct(bound.construct, key, &inner_lookup)
    }
}

fn current_scope_depth(macro_ctx: &MacroContext) -> usize {
    macro_ctx.bindings.len().saturating_sub(1)
}

fn compile_construct<'a>(
    construct: &'a Construct<'a>,
    tree_nodes: &mut TreeNodeGroup,
    current_idx: &mut TreeNodeType,
    macro_ctx: &mut MacroContext<'a>,
) {
    match construct {
        Construct::Rule { selectors, body } => {
            compile_rule(selectors, body, tree_nodes, current_idx, macro_ctx);
        }

        Construct::Assignment { left, right, .. } => {
            compile_assignment(left, right.as_deref(), tree_nodes, current_idx, macro_ctx);
        }

        Construct::Priority { body, .. } => {
            if let TreeNodeType::Node(node_idx) = *current_idx {
                if let Some(body) = body {
                    let idx = *current_idx;
                    let active_scope_depth = current_scope_depth(macro_ctx);
                    let lookup = CompilerLookup {
                        tree_nodes,
                        idx,
                        macro_ctx: Some(&*macro_ctx),
                        active_scope_depth,
                    };
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
                            let active_scope_depth = current_scope_depth(macro_ctx);
                            let lookup = CompilerLookup {
                                tree_nodes,
                                idx,
                                macro_ctx: Some(&*macro_ctx),
                                active_scope_depth,
                            };
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

        Construct::MacroCall { name, body, .. } => {
            compile_macro_call(name, body, tree_nodes, current_idx, macro_ctx);
        }

        Construct::Derive { .. } | Construct::Macro { .. } => {}

        _ => {}
    }
}

fn compile_rule<'a>(
    selectors: &'a Option<Vec<SelectorNode<'a>>>,
    body: &'a Option<Delimited<'a>>,
    tree_nodes: &mut TreeNodeGroup,
    current_idx: &mut TreeNodeType,
    macro_ctx: &mut MacroContext<'a>,
) {
    let selector_string = selectors.as_ref().map(|s| {
        let expanded = expand_selector_macros(s, macro_ctx);
        build_selector_string(&expanded)
    });

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
                compile_construct(construct, tree_nodes, current_idx, macro_ctx);
            }

            *current_idx = saved_idx;
        }
    }
}

fn compile_assignment<'a>(
    left: &Node<'a>,
    right: Option<&'a Construct<'a>>,
    tree_nodes: &mut TreeNodeGroup,
    current_idx: &mut TreeNodeType,
    macro_ctx: &mut MacroContext<'a>,
) {
    let Some(right) = right else { return };
    let idx = *current_idx;
    let active_scope_depth = current_scope_depth(macro_ctx);
    let lookup = CompilerLookup {
        tree_nodes,
        idx,
        macro_ctx: Some(&*macro_ctx),
        active_scope_depth,
    };

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

fn compile_macro_call<'a>(
    name: &Node<'a>,
    call_body: &'a Option<Delimited<'a>>,
    tree_nodes: &mut TreeNodeGroup,
    current_idx: &mut TreeNodeType,
    macro_ctx: &mut MacroContext<'a>,
) {
    let Token::MacroCallIdentifier(Some(macro_name)) = name.token.value() else {
        return;
    };
    let macro_name_str = *macro_name;

    if macro_ctx
        .expansion_stack
        .iter()
        .any(|n| n == macro_name_str)
    {
        return;
    }

    let call_args = collect_call_args(call_body);
    let arg_count = call_args.len();

    let (arg_names, body): (Vec<String>, &MacroBodyContent<'a>) = {
        let from_local = macro_ctx
            .local
            .get(macro_name_str)
            .and_then(|defs| defs.iter().find(|d| d.arg_names.len() == arg_count))
            .and_then(|def| {
                def.body
                    .map(|b| (def.arg_names.iter().map(|s| s.to_string()).collect(), b))
            });

        if let Some(pair) = from_local {
            pair
        } else if let Some(pair) = crate::builtins::BUILTINS
            .registry
            .get(macro_name_str)
            .and_then(|defs| defs.iter().find(|d| d.arg_names.len() == arg_count))
            .and_then(|def| {
                def.body
                    .map(|b| (def.arg_names.iter().map(|s| s.to_string()).collect(), b))
            })
        {
            pair
        } else {
            return;
        }
    };

    let MacroBodyContent::Construct(Some(constructs)) = body else {
        return;
    };

    let caller_scope = current_scope_depth(macro_ctx);
    let mut new_frame: BindingFrame<'a> = HashMap::new();
    for (arg_name, arg_value) in arg_names.iter().zip(call_args.iter()) {
        new_frame.insert(
            arg_name.clone(),
            BoundArg {
                construct: *arg_value,
                scope_depth: caller_scope,
            },
        );
    }

    macro_ctx.bindings.push(new_frame);
    macro_ctx.expansion_stack.push(macro_name_str.to_string());

    for construct in constructs.iter() {
        compile_construct(construct, tree_nodes, current_idx, macro_ctx);
    }

    macro_ctx.expansion_stack.pop();
    macro_ctx.bindings.pop();
}

fn is_selector_comma(node: &SelectorNode) -> bool {
    matches!(node, SelectorNode::Token(n) if matches!(n.token.value(), Token::Comma))
}

fn expand_selector_macros<'a>(
    selectors: &'a [SelectorNode<'a>],
    macro_ctx: &mut MacroContext<'a>,
) -> Vec<&'a SelectorNode<'a>> {
    let mut out: Vec<&'a SelectorNode<'a>> = Vec::with_capacity(selectors.len());
    let mut last_was_comma = true;
    expand_selectors_into(selectors, macro_ctx, &mut out, &mut last_was_comma);
    if out.last().is_some_and(|n| is_selector_comma(n)) {
        out.pop();
    }
    out
}

fn expand_selectors_into<'a>(
    selectors: &'a [SelectorNode<'a>],
    macro_ctx: &mut MacroContext<'a>,
    out: &mut Vec<&'a SelectorNode<'a>>,
    last_was_comma: &mut bool,
) {
    for selector_node in selectors {
        if let SelectorNode::MacroCall { name, body } = selector_node {
            let Token::MacroCallIdentifier(Some(macro_name)) = name.token.value() else {
                continue;
            };
            let macro_name_str: &'a str = *macro_name;

            if macro_ctx
                .expansion_stack
                .iter()
                .any(|n| n == macro_name_str)
            {
                continue;
            }

            let arg_count = collect_call_args(body).len();

            let matched_body: Option<&'a MacroBodyContent<'a>> = macro_ctx
                .local
                .get(macro_name_str)
                .and_then(|defs| defs.iter().find(|d| d.arg_names.len() == arg_count))
                .and_then(|def| def.body)
                .or_else(|| {
                    crate::builtins::BUILTINS
                        .registry
                        .get(macro_name_str)
                        .and_then(|defs| defs.iter().find(|d| d.arg_names.len() == arg_count))
                        .and_then(|def| def.body)
                });

            let Some(MacroBodyContent::Selector(Some(inner))) = matched_body else {
                continue;
            };

            macro_ctx.expansion_stack.push(macro_name_str.to_string());
            expand_selectors_into(inner, macro_ctx, out, last_was_comma);
            macro_ctx.expansion_stack.pop();
            continue;
        }

        if is_selector_comma(selector_node) {
            if *last_was_comma {
                continue;
            }
            *last_was_comma = true;
        } else {
            *last_was_comma = false;
        }
        out.push(selector_node);
    }
}

fn collect_call_args<'a>(body: &'a Option<Delimited<'a>>) -> Vec<&'a Construct<'a>> {
    let Some(body) = body else {
        return Vec::new();
    };
    let Some(content) = &body.content else {
        return Vec::new();
    };
    content
        .iter()
        .filter(|c| {
            !matches!(
                c,
                Construct::Node { node } if matches!(node.token.value(), Token::Comma)
            )
        })
        .collect()
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
