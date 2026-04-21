use std::{mem::discriminant, slice::Iter};

use indexmap::IndexSet;

use crate::{
    lexer::{SpannedToken, Token, TokenKind},
    list::TokenKindList,
    parser::{AstErrors, Construct, Delimited, Node, SelectorNode},
    range_from_span::RangeFromSpan,
    token_kind_list,
};

use phf_macros::phf_set;
use ropey::Rope;
use crate::types::Range;

use crate::typechecker::{DefinitionKind, ReportTypeError, ResolvedTypes, Typechecker, type_error::*};
use crate::typechecker::macro_check::{MacroKey, MacroRegistry, MacroReturnContext};

impl<'a> Typechecker<'a> {
    pub(super) fn typecheck_rule(
        &mut self,
        (selectors, body): (&Option<Vec<SelectorNode<'a>>>, &Option<Delimited<'a>>),
        parent_classes: &Vec<String>,
        ast_errors: &mut AstErrors,
        definitions: &mut crate::typechecker::Definitions,
        resolved_types: &mut ResolvedTypes,
    ) {
        let current_classes = if let Some(selectors) = selectors {
            self.typecheck_selectors(selectors, parent_classes, ast_errors, definitions)
        } else {
            parent_classes.clone()
        };

        let Some(body) = body.as_ref() else { return };

        let body_start = body.left.token.start();
        let body_end = body
            .right
            .as_ref()
            .map(|r| r.token.end())
            .unwrap_or(body.left.token.end());
        definitions.insert(
            body_start..=body_end,
            DefinitionKind::Scope {
                type_definition: current_classes.clone(),
            },
        );

        let Some(content) = body.content.as_ref() else {
            return;
        };

        self.static_scopes.push(std::collections::HashMap::new());
        self.declared_tokens.push(std::collections::HashSet::new());

        for construct in content {
            match construct {
                Construct::Rule { selectors, body } => {
                    self.typecheck_rule((selectors, body), &current_classes, ast_errors, definitions, resolved_types)
                }

                Construct::Assignment {
                    left,
                    right,
                    ..
                } => {
                    if let Some(right) = right {
                        self.validate_token_refs(right, ast_errors);
                        self.validate_macro_arg_refs(right, None, ast_errors);
                        self.validate_annotation(right, ast_errors);
                        if let Construct::MacroCall { name, body, .. } = right.as_ref() {
                            self.validate_macro_call(name, body, MacroReturnContext::Datatype, ast_errors);
                        }
                        self.resolve_token_assignment(left, right, &current_classes, ast_errors, definitions, resolved_types);
                    }
                }

                Construct::Tween {
                    body: Some(body),
                    ..
                } => {
                    self.typecheck_tween(body, ast_errors);
                }

                Construct::Derive { .. } => {
                    ast_errors.report(
                        TypeError::NotAllowedInContext { name: construct.name_plural(), context: "non-global scopes" },
                        Range::from_span(&self.parsed.rope, construct.span()),
                    );
                }

                Construct::MacroCall { name, body, .. } => {
                    self.validate_macro_call(name, body, MacroReturnContext::Construct, ast_errors);
                }

                Construct::Macro { args, body, .. } => {
                    ast_errors.report(
                        TypeError::NotAllowedInContext { name: construct.name_plural(), context: "rules" },
                        Range::from_span(&self.parsed.rope, construct.span()),
                    );
                    self.typecheck_macro(args, body, ast_errors);
                }

                _ => (),
            }
        }

        self.static_scopes.pop();
        self.declared_tokens.pop();
    }

    fn typecheck_selectors(
        &self,
        selectors: &Vec<SelectorNode<'a>>,
        parent_classes: &Vec<String>,
        ast_errors: &mut AstErrors,
        definitions: &mut crate::typechecker::Definitions,
    ) -> Vec<String> {
        TypecheckSelectors::new(
            selectors,
            parent_classes,
            &self.parsed.rope,
            ast_errors,
            definitions,
            &self.macro_registry,
        )
        .classes
        .into_iter()
        .collect()
    }
}

