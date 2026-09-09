use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{OnceLock, RwLock},
};

use crate::{
    datatype::Datatype,
    lexer::Token,
    macro_registry::{MacroReturnContext, collect_macro_def_arg_names, macro_return_context},
    parser::{
        Construct, DeriveWithClause, DeriveWithItems, MacroBodyContent, ParsedRsml, RsmlParser,
    },
    schema_registry::{SchemaField, collect_schema_fields},
};

#[cfg(feature = "typechecker")]
use crate::{
    parser::AstErrors,
    range_from_span::RangeFromSpan,
    typechecker::{ReportTypeError, TypeError},
};

/// Process-global cache of parsed files used for cross-file resolution.
///
/// Each entry's source is `Box::leak`'d to give it a `'static` lifetime so the
/// borrowed AST (`ParsedRsml<'static>`) can be shared across consumers without
/// self-referential lifetime gymnastics. Entries are inserted on first read
/// and never invalidated; for long-running LSP usage this is a known leak,
/// bounded by the number of unique imported files.
pub(crate) static PARSED_FILE_CACHE: OnceLock<
    RwLock<HashMap<PathBuf, &'static ParsedRsml<'static>>>,
> = OnceLock::new();

pub(crate) fn load_parsed_file(path: &Path) -> Option<&'static ParsedRsml<'static>> {
    let cache = PARSED_FILE_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

    if let Ok(read) = cache.read() {
        if let Some(parsed) = read.get(path) {
            return Some(*parsed);
        }
    }

    let source = std::fs::read_to_string(path).ok()?;
    let leaked: &'static str = Box::leak(source.into_boxed_str());
    let parsed = RsmlParser::from_source(leaked);
    let leaked_parsed: &'static ParsedRsml<'static> = Box::leak(Box::new(parsed));

    let mut write = cache.write().ok()?;
    let entry = write.entry(path.to_path_buf()).or_insert(leaked_parsed);
    Some(*entry)
}

/// Cache of extracted public-symbol surfaces keyed by canonical path.
static PUBLIC_SYMBOLS_CACHE: OnceLock<RwLock<HashMap<PathBuf, &'static PublicSymbols>>> =
    OnceLock::new();

/// Owned representation of a macro made public by some file. Body references
/// are `'static` because they point into a leaked `ParsedRsml<'static>`.
#[derive(Debug, Clone)]
pub struct ImportedMacroDef {
    pub arg_names: Vec<&'static str>,
    pub body: Option<&'static MacroBodyContent<'static>>,
    pub return_context: MacroReturnContext,
}

#[derive(Debug, Clone)]
pub struct ImportedSchemaDef {
    pub fields: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct ImportedStaticDef {
    pub value: Datatype,
}

#[derive(Debug, Clone)]
pub struct ImportedTokenDef {
    pub value: Datatype,
}

#[derive(Debug, Default)]
pub struct PublicSymbols {
    pub macros: HashMap<(String, usize), ImportedMacroDef>,
    pub schemas: HashMap<String, ImportedSchemaDef>,
    pub statics: HashMap<String, ImportedStaticDef>,
    /// Dynamic tokens (`$Name = …;`) declared at the top level of the
    /// derived file. Unlike macros/schemas/statics, dynamic tokens are
    /// always exported — they don't require `@pub` and are imported by
    /// every consumer that derives this file, regardless of `@with`.
    pub tokens: HashMap<String, ImportedTokenDef>,
}

/// Returns the public surface of `path`, parsing the file if necessary.
/// Bodies/identifiers in the result reference the cached parsed AST.
pub(crate) fn load_public_symbols(path: &Path) -> Option<&'static PublicSymbols> {
    let cache = PUBLIC_SYMBOLS_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

    if let Ok(read) = cache.read() {
        if let Some(syms) = read.get(path) {
            return Some(*syms);
        }
    }

    let parsed = load_parsed_file(path)?;
    let extracted = extract_public_symbols(parsed);
    let leaked: &'static PublicSymbols = Box::leak(Box::new(extracted));

