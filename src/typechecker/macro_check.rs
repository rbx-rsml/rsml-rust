use std::collections::{HashMap, HashSet};

use crate::{
    lexer::Token,
    parser::{AstErrors, Construct, Delimited, MacroBody, MacroBodyContent, Node, SelectorNode},
    range_from_span::RangeFromSpan,
};

use crate::typechecker::{ReportTypeError, Typechecker, type_error::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroReturnContext {
    Construct,
    Datatype,
    Selector,
}

impl MacroReturnContext {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Construct => "Construct",
            Self::Datatype => "Datatype",
            Self::Selector => "Selector",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MacroDefinition<'a> {
    pub arg_names: Vec<&'a str>,
    pub body: Option<&'a MacroBodyContent<'a>>,
    pub return_context: MacroReturnContext,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct MacroKey<'a> {
    pub name: &'a str,
    pub arity: usize,
}

pub type MacroRegistry<'a> = HashMap<MacroKey<'a>, MacroDefinition<'a>>;

pub fn collect_macro_def_arg_names<'a>(args: &Option<Delimited<'a>>) -> Vec<&'a str> {
    let Some(args) = args else { return Vec::new() };
    let Some(content) = &args.content else {
        return Vec::new();
    };
    content
        .iter()
        .filter_map(|construct| {
            if let Construct::Node { node } = construct {
                if let Token::MacroArgIdentifier(Some(name)) = node.token.value() {
                    return Some(*name);
                }
            }
            None
        })
        .collect()
}

pub(super) fn count_macro_call_args(body: &Option<Delimited>) -> usize {
    let Some(body) = body else { return 0 };
    let Some(content) = &body.content else {
        return 0;
    };
    if content.is_empty() {
        return 0;
    }
    content
        .iter()
        .filter(|construct| {
            matches!(
                construct,
                Construct::Node { node } if matches!(node.token.value(), Token::Comma)
            )
        })
        .count()
        + 1
}

pub fn macro_return_context(return_type: &Option<(Node, Option<Node>)>) -> MacroReturnContext {
    if let Some((_, Some(ident))) = return_type {
        match ident.token.value() {
            Token::Identifier("Datatype") => MacroReturnContext::Datatype,
            Token::Identifier("Selector") => MacroReturnContext::Selector,
            _ => MacroReturnContext::Construct,
        }
    } else {
        MacroReturnContext::Construct
    }
}

