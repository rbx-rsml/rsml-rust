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

use crate::typechecker::luaurc::Luaurc;
use crate::typechecker::normalize_path::NormalizePath;

use crate::typechecker::{ReportTypeError, Typechecker, type_error::*};

impl<'a> Typechecker<'a> {
    pub(super) fn typecheck_derive<'b>(
        &'b self,
        body: &'b Construct<'a>,
        ast_errors: &'b mut AstErrors,
        current_path: &'b Path,
        mut luaurc: Option<&'b mut Luaurc>,
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

                _ => ast_errors.report(
                    TypeError::InvalidType {
                        expected: Some(ExpectedDatatype::String),
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
        dependencies: &mut HashSet<PathBuf>,
        derives: &mut HashMap<PathBuf, RangeInclusive<usize>>,
    ) {
        let trimmed = content.trim();
        let mut path = self.resolve_derive_alias(trimmed, current_path, luaurc);

        match path.extension() {
            None => {
                path.set_extension("rsml");
            }
            Some(ext) if ext.eq_ignore_ascii_case("rsml") => {}
            Some(_) => {
                ast_errors.report(
                    TypeError::InvalidDeriveTarget {
                        path: trimmed,
                        reason: "must be a `.rsml` file",
                    },
                    self.parsed.range_from_span(span),
                );
                return;
            }
        }

        match path.canonicalize() {
            Ok(canonicalized) => {
                if !canonicalized.is_file() {
                    ast_errors.report(
                        TypeError::InvalidDeriveTarget {
                            path: &canonicalized.to_string_lossy(),
                            reason: "is not a file",
                        },
                        self.parsed.range_from_span(span),
                    );
                    return;
                }

                if &canonicalized == current_path {
                    ast_errors.report(
                        TypeError::CyclicDerive {
                            kind: CyclicKind::Internal,
                        },
                        self.parsed.range_from_span(span),
                    );
                } else {
                    if self.parsed.directives.static_file
                        && !derived_file_is_static(&canonicalized).await
                    {
                        ast_errors.report(
                            TypeError::NonStaticDerive {
                                path: &canonicalized.to_string_lossy(),
                            },
                            self.parsed.range_from_span(span),
                        );
                    }

                    dependencies.insert(canonicalized.clone());
                    derives.insert(canonicalized, span.0..=span.1);
                }
            }

            Err(_) => {
                let normalized_path = path.normalize();

                ast_errors.report(
                    TypeError::UnknownDerive {
                        path: Some(&normalized_path.to_string_lossy()),
                    },
                    self.parsed.range_from_span(span),
                );
            }
        }
    }
}

async fn derived_file_is_static(path: &Path) -> bool {
    let Ok(source) = tokio::fs::read_to_string(path).await else {
        return true;
    };
    crate::parser::RsmlParser::from_source(&source)
        .directives
        .static_file
}