static ALLOWED_PSEUDO_SELECTORS: phf::Set<&str> = phf_set! {
    "UICorner",
    "UIGradient",
    "UIPadding",
    "UIStroke",
    "UIListLayout",
    "UIGridStyleLayout",
    "UIGridLayout",
    "UIPageLayout",
    "UIAspectRatioConstraint",
    "UISizeConstraint",
    "UITextSizeConstraint",
    "UIScale",
    "UIFlexItem",
    "StyleQuery"
};

static ALLOWED_STATE_SELECTORS: phf::Set<&str> = phf_set! {
    "idle",
    "hover",
    "press",
    "pressed",
    "noninteractable"
};

struct TypecheckSelectors<'a> {
    iter: Iter<'a, SelectorNode<'a>>,
    parent_classes: &'a Vec<String>,
    classes: IndexSet<String>,

    part: Option<&'a Node<'a>>,

    rope: &'a Rope,
    ast_errors: &'a mut AstErrors,
    macro_registry: &'a MacroRegistry<'a>,
}

impl<'a> TypecheckSelectors<'a> {
    fn new(
        selectors: &'a Vec<SelectorNode<'a>>,
        parent_classes: &'a Vec<String>,
        rope: &'a Rope,
        ast_errors: &'a mut AstErrors,
        definitions: &mut crate::typechecker::Definitions,
        macro_registry: &'a MacroRegistry<'a>,
    ) -> Self {
        let mut typecheck_selectors = Self {
            iter: selectors.iter(),
            parent_classes,
            classes: IndexSet::new(),
            part: None,
            rope,
            ast_errors,
            macro_registry,
        };

        typecheck_selectors.begin(definitions);

        typecheck_selectors
    }

    fn next(&mut self) -> Option<&'a Node<'a>> {
        loop {
            let next_item = self.iter.next()?;
            match next_item {
                SelectorNode::Token(node) => {
                    self.part = Some(node);
                    return Some(node);
                }
                SelectorNode::MacroCall { name, body } => {
                    self.validate_selector_macro_call(name, body);
                    continue;
                }
            }
        }
    }

    fn begin_iteration(&mut self, part: &'a Node<'a>) {
        if self.parent_classes.is_empty() {
            self.from_new(part);
        } else {
            self.from_parent(part, false);
        }
    }

    fn begin(&mut self, definitions: &mut crate::typechecker::Definitions) {
        let Some(part) = self.next() else { return };
        let span_start = part.token.start();

        self.begin_iteration(part);

        let span_end = self
            .part
            .map(|x| x.token.end())
            .unwrap_or_else(|| part.token.end());

        definitions.insert(
            span_start..=span_end,
            DefinitionKind::selector(self.classes.iter().cloned().collect()),
        );
    }

    fn from_new(&mut self, part: &'a Node<'a>) {
        match part.token.value() {
            Token::TagSelectorOrEnumPart(_) | Token::NameSelector(_) | Token::QuerySelector(_) => {
                self.classes.insert("Instance".to_string());
                self.consume_past_comma();
            }

            Token::Identifier(class) => {
                let validated_class = self.validate_class(class, &part.token);

                match self.consume_with_error(
                    TokenKind::Identifier,
                    token_kind_list![PseudoSelector, StateSelectorOrEnumPart],
                    Some(token_kind_list![TagSelectorOrEnumPart, NameSelector]),
                ) {
                    ConsumeResult::Some(part) => match part.token.value() {
                        Token::PseudoSelector(class) => {
                            let validated_class =
                                self.validate_instance_class(class, &part.token, "Pseudo");
                            self.classes.insert(validated_class.to_string());
                            self.consume_past_comma();
                        }

                        Token::StateSelectorOrEnumPart(Some(class)) => {
                            self.classes.insert(validated_class.to_string());
                            self.validate_state(class, &part.token);
                            self.consume_past_comma();
                        }

                        _ => (),
                    },

                    ConsumeResult::Err(delimiter) => {
                        if matches!(delimiter.token.value(), Token::Comma) {
                            self.classes.insert(validated_class.to_string());
                        }

                        let Some(part) = self.next() else { return };
                        self.begin_iteration(part);
                    }

                    ConsumeResult::None => {
                        self.classes.insert(validated_class.to_string());
                    }
                }
            }

            Token::PseudoSelector(class) => {
                let validated_class = self.validate_instance_class(class, &part.token, "Pseudo");
                self.classes.insert(validated_class.to_string());
                self.consume_past_comma();
            }

            Token::StateSelectorOrEnumPart(Some(state)) => {
                self.classes.insert("Instance".to_string());
                self.validate_state(state, &part.token);
                self.consume_past_comma();
            }

            _ => (),
        }
    }

