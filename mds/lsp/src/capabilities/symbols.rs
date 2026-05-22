use crate::state::WorkspaceState;
use std::collections::HashSet;
use tower_lsp::lsp_types::*;
#[allow(deprecated)]
pub fn document_symbols(text: &str) -> Vec<SymbolInformation> {
    let mut symbols = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        if let Some(title) = line.strip_prefix("## ") {
            let title = title.trim();
            if !title.is_empty() {
                #[allow(deprecated)]
                symbols.push(SymbolInformation {
                    name: title.to_string(),
                    kind: SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: Url::parse("file:///").unwrap_or_else(|_| {
                            Url::parse("untitled:symbol").expect("fallback URI")
                        }),
                        range: Range {
                            start: Position {
                                line: idx as u32,
                                character: 0,
                            },
                            end: Position {
                                line: idx as u32,
                                character: line.len() as u32,
                            },
                        },
                    },
                    container_name: None,
                });
            }
        } else if let Some(title) = line.strip_prefix("### ") {
            let title = title.trim();
            if !title.is_empty() {
                #[allow(deprecated)]
                symbols.push(SymbolInformation {
                    name: title.to_string(),
                    kind: SymbolKind::FIELD,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: Url::parse("file:///").unwrap_or_else(|_| {
                            Url::parse("untitled:symbol").expect("fallback URI")
                        }),
                        range: Range {
                            start: Position {
                                line: idx as u32,
                                character: 0,
                            },
                            end: Position {
                                line: idx as u32,
                                character: line.len() as u32,
                            },
                        },
                    },
                    container_name: None,
                });
            }
        } else if let Some(title) = line.strip_prefix("##### ") {
            let title = title.trim();
            if !title.is_empty() {
                #[allow(deprecated)]
                symbols.push(SymbolInformation {
                    name: title.to_string(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: Url::parse("file:///").unwrap_or_else(|_| {
                            Url::parse("untitled:symbol").expect("fallback URI")
                        }),
                        range: Range {
                            start: Position {
                                line: idx as u32,
                                character: 0,
                            },
                            end: Position {
                                line: idx as u32,
                                character: line.len() as u32,
                            },
                        },
                    },
                    container_name: Some("shared definitions".to_string()),
                });
            }
        }
    }

    symbols
}

#[allow(deprecated)]
pub fn workspace_symbols(query: &str, state: &WorkspaceState) -> Vec<SymbolInformation> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    for pkg in &state.packages {
        for (name, paths) in &pkg.index.module_index {
            if query.is_empty() || name.to_lowercase().contains(query) {
                for path in paths {
                    if !seen.insert((name.clone(), path.clone(), "module")) {
                        continue;
                    }
                    if let Ok(uri) = Url::from_file_path(path) {
                        #[allow(deprecated)]
                        symbols.push(SymbolInformation {
                            name: name.clone(),
                            kind: SymbolKind::MODULE,
                            tags: None,
                            deprecated: None,
                            location: Location {
                                uri,
                                range: Range::default(),
                            },
                            container_name: Some(
                                pkg.package
                                    .root
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                            ),
                        });
                    }
                }
            }
        }

        for ((module, symbol), paths) in &pkg.index.symbol_index {
            if !query.is_empty()
                && !symbol.to_lowercase().contains(query)
                && !module.to_lowercase().contains(query)
            {
                continue;
            }
            for path in paths {
                if !seen.insert((format!("{module}#{symbol}"), path.clone(), "function")) {
                    continue;
                }
                if let Ok(uri) = Url::from_file_path(path) {
                    #[allow(deprecated)]
                    symbols.push(SymbolInformation {
                        name: symbol.clone(),
                        kind: SymbolKind::FUNCTION,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri,
                            range: Range::default(),
                        },
                        container_name: Some(module.clone()),
                    });
                }
            }
        }
    }

    symbols
}
