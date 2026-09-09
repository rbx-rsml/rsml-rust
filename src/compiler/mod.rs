use std::collections::{HashMap, HashSet};

use rbx_types::Variant;

use crate::datatype::{Datatype, StaticLookup, evaluate_construct};
use crate::lexer::{MultilineString, Token};
use crate::macro_registry::{
    MacroDefinition, MacroKey, MacroRegistry, collect_macro_def_arg_names, macro_return_context,
};
use crate::parser::types::{Construct, Delimited, MacroBodyContent, Node, SelectorNode};
use crate::parser::{ParsedRsml, RsmlParser};

mod selector;
pub mod tree_node;

use selector::build_selector_string;
use tree_node::*;

pub struct RsmlCompiler<'a> {
    pub parsed: ParsedRsml<'a>,
}

#[derive(Clone, Copy)]
pub struct BoundArg<'a> {
    pub construct: &'a Construct<'a>,
    pub scope_depth: usize,
}

pub type BindingFrame<'a> = HashMap<String, BoundArg<'a>>;

pub struct MacroContext<'a> {
    pub local: MacroRegistry<'a>,
    pub imported_macros: HashMap<(String, usize), crate::cross_file_imports::ImportedMacroDef>,
    pub bindings: Vec<BindingFrame<'a>>,
    pub active_expansions: HashSet<MacroKey<'a>>,
    pub active_imported_expansions: HashSet<(String, usize)>,
    pub nobuiltins: bool,
}

impl<'a> RsmlCompiler<'a> {
    pub fn new(parsed: ParsedRsml<'a>) -> CompiledRsml {
        Self::compile(parsed, Vec::new())
    }

    fn compile(
        parsed: ParsedRsml<'a>,
        imported_macro_sources: Vec<ParsedRsml<'a>>,
    ) -> CompiledRsml {
        let compiler = Self { parsed };
        let mut tree_nodes = CompiledRsml::new();
        tree_nodes.is_static = compiler.parsed.directives.static_file;
        let is_static = tree_nodes.is_static;
        let mut current_idx = TreeNodeType::Root;

        let mut local = MacroRegistry::new();
        for imported in &imported_macro_sources {
            local.extend(collect_user_macros(&imported.ast));
        }
        local.extend(collect_user_macros(&compiler.parsed.ast));

        let (imported_macros, imported_statics) = collect_imports(&compiler.parsed.ast);
        let mut macro_ctx = MacroContext {
            local,
            imported_macros,
            bindings: vec![HashMap::new()],
            active_expansions: HashSet::new(),
            active_imported_expansions: HashSet::new(),
            nobuiltins: compiler.parsed.directives.nobuiltins,
        };

        for (name, value) in imported_statics {
            if let Some(static_val) = value.coerce_to_static(Some(&name)) {
                if let Some(node) = tree_nodes.get_root_mut() {
                    node.static_attributes.insert(name, static_val);
                }
            }
        }

        for construct in &compiler.parsed.ast {
            if is_static && !construct.allowed_in_static_file() {
                continue;
            }
            if let Construct::Derive {
                body: Some(body), ..
            } = construct
            {
                match classify_derive(body) {
                    DeriveClass::Invalid => continue,
                    DeriveClass::NonStatic if is_static => continue,
                    _ => {}
                }
            }
            compile_construct(construct, &mut tree_nodes, &mut current_idx, &mut macro_ctx);
        }

        tree_nodes
    }

    pub fn from_source(source: &'a str) -> CompiledRsml {
        Self::new(RsmlParser::from_source(source))
    }

    /// Compiles `source` with macros declared by already-resolved derived
    /// sources. Sources are applied in iterator order and macros declared in
    /// the root source take precedence.
    pub fn from_source_with_macro_sources(
        source: &'a str,
        macro_sources: impl IntoIterator<Item = &'a str>,
    ) -> CompiledRsml {
        let parsed = RsmlParser::from_source(source);
        let imported_macro_sources = macro_sources
            .into_iter()
            .map(RsmlParser::from_source)
            .collect();

        Self::compile(parsed, imported_macro_sources)
    }
}

enum DeriveClass {
    /// Body isn't a single string literal (e.g. a table of paths). The
    /// typechecker handles per-item validation; the compiler keeps these as-is.
    Other,
    /// Resolves to a valid `.rsml` file with `--!static`.
    Static,
    /// Resolves to a valid `.rsml` file without `--!static`.
    NonStatic,
    /// Wrong extension, missing, points at a directory, or unreadable.
    Invalid,
}

fn classify_derive(body: &Construct<'_>) -> DeriveClass {
    let Some(literal_path) = extract_derive_string_literal(body) else {
        return DeriveClass::Other;
    };

    let mut path = std::path::PathBuf::from(literal_path.trim());
    match path.extension() {
        None => {
            path.set_extension("rsml");
        }
        Some(ext) if ext.eq_ignore_ascii_case("rsml") => {}
        Some(_) => return DeriveClass::Invalid,
    }

    let Ok(canonical) = path.canonicalize() else {
        return DeriveClass::Invalid;
    };
    if !canonical.is_file() {
        return DeriveClass::Invalid;
    }
    let Ok(source) = std::fs::read_to_string(&canonical) else {
        return DeriveClass::Invalid;
    };

    if RsmlParser::from_source(&source).directives.static_file {
        DeriveClass::Static
    } else {
        DeriveClass::NonStatic
    }
}

