use crate::capabilities::authoring::position_in_code_fence;
use crate::convert::line_at;
use crate::convert::table_cell_at_position;
use crate::convert::word_at_position;
use crate::state::{
    resolve_markdown_suffixes, with_descriptor_root_for_path, PackageState, WorkspaceState,
};
use mds_core::descriptor::{markdown_module_id_for_lang, markdown_module_path_for_lang};
use mds_core::markdown::source_markdown_root;
use mds_core::ImplDoc;
use std::collections::HashSet;
use std::path::Path;
use tower_lsp::lsp_types::*;

fn text_for_path(path: &Path, state: &WorkspaceState) -> Option<String> {
    state
        .open_file_text_for_path(path)
        .map(ToOwned::to_owned)
        .or_else(|| std::fs::read_to_string(path).ok())
}

fn resolve_internal_target_path(
    source_path: &Path,
    state: &WorkspaceState,
    target: &str,
    markdown_root: &Path,
) -> Option<std::path::PathBuf> {
    for suffix in resolve_markdown_suffixes(source_path, Some(state)) {
        let target_path = markdown_root.join(format!("{target}{suffix}"));
        if !target_path.exists() {
            continue;
        }
        if let (Ok(canonical_root), Ok(canonical_target)) =
            (markdown_root.canonicalize(), target_path.canonicalize())
        {
            if !canonical_target.starts_with(&canonical_root) {
                continue;
            }
        }
        return Some(target_path);
    }

    None
}

fn module_keys(module_path: String, module_id: String) -> Vec<String> {
    if module_id == module_path {
        vec![module_path]
    } else {
        vec![module_path, module_id]
    }
}

fn doc_module_parts(doc: &ImplDoc, state: &WorkspaceState) -> (String, String) {
    with_descriptor_root_for_path(Some(&doc.path), Some(state), || {
        (
            markdown_module_path_for_lang(&doc.lang, &doc.markdown_relative_path),
            markdown_module_id_for_lang(&doc.lang, &doc.markdown_relative_path),
        )
    })
}

fn doc_module_keys(doc: &ImplDoc, state: &WorkspaceState) -> Vec<String> {
    let (module_path, module_id) = doc_module_parts(doc, state);
    module_keys(module_path, module_id)
}

fn matching_doc_module_ids(
    reference_path: &Path,
    module: &str,
    state: &WorkspaceState,
) -> Vec<String> {
    let Some(pkg_state) = state.package_for_path(reference_path) else {
        return vec![module.to_string()];
    };

    let mut module_ids = Vec::new();
    let mut seen = HashSet::new();
    for doc in pkg_state.index.docs.values() {
        let (module_path, module_id) = doc_module_parts(doc, state);
        if module != module_path && module != module_id {
            continue;
        }
        if seen.insert(module_id.clone()) {
            module_ids.push(module_id);
        }
    }

    if module_ids.is_empty() {
        module_ids.push(module.to_string());
    }

    module_ids
}

fn find_module_locations_for_path(
    reference_path: &Path,
    module: &str,
    state: &WorkspaceState,
) -> Vec<std::path::PathBuf> {
    state
        .package_for_path(reference_path)
        .and_then(|pkg_state| pkg_state.index.module_index.get(module).cloned())
        .unwrap_or_else(|| state.find_module_locations(module))
}

fn find_symbol_locations_for_path(
    reference_path: &Path,
    module: &str,
    symbol: &str,
    state: &WorkspaceState,
) -> Vec<std::path::PathBuf> {
    let Some(pkg_state) = state.package_for_path(reference_path) else {
        return state.find_symbol_locations(module, symbol);
    };

    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for module_id in matching_doc_module_ids(reference_path, module, state) {
        if let Some(module_paths) = pkg_state
            .index
            .symbol_index
            .get(&(module_id, symbol.to_string()))
        {
            for path in module_paths {
                if seen.insert(path.clone()) {
                    paths.push(path.clone());
                }
            }
        }
    }

    if paths.is_empty() {
        state.find_symbol_locations(module, symbol)
    } else {
        paths
    }
}

fn line_location(path: &Path, line_idx: usize, line: &str) -> Option<Location> {
    let char_end = u32::try_from(line.len()).unwrap_or(u32::MAX);
    Some(Location {
        uri: Url::from_file_path(path).ok()?,
        range: Range {
            start: Position {
                line: line_idx as u32,
                character: 0,
            },
            end: Position {
                line: line_idx as u32,
                character: char_end,
            },
        },
    })
}

fn trim_link_target(target: &str) -> &str {
    target.trim().trim_matches('<').trim_matches('>')
}

