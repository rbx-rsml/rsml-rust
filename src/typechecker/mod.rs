use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut, RangeInclusive},
    path::{Path, PathBuf},
};

use crate::{
    lexer::Token,
    parser::{AstErrors, Construct, ParsedRsml},
    range_from_span::RangeFromSpan,
    types::{Diagnostic, Range},
};

use self::luaurc::Luaurc;
use macro_check::{
    MacroRegistry, MacroReturnContext, MacroSignature, count_macro_def_args, macro_return_context,
};

use rangemap::RangeInclusiveMap;

mod derive;
pub mod luaurc;
mod macro_check;
pub(crate) mod multibimap;
pub(crate) mod normalize_path;
mod selectors;
mod tween;
mod type_error;

pub use type_error::*;

pub trait PushTypeError {
    fn push(&mut self, error: TypeError, range: Range);
}

impl PushTypeError for AstErrors {
    fn push(&mut self, error: TypeError, range: Range) {
        self.0.push(Diagnostic {
            range,
            severity: error.severity(),
            code: error.to_string(),
            message: error.message(),
            data: error.data(),
        });
    }
}

pub struct Definitions(RangeInclusiveMap<usize, DefinitionKind>);

impl Definitions {
    pub fn new() -> Self {
        Self(RangeInclusiveMap::new())
    }
}

impl Deref for Definitions {
    type Target = RangeInclusiveMap<usize, DefinitionKind>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Definitions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum DefinitionKind {
    Derive {
        path: PathBuf,
    },
    Selector {
        type_definition: Vec<String>,
        hint: String,
    },
    Scope {
        type_definition: Vec<String>,
    },
    Assignment {
        property_name: String,
        type_definition: Vec<String>,
    },
    EnumName,
    EnumVariant {
        enum_name: String,
    },
    Declaration,
    FilteredEnumName {
        enum_name: String,
    },
}

impl DefinitionKind {
    fn selector_hint(classes: &Vec<String>) -> String {
        classes.join(" | ")
    }

    pub fn selector(type_definition: Vec<String>) -> Self {
        let hint = Self::selector_hint(&type_definition);
        Self::Selector {
            type_definition,
            hint,
        }
    }
}

pub struct TypecheckedRsml {
    pub errors: AstErrors,
    pub derives: HashMap<PathBuf, RangeInclusive<usize>>,
    pub dependencies: HashSet<PathBuf>,
    pub definitions: Definitions,
}

pub struct Typechecker<'a> {
    pub parsed: &'a ParsedRsml<'a>,
    macro_registry: MacroRegistry<'a>,
}

impl<'a> Typechecker<'a> {
    pub async fn new(
        parsed: &'a ParsedRsml<'a>,
        current_path: &Path,
        mut luaurc: Option<&mut Luaurc>,
    ) -> TypecheckedRsml {
        let mut typechecker: Typechecker<'a> = Self {
            parsed,
            macro_registry: HashMap::new(),
        };

        // We need to use a different ast errors
        // vec due to borrow checker issues.
        let mut ast_errors = AstErrors::new();

        let mut derives: HashMap<PathBuf, RangeInclusive<usize>> = HashMap::new();
        let mut definitions = Definitions::new();
        let mut dependencies = HashSet::new();