fn extract_derive_string_literal<'a>(body: &'a Construct<'a>) -> Option<&'a str> {
    let Construct::Node { node } = body else {
        return None;
    };
    match node.token.value() {
        Token::StringSingle(content) => Some(*content),
        Token::StringMulti(MultilineString { content, .. }) => Some(*content),
        _ => None,
    }
}

/// Walks the AST for `@derive` constructs that carry a `@with` clause and
/// pulls the requested public macros + static tokens out of each derived
/// file. Diagnostics on import failures are the typechecker's job; the
/// compiler silently drops anything it can't resolve.
fn collect_imports<'a>(
    ast: &'a [Construct<'a>],
) -> (
    HashMap<(String, usize), crate::cross_file_imports::ImportedMacroDef>,
    Vec<(String, Datatype)>,
) {
    use crate::cross_file_imports::{
        ImportSelector, build_import_selector_silent, load_public_symbols,
    };

    let mut macros: HashMap<(String, usize), crate::cross_file_imports::ImportedMacroDef> =
        HashMap::new();
    let mut statics: Vec<(String, Datatype)> = Vec::new();

    for construct in ast {
        let Construct::Derive {
            body: Some(body),
            with_clause: Some(with_clause),
            ..
        } = construct
        else {
            continue;
        };

        let Some(literal) = extract_derive_string_literal(body) else {
            continue;
        };

        let mut path = std::path::PathBuf::from(literal.trim());
        if path.extension().is_none() {
            path.set_extension("rsml");
        }
        let Ok(canonical) = path.canonicalize() else {
            continue;
        };

        let Some(symbols) = load_public_symbols(&canonical) else {
            continue;
        };

        let Some(selector) = build_import_selector_silent(with_clause) else {
            continue;
        };

        match selector {
            ImportSelector::All => {
                for ((name, arity), def) in &symbols.macros {
                    macros.insert((name.clone(), *arity), def.clone());
                }
                for (name, def) in &symbols.statics {
                    statics.push((name.clone(), def.value.clone()));
                }
            }
            ImportSelector::Named(items) => {
                for item in items {
                    if item.is_static_token {
                        if let Some(def) = symbols.statics.get(&item.name) {
                            statics.push((item.name.clone(), def.value.clone()));
                        }
                    } else {
                        for ((mname, arity), mdef) in &symbols.macros {
                            if mname == &item.name {
                                macros.insert((mname.clone(), *arity), mdef.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    (macros, statics)
}

fn collect_user_macros<'a>(ast: &'a [Construct<'a>]) -> MacroRegistry<'a> {
    let mut registry = MacroRegistry::new();
    for construct in ast {
        if let Construct::Macro {
            pub_modifier,
            name: Some(name_node),
            args,
            body,
            return_type,
            ..
        } = construct
        {
            if let Token::Identifier(name_str) = name_node.token.value() {
                let arg_names = collect_macro_def_arg_names(args);

                registry.insert(
                    MacroKey {
                        name: *name_str,
                        arity: arg_names.len(),
                    },
                    MacroDefinition {
                        arg_names,
                        body: body.as_ref().map(|b| &b.content),
                        return_context: macro_return_context(return_type),
                        is_public: pub_modifier.is_some(),
                    },
                );
            }
        }
    }
    registry
}

struct CompilerLookup<'a> {
    tree_nodes: &'a CompiledRsml,
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
    tree_nodes: &mut CompiledRsml,
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

                    if let Some(Datatype::Variant(Variant::Float64(value))) =
                        evaluate_construct(body, None, &lookup)
                    {
                        if let Some(node) = tree_nodes[node_idx].as_mut() {
                            node.priority = Some(value as i32);
                        }
                    }
                }
            }
        }

        Construct::Tween { name, .. } => {
            let TreeNodeType::Node(node_idx) = *current_idx else {
                return;
            };
            let Some(name_node) = name else { return };
            let Token::Identifier(tween_name) = name_node.token.value() else {
                return;
            };

            let idx = *current_idx;
            let active_scope_depth = current_scope_depth(macro_ctx);
            let lookup = CompilerLookup {
                tree_nodes,
                idx,
                macro_ctx: Some(&*macro_ctx),
                active_scope_depth,
            };

            let Some(datatype) = evaluate_construct(construct, None, &lookup) else {
                return;
            };
            let Some(variant @ Variant::TweenInfo(_)) = datatype.coerce_to_variant(None) else {
                return;
            };
            let Some(node) = tree_nodes[node_idx].as_mut() else {
                return;
            };

            node.tweens.insert(tween_name.to_string(), variant);
        }

        Construct::MacroCall { name, body, .. } => {
            compile_macro_call(name, body, tree_nodes, current_idx, macro_ctx);
        }

        Construct::Derive { .. }
        | Construct::Macro { .. }
        | Construct::Schema { .. }
        | Construct::Extends { .. } => {}

        _ => {}
    }
}

fn compile_rule<'a>(
    selectors: &'a Option<Vec<SelectorNode<'a>>>,
    body: &'a Option<Delimited<'a>>,
    tree_nodes: &mut CompiledRsml,
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
    tree_nodes: &mut CompiledRsml,
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
    tree_nodes: &mut CompiledRsml,
    current_idx: &mut TreeNodeType,
    macro_ctx: &mut MacroContext<'a>,
) {
    let Token::MacroCallIdentifier(Some(macro_name)) = name.token.value() else {
        return;
    };

    let macro_name_str = *macro_name;
    let call_args = collect_call_args(call_body);
    let arg_count = call_args.len();
    let key = MacroKey {
        name: macro_name_str,
        arity: arg_count,
    };

    if macro_ctx.active_expansions.contains(&key) {
        return;
    }

    let imported_key = (macro_name_str.to_string(), arg_count);
    if macro_ctx.active_imported_expansions.contains(&imported_key) {
        return;
    }

    let (arg_names, body): (Vec<String>, &'a MacroBodyContent<'a>) = {
        let from_local = macro_ctx.local.get(&key).and_then(|def| {
            def.body
                .map(|b| (def.arg_names.iter().map(|s| s.to_string()).collect(), b))
        });

        if let Some(pair) = from_local {
            pair
        } else if !macro_ctx.nobuiltins
            && let Some(pair) = crate::builtins::BUILTINS
                .registry
                .get(&key)
                .and_then(|def| {
                    def.body
                        .map(|b| (def.arg_names.iter().map(|s| s.to_string()).collect(), b))
                })
        {
            pair
        } else if let Some(pair) = macro_ctx
            .imported_macros
            .get(&imported_key)
            .and_then(|def| {
                def.body.map(|b| {
                    let names: Vec<String> = def.arg_names.iter().map(|s| s.to_string()).collect();
                    let coerced: &'a MacroBodyContent<'a> = b;
                    (names, coerced)
                })
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
    macro_ctx.active_expansions.insert(key);

    for construct in constructs.iter() {
        compile_construct(construct, tree_nodes, current_idx, macro_ctx);
    }

    macro_ctx.active_expansions.remove(&key);
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
            let arg_count = collect_call_args(body).len();
            let key = MacroKey {
                name: macro_name_str,
                arity: arg_count,
            };

            if macro_ctx.active_expansions.contains(&key) {
                continue;
            }

            let matched_body: Option<&'a MacroBodyContent<'a>> = macro_ctx
                .local
                .get(&key)
                .and_then(|def| def.body)
                .or_else(|| {
                    if macro_ctx.nobuiltins {
                        return None;
                    }

                    crate::builtins::BUILTINS
                        .registry
                        .get(&key)
                        .and_then(|def| def.body)
                })
                .or_else(|| {
                    macro_ctx
                        .imported_macros
                        .get(&(macro_name_str.to_string(), arg_count))
                        .and_then(|def| def.body)
                        .map(|b| {
                            let coerced: &'a MacroBodyContent<'a> = b;
                            coerced
                        })
                });

            let Some(MacroBodyContent::Selector(Some(inner))) = matched_body else {
                continue;
            };

            macro_ctx.active_expansions.insert(key);
            expand_selectors_into(inner, macro_ctx, out, last_was_comma);
            macro_ctx.active_expansions.remove(&key);
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

fn resolve_static_attribute(name: &str, tree_nodes: &CompiledRsml, idx: TreeNodeType) -> Datatype {
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

#[cfg(test)]
mod tests {
    use super::RsmlCompiler;
    use rbx_types::Variant;

    #[test]
    fn compiles_macros_from_imported_sources() {
        let macro_source = r#"
@macro Fade -> Construct {
    BackgroundTransparency = 0.5;
}
"#;
        let source = r#"
Frame {
    Fade!();
}
"#;

        let mut compiled = RsmlCompiler::from_source_with_macro_sources(source, [macro_source]);
        let root = compiled.take_root().unwrap();
        let frame = compiled.take_node(root.child_rules[0]).unwrap();

        assert!(frame.properties.get("BackgroundTransparency").is_some());
    }

    #[test]
    fn root_macros_override_imported_macros() {
        let macro_source = r#"
@macro Fade -> Construct {
    BackgroundTransparency = 0.5;
}
"#;
        let source = r#"
@macro Fade -> Construct {
    BackgroundTransparency = 0.75;
}

Frame {
    Fade!();
}
"#;

        let mut compiled = RsmlCompiler::from_source_with_macro_sources(source, [macro_source]);
        let root = compiled.take_root().unwrap();
        let frame = compiled.take_node(root.child_rules[0]).unwrap();

        assert_eq!(
            frame.properties.get("BackgroundTransparency"),
            Some(&Variant::Float64(0.75))
        );
    }
}
