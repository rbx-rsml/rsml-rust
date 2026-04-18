use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut, RangeInclusive},
    path::{Path, PathBuf},
};

use crate::{
    datatype::{Datatype, StaticLookup, evaluate_construct, shorthand_rebind},
    lexer::Token,
    parser::{AstErrors, Construct, Delimited, Node, ParsedRsml},
    range_from_span::RangeFromSpan,
    types::{Diagnostic, Range},
};


use self::luaurc::Luaurc;
use macro_check::{
    MacroRegistry, MacroReturnContext, MacroSignature, count_macro_def_args, macro_return_context,
};

use rangemap::RangeInclusiveMap;

mod annotations;
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
    Token {
        name: String,
        is_static: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResolvedTypeKey {
    Token { name: String, is_static: bool },
    Property { start: usize },
}

pub type ResolvedTypes = HashMap<ResolvedTypeKey, Datatype>;

#[derive(Clone, Copy)]
enum LhsKind<'a> {
    Token { name: &'a str, is_static: bool },
    Property { name: &'a str },
}

impl<'a> LhsKind<'a> {
    fn name(&self) -> &'a str {
        match *self {
            LhsKind::Token { name, .. } | LhsKind::Property { name } => name,
        }
    }
}

/// Tokens like `StateSelectorOrEnumPart` and `TagSelectorOrEnumPart` span the
/// leading `:` or `.` sigil alongside the identifier. Diagnostics that point
/// at just the name should skip that single byte prefix.
fn strip_sigil_span(span: (usize, usize)) -> (usize, usize) {
    let (start, end) = span;
    (start.saturating_add(1).min(end), end)
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
    pub resolved_types: ResolvedTypes,
}

pub struct Typechecker<'a> {
    pub parsed: &'a ParsedRsml<'a>,
    macro_registry: MacroRegistry<'a>,
    pub(crate) static_scopes: Vec<HashMap<String, Datatype>>,
    pub(crate) declared_tokens: Vec<HashSet<ResolvedTypeKey>>,
}

pub(crate) struct TypecheckerLookup<'a> {
    pub scopes: &'a [HashMap<String, Datatype>],
}

impl<'a> StaticLookup for TypecheckerLookup<'a> {
    fn resolve_static(&self, name: &str) -> Datatype {
        for scope in self.scopes.iter().rev() {
            if let Some(dt) = scope.get(name) {
                return dt.clone();
            }
        }
        Datatype::None
    }