        for construct in &typechecker.parsed.ast {
            match construct {
                Construct::Derive {
                    body: Some(derive_body),
                    ..
                } => {
                    typechecker
                        .typecheck_derive(
                            derive_body,
                            &mut ast_errors,
                            current_path,
                            luaurc.as_deref_mut(),
                            &mut dependencies,
                            &mut derives,
                        )
                        .await;
                }

                Construct::Tween {
                    body: Some(body), ..
                } => {
                    ast_errors.push(
                        TypeError::NotAllowedInContext {
                            name: construct.name_plural(),
                            context: "the global scope",
                        },
                        Range::from_span(&typechecker.parsed.rope, construct.span()),
                    );
                    typechecker.typecheck_tween(body, &mut ast_errors);
                }

                Construct::Rule { selectors, body } => {
                    typechecker.typecheck_rule(
                        (selectors, body),
                        &vec![],
                        &mut ast_errors,
                        &mut definitions,
                    );
                }

                Construct::Macro {
                    name,
                    args,
                    return_type,
                    body,
                    ..
                } => {
                    if let Some(name_node) = name {
                        if let Token::Identifier(name_str) = name_node.token.value() {
                            let arg_count = count_macro_def_args(args);
                            let context = macro_return_context(return_type);
                            let signatures = typechecker
                                .macro_registry
                                .entry(name_str)
                                .or_insert_with(Vec::new);

                            if signatures.iter().any(|sig| sig.arg_count == arg_count) {
                                ast_errors.push(
                                    TypeError::DuplicateMacro {
                                        name: name_str,
                                        arg_count,
                                    },
                                    Range::from_span(&typechecker.parsed.rope, construct.span()),
                                );
                            } else {
                                signatures.push(MacroSignature {
                                    arg_count,
                                    return_context: context,
                                });
                            }
                        }
                    }
                    typechecker.typecheck_macro(args, body, &mut ast_errors);
                }

                Construct::MacroCall { name, body, .. } => {
                    typechecker.validate_macro_call(
                        name,
                        body,
                        MacroReturnContext::Construct,
                        &mut ast_errors,
                    );
                }

                Construct::Assignment {
                    left,
                    right: Some(right),
                    ..
                } => {
                    if matches!(left.token.value(), Token::Identifier(_)) {
                        ast_errors.push(
                            TypeError::NotAllowedInContext {
                                name: construct.name_plural(),
                                context: "the global scope",
                            },
                            Range::from_span(&typechecker.parsed.rope, construct.span()),
                        );
                    }
                    typechecker.validate_macro_arg_refs(right, None, &mut ast_errors);
                    if let Construct::MacroCall { name, body, .. } = right.as_ref() {
                        typechecker.validate_macro_call(
                            name,
                            body,
                            MacroReturnContext::Assignment,
                            &mut ast_errors,
                        );
                    }
                }

                Construct::Priority { .. } => {
                    ast_errors.push(
                        TypeError::NotAllowedInContext {
                            name: construct.name_plural(),
                            context: "the global scope",
                        },
                        Range::from_span(&typechecker.parsed.rope, construct.span()),
                    );
                }

                _ => (),
            }
        }

        TypecheckedRsml {
            errors: ast_errors,
            derives,
            dependencies,
            definitions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::Lexer, parser::Parser};

    use std::path::PathBuf;

    struct TypecheckResult {
        selectors: Vec<(usize, usize, Vec<String>)>,
        scopes: Vec<(usize, usize, Vec<String>)>,
        errors: Vec<String>,
    }

    async fn typecheck(source: &str) -> TypecheckResult {
        let lexer = Lexer::new(source);
        let parsed = Parser::new(lexer);
        let dummy_path = PathBuf::from("/test.rsml");

        let TypecheckedRsml {
            errors: ast_errors,
            derives: _derives,
            definitions,
            dependencies: _dependencies,
        } = Typechecker::new(&parsed, &dummy_path, None).await;

        let selectors: Vec<(usize, usize, Vec<String>)> = definitions
            .iter()
            .filter_map(|(range, kind)| {
                if let DefinitionKind::Selector {
                    type_definition, ..
                } = kind
                {
                    Some((*range.start(), *range.end(), type_definition.clone()))
                } else {
                    None
                }
            })
            .collect();

        let scopes: Vec<(usize, usize, Vec<String>)> = definitions
            .iter()
            .filter_map(|(range, kind)| {
                if let DefinitionKind::Scope {
                    type_definition, ..
                } = kind
                {
                    Some((*range.start(), *range.end(), type_definition.clone()))
                } else {
                    None
                }
            })
            .collect();

        let errors: Vec<String> = ast_errors
            .0
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect();

        TypecheckResult {
            selectors,
            scopes,
            errors,
        }
    }

    // ── Top-level selectors ────────────────────────────────────────