    fn from_parent(&mut self, part: &'a Node<'a>, after_combinator: bool) {
        match part.token.value() {
            Token::Identifier(class) => {
                if !after_combinator {
                    self.ast_errors.report(
                        TypeError::InvalidSelector {
                            msg: Some("Class Selectors can't be nested inside another selector without a children (>) or descendants selector."),
                        },
                        self.range_from_span(part.token.span()),
                    );
                }

                let validated_class = self.validate_class(class, &part.token);
                self.classes.insert(validated_class.to_string());

                match self.consume_with_error(
                    TokenKind::Identifier,
                    token_kind_list![PseudoSelector, StateSelectorOrEnumPart],
                    Some(token_kind_list![TagSelectorOrEnumPart, NameSelector]),
                ) {
                    ConsumeResult::Some(part) => match part.token.value() {
                        Token::PseudoSelector(class) => {
                            self.classes.pop();
                            let validated_class =
                                self.validate_instance_class(class, &part.token, "Pseudo");
                            self.classes.insert(validated_class.to_string());
                            self.consume_past_comma();
                        }

                        Token::StateSelectorOrEnumPart(Some(state)) => {
                            self.validate_state(state, &part.token);
                            self.consume_past_comma();
                        }

                        _ => (),
                    },

                    ConsumeResult::Err(_) => {
                        let Some(part) = self.next() else { return };
                        self.begin_iteration(part);
                    }

                    ConsumeResult::None => (),
                }
            }

            Token::PseudoSelector(class) => {
                let validated_class = self.validate_instance_class(class, &part.token, "Pseudo");
                self.classes.insert(validated_class.to_string());
                self.consume_past_comma();
            }

            Token::StateSelectorOrEnumPart(Some(state)) => {
                self.classes.extend(self.parent_classes.iter().cloned());
                self.validate_state(state, &part.token);
                self.consume_past_comma();
            }

            Token::TagSelectorOrEnumPart(_) | Token::NameSelector(_) | Token::QuerySelector(_) => {
                self.classes.insert("Instance".to_string());
                self.consume_past_comma();
            }

            Token::ChildrenSelector | Token::DescendantsSelector => {
                let Some(next) = self.next() else { return };
                self.from_parent(next, true);
            }

            _ => (),
        }
    }

    fn consume_past_comma(&mut self) {
        let Some(part) = self.next() else { return };
        if matches!(part.token.value(), Token::Comma) {
            let Some(next) = self.next() else { return };
            self.begin_iteration(next);
        }
    }