    let mut write = cache.write().ok()?;
    let entry = write.entry(path.to_path_buf()).or_insert(leaked);
    Some(*entry)
}

/// Walks a parsed file and extracts owned representations of every `@pub`
/// item declared at the top level. Static-token values are evaluated against
/// the file's own private + public static tokens (so a public token that
/// references a private one in the same file resolves correctly), but no
/// recursion across `@derive` boundaries — public symbols are never
/// transitively re-exported.
fn extract_public_symbols(parsed: &'static ParsedRsml<'static>) -> PublicSymbols {
    use crate::datatype::{StaticLookup, evaluate_construct};

    struct LocalLookup<'b> {
        scope: &'b HashMap<String, Datatype>,
    }

    impl<'b> StaticLookup for LocalLookup<'b> {
        fn resolve_static(&self, name: &str) -> Datatype {
            self.scope.get(name).cloned().unwrap_or(Datatype::None)
        }

        fn resolve_dynamic(&self, _name: &str) -> Datatype {
            Datatype::None
        }
    }

    let mut symbols = PublicSymbols::default();
    let mut static_scope: HashMap<String, Datatype> = HashMap::new();

    for construct in &parsed.ast {
        match construct {
            Construct::Macro {
                pub_modifier: Some(_),
                name: Some(name_node),
                args,
                return_type,
                body,
                ..
            } => {
                let Token::Identifier(name_str) = name_node.token.value() else {
                    continue;
                };
                let arg_names = collect_macro_def_arg_names(args);
                let arity = arg_names.len();
                let def = ImportedMacroDef {
                    arg_names,
                    body: body.as_ref().map(|b| &b.content),
                    return_context: macro_return_context(return_type),
                };
                symbols.macros.insert((name_str.to_string(), arity), def);
            }

            Construct::Schema {
                pub_modifier: Some(_),
                name: Some(name_node),
                body,
                ..
            } => {
                let Token::Identifier(schema_name) = name_node.token.value() else {
                    continue;
                };
                let fields: Vec<(String, String)> = collect_schema_fields(body)
                    .into_iter()
                    .map(|f: SchemaField<'static>| (f.name.to_string(), f.type_name.to_string()))
                    .collect();
                symbols
                    .schemas
                    .insert(schema_name.to_string(), ImportedSchemaDef { fields });
            }

            Construct::Assignment {
                left,
                right: Some(right),
                pub_modifier,
                ..
            } => match left.token.value() {
                Token::StaticTokenIdentifier(name) => {
                    let lookup = LocalLookup {
                        scope: &static_scope,
                    };
                    let value =
                        evaluate_construct(right, Some(name), &lookup).unwrap_or(Datatype::None);
                    static_scope.insert(name.to_string(), value.clone());

                    if pub_modifier.is_some() {
                        symbols
                            .statics
                            .insert(name.to_string(), ImportedStaticDef { value });
                    }
                }

                Token::TokenIdentifier(name) => {
                    // Dynamic tokens are always exported — no `@pub` gate.
                    // Their value is the resolved type for hover; references
                    // are resolved at runtime by the Roblox StyleSheet
                    // hierarchy, so an exact value isn't required here.
                    let lookup = LocalLookup {
                        scope: &static_scope,
                    };
                    let value =
                        evaluate_construct(right, Some(name), &lookup).unwrap_or(Datatype::None);
                    symbols
                        .tokens
                        .insert(name.to_string(), ImportedTokenDef { value });
                }

                _ => {}
            },

            _ => {}
        }
    }

    symbols
}

/// Collected at typecheck time from each `@derive ... @with ...` and resolved
/// in a second pass after all local items have been registered.
#[derive(Debug, Clone)]
pub struct PendingImport {
    pub source_path: PathBuf,
    pub source_display: String,
    pub span: (usize, usize),
    pub selector: ImportSelector,
}