    fn resolve_dynamic(&self, _name: &str) -> Datatype {
        Datatype::None
    }
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
            static_scopes: vec![HashMap::new()],
            declared_tokens: vec![HashSet::new()],
        };

        // We need to use a different ast errors
        // vec due to borrow checker issues.
        let mut ast_errors = AstErrors::new();

        let mut derives: HashMap<PathBuf, RangeInclusive<usize>> = HashMap::new();
        let mut definitions = Definitions::new();
        let mut resolved_types: ResolvedTypes = HashMap::new();
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
                        &mut resolved_types,
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
                    typechecker.validate_token_refs(right, &mut ast_errors);
                    typechecker.validate_macro_arg_refs(right, None, &mut ast_errors);
                    typechecker.validate_annotation(right, &mut ast_errors);
                    if let Construct::MacroCall { name, body, .. } = right.as_ref() {
                        typechecker.validate_macro_call(
                            name,
                            body,
                            MacroReturnContext::Assignment,
                            &mut ast_errors,
                        );
                    }
                    typechecker.resolve_token_assignment(left, right, &mut ast_errors, &mut definitions, &mut resolved_types);
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
            resolved_types,
        }
    }

    pub(crate) fn resolve_token_assignment(
        &mut self,
        left: &Node<'a>,
        right: &Construct<'a>,
        ast_errors: &mut AstErrors,
        definitions: &mut Definitions,
        resolved_types: &mut ResolvedTypes,
    ) {
        let lhs_kind = match left.token.value() {
            Token::TokenIdentifier(name) => LhsKind::Token { name: *name, is_static: false },
            Token::StaticTokenIdentifier(name) => LhsKind::Token { name: *name, is_static: true },
            Token::Identifier(name) => LhsKind::Property { name: *name },
            _ => return,
        };

        let name = lhs_kind.name();

        // Validate any enum references on the RHS. If invalid, the LHS type
        // collapses to `unknown`.
        let enum_valid = self.validate_enum_refs(left, right, ast_errors);

        let resolved_type = if !enum_valid {
            Datatype::None
        } else {
            let lookup = TypecheckerLookup { scopes: &self.static_scopes };

            let evaluated = match lhs_kind {
                LhsKind::Token { .. } => {
                    if let Construct::Node { node } = right {
                        if let Token::StateSelectorOrEnumPart(Some(value)) = node.token.value() {
                            Some(Datatype::IncompleteEnumShorthand(value.to_string()))
                        } else {
                            evaluate_construct(right, Some(name), &lookup)
                        }
                    } else {
                        evaluate_construct(right, Some(name), &lookup)
                    }
                }
                LhsKind::Property { .. } => evaluate_construct(right, Some(name), &lookup),
            };

            match lhs_kind {
                LhsKind::Token { is_static, .. } => match evaluated {
                    Some(Datatype::IncompleteEnumShorthand(variant)) => {
                        Datatype::IncompleteEnumShorthand(variant)
                    }
                    Some(d) if is_static => d,
                    Some(d) => d
                        .coerce_to_variant(Some(name))
                        .map(Datatype::Variant)
                        .unwrap_or(Datatype::None),
                    None => Datatype::None,
                },
                LhsKind::Property { .. } => match evaluated {
                    Some(d) => d
                        .coerce_to_variant(Some(name))
                        .map(Datatype::Variant)
                        .unwrap_or(Datatype::None),
                    None => Datatype::None,
                },
            }
        };

        let (start, end) = left.token.span();

        match lhs_kind {
            LhsKind::Token { is_static, .. } => {
                if is_static {
                    if let Some(frame) = self.static_scopes.last_mut() {
                        frame.insert(name.to_string(), resolved_type.clone());
                    }
                }

                let key = ResolvedTypeKey::Token { name: name.to_string(), is_static };
                resolved_types.insert(key.clone(), resolved_type);

                if let Some(frame) = self.declared_tokens.last_mut() {
                    frame.insert(key);
                }

                definitions.insert(
                    start..=end,
                    DefinitionKind::Token { name: name.to_string(), is_static },
                );
            }
            LhsKind::Property { .. } => {
                let type_definition = vec![resolved_type.type_name()];
                resolved_types.insert(
                    ResolvedTypeKey::Property { start },
                    resolved_type,
                );
                definitions.insert(
                    start..=end,
                    DefinitionKind::Assignment {
                        property_name: name.to_string(),
                        type_definition,
                    },
                );
            }
        }
    }

    /// Validates every enum reference on the RHS of an assignment against the
    /// reflection DB. The `left` node supplies the implicit enum name used by
    /// a top-level shorthand form (`:Variant`) — its name is rebinded via
    /// [`shorthand_rebind`] to match runtime evaluator behavior. Returns
    /// `false` when any error was pushed.
    pub(crate) fn validate_enum_refs(
        &self,
        left: &Node<'a>,
        right: &Construct<'a>,
        ast_errors: &mut AstErrors,
    ) -> bool {
        let mut ok = true;

        // Top-level shorthand `:Variant` — derive enum name from the LHS.
        if let Construct::Node { node } = right {
            if let Token::StateSelectorOrEnumPart(Some(variant)) = node.token.value() {
                let lhs_name = match left.token.value() {
                    Token::Identifier(n)
                    | Token::TokenIdentifier(n)
                    | Token::StaticTokenIdentifier(n) => Some(*n),
                    _ => None,
                };

                if let Some(lhs_name) = lhs_name {
                    let enum_name = shorthand_rebind(lhs_name);
                    let variant_span = strip_sigil_span(node.token.span());
                    ok &= self.check_enum_name_and_variant(
                        enum_name,
                        variant,
                        variant_span,
                        variant_span,
                        ast_errors,
                    );
                }

                return ok;
            }
        }

        ok &= self.validate_enum_refs_inner(right, ast_errors);
        ok
    }

    fn validate_enum_refs_inner(
        &self,
        construct: &Construct<'a>,
        ast_errors: &mut AstErrors,
    ) -> bool {
        let mut ok = true;
        match construct {
            Construct::Enum { name: Some(name_node), variant: Some(variant_node), .. } => {
                let enum_name = annotations::enum_identifier(name_node.token.value());
                let variant = annotations::enum_identifier(variant_node.token.value());

                if let Some(enum_name) = enum_name {
                    let name_span = strip_sigil_span(name_node.token.span());
                    let variant_span = strip_sigil_span(variant_node.token.span());
                    ok &= self.check_enum_name_and_variant(
                        enum_name,
                        variant.unwrap_or(""),
                        name_span,
                        variant_span,
                        ast_errors,
                    );
                }
            }
            Construct::MathOperation { left, right, .. } => {
                ok &= self.validate_enum_refs_inner(left, ast_errors);
                if let Some(right) = right {
                    ok &= self.validate_enum_refs_inner(right, ast_errors);
                }
            }
            Construct::UnaryMinus { operand, .. } => {
                ok &= self.validate_enum_refs_inner(operand, ast_errors);
            }
            Construct::Table { body } => {
                ok &= self.validate_enum_refs_delimited(body, ast_errors);
            }
            Construct::AnnotatedTable { body: Some(body), .. } => {
                ok &= self.validate_enum_refs_delimited(body, ast_errors);
            }
            Construct::MacroCall { body: Some(body), .. } => {
                ok &= self.validate_enum_refs_delimited(body, ast_errors);
            }
            _ => {}
        }
        ok
    }

    fn validate_enum_refs_delimited(
        &self,
        delim: &Delimited<'a>,
        ast_errors: &mut AstErrors,
    ) -> bool {
        let Some(content) = delim.content.as_ref() else {
            return true;
        };
        let mut ok = true;
        for item in content {
            ok &= self.validate_enum_refs_inner(item, ast_errors);
        }
        ok
    }

    fn check_enum_name_and_variant(
        &self,
        enum_name: &str,
        variant: &str,
        name_span: (usize, usize),
        variant_span: (usize, usize),
        ast_errors: &mut AstErrors,
    ) -> bool {
        if !annotations::enum_exists(enum_name) {
            ast_errors.push(
                TypeError::UnknownEnum { name: enum_name.to_string() },
                self.parsed.range_from_span(name_span),
            );
            return false;
        }

        if variant.is_empty() {
            return true;
        }

        if !annotations::validate_enum_variant(variant, enum_name) {
            ast_errors.push(
                TypeError::UnknownEnumVariant {
                    enum_name: enum_name.to_string(),
                    variant: variant.to_string(),
                },
                self.parsed.range_from_span(variant_span),
            );
            return false;
        }

        true
    }

    pub(crate) fn validate_token_refs(
        &self,
        construct: &Construct<'a>,
        ast_errors: &mut AstErrors,
    ) {
        match construct {
            Construct::Node { node } => {
                let (name, is_static) = match node.token.value() {
                    Token::TokenIdentifier(n) => (*n, false),
                    Token::StaticTokenIdentifier(n) => (*n, true),
                    _ => return,
                };
                let key = ResolvedTypeKey::Token {
                    name: name.to_string(),
                    is_static,
                };
                let in_scope = self
                    .declared_tokens
                    .iter()
                    .rev()
                    .any(|frame| frame.contains(&key));
                if !in_scope {
                    ast_errors.push(
                        TypeError::UndefinedToken { name, is_static },
                        self.parsed.range_from_span(node.token.span()),
                    );
                }
            }
            Construct::MathOperation { left, right, .. } => {
                self.validate_token_refs(left, ast_errors);
                if let Some(right) = right {
                    self.validate_token_refs(right, ast_errors);
                }
            }
            Construct::UnaryMinus { operand, .. } => {
                self.validate_token_refs(operand, ast_errors);
            }
            Construct::Table { body } => {
                self.validate_token_refs_delimited(body, ast_errors);
            }
            Construct::AnnotatedTable {
                body: Some(body), ..
            } => {
                self.validate_token_refs_delimited(body, ast_errors);
            }
            Construct::MacroCall {
                body: Some(body), ..
            } => {
                self.validate_token_refs_delimited(body, ast_errors);
            }
            _ => {}
        }
    }

    fn validate_token_refs_delimited(
        &self,
        delim: &Delimited<'a>,
        ast_errors: &mut AstErrors,
    ) {
        let Some(content) = delim.content.as_ref() else {
            return;
        };
        for item in content {
            self.validate_token_refs(item, ast_errors);
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
        tokens: Vec<(usize, usize, String, bool, Datatype)>,
        properties: Vec<(usize, usize, String, Datatype)>,
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
            resolved_types,
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

        let tokens: Vec<(usize, usize, String, bool, Datatype)> = definitions
            .iter()
            .filter_map(|(range, kind)| {
                if let DefinitionKind::Token { name, is_static } = kind {
                    let resolved_type = resolved_types
                        .get(&ResolvedTypeKey::Token {
                            name: name.clone(),
                            is_static: *is_static,
                        })
                        .cloned()
                        .unwrap_or(Datatype::None);
                    Some((
                        *range.start(),
                        *range.end(),
                        name.clone(),
                        *is_static,
                        resolved_type,
                    ))
                } else {
                    None
                }
            })
            .collect();

        let properties: Vec<(usize, usize, String, Datatype)> = definitions
            .iter()
            .filter_map(|(range, kind)| {
                if let DefinitionKind::Assignment { property_name, .. } = kind {
                    let resolved_type = resolved_types
                        .get(&ResolvedTypeKey::Property { start: *range.start() })
                        .cloned()
                        .unwrap_or(Datatype::None);
                    Some((
                        *range.start(),
                        *range.end(),
                        property_name.clone(),
                        resolved_type,
                    ))
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
            tokens,
            properties,
            errors,
        }
    }

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

    #[tokio::test]
    async fn annotation_unknown_name_errors() {
        let result = typecheck("Frame { Size = notareal(1, 2); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Unknown Annotation") && err.contains("notareal")),
            "expected unknown annotation error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_valid_udim2_no_error() {
        let result = typecheck("Frame { Size = udim2(1, 0, 1, 0); }").await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_valid_vec3_no_error() {
        let result = typecheck("Frame { Position = vec3(1, 2, 3); }").await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_too_many_args_errors() {
        let result = typecheck("Frame { Size = vec2(1, 2, 3); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Count")),
            "expected arg count error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_too_few_args_errors() {
        let result = typecheck("Frame { Size = lerp(); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Count")),
            "expected arg count error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_wrong_arg_type_errors() {
        let result = typecheck("Frame { Size = vec2(\"hello\", \"world\"); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Type")),
            "expected arg type error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_variadic_colorseq_many_args() {
        let result = typecheck("Frame { Color = colorseq(#ff0000, #00ff00, #0000ff); }").await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_variadic_colorseq_empty_errors() {
        let result = typecheck("Frame { Color = colorseq(); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Count")),
            "expected arg count error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_nested_annotation_validated() {
        let result = typecheck("Frame { Size = udim2(vec2(1, 2, 3), 0); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Count")),
            "expected nested vec2 arg count error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_case_insensitive_matching() {
        let result = typecheck("Frame { Size = UDim2(1, 0, 1, 0); }").await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_zero_args_errors() {
        let result = typecheck("Frame { Color = brickcolor(); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Count")),
            "expected arg count error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_color3_accepts_color_arg() {
        let result = typecheck("Frame { BackgroundColor3 = color3(#ff0000); }").await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_color3_three_numbers() {
        let result = typecheck("Frame { BackgroundColor3 = color3(1, 0, 0); }").await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_udim2_with_percent_scale() {
        let result = typecheck("Frame { Size = udim2(50%, 50%); }").await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_font_with_enum() {
        let result =
            typecheck("Frame { FontFace = font(\"rbxasset://fonts/arial.ttf\", Enum.FontWeight.Bold); }")
                .await;
        let annotation_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Annotation"))
            .collect();
        assert!(
            annotation_errors.is_empty(),
            "unexpected errors: {:?}",
            annotation_errors
        );
    }

    #[tokio::test]
    async fn annotation_at_top_level_is_validated() {
        let result = typecheck("$Size = vec2(1, 2, 3);").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Count")),
            "expected arg count error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_in_macro_body_is_validated() {
        let result =
            typecheck("@macro Foo () { Frame { Size = vec2(1, 2, 3); } }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Wrong Annotation Argument Count")),
            "expected arg count error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_token_arg_errors() {
        let result = typecheck("Frame { Size = udim2($Width, 0, 1, 0); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Tokens are not allowed in tuple annotations")),
            "expected token-in-annotation error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_token_nested_inside_math_errors() {
        let result = typecheck("Frame { Size = udim2($Width + 10, 0, 1, 0); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Tokens are not allowed in tuple annotations")),
            "expected token-in-annotation error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_static_token_arg_allowed() {
        let result = typecheck("Frame { Size = udim2($!Width, 0, 1, 0); }").await;
        let token_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Tokens are not allowed"))
            .collect();
        assert!(
            token_errors.is_empty(),
            "unexpected static-token error: {:?}",
            token_errors
        );
    }

    fn annotation_arg_type_errors(result: &TypecheckResult) -> Vec<&String> {
        result
            .errors
            .iter()
            .filter(|err| err.contains("must be"))
            .collect()
    }

    #[tokio::test]
    async fn annotation_static_token_measurement_valid() {
        let result = typecheck("$!W = 100; Frame { Size = udim2($!W, 0%); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
    }

    #[tokio::test]
    async fn annotation_static_token_scale_measurement_valid() {
        let result = typecheck("$!Hello = 50%; Frame { Hello = udim2(50%, $!Hello); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
    }

    #[tokio::test]
    async fn annotation_static_token_number_valid() {
        let result = typecheck("$!N = 10; Frame { Size = vec3($!N, $!N, $!N); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
    }

    #[tokio::test]
    async fn annotation_static_token_color_valid() {
        let result =
            typecheck("$!C = #ff0000; Frame { BackgroundColor3 = color3($!C); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
    }

    #[tokio::test]
    async fn annotation_static_token_oklab_color_valid() {
        let result =
            typecheck("$!C = tw:red:500; Frame { BackgroundColor3 = color3($!C); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
    }

    #[tokio::test]
    async fn annotation_static_token_wrong_type_errors() {
        let result = typecheck("$!S = \"hi\"; Frame { Size = udim2($!S, 0%); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(
            !errs.is_empty(),
            "expected a Wrong Annotation Argument Type error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_static_token_unresolved_permissive() {
        let result = typecheck("Frame { Size = udim2($!Unknown, 0%); }").await;
        let errs = annotation_arg_type_errors(&result);
        let token_errs: Vec<_> = result
            .errors
            .iter()
            .filter(|err| err.contains("Tokens are not allowed"))
            .collect();
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
        assert!(token_errs.is_empty(), "unexpected token errors: {:?}", token_errs);
    }

    #[tokio::test]
    async fn annotation_regular_token_still_errors() {
        let result = typecheck("$W = 10; Frame { Size = udim2($W, 0, 1, 0); }").await;
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Tokens are not allowed")),
            "expected token-in-annotation error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn annotation_static_token_in_math_permissive() {
        let result = typecheck("$!W = 10; Frame { Size = udim2($!W + 5, 0%); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
    }

    #[tokio::test]
    async fn annotation_static_token_enum_valid() {
        let result = typecheck(
            "$!B = Enum.FontWeight.Bold; Frame { FontFace = font(\"rbxasset://fonts/arial.ttf\", $!B); }",
        )
        .await;
        let errs = annotation_arg_type_errors(&result);
        assert!(errs.is_empty(), "unexpected arg-type errors: {:?}", errs);
    }

    #[tokio::test]
    async fn annotation_static_token_enum_wrong_type_errors() {
        let result =
            typecheck("$!B = Enum.FontWeight.Bold; Frame { Size = udim2($!B, 0%); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(
            !errs.is_empty(),
            "expected a Wrong Annotation Argument Type error, got: {:?}",
            result.errors
        );
    }

    fn find_token<'a>(
        result: &'a TypecheckResult,
        name: &str,
        is_static: bool,
    ) -> &'a Datatype {
        result
            .tokens
            .iter()
            .find(|(_, _, n, s, _)| n == name && *s == is_static)
            .map(|(_, _, _, _, dt)| dt)
            .unwrap_or_else(|| {
                panic!(
                    "no token `{}` (static={}) found; tokens={:?}",
                    name,
                    is_static,
                    result.tokens.iter().map(|(_, _, n, s, _)| (n, s)).collect::<Vec<_>>()
                )
            })
    }

    fn find_property<'a>(result: &'a TypecheckResult, name: &str) -> &'a Datatype {
        result
            .properties
            .iter()
            .find(|(_, _, n, _)| n == name)
            .map(|(_, _, _, dt)| dt)
            .unwrap_or_else(|| {
                panic!(
                    "no property `{}` found; properties={:?}",
                    name,
                    result.properties.iter().map(|(_, _, n, _)| n).collect::<Vec<_>>()
                )
            })
    }

    #[tokio::test]
    async fn token_number_type() {
        let result = typecheck("$X = 10;").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(dt, Datatype::Variant(rbx_types::Variant::Float32(n)) if *n == 10.0),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn token_color_hex_coerces_for_regular() {
        let result = typecheck("$X = #ff0000;").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(dt, Datatype::Variant(rbx_types::Variant::Color3(_))),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn token_color_tailwind_coerces_to_color3() {
        let result = typecheck("$X = tw:red:500;").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(dt, Datatype::Variant(rbx_types::Variant::Color3(_))),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn static_token_keeps_oklab() {
        let result = typecheck("$!X = tw:red:500;").await;
        let dt = find_token(&result, "X", true);
        assert!(matches!(dt, Datatype::Oklab(_)), "got {:?}", dt);
    }

    #[tokio::test]
    async fn token_udim2() {
        let result = typecheck("$X = udim2(1, 0, 1, 0);").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(dt, Datatype::Variant(rbx_types::Variant::UDim2(_))),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn token_string() {
        let result = typecheck("$X = \"hi\";").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(dt, Datatype::Variant(rbx_types::Variant::String(s)) if s == "hi"),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn static_token_cross_ref() {
        let result = typecheck("$!A = 10; $!B = $!A;").await;
        let a = find_token(&result, "A", true);
        let b = find_token(&result, "B", true);
        assert!(
            matches!(a, Datatype::Variant(rbx_types::Variant::Float32(n)) if *n == 10.0),
            "got A={:?}",
            a
        );
        assert!(
            matches!(b, Datatype::Variant(rbx_types::Variant::Float32(n)) if *n == 10.0),
            "got B={:?}",
            b
        );
    }

    #[tokio::test]
    async fn static_token_math() {
        let result = typecheck("$!A = 10; $!B = $!A + 5;").await;
        let b = find_token(&result, "B", true);
        assert!(
            matches!(b, Datatype::Variant(rbx_types::Variant::Float32(n)) if *n == 15.0),
            "got {:?}",
            b
        );
    }

    #[tokio::test]
    async fn token_inside_rule_body() {
        let result = typecheck("Frame { $X = 10; }").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(dt, Datatype::Variant(rbx_types::Variant::Float32(n)) if *n == 10.0),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn static_token_parent_scope_lookup() {
        let result = typecheck("$!A = 10; Frame { $!B = $!A; }").await;
        let b = find_token(&result, "B", true);
        assert!(
            matches!(b, Datatype::Variant(rbx_types::Variant::Float32(n)) if *n == 10.0),
            "got {:?}",
            b
        );
    }

    #[tokio::test]
    async fn regular_token_ref_is_unknown() {
        let result = typecheck("$A = 10; $B = $A;").await;
        let b = find_token(&result, "B", false);
        assert!(matches!(b, Datatype::None), "got {:?}", b);
    }

    #[tokio::test]
    async fn token_invalid_rhs() {
        let result = typecheck("$X = ;").await;
        if let Some((_, _, _, _, dt)) = result
            .tokens
            .iter()
            .find(|(_, _, n, s, _)| n == "X" && !*s)
        {
            assert!(matches!(dt, Datatype::None), "got {:?}", dt);
        }
    }

    #[tokio::test]
    async fn token_enum_shorthand_dynamic_unknown_enum() {
        let result = typecheck("$X = :Hello;").await;
        let dt = find_token(&result, "X", false);
        assert!(matches!(dt, Datatype::None), "got {:?}", dt);
        assert!(
            result.errors.iter().any(|err| err.contains("Unknown Enum")),
            "expected Unknown Enum error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn token_enum_shorthand_static_unknown_enum() {
        let result = typecheck("$!X = :Hello;").await;
        let dt = find_token(&result, "X", true);
        assert!(matches!(dt, Datatype::None), "got {:?}", dt);
        assert!(
            result.errors.iter().any(|err| err.contains("Unknown Enum")),
            "expected Unknown Enum error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn token_full_enum_valid_dynamic() {
        let result = typecheck("$X = Enum.Material.Plastic;").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(
                dt,
                Datatype::Variant(rbx_types::Variant::EnumItem(item)) if item.ty == "Material"
            ),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn token_full_enum_valid_static() {
        let result = typecheck("$!X = Enum.Material.Plastic;").await;
        let dt = find_token(&result, "X", true);
        assert!(
            matches!(
                dt,
                Datatype::Variant(rbx_types::Variant::EnumItem(item)) if item.ty == "Material"
            ),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn token_full_enum_unresolvable_dynamic() {
        let result = typecheck("$X = Enum.NotReal.xyz;").await;
        let dt = find_token(&result, "X", false);
        assert!(matches!(dt, Datatype::None), "got {:?}", dt);
        assert!(
            result.errors.iter().any(|err| err.contains("Unknown Enum")),
            "expected Unknown Enum error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn token_full_enum_unresolvable_static() {
        let result = typecheck("$!X = Enum.NotReal.xyz;").await;
        let dt = find_token(&result, "X", true);
        assert!(matches!(dt, Datatype::None), "got {:?}", dt);
        assert!(
            result.errors.iter().any(|err| err.contains("Unknown Enum")),
            "expected Unknown Enum error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn token_boolean_dynamic() {
        let result = typecheck("$X = true;").await;
        let dt = find_token(&result, "X", false);
        assert!(
            matches!(dt, Datatype::Variant(rbx_types::Variant::Bool(true))),
            "got {:?}",
            dt
        );
    }

    #[tokio::test]
    async fn static_token_oklch_not_coerced() {
        let result = typecheck("$!X = oklch(0.5, 0.1, 180);").await;
        let dt = find_token(&result, "X", true);
        assert!(matches!(dt, Datatype::Oklch(_)), "got {:?}", dt);
    }

    fn has_undefined_token_error(result: &TypecheckResult) -> bool {
        result.errors.iter().any(|err| err.contains("Undefined Token"))
    }

    #[tokio::test]
    async fn undefined_dynamic_token_direct() {
        let result = typecheck("$A = $nope;").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn undefined_static_token_direct() {
        let result = typecheck("$!A = $!nope;").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn undefined_token_in_property_assignment() {
        let result = typecheck("Frame { Size = $nope; }").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn undefined_token_in_annotated_tuple() {
        let result = typecheck("Frame { Size = udim2(0%, $!Hello, 0%, 0%); }").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn undefined_token_in_math() {
        let result = typecheck("$!A = 10; $!B = $!A + $!nope;").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn undefined_token_in_table() {
        let result = typecheck("$A = { $nope };").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn same_statement_self_ref_errors() {
        let result = typecheck("$A = $A;").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn dynamic_and_static_distinct_keys() {
        let result = typecheck("$A = 10; $B = $!A;").await;
        assert!(
            has_undefined_token_error(&result),
            "expected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn defined_static_token_no_error() {
        let result = typecheck("$!A = 10; $!B = $!A;").await;
        assert!(
            !has_undefined_token_error(&result),
            "unexpected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn defined_dynamic_token_no_error() {
        let result = typecheck("$A = 10; Frame { Size = $A; }").await;
        assert!(
            !has_undefined_token_error(&result),
            "unexpected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn nested_rule_inherits_outer_token() {
        let result = typecheck("$!A = 10; Frame { $!B = $!A; }").await;
        assert!(
            !has_undefined_token_error(&result),
            "unexpected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn inner_shadow_resolves_to_outer_in_same_rhs() {
        let result = typecheck("$!A = 10; Frame { $!A = $!A; }").await;
        assert!(
            !has_undefined_token_error(&result),
            "unexpected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn declared_unknown_static_token_errors_in_annotation_arg() {
        let result =
            typecheck("$!Hello = Enum.Hello.world; Frame { Size = udim2(50%, $!Hello); }").await;
        let errs = annotation_arg_type_errors(&result);
        assert!(
            !errs.is_empty(),
            "expected a Wrong Annotation Argument Type error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn same_scope_redeclaration_still_declared() {
        let result = typecheck("$A = 10; $A = 20; Frame { Size = $A; }").await;
        assert!(
            !has_undefined_token_error(&result),
            "unexpected Undefined Token error, got: {:?}",
            result.errors
        );
    }

    fn has_unknown_enum_error(result: &TypecheckResult) -> bool {
        result.errors.iter().any(|err| err.contains("Unknown Enum"))
    }

    #[tokio::test]
    async fn property_shorthand_unknown_enum_name() {
        let result = typecheck("Frame { Hello = :World; }").await;
        assert!(
            has_unknown_enum_error(&result),
            "expected Unknown Enum error, got: {:?}",
            result.errors
        );
        let dt = find_property(&result, "Hello");
        assert!(matches!(dt, Datatype::None), "got {:?}", dt);
    }

    #[tokio::test]
    async fn property_full_enum_unknown_name() {
        let result = typecheck("Frame { Foo = Enum.Hello.World; }").await;
        assert!(
            has_unknown_enum_error(&result),
            "expected Unknown Enum error, got: {:?}",
            result.errors
        );
        let dt = find_property(&result, "Foo");
        assert!(matches!(dt, Datatype::None), "got {:?}", dt);
    }

    #[tokio::test]
    async fn token_full_enum_unknown_variant() {
        let result = typecheck("$X = Enum.Material.NotAVariant;").await;
        let dt = find_token(&result, "X", false);
        assert!(matches!(dt, Datatype::None), "got {:?}", dt);
        assert!(
            result
                .errors
                .iter()
                .any(|err| err.contains("Unknown Enum Variant")),
            "expected Unknown Enum Variant error, got: {:?}",
            result.errors
        );
    }

    #[tokio::test]
    async fn property_full_enum_valid() {
        let result = typecheck("Frame { Material = Enum.Material.Plastic; }").await;
        assert!(
            !has_unknown_enum_error(&result),
            "unexpected Unknown Enum error, got: {:?}",
            result.errors
        );
        let dt = find_property(&result, "Material");
        assert!(
            matches!(
                dt,
                Datatype::Variant(rbx_types::Variant::EnumItem(item)) if item.ty == "Material"
            ),
            "got {:?}",
            dt
        );
    }
}