impl<'a> Typechecker<'a> {
    pub(super) fn typecheck_macro(
        &self,
        args: &Option<Delimited<'a>>,
        body: &Option<MacroBody<'a>>,
        ast_errors: &mut AstErrors,
    ) {
        let macro_args = collect_macro_arg_names(args);
        let Some(body) = body else { return };

        match &body.content {
            MacroBodyContent::Construct(Some(content)) => {
                self.typecheck_macro_body_content(content, &macro_args, ast_errors);
            }
            MacroBodyContent::Datatype(Some(content)) => {
                self.validate_macro_arg_refs(content, Some(&macro_args), ast_errors);
                self.validate_annotation(content, ast_errors);
                if let Construct::MacroCall { name, body, .. } = content.as_ref() {
                    self.validate_macro_call(name, body, MacroReturnContext::Datatype, ast_errors);
                }
            }
            MacroBodyContent::Selector(Some(selectors)) => {
                for selector in selectors {
                    if let SelectorNode::MacroCall { name, body } = selector {
                        self.validate_macro_call(
                            name,
                            body,
                            MacroReturnContext::Selector,
                            ast_errors,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn typecheck_macro_body_content(
        &self,
        content: &Vec<Construct<'a>>,
        macro_args: &HashSet<&str>,
        ast_errors: &mut AstErrors,
    ) {
        for construct in content {
            match construct {
                Construct::Assignment { right, .. } => {
                    if let Some(right) = right {
                        self.validate_macro_arg_refs(right, Some(macro_args), ast_errors);
                        self.validate_annotation(right, ast_errors);
                        if let Construct::MacroCall { name, body, .. } = right.as_ref() {
                            self.validate_macro_call(
                                name,
                                body,
                                MacroReturnContext::Datatype,
                                ast_errors,
                            );
                        }
                    }
                }

                Construct::Rule { body, .. } => {
                    if let Some(body) = body {
                        if let Some(content) = &body.content {
                            self.typecheck_macro_body_content(content, macro_args, ast_errors);
                        }
                    }
                }

                Construct::Tween { body, .. } => {
                    if let Some(body) = body {
                        self.validate_macro_arg_refs(body, Some(macro_args), ast_errors);
                    }
                }

                Construct::MacroCall { name, body, .. } => {
                    self.validate_macro_call(name, body, MacroReturnContext::Construct, ast_errors);
                }

                Construct::Macro { .. } => {
                    ast_errors.report(
                        TypeError::NotAllowedInContext {
                            name: construct.name_plural(),
                            context: "other macros",
                        },
                        self.range_from_span(construct.span()),
                    );
                }

                Construct::Derive { .. } => {
                    ast_errors.report(
                        TypeError::NotAllowedInContext {
                            name: construct.name_plural(),
                            context: "non-global scopes",
                        },
                        self.range_from_span(construct.span()),
                    );
                }

                _ => (),
            }
        }
    }

    pub(super) fn validate_macro_call(
        &self,
        name: &Node<'a>,
        body: &Option<Delimited<'a>>,
        expected_context: MacroReturnContext,
        ast_errors: &mut AstErrors,
    ) {
        let Token::MacroCallIdentifier(Some(macro_name)) = name.token.value() else {
            return;
        };

        let local_arities = self
            .macro_registry
            .keys()
            .filter(|k| k.name == *macro_name)
            .map(|k| k.arity);
        let builtin_arities = crate::builtins::BUILTINS
            .registry
            .keys()
            .filter(|k| k.name == *macro_name)
            .map(|k| k.arity);

        let mut expected_counts: Vec<usize> = local_arities.chain(builtin_arities).collect();

        if expected_counts.is_empty() {
            ast_errors.report(
                TypeError::UndefinedMacro { name: macro_name },
                self.range_from_span(name.token.span()),
            );
            return;
        }

        let call_arg_count = count_macro_call_args(body);
        let key = MacroKey {
            name: *macro_name,
            arity: call_arg_count,
        };

        let matching_context = self
            .macro_registry
            .get(&key)
            .map(|def| def.return_context)
            .or_else(|| {
                crate::builtins::BUILTINS
                    .registry
                    .get(&key)
                    .map(|def| def.return_context)
            });

        let Some(matching_context) = matching_context else {
            expected_counts.sort();
            expected_counts.dedup();

            ast_errors.report(
                TypeError::WrongMacroArgCount {
                    name: macro_name,
                    expected: expected_counts,
                    got: call_arg_count,
                },
                self.range_from_span(name.token.span()),
            );
            return;
        };

        if matching_context != expected_context {
            ast_errors.report(
                TypeError::WrongMacroContext {
                    name: macro_name,
                    expected: matching_context.name(),
                    got: expected_context.name(),
                },
                self.range_from_span(name.token.span()),
            );
        }
    }

    pub(super) fn validate_macro_arg_refs(
        &self,
        construct: &Construct<'a>,
        macro_args: Option<&HashSet<&str>>,
        ast_errors: &mut AstErrors,
    ) {
        match construct {
            Construct::Node { node } => {
                if let Token::MacroArgIdentifier(name) = node.token.value() {
                    let is_valid = match macro_args {
                        Some(args) => name.is_some_and(|arg_name| args.contains(arg_name)),
                        None => false,
                    };

                    if !is_valid {
                        if let Some(arg_name) = name {
                            ast_errors.report(
                                TypeError::InvalidMacroArg {
                                    msg: &format!(
                                        "No macro argument named \"{}\" exists.",
                                        arg_name
                                    ),
                                },
                                self.range_from_span(node.token.span()),
                            );
                        } else {
                            ast_errors.report(
                                TypeError::InvalidMacroArg {
                                    msg: "Missing macro argument name.",
                                },
                                self.range_from_span(node.token.span()),
                            );
                        }
                    }
                }
            }

            Construct::MathOperation { left, right, .. } => {
                self.validate_macro_arg_refs(left, macro_args, ast_errors);
                if let Some(right) = right {
                    self.validate_macro_arg_refs(right, macro_args, ast_errors);
                }
            }

            Construct::UnaryMinus { operand, .. } => {
                self.validate_macro_arg_refs(operand, macro_args, ast_errors);
            }

            Construct::Table { body } => {
                let Some(content) = &body.content else { return };
                for item in content {
                    self.validate_macro_arg_refs(item, macro_args, ast_errors);
                }
            }

            Construct::AnnotatedTable { body, .. } => {
                let Some(body) = body else { return };
                let Some(content) = &body.content else { return };
                for item in content {
                    self.validate_macro_arg_refs(item, macro_args, ast_errors);
                }
            }

            _ => (),
        }
    }

    fn range_from_span(&self, span: (usize, usize)) -> crate::types::Range {
        crate::types::Range::from_span(&self.parsed.rope, span)
    }
}

fn collect_macro_arg_names<'a>(args: &Option<Delimited<'a>>) -> HashSet<&'a str> {
    let mut names = HashSet::new();
    if let Some(args) = args {
        if let Some(content) = &args.content {
            for construct in content {
                if let Construct::Node { node } = construct {
                    if let Token::MacroArgIdentifier(Some(name)) = node.token.value() {
                        names.insert(*name);
                    }
                }
            }
        }
    }
    names
}

fn for_each_macro_call_in_body<'a, F>(body: &MacroBodyContent<'a>, cb: &mut F)
where
    F: FnMut(&'a str, usize, (usize, usize)),
{
    match body {
        MacroBodyContent::Construct(Some(content)) => {
            for construct in content {
                visit_construct_for_calls(construct, cb);
            }
        }

        MacroBodyContent::Datatype(Some(content)) => {
            visit_construct_for_calls(content, cb);
        }

        MacroBodyContent::Selector(Some(selectors)) => {
            visit_selectors_for_calls(selectors, cb);
        }

        _ => {}
    }
}

fn visit_construct_for_calls<'a, F>(construct: &Construct<'a>, cb: &mut F)
where
    F: FnMut(&'a str, usize, (usize, usize)),
{
    match construct {
        Construct::MacroCall { name, body, .. } => {
            if let Token::MacroCallIdentifier(Some(n)) = name.token.value() {
                cb(*n, count_macro_call_args(body), name.token.span());
            }
        }

        Construct::Assignment { right, .. } => {
            if let Some(right) = right {
                visit_construct_for_calls(right, cb);
            }
        }

        Construct::Rule { selectors, body } => {
            if let Some(selectors) = selectors {
                visit_selectors_for_calls(selectors, cb);
            }

            if let Some(body) = body {
                if let Some(content) = &body.content {
                    for inner in content {
                        visit_construct_for_calls(inner, cb);
                    }
                }
            }
        }

        _ => {}
    }
}

fn visit_selectors_for_calls<'a, F>(selectors: &[SelectorNode<'a>], cb: &mut F)
where
    F: FnMut(&'a str, usize, (usize, usize)),
{
    for selector in selectors {
        if let SelectorNode::MacroCall { name, body } = selector {
            if let Token::MacroCallIdentifier(Some(n)) = name.token.value() {
                cb(*n, count_macro_call_args(body), name.token.span());
            }
        }
    }
}

enum DfsColor {
    Gray,
    Black,
}

impl<'a> Typechecker<'a> {
    pub(super) fn detect_recursive_macro_calls(&self, ast_errors: &mut AstErrors) {
        let mut color: HashMap<MacroKey<'a>, DfsColor> = HashMap::new();

        let roots: Vec<MacroKey<'a>> = self.macro_registry.keys().copied().collect();
        for root in roots {
            if color.contains_key(&root) {
                continue;
            }

            self.dfs_macro_cycle(root, &mut color, ast_errors);
        }
    }

    fn dfs_macro_cycle(
        &self,
        key: MacroKey<'a>,
        color: &mut HashMap<MacroKey<'a>, DfsColor>,
        ast_errors: &mut AstErrors,
    ) {
        color.insert(key, DfsColor::Gray);

        let Some(def) = self.macro_registry.get(&key) else {
            color.insert(key, DfsColor::Black);
            return;
        };
        let Some(body) = def.body else {
            color.insert(key, DfsColor::Black);
            return;
        };

        let mut calls: Vec<(&'a str, usize, (usize, usize))> = Vec::new();
        for_each_macro_call_in_body(body, &mut |name, arity, span| {
            calls.push((name, arity, span));
        });

        for (name, arity, span) in calls {
            let callee = MacroKey { name, arity };

            if !self.macro_registry.contains_key(&callee) {
                continue;
            }

            match color.get(&callee) {
                Some(DfsColor::Gray) => {
                    ast_errors.report(TypeError::RecursiveMacroCall, self.range_from_span(span))
                }
                Some(DfsColor::Black) => {}
                None => self.dfs_macro_cycle(callee, color, ast_errors),
            }
        }

        color.insert(key, DfsColor::Black);
    }
}