#[derive(Debug, Clone)]
pub enum ImportSelector {
    All,
    Named(Vec<NamedImport>),
}

#[derive(Debug, Clone)]
pub struct NamedImport {
    pub name: String,
    pub is_static_token: bool,
    pub span: (usize, usize),
}

/// Aggregated imports merged into the deriving file. Mirrors `PublicSymbols`
/// but only contains entries the consumer asked for via `@with`.
#[derive(Debug, Default)]
pub struct ImportedSymbols {
    pub macros: HashMap<(String, usize), ImportedMacroDef>,
    pub schemas: HashMap<String, ImportedSchemaDef>,
    pub statics: HashMap<String, ImportedStaticDef>,
    /// For each imported name, the source file it was pulled from. Used to
    /// produce informative collision diagnostics.
    pub provenance: HashMap<String, String>,
}

impl ImportedSymbols {
    pub fn macro_arity_exists(&self, name: &str, arity: usize) -> bool {
        self.macros.contains_key(&(name.to_string(), arity))
    }

    pub fn any_macro_with_name(&self, name: &str) -> bool {
        self.macros.keys().any(|(n, _)| n == name)
    }

    pub fn schema(&self, name: &str) -> Option<&ImportedSchemaDef> {
        self.schemas.get(name)
    }

    pub fn static_value(&self, name: &str) -> Option<&Datatype> {
        self.statics.get(name).map(|s| &s.value)
    }
}

/// Translates a parsed `@with` clause into the typechecker's selector form.
/// Reports any malformed entries (e.g. tokens that aren't identifiers or
/// static-token names) as parser-style errors. Returns `None` if the clause
/// was so broken that no meaningful imports can be derived from it.
#[cfg(feature = "typechecker")]
pub(crate) fn build_import_selector<'a>(
    clause: &DeriveWithClause<'a>,
    parsed: &'a ParsedRsml<'a>,
    ast_errors: &mut AstErrors,
) -> Option<ImportSelector> {
    let items = clause.items.as_ref()?;
    match items {
        DeriveWithItems::All(_) => Some(ImportSelector::All),
        DeriveWithItems::List(list) => {
            let entries = list.content.as_ref()?;
            let mut named = Vec::with_capacity(entries.len());
            for entry in entries {
                let span = entry.name.token.span();
                match entry.name.token.value() {
                    Token::Identifier(name) => {
                        named.push(NamedImport {
                            name: name.to_string(),
                            is_static_token: false,
                            span,
                        });
                    }
                    Token::StaticTokenIdentifier(name) => {
                        named.push(NamedImport {
                            name: name.to_string(),
                            is_static_token: true,
                            span,
                        });
                    }
                    _ => {
                        ast_errors.report(
                            TypeError::InvalidType { expected: None },
                            crate::types::Range::from_span(&parsed.rope, span),
                        );
                    }
                }
            }
            Some(ImportSelector::Named(named))
        }
    }
}

/// Compiler-side selector builder. Same shape as the typechecker version but
/// silently drops malformed entries (the typechecker has already reported
/// them as diagnostics during typecheck).
pub(crate) fn build_import_selector_silent<'a>(
    clause: &DeriveWithClause<'a>,
) -> Option<ImportSelector> {
    let items = clause.items.as_ref()?;
    match items {
        DeriveWithItems::All(_) => Some(ImportSelector::All),
        DeriveWithItems::List(list) => {
            let entries = list.content.as_ref()?;
            let mut named = Vec::with_capacity(entries.len());
            for entry in entries {
                let span = entry.name.token.span();
                match entry.name.token.value() {
                    Token::Identifier(name) => named.push(NamedImport {
                        name: name.to_string(),
                        is_static_token: false,
                        span,
                    }),
                    Token::StaticTokenIdentifier(name) => named.push(NamedImport {
                        name: name.to_string(),
                        is_static_token: true,
                        span,
                    }),
                    _ => {}
                }
            }
            Some(ImportSelector::Named(named))
        }
    }
}