fn is_external_target(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://") || target.starts_with("mailto:")
}

fn split_target_fragment(target: &str) -> (&str, Option<&str>) {
    match target.split_once('#') {
        Some((base, fragment)) => (base.trim(), Some(fragment.trim())),
        None => (target.trim(), None),
    }
}

#[derive(Debug)]
struct ReferenceQuery {
    module_keys: Vec<String>,
    symbol: Option<String>,
    fallback_terms: Vec<String>,
}

fn reference_query(
    text: &str,
    position: Position,
    doc: &ImplDoc,
    state: &WorkspaceState,
) -> Option<ReferenceQuery> {
    let line_text = line_at(text, position.line)?;
    let word = word_at_position(text, position)?;
    if word.is_empty() {
        return None;
    }

    let module_keys = doc_module_keys(doc, state);
    let module_only =
        line_text.trim_start().starts_with("# ") || module_keys.iter().any(|key| key == &word);

    let mut fallback_terms = Vec::new();
    let mut seen = HashSet::new();
    for term in module_keys
        .iter()
        .cloned()
        .chain(std::iter::once(word.clone()))
    {
        if seen.insert(term.clone()) {
            fallback_terms.push(term);
        }
    }

    Some(ReferenceQuery {
        module_keys,
        symbol: (!module_only).then_some(word),
        fallback_terms,
    })
}

fn fragment_matches_symbol(fragment: Option<&str>, symbol: Option<&str>) -> bool {
    match symbol {
        Some(symbol) => fragment.is_some_and(|fragment| {
            let fragment = fragment.trim_start_matches('#');
            fragment == symbol || fragment == slugify_heading(symbol)
        }),
        None => fragment.is_none(),
    }
}

fn resolve_structured_target_path(
    reference_path: &Path,
    state: &WorkspaceState,
    markdown_root: &Path,
    base: &str,
) -> Option<std::path::PathBuf> {
    let base = base.trim();
    if base.is_empty() {
        return None;
    }
    if base.ends_with(".md") {
        return Some(reference_path.parent()?.join(base));
    }
    if base.contains('/') {
        return resolve_internal_target_path(reference_path, state, base, markdown_root);
    }
    None
}

fn structured_target_matches(
    reference_path: &Path,
    target: &str,
    query: &ReferenceQuery,
    current_path: &Path,
    markdown_root: &Path,
    state: &WorkspaceState,
) -> bool {
    let target = trim_link_target(target);
    if target.is_empty() || target.starts_with('#') || is_external_target(target) {
        return false;
    }

    let (base, fragment) = split_target_fragment(target);
    if let Some(target_path) =
        resolve_structured_target_path(reference_path, state, markdown_root, base)
    {
        if target_path != current_path {
            return false;
        }
        return fragment_matches_symbol(fragment, query.symbol.as_deref());
    }

    query
        .module_keys
        .iter()
        .any(|module_key| module_key == base)
        && fragment_matches_symbol(fragment, query.symbol.as_deref())
}

fn wiki_link_targets(line: &str) -> impl Iterator<Item = &str> {
    let mut targets = Vec::new();
    let mut start = 0;

    while let Some(open_rel) = line[start..].find("[[") {
        let open = start + open_rel + 2;
        let Some(close_rel) = line[open..].find("]]") else {
            break;
        };
        let close = open + close_rel;
        targets.push(
            line[open..close]
                .split('|')
                .next()
                .unwrap_or_default()
                .trim(),
        );
        start = close + 2;
    }

    targets.into_iter()
}

fn markdown_link_targets(line: &str) -> impl Iterator<Item = &str> {
    let mut targets = Vec::new();
    let mut start = 0;

    while let Some(middle_rel) = line[start..].find("](") {
        let middle = start + middle_rel;
        let target_start = middle + 2;
        let Some(target_end_rel) = line[target_start..].find(')') else {
            break;
        };
        let target_end = target_start + target_end_rel;
        if line[..middle].rfind('[').is_some() {
            targets.push(line[target_start..target_end].trim());
        }
        start = target_end + 1;
    }

    targets.into_iter()
}

fn table_line_matches(line: &str, query: &ReferenceQuery) -> bool {
    if !line.trim_start().starts_with('|') {
        return false;
    }

    let cells = line
        .trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect::<Vec<_>>();

    let module_match = cells.iter().any(|cell| {
        query
            .module_keys
            .iter()
            .any(|module_key| module_key == cell)
    });
    if !module_match {
        return false;
    }

    match query.symbol.as_deref() {
        Some(symbol) => cells
            .iter()
            .any(|cell| *cell == symbol || *cell == slugify_heading(symbol)),
        None => true,
    }
}