    #[tokio::test]
    async fn simple_class_selector() {
        let result = typecheck("Frame {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn class_with_pseudo_selector() {
        let result = typecheck("Frame ::UIPadding {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["UIPadding"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn class_with_state_selector() {
        let result = typecheck("Frame :hover {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn comma_separated_selectors() {
        let result = typecheck("Frame, TextButton {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame", "TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn invalid_class_name() {
        let result = typecheck("NotARealClass {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("No class named \"NotARealClass\" exists"));
    }

    #[tokio::test]
    async fn invalid_pseudo_not_a_class() {
        let result = typecheck("Frame ::NotAClass {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No class named \"NotAClass\" exists"))
        );
    }

    #[tokio::test]
    async fn invalid_pseudo_not_allowed() {
        let result = typecheck("Frame ::Frame {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("can't be used as a Pseudo instance"))
        );
    }

    #[tokio::test]
    async fn invalid_state_selector() {
        let result = typecheck("Frame :notastate {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No state named \"notastate\" exists"))
        );
    }

    #[tokio::test]
    async fn nested_class_without_combinator_errors() {
        let result = typecheck("Frame { TextButton {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["TextButton"]);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("can't be nested"));
    }

    #[tokio::test]
    async fn nested_child_selector() {
        let result = typecheck("Frame { > TextButton {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_pseudo_selector() {
        let result = typecheck("Frame { ::UIPadding {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["UIPadding"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_state_selector() {
        let result = typecheck("Frame { :hover {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn multiple_nesting_levels() {
        let result = typecheck("Frame { TextButton { TextLabel {} } }").await;
        assert_eq!(result.selectors.len(), 3);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["TextButton"]);
        assert_eq!(result.selectors[2].2, vec!["TextLabel"]);
        assert_eq!(result.errors.len(), 2);
        assert!(
            result
                .errors
                .iter()
                .all(|err| err.contains("can't be nested"))
        );
    }

    #[tokio::test]
    async fn nested_child_combinator_with_nesting() {
        let result = typecheck("Frame { > TextButton { > TextLabel {} } }").await;
        assert_eq!(result.selectors.len(), 3);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["TextButton"]);
        assert_eq!(result.selectors[2].2, vec!["TextLabel"]);
        assert!(result.errors.is_empty());
    }

    // ── Top-level combinator class resolution ──────────────────────

    #[tokio::test]
    async fn top_level_child_selector_resolves_to_child() {
        let result = typecheck("Frame > TextButton {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn top_level_child_with_pseudo_resolves_to_pseudo() {
        let result = typecheck("Frame > TextButton ::UIPadding {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["UIPadding"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn top_level_child_with_state_resolves_to_child() {
        let result = typecheck("Frame > TextButton :hover {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["TextButton"]);
        assert!(result.errors.is_empty());
    }

    // ── Name selector coerces to Instance ──────────────────────────

    #[tokio::test]
    async fn top_level_chain_with_name_selector_coerces_to_instance() {
        let result = typecheck("Frame > TextButton > .Hello {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn top_level_child_with_name_selector_coerces_to_instance() {
        let result = typecheck("Frame > .Hello {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_child_with_name_selector_coerces_to_instance() {
        let result = typecheck("Frame { > .Hello {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["Instance"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn chain_with_tag_then_comma() {
        let result = typecheck("Frame >> TextButton > .Hello, Frame {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance", "Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn tag_selector_then_comma_at_top_level() {
        let result = typecheck(".Hello, TextButton {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance", "TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_tag_then_comma() {
        let result = typecheck("Frame { > .Hello, > TextButton {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["Instance", "TextButton"]);
        assert!(result.errors.is_empty());
    }

    // ── Deduplication ─────────────────────────────────────────────

    #[tokio::test]
    async fn duplicate_comma_selectors_are_deduplicated() {
        let result = typecheck("Frame, Frame, TextButton {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame", "TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn all_duplicate_selectors() {
        let result = typecheck("Frame, Frame, Frame {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn duplicate_with_combinator() {
        let result = typecheck("Frame > TextButton, Frame > TextButton {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn duplicate_instance_coercion() {
        let result = typecheck(".Hello, .World {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn duplicate_with_state_selectors() {
        let result = typecheck("Frame :hover, Frame :press {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn duplicate_pseudo_selectors() {
        let result = typecheck("Frame ::UIPadding, TextButton ::UIPadding {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["UIPadding"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_duplicate_selectors() {
        let result = typecheck("Frame { > TextButton, > TextButton {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn no_dedup_different_types() {
        let result = typecheck("Frame, TextButton {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame", "TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn preserves_order_after_dedup() {
        let result = typecheck("TextButton, Frame, TextButton {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["TextButton", "Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn scope_inserted_for_rule_body() {
        let result = typecheck("Frame {}").await;
        assert_eq!(result.scopes.len(), 1);
        assert_eq!(result.scopes[0].2, vec!["Frame"]);
    }

    #[tokio::test]
    async fn scope_has_union_types() {
        let result = typecheck("Frame, TextButton {}").await;
        assert_eq!(result.scopes.len(), 1);
        assert_eq!(result.scopes[0].2, vec!["Frame", "TextButton"]);
    }

    #[tokio::test]
    async fn nested_scopes_have_correct_types() {
        let result = typecheck("Frame { > TextButton {} }").await;
        // Outer scope gets split by inner scope insertion, so 3 entries:
        // two halves of the outer Frame scope + the inner TextButton scope
        assert!(result.scopes.len() >= 2);
        let scope_types: Vec<&Vec<String>> = result.scopes.iter().map(|s| &s.2).collect();
        assert!(scope_types.contains(&&vec!["Frame".to_string()]));
        assert!(scope_types.contains(&&vec!["TextButton".to_string()]));
    }

    #[tokio::test]
    async fn scope_with_combinator() {
        let result = typecheck("Frame > TextButton {}").await;
        assert_eq!(result.scopes.len(), 1);
        assert_eq!(result.scopes[0].2, vec!["TextButton"]);
    }

    #[tokio::test]
    async fn scope_with_pseudo_selector() {
        let result = typecheck("Frame ::UIPadding {}").await;
        assert_eq!(result.scopes.len(), 1);
        assert_eq!(result.scopes[0].2, vec!["UIPadding"]);
    }

    // ── Top-level state selectors ────────────────────────────────

    #[tokio::test]
    async fn top_level_state_selector_resolves_to_instance() {
        let result = typecheck(":hover {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn top_level_state_selector_invalid_state() {
        let result = typecheck(":notastate {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No state named \"notastate\" exists"))
        );
    }

    #[tokio::test]
    async fn nested_state_selector_inherits_parent_class() {
        let result = typecheck("Frame { :hover {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["Frame"]);
        assert!(result.errors.is_empty());
    }

    // ── Standalone pseudo selectors (::ClassName) ───────────────

    #[tokio::test]
    async fn top_level_pseudo_selector_resolves_instance_type() {
        let result = typecheck("::UIPadding {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["UIPadding"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn top_level_pseudo_selector_scope_resolves() {
        let result = typecheck("::UIPadding {}").await;
        assert_eq!(result.scopes.len(), 1);
        assert_eq!(result.scopes[0].2, vec!["UIPadding"]);
    }

    #[tokio::test]
    async fn top_level_pseudo_selector_invalid_class() {
        let result = typecheck("::NotARealClass {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Instance"]);
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No class named \"NotARealClass\" exists"))
        );
    }

    #[tokio::test]
    async fn top_level_pseudo_selector_not_allowed_class() {
        let result = typecheck("::Frame {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("can't be used as a Pseudo instance"))
        );
    }

    #[tokio::test]
    async fn top_level_pseudo_selectors_with_comma() {
        let result = typecheck("::UIPadding, ::UICorner {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["UIPadding", "UICorner"]);
        assert!(result.errors.is_empty());
    }

    // ── Comma after state/pseudo selectors ────────────────────────

    #[tokio::test]
    async fn comma_after_state_selector_continues() {
        let result = typecheck("Frame :hover, TextButton :hover {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["Frame", "TextButton"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn comma_after_pseudo_selector_continues() {
        let result = typecheck("Frame ::UIPadding, TextButton ::UICorner {}").await;
        assert_eq!(result.selectors.len(), 1);
        assert_eq!(result.selectors[0].2, vec!["UIPadding", "UICorner"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_comma_after_state_selector_continues() {
        let result = typecheck("Frame { > TextButton :hover, > TextLabel :press {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[1].2, vec!["TextButton", "TextLabel"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_comma_after_pseudo_selector_continues() {
        let result = typecheck("Frame { ::UIPadding, ::UICorner {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[1].2, vec!["UIPadding", "UICorner"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_standalone_pseudo_selector_resolves() {
        let result = typecheck("Frame { ::UIPadding {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[0].2, vec!["Frame"]);
        assert_eq!(result.selectors[1].2, vec!["UIPadding"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_comma_after_state_selector_inherits_parent() {
        let result = typecheck("Frame { :hover, :press {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[1].2, vec!["Frame"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn nested_comma_pseudo_with_class_prefix() {
        let result =
            typecheck("Frame { > TextButton ::UIPadding, > TextLabel ::UICorner {} }").await;
        assert_eq!(result.selectors.len(), 2);
        assert_eq!(result.selectors[1].2, vec!["UIPadding", "UICorner"]);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn macro_arg_nonexistent_errors() {
        let result =
            typecheck("@macro Padding (&x) { ::UIPadding { PaddingTop = &nonexistent; } }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No macro argument named"))
        );
    }

    #[tokio::test]
    async fn macro_arg_valid_no_error() {
        let result =
            typecheck("@macro Padding (&all) { ::UIPadding { PaddingTop = &all; } }").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected macro errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_arg_outside_macro_errors() {
        let result = typecheck("Frame { PaddingTop = &all; }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No macro argument named \"all\" exists."))
        );
    }

    // ── Macro call typechecking ───────────────────────────────────

    #[tokio::test]
    async fn macro_call_after_definition_no_error() {
        let result = typecheck("@macro Padding () { ::UIPadding {} }\nPadding!();").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Undefined Macro") || err.contains("Wrong Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected macro errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_before_definition_errors() {
        let result = typecheck("Padding!();\n@macro Padding () { ::UIPadding {} }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No macro named `Padding` has been defined"))
        );
    }

    #[tokio::test]
    async fn macro_call_undefined_errors() {
        let result = typecheck("DoesNotExist!();").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No macro named `DoesNotExist` has been defined"))
        );
    }

    #[tokio::test]
    async fn macro_call_wrong_arg_count_errors() {
        let result = typecheck("@macro Padding (&all) { ::UIPadding {} }\nPadding!();").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Macro Argument Count"))
        );
    }

    #[tokio::test]
    async fn macro_call_correct_arg_count_no_error() {
        let result = typecheck("@macro Padding (&all) { ::UIPadding {} }\nPadding!(10);").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Argument Count"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_overloaded_correct_arg_count() {
        let result = typecheck(
            "@macro Padding (&all) { ::UIPadding {} }\n@macro Padding (&x, &y) { ::UIPadding {} }\nPadding!(1, 2);"
        ).await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Argument Count"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_overloaded_wrong_arg_count() {
        let result = typecheck(
            "@macro Padding (&all) { ::UIPadding {} }\n@macro Padding (&x, &y) { ::UIPadding {} }\nPadding!(1, 2, 3);"
        ).await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Macro Argument Count"))
        );
    }

    #[tokio::test]
    async fn macro_call_construct_in_assignment_context_errors() {
        let result = typecheck("@macro Foo () { Frame {} }\nFrame { Size = Foo!(); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Macro Context"))
        );
    }

    #[tokio::test]
    async fn macro_call_assignment_in_construct_context_errors() {
        let result = typecheck("@macro Foo () -> Assignment { 10 }\nFoo!();").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Macro Context"))
        );
    }

    #[tokio::test]
    async fn macro_call_assignment_in_assignment_context_no_error() {
        let result =
            typecheck("@macro Foo () -> Assignment { 10 }\nFrame { Size = Foo!(); }").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Context"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_selector_in_selector_context_no_error() {
        let result = typecheck("@macro Sel () -> Selector { Frame }\nSel!() {}").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Context") || err.contains("Undefined Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_selector_in_selector_context_with_comma_no_error() {
        let result = typecheck("@macro Sel () -> Selector { Frame }\nFrame, Sel!() {}").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Context") || err.contains("Undefined Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_construct_in_selector_context_errors() {
        let result = typecheck("@macro Foo () { Frame {} }\nFoo!() {}").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Macro Context"))
        );
    }

    #[tokio::test]
    async fn macro_call_in_rule_body() {
        let result = typecheck("@macro Padding () { ::UIPadding {} }\nFrame { Padding!(); }").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Undefined Macro") || err.contains("Wrong Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_no_return_type_defaults_to_construct() {
        let result = typecheck("@macro Foo () { Frame {} }\nFoo!();").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Context") || err.contains("Undefined Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_inside_macro_body() {
        let result =
            typecheck("@macro Inner () { ::UIPadding {} }\n@macro Outer () { Inner!(); }").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Undefined Macro") || err.contains("Wrong Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_inside_macro_body_undefined_errors() {
        let result = typecheck("@macro Outer () { NotDefined!(); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("No macro named `NotDefined` has been defined"))
        );
    }

    #[tokio::test]
    async fn macro_duplicate_same_name_same_args_errors() {
        let result =
            typecheck("@macro Test () { Frame {} }\n@macro Test () -> Selector { Frame }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Duplicate Macro"))
        );
    }

    #[tokio::test]
    async fn macro_duplicate_same_name_different_args_no_error() {
        let result =
            typecheck("@macro Test (&a) { Frame {} }\n@macro Test (&a, &b) { Frame {} }").await;
        let duplicate_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Duplicate Macro"))
            .collect();
        assert!(
            duplicate_errors.is_empty(),
            "unexpected errors: {:?}",
            duplicate_errors
        );
    }

    #[tokio::test]
    async fn macro_call_selector_no_args_no_error() {
        let result = typecheck("@macro Foo -> Selector { }\nFoo!() {}").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Context") || err.contains("Undefined Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }

    #[tokio::test]
    async fn macro_call_selector_no_args_with_comma_no_error() {
        let result = typecheck("@macro Foo -> Selector { }\nFoo!(), Frame {}").await;
        let macro_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Wrong Macro Context") || err.contains("Undefined Macro"))
            .collect();
        assert!(
            macro_errors.is_empty(),
            "unexpected errors: {:?}",
            macro_errors
        );
    }
}