    fn consume_with_error<const N: usize>(
        &mut self,
        origin_kind: TokenKind,
        allow_list: &TokenKindList<N>,
        error_exclude_list: Option<&TokenKindList<N>>,
    ) -> ConsumeResult<'a> {
        self.consume(allow_list, |checker, part| {
            checker.error(
                error_exclude_list,
                origin_kind,
                part.token.value().kind(),
                part.token.span(),
            )
        })
    }

    fn consume<const N: usize, F: FnMut(&mut TypecheckSelectors<'a>, &'a Node<'a>) -> ()>(
        &mut self,
        allow_list: &TokenKindList<N>,
        mut error_callback: F,
    ) -> ConsumeResult<'a> {
        while let Some(part) = self.next() {
            let token = part.token.value();
            let token_discriminant = token.discriminant();

            if allow_list.has_discriminant(&token_discriminant) {
                return ConsumeResult::Some(part);
            } else if matches!(
                token,
                Token::Comma | Token::ChildrenSelector | Token::DescendantsSelector
            ) {
                return ConsumeResult::Err(part);
            } else {
                error_callback(self, part)
            }
        }

        ConsumeResult::None
    }

    fn error<const N: usize>(
        &mut self,
        error_exclude_list: Option<&TokenKindList<N>>,
        origin_kind: TokenKind,
        subject_kind: TokenKind,
        subject_span: (usize, usize),
    ) {
        if let Some(error_exclude_list) = error_exclude_list
            && error_exclude_list.has_discriminant(&discriminant(&subject_kind))
        {
            return;
        }

        let origin_name = self.selector_name(origin_kind);
        let subject_name = self.selector_name(subject_kind);
        let msg = if origin_kind == subject_kind {
            format!(
                "{origin_name} Selectors can't be defined after another {origin_name} Selector without a children (>) or descendants selector."
            )
        } else {
            format!(
                "{origin_name} Selectors can't be defined after a {subject_name} Selector without a children (>) or descendants selector."
            )
        };

        self.ast_errors.report(
            TypeError::InvalidSelector { msg: Some(&msg) },
            self.range_from_span(subject_span),
        );
    }

    fn selector_name(&self, kind: TokenKind) -> &'static str {
        match kind {
            TokenKind::Identifier => "Class",
            TokenKind::TagSelectorOrEnumPart => "Tag",
            TokenKind::StateSelectorOrEnumPart => "State",
            TokenKind::NameSelector => "Name",
            TokenKind::QuerySelector => "Query",
            TokenKind::ChildrenSelector => "Children",
            TokenKind::DescendantsSelector => "Descendants",
            _ => "Unknown",
        }
    }

    /// Returns the class if it is valid. Falls back to `"Instance"` otherwise.
    fn validate_class<'b>(&mut self, class: &'a str, token: &SpannedToken) -> &'a str {
        if let Ok(db) = rbx_reflection_database::get()
            && db.classes.contains_key(class)
        {
            return class;
        }

        self.ast_errors.report(
            TypeError::InvalidSelector {
                msg: Some(&format!("No class named \"{}\" exists.", class)),
            },
            self.range_from_span(token.span()),
        );

        "Instance"
    }

    fn validate_instance_class(
        &mut self,
        class: &'a str,
        token: &SpannedToken,
        selector_kind: &str,
    ) -> &'a str {
        let validated = self.validate_class(class, token);

        if validated != "Instance" && !ALLOWED_PSEUDO_SELECTORS.contains(class) {
            self.ast_errors.report(
                TypeError::InvalidSelector {
                    msg: Some(&format!(
                        "Class \"{}\" can't be used as a {} instance.",
                        class, selector_kind,
                    )),
                },
                self.range_from_span(token.span()),
            );
        }

        validated
    }

    fn validate_state(&mut self, name: &'a str, token: &SpannedToken) -> bool {
        if ALLOWED_STATE_SELECTORS.contains(name) {
            return true;
        }

        self.ast_errors.report(
            TypeError::InvalidSelector {
                msg: Some(&format!("No state named \"{}\" exists.", name)),
            },
            self.range_from_span(token.span()),
        );

        false
    }

    fn validate_selector_macro_call(
        &mut self,
        name: &Node<'a>,
        body: &Option<Delimited<'a>>,
    ) {
        use crate::typechecker::macro_check::count_macro_call_args;

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
            self.ast_errors.report(
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

            self.ast_errors.report(
                TypeError::WrongMacroArgCount {
                    name: macro_name,
                    expected: expected_counts,
                    got: call_arg_count,
                },
                self.range_from_span(name.token.span()),
            );
            return;
        };

        if matching_context != MacroReturnContext::Selector {
            self.ast_errors.report(
                TypeError::WrongMacroContext {
                    name: macro_name,
                    expected: matching_context.name(),
                    got: MacroReturnContext::Selector.name(),
                },
                self.range_from_span(name.token.span()),
            );
        }
    }

    fn range_from_span(&self, span: (usize, usize)) -> Range {
        Range::from_span(&self.rope, span)
    }
}

enum ConsumeResult<'a> {
    Some(&'a Node<'a>),
    None,
    Err(&'a Node<'a>),
}