fn line_contains_structured_reference(
    reference_path: &Path,
    line: &str,
    query: &ReferenceQuery,
    current_path: &Path,
    markdown_root: &Path,
    state: &WorkspaceState,
) -> bool {
    wiki_link_targets(line).any(|target| {
        structured_target_matches(
            reference_path,
            target,
            query,
            current_path,
            markdown_root,
            state,
        )
    }) || markdown_link_targets(line).any(|target| {
        structured_target_matches(
            reference_path,
            target,
            query,
            current_path,
            markdown_root,
            state,
        )
    }) || table_line_matches(line, query)
}

fn find_structured_references(
    pkg_state: &PackageState,
    query: &ReferenceQuery,
    current_path: &Path,
    state: &WorkspaceState,
) -> Vec<Location> {
    let markdown_root = source_markdown_root(&pkg_state.package);
    let mut locations = Vec::new();
    let mut seen = HashSet::new();

    for doc_path in pkg_state.index.docs.keys() {
        if doc_path == current_path {
            continue;
        }
        let Some(file_text) = text_for_path(doc_path, state) else {
            continue;
        };
        let mut in_code_fence = false;
        for (idx, line) in file_text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                in_code_fence = !in_code_fence;
                continue;
            }
            if in_code_fence {
                continue;
            }
            if !line_contains_structured_reference(
                doc_path,
                line,
                query,
                current_path,
                markdown_root.as_path(),
                state,
            ) {
                continue;
            }
            if !seen.insert((doc_path.clone(), idx)) {
                continue;
            }
            if let Some(location) = line_location(doc_path, idx, line) {
                locations.push(location);
            }
        }
    }

    locations
}

pub fn goto_definition(
    text: &str,
    position: Position,
    path: &Path,
    state: &WorkspaceState,
) -> Option<GotoDefinitionResponse> {
    let line_text = line_at(text, position.line)?;

    if let Some(location) = wiki_link_location_at(line_text, position, path, state) {
        return Some(GotoDefinitionResponse::Scalar(location));
    }

    if let Some(location) = markdown_link_location_at(line_text, position, path, state) {
        return Some(GotoDefinitionResponse::Scalar(location));
    }

    if !line_text.trim_start().starts_with('|') {
        return None;
    }

    // Try to resolve the target cell
    let cell = table_cell_at_position(text, position)?;
    if cell.is_empty() {
        return None;
    }

    if let Some(location) = markdown_link_location(path, &cell, state) {
        return Some(GotoDefinitionResponse::Scalar(location));
    }

    if cell.contains("..") || cell.starts_with('/') || cell.contains('\\') {
        return None;
    }

    let pkg_state = state.package_for_path(path)?;
    let package = &pkg_state.package;
    let markdown_root = source_markdown_root(package);

    // Try as internal target: resolve relative to markdown root
    if let Some(target_path) =
        resolve_internal_target_path(path, state, &cell, markdown_root.as_path())
    {
        let uri = Url::from_file_path(&target_path).ok()?;
        return Some(GotoDefinitionResponse::Scalar(Location {
            uri,
            range: h5_range_for_name(&target_path, &cell, state).unwrap_or_default(),
        }));
    }

    // Try looking up in expose index
    let locations = state.find_expose_locations(&cell);
    if !locations.is_empty() {
        let locs: Vec<Location> = locations
            .iter()
            .filter_map(|p| {
                Url::from_file_path(p).ok().map(|uri| Location {
                    uri,
                    range: h5_range_for_name(p, &cell, state).unwrap_or_default(),
                })
            })
            .collect();
        if locs.len() == 1 {
            return Some(GotoDefinitionResponse::Scalar(locs.into_iter().next()?));
        }
        return Some(GotoDefinitionResponse::Array(locs));
    }

    None
}

fn wiki_link_location_at(
    line_text: &str,
    position: Position,
    path: &Path,
    state: &WorkspaceState,
) -> Option<Location> {
    let col = position.character as usize;
    let upto_cursor = &line_text[..line_text.len().min(col)];
    let start = upto_cursor.rfind("[[")?;
    let end = line_text[start + 2..].find("]]")? + start + 2;
    if col > end + 2 {
        return None;
    }
    let target = line_text[start + 2..end]
        .split('|')
        .next()
        .unwrap_or_default()
        .trim();
    let (module, symbol) = target.split_once('#').unwrap_or((target, ""));
    let paths = if symbol.is_empty() {
        find_module_locations_for_path(path, module, state)
    } else {
        find_symbol_locations_for_path(path, module, symbol, state)
    };
    let path = paths.into_iter().next()?;
    let range = if symbol.is_empty() {
        Range::default()
    } else {
        h5_range_for_name(&path, symbol, state)
            .or_else(|| heading_range_for_anchor(&path, symbol, state))
            .unwrap_or_default()
    };
    Some(Location {
        uri: Url::from_file_path(path).ok()?,
        range,
    })
}

