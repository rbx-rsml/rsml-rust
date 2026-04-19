use std::collections::{HashMap, HashSet};

use crate::{
    lexer::Token,
    parser::{AstErrors, Construct, Delimited, MacroBody, MacroBodyContent, Node, SelectorNode},
    range_from_span::RangeFromSpan,
};

use crate::typechecker::{PushTypeError, Typechecker, type_error::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroReturnContext {
    Construct,
    Assignment,
    Selector,
}

impl MacroReturnContext {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Construct => "Construct",
            Self::Assignment => "Assignment",
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

pub type MacroRegistry<'a> = HashMap<String, Vec<MacroDefinition<'a>>>;

pub fn collect_macro_def_arg_names<'a>(args: &Option<Delimited<'a>>) -> Vec<&'a str> {
    let Some(args) = args else { return Vec::new() };
    let Some(content) = &args.content else { return Vec::new() };
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
    let Some(content) = &body.content else { return 0 };
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

pub fn macro_return_context(
    return_type: &Option<(Node, Option<Node>)>,
) -> MacroReturnContext {
    if let Some((_, Some(ident))) = return_type {
        match ident.token.value() {
            Token::Identifier("Assignment") => MacroReturnContext::Assignment,
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
            MacroBodyContent::Assignment(Some(content)) => {
                self.validate_macro_arg_refs(content, Some(&macro_args), ast_errors);
                self.validate_annotation(content, ast_errors);
                if let Construct::MacroCall { name, body, .. } = content.as_ref() {
                    self.validate_macro_call(name, body, MacroReturnContext::Assignment, ast_errors);
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
                                MacroReturnContext::Assignment,
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
                    ast_errors.push(
                        TypeError::NotAllowedInContext { name: construct.name_plural(), context: "other macros" },
                        self.range_from_span(construct.span()),
                    );
                }

                Construct::Derive { .. } => {
                    ast_errors.push(
                        TypeError::NotAllowedInContext { name: construct.name_plural(), context: "non-global scopes" },
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

        let local = self.macro_registry.get(*macro_name);
        let builtin = crate::builtins::BUILTINS.registry.get(*macro_name);
        if local.is_none() && builtin.is_none() {
            ast_errors.push(
                TypeError::UndefinedMacro { name: macro_name },
                self.range_from_span(name.token.span()),
            );
            return;
        }

        let call_arg_count = count_macro_call_args(body);

        let local_arg_counts = local
            .into_iter()
            .flat_map(|defs| defs.iter().map(|def| def.arg_names.len()));
        let builtin_arg_counts = builtin
            .into_iter()
            .flat_map(|defs| defs.iter().map(|def| def.arg_names.len()));

        let local_match_context = local
            .into_iter()
            .flat_map(|defs| defs.iter())
            .find(|def| def.arg_names.len() == call_arg_count)
            .map(|def| def.return_context);
        let builtin_match_context = builtin
            .into_iter()
            .flat_map(|defs| defs.iter())
            .find(|def| def.arg_names.len() == call_arg_count)
            .map(|def| def.return_context);

        let matching_context = local_match_context.or(builtin_match_context);

        if matching_context.is_none() {
            let mut expected_counts: Vec<usize> =
                local_arg_counts.chain(builtin_arg_counts).collect();
            expected_counts.sort();
            expected_counts.dedup();
            ast_errors.push(
                TypeError::WrongMacroArgCount {
                    name: macro_name,
                    expected: expected_counts,
                    got: call_arg_count,
                },
                self.range_from_span(name.token.span()),
            );
            return;
        }

        let matching_context = matching_context.unwrap();
        if matching_context != expected_context {
            ast_errors.push(
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
                            ast_errors.push(
                                TypeError::InvalidMacroArg {
                                    msg: &format!(
                                        "No macro argument named \"{}\" exists.",
                                        arg_name
                                    ),
                                },
                                self.range_from_span(node.token.span()),
                            );
                        } else {
                            ast_errors.push(
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
                if let Some(content) = &body.content {
                    for item in content {
                        self.validate_macro_arg_refs(item, macro_args, ast_errors);
                    }
                }
            }

            Construct::AnnotatedTable { body, .. } => {
                if let Some(body) = body {
                    if let Some(content) = &body.content {
                        for item in content {
                            self.validate_macro_arg_refs(item, macro_args, ast_errors);
                        }
                    }
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
