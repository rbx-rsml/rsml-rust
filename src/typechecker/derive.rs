use std::{
    collections::{HashMap, HashSet},
    ops::RangeInclusive,
    path::{Path, PathBuf},
    pin::Pin,
};

use crate::{
    lexer::{MultilineString, SpannedToken, Token},
    parser::{AstErrors, Construct, Delimited, Node},
};

use super::luaurc::Luaurc;
use super::normalize_path::NormalizePath;

use super::{DefinitionKind, PushTypeError, Typechecker, type_error::*};

impl<'a> Typechecker<'a> {
    pub(super) fn typecheck_derive<'b>(
        &'b self,
        body: &'b Construct<'a>,
        ast_errors: &'b mut AstErrors,
        current_path: &'b Path,
        mut luaurc: Option<&'b mut Luaurc>,
        definitions: &'b mut super::Definitions,
        dependencies: &'b mut HashSet<PathBuf>,
        derives: &'b mut HashMap<PathBuf, RangeInclusive<usize>>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'b + Send>> {
        Box::pin(async move {
            match body {
                Construct::Node {
                    node:
                        Node {
                            token:
                                SpannedToken(
                                    span_start,
                                    Token::StringSingle(content)
                                    | Token::StringMulti(MultilineString { content, .. }),
                                    span_end,
                                ),
                            ..
                        },
                } => {
                    self.resolve_derive(
                        content,
                        (*span_start, *span_end),
                        ast_errors,
                        current_path,
                        luaurc.as_deref_mut(),
                        definitions,
                        dependencies,
                        derives,
                    )
                    .await;
                }

                Construct::Table {
                    body: Delimited { content, .. },
                } => 'table: {
                    let Some(content) = content.as_ref() else {
                        break 'table;
                    };

                    for item in content {
                        let datatype = if let Construct::Node {
                            node:
                                Node {
                                    token: SpannedToken(_, Token::SemiColon, _),
                                    ..
                                },
                            ..
                        } = item
                        {
                            continue;
                        } else {
                            item
                        };

                        self.typecheck_derive(
                            &datatype,
                            ast_errors,
                            current_path,
                            luaurc.as_deref_mut(),
                            definitions,
                            dependencies,
                            derives,
                        )
                        .await;
                    }
                }

                Construct::Node {
                    node:
                        Node {
                            token: SpannedToken(_, Token::Comma, _),
                            ..
                        },
                } => (),

                _ => ast_errors.push(
                    TypeError::InvalidType {
                        expected: Some(Datatype::String),
                    },
                    self.parsed.range_from_span(body.span()),
                ),
            }
        })
    }

    fn resolve_derive_alias(
        &self,
        derived_path: &str,
        current_path: &Path,
        luaurc: Option<&mut Luaurc>,
    ) -> PathBuf {
        let path = 'core: {
            let derived_path = PathBuf::from(derived_path).normalize();
            let Some(luaurc) = luaurc else {
                break 'core derived_path;
            };

            let mut components = derived_path.components();

            let Some(component) = components.next() else {
                break 'core derived_path;
            };
            let component_str = component.as_os_str().to_string_lossy();

            if component_str.starts_with("@") {
                let alias = &component_str.as_ref()[1..];

                luaurc
                    .dependants
                    .insert(alias.to_string(), current_path.to_path_buf());

                if let Some(alias) = luaurc.aliases.get(alias) {
                    let mut derived_path = PathBuf::from(alias);

                    derived_path.push(components);

                    return derived_path;
                } else {
                    derived_path
                }
            } else {
                derived_path
            }
        };

        current_path.join("../").join(path)
    }

    async fn resolve_derive(
        &self,
        content: &str,
        span: (usize, usize),
        ast_errors: &mut AstErrors,
        current_path: &Path,
        luaurc: Option<&mut Luaurc>,
        definitions: &mut super::Definitions,
        dependencies: &mut HashSet<PathBuf>,
        derives: &mut HashMap<PathBuf, RangeInclusive<usize>>,
    ) {
        let mut path = self.resolve_derive_alias(content.trim(), current_path, luaurc);
        path.set_extension("rsml");

        match path.canonicalize() {
            Ok(canonicalized) => {
                if &canonicalized == current_path {
                    ast_errors.push(
                        TypeError::CyclicDerive {
                            kind: CyclicKind::Internal,
                        },
                        self.parsed.range_from_span(span),
                    );
                } else {
                    dependencies.insert(canonicalized.clone());
                    definitions.insert(
                        span.0..=span.1,
                        DefinitionKind::Derive {
                            path: canonicalized.clone(),
                        },
                    );

                    derives.insert(canonicalized, span.0..=span.1);
                }
            }

            Err(_) => {
                let normalized_path = path.normalize();

                ast_errors.push(
                    TypeError::UnknownDerive {
                        path: Some(&normalized_path.to_string_lossy()),
                    },
                    self.parsed.range_from_span(span),
                );

                definitions.insert(
                    span.0..=span.1,
                    DefinitionKind::Derive {
                        path: normalized_path,
                    },
                );
            }
        }
    }
}