fn markdown_link_location_at(
    line_text: &str,
    position: Position,
    path: &Path,
    state: &WorkspaceState,
) -> Option<Location> {
    let link = markdown_link_at(line_text, position)?;
    markdown_link_location(path, link, state)
}

fn markdown_link_at(line_text: &str, position: Position) -> Option<&str> {
    let col = position.character as usize;
    let mut start = 0;

    while let Some(middle_rel) = line_text[start..].find("](") {
        let middle = start + middle_rel;
        let Some(open) = line_text[..middle].rfind('[') else {
            start = middle + 2;
            continue;
        };
        if open > 0 && line_text.as_bytes()[open - 1] == b'!' {
            start = middle + 2;
            continue;
        }

        let target_start = middle + 2;
        let Some(target_end_rel) = line_text[target_start..].find(')') else {
            break;
        };
        let target_end = target_start + target_end_rel;
        if col >= open && col <= target_end + 1 {
            return Some(&line_text[open..=target_end]);
        }

        start = target_end + 1;
    }

    None
}

fn markdown_link_location(
    source_path: &Path,
    cell: &str,
    state: &WorkspaceState,
) -> Option<Location> {
    let (label, target) = markdown_link_parts(cell)?;
    let (target_file, anchor) = target.split_once('#').unwrap_or((target, ""));
    if target_file.contains("..") || target_file.starts_with('/') || target_file.contains('\\') {
        return None;
    }
    let target_path = source_path.parent()?.join(target_file);
    if text_for_path(&target_path, state).is_none() {
        return None;
    }
    let range = if anchor.is_empty() {
        h5_range_for_name(&target_path, label, state).unwrap_or_default()
    } else {
        heading_range_for_anchor(&target_path, anchor, state).unwrap_or_default()
    };
    Some(Location {
        uri: Url::from_file_path(target_path).ok()?,
        range,
    })
}

fn markdown_link_parts(value: &str) -> Option<(&str, &str)> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(')') {
        return None;
    }
    let middle = value.find("](")?;
    Some((&value[1..middle], &value[middle + 2..value.len() - 1]))
}

fn h5_range_for_name(path: &Path, name: &str, state: &WorkspaceState) -> Option<Range> {
    heading_range_for_anchor(path, &slugify_heading(name), state)
}

fn heading_range_for_anchor(path: &Path, anchor: &str, state: &WorkspaceState) -> Option<Range> {
    let text = text_for_path(path, state)?;
    for (idx, line) in text.lines().enumerate() {
        let Some(title) = line.trim_start().strip_prefix('#') else {
            continue;
        };
        let title = title.trim_start_matches('#').trim();
        if slugify_heading(title) == anchor.trim_start_matches('#') {
            let char_end = u32::try_from(line.len()).unwrap_or(u32::MAX);
            return Some(Range {
                start: Position {
                    line: idx as u32,
                    character: 0,
                },
                end: Position {
                    line: idx as u32,
                    character: char_end,
                },
            });
        }
    }
    None
}

fn slugify_heading(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn find_references(
    text: &str,
    position: Position,
    path: &Path,
    state: &WorkspaceState,
) -> Option<Vec<Location>> {
    if position_in_code_fence(text, position) {
        return None;
    }

    let pkg_state = state.package_for_path(path)?;
    let doc = pkg_state.index.docs.get(path)?;
    let query = reference_query(text, position, doc, state)?;

    let structured = find_structured_references(pkg_state, &query, path, state);
    if !structured.is_empty() {
        return Some(structured);
    }

    let mut locations = Vec::new();
    let mut seen_files = HashSet::new();

    for doc_path in pkg_state.index.docs.keys() {
        if doc_path == path {
            continue;
        }
        let Some(file_text) = text_for_path(doc_path, state) else {
            continue;
        };
        for (idx, line) in file_text.lines().enumerate() {
            if !query.fallback_terms.iter().any(|term| line.contains(term)) {
                continue;
            }
            if !seen_files.insert((doc_path.clone(), idx)) {
                continue;
            }
            if let Some(location) = line_location(doc_path, idx, line) {
                locations.push(location);
            }
        }
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}
