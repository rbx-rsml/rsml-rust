use palette::Srgb;
use rbx_types::{Color3uint8, Content, EnumItem, UDim, Variant};
use rbx_types_ops::BasicOperations;

use crate::lexer::Token;
use crate::parser::types::{Construct, Delimited, Node, SelectorNode};
use crate::parser::ParsedRsml;

pub mod datatype;
pub mod tree_node;
mod colors;
mod selector;
mod tuple;
pub mod variants;

use colors::{BRICK_COLORS, CSS_COLORS, SKIN_COLORS, TAILWIND_COLORS};
use datatype::Datatype;
use selector::build_selector_string;
use tree_node::*;
use variants::EnumItemFromNameAndValueName;

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
                    if let Some(Datatype::Variant(Variant::Float32(value))) =
                        evaluate_construct(body, None, tree_nodes, &idx)
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
                            if let Some(datatype) =
                                evaluate_construct(body, None, tree_nodes, &idx)
                            {
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

    match left.token.value() {
        Token::Identifier(prop_name) => {
            if let TreeNodeType::Node(node_idx) = idx {
                let datatype =
                    evaluate_construct(right, Some(prop_name), tree_nodes, &idx);
                let variant = datatype.and_then(|d| d.coerce_to_variant(Some(prop_name)));

                if let Some(variant) = variant {
                    if let Some(node) = tree_nodes[node_idx].as_mut() {
                        node.properties.insert(prop_name.to_string(), variant);
                    }
                }
            }
        }

        Token::TokenIdentifier(attr_name) => {
            let datatype =
                evaluate_construct(right, Some(attr_name), tree_nodes, &idx);
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
            let datatype =
                evaluate_construct(right, Some(static_name), tree_nodes, &idx);
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

fn evaluate_construct(
    construct: &Construct,
    key: Option<&str>,
    tree_nodes: &TreeNodeGroup,
    current_idx: &TreeNodeType,
) -> Option<Datatype> {
    match construct {
        Construct::Node { node } => evaluate_token(node, key, tree_nodes, current_idx),

        Construct::MathOperation {
            left,
            operators,
            right,
        } => {
            let left_val = evaluate_construct(left, key, tree_nodes, current_idx)?;
            let right_val = right
                .as_ref()
                .and_then(|r| evaluate_construct(r, key, tree_nodes, current_idx));

            let Some(right_val) = right_val else {
                return Some(left_val);
            };

            let left_variant = left_val.coerce_to_variant(key)?;
            let right_variant = right_val.coerce_to_variant(key)?;

            let result = if let Some(first_op) = operators.first() {
                apply_operator(first_op, &left_variant, &right_variant)
            } else {
                None
            };

            result.map(Datatype::Variant)
        }

        Construct::UnaryMinus { operand, .. } => {
            let val = evaluate_construct(operand, key, tree_nodes, current_idx)?;
            let variant = val.coerce_to_variant(key)?;
            negate_variant(&variant).map(Datatype::Variant)
        }

        Construct::Table { body } => {
            let datatypes = evaluate_delimited_to_vec(body, tree_nodes, current_idx);
            coerce_tuple_data(datatypes, None)
        }

        Construct::AnnotatedTable { annotation, body } => {
            let annotation_name = match annotation.token.value() {
                Token::Identifier(name) => Some(*name),
                _ => None,
            };

            if let Some(body) = body {
                let datatypes =
                    evaluate_delimited_to_vec(body, tree_nodes, current_idx);
                coerce_tuple_data(datatypes, annotation_name)
            } else {
                coerce_tuple_data(vec![], annotation_name)
            }
        }

        Construct::Enum {
            name, variant, ..
        } => {
            let enum_name = name.as_ref().and_then(|n| match n.token.value() {
                Token::TagSelectorOrEnumPart(Some(s)) => Some(*s),
                _ => None,
            });

            let enum_value = variant.as_ref().and_then(|v| match v.token.value() {
                Token::Identifier(s) => Some(*s),
                Token::TagSelectorOrEnumPart(Some(s)) => Some(*s),
                Token::StateSelectorOrEnumPart(Some(s)) => Some(*s),
                _ => None,
            });

            match (enum_name, enum_value) {
                (Some(name), Some(value)) => EnumItem::from_name_and_value_name(name, value)
                    .map(|item| Datatype::Variant(Variant::EnumItem(item)))
                    .or(Some(Datatype::None)),
                _ => Some(Datatype::None),
            }
        }

        Construct::Assignment { right, .. } => right
            .as_ref()
            .and_then(|r| evaluate_construct(r, key, tree_nodes, current_idx)),

        _ => None,
    }
}

fn evaluate_token(
    node: &Node,
    key: Option<&str>,
    tree_nodes: &TreeNodeGroup,
    current_idx: &TreeNodeType,
) -> Option<Datatype> {
    match node.token.value() {
        Token::Number(s) => parse_number_str(s).map(|n| Datatype::Variant(Variant::Float32(n))),

        Token::NumberOffset(s) => {
            let num_str = s.strip_suffix("px").unwrap_or(s);
            let offset = parse_number_str(num_str).map(|n| n as i32).unwrap_or(0);
            Some(Datatype::Variant(Variant::UDim(UDim::new(0.0, offset))))
        }

        Token::NumberScale(s) => {
            let num_str = s.strip_suffix('%').unwrap_or(s);
            let scale = parse_number_str(num_str).unwrap_or(0.0) / 100.0;
            Some(Datatype::Variant(Variant::UDim(UDim::new(scale, 0))))
        }

        Token::StringSingle(s) => Some(Datatype::Variant(Variant::String(s.to_string()))),

        Token::StringMulti(multi) => {
            Some(Datatype::Variant(Variant::String(multi.content.to_string())))
        }

        Token::RbxAsset(slice) => {
            Some(Datatype::Variant(Variant::String(slice.to_string())))
        }

        Token::RbxContent(slice) => {
            Some(Datatype::Variant(Variant::Content(Content::from(
                slice.to_string(),
            ))))
        }

        Token::Boolean(s) => {
            let val = *s == "true";
            Some(Datatype::Variant(Variant::Bool(val)))
        }

        Token::Nil => Some(Datatype::None),

        Token::ColorHex(slice) => {
            let hex = normalize_hex(slice);
            let color: Result<Srgb<u8>, _> = hex.parse();
            color.ok().map(|c| {
                Datatype::Variant(Variant::Color3(
                    Color3uint8::new(c.red, c.green, c.blue).into(),
                ))
            })
        }

        Token::ColorTailwind(slice) => {
            TAILWIND_COLORS
                .get(&slice.to_lowercase())
                .map(|color| Datatype::Oklab(***color))
        }

        Token::ColorSkin(slice) => {
            SKIN_COLORS
                .get(&slice.to_lowercase())
                .map(|color| Datatype::Oklab(***color))
        }

        Token::ColorCss(slice) => {
            CSS_COLORS
                .get(&slice.to_lowercase())
                .map(|color| Datatype::Oklab(***color))
        }

        Token::ColorBrick(slice) => {
            BRICK_COLORS
                .get(&slice.to_lowercase())
                .map(|color| Datatype::Oklab(***color))
        }

        Token::TokenIdentifier(attr_name) => {
            Some(Datatype::Variant(Variant::String(format!("${}", attr_name))))
        }

        Token::StaticTokenIdentifier(static_name) => {
            Some(resolve_static_attribute(static_name, tree_nodes, *current_idx))
        }

        Token::StateSelectorOrEnumPart(Some(value)) => {
            if let Some(key) = key {
                let rebinded_key = shorthand_rebind(key);
                EnumItem::from_name_and_value_name(rebinded_key, value)
                    .map(|item| Datatype::Variant(Variant::EnumItem(item)))
                    .or(Some(Datatype::None))
            } else {
                Some(Datatype::IncompleteEnumShorthand(value.to_string()))
            }
        }

        _ => None,
    }
}

fn evaluate_delimited_to_vec(
    delimited: &Delimited,
    tree_nodes: &TreeNodeGroup,
    current_idx: &TreeNodeType,
) -> Vec<Datatype> {
    let Some(content) = &delimited.content else {
        return vec![];
    };

    content
        .iter()
        .filter_map(|c| evaluate_construct(c, None, tree_nodes, current_idx))
        .collect()
}

fn coerce_tuple_data(datatypes: Vec<Datatype>, name: Option<&str>) -> Option<Datatype> {
    let mut t = tuple::Tuple::new(name.map(|s| s.to_string()), None);
    for d in datatypes {
        t.push(d);
    }
    let result = t.coerce_to_datatype();
    match result {
        Datatype::None => None,
        other => Some(other),
    }
}

fn negate_variant(variant: &Variant) -> Option<Variant> {
    match variant {
        Variant::Float32(n) => Some(Variant::Float32(-n)),
        Variant::UDim(udim) => Some(Variant::UDim(UDim::new(-udim.scale, -udim.offset))),
        _ => None,
    }
}

fn apply_operator(op_node: &Node, left: &Variant, right: &Variant) -> Option<Variant> {
    match op_node.token.value() {
        Token::OpAdd => left.add(right),
        Token::OpSub => left.sub(right),
        Token::OpMult => left.mult(right),
        Token::OpDiv => left.div(right),
        Token::OpFloorDiv => left.floor_div(right),
        Token::OpMod => left.modulus(right),
        Token::OpPow => left.pow(right),
        _ => None,
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
            .cloned()
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

fn normalize_hex(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 | 6 => hex.into(),
        1..=5 => format!("{:0<6}", hex),
        _ => hex.into(),
    }
}

const SHORTHAND_REBINDS: phf::Map<&'static str, &'static str> = phf_macros::phf_map! {
    "FlexMode" => "UIFlexMode",
    "HorizontalFlex" => "UIFlexAlignment",
    "VerticalFlex" => "UIFlexAlignment",
};

fn shorthand_rebind<'a>(key: &'a str) -> &'a str {
    SHORTHAND_REBINDS.get(key).copied().unwrap_or(key)
}

fn parse_number_str(s: &str) -> Option<f32> {
    let cleaned: String = s.chars().filter(|c| *c != '_').collect();
    cleaned.parse::<f32>().ok()
}
