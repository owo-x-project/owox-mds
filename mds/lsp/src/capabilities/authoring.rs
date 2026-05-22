use std::collections::HashMap;
use std::path::{Component, Path};

use crate::state::{resolve_active_language, with_descriptor_root_for_path, WorkspaceState};
use mds_core::descriptor::{
    is_overview_markdown_path, is_root_module_markdown_path, package_overview_kind_for_path,
    PackageOverviewKind,
};
use mds_core::model::{Config, DocKind};
use tower_lsp::lsp_types::Position;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidedAuthoringPathKind {
    Source,
    Test,
    Overview,
    SourceOverview,
    RootModule,
}

pub fn guided_authoring_path_kind(
    path: Option<&Path>,
    config: &Config,
    workspace_state: Option<&WorkspaceState>,
) -> GuidedAuthoringPathKind {
    let Some(path) = path else {
        return match doc_kind_for_path(None, config, workspace_state) {
            DocKind::Source => GuidedAuthoringPathKind::Source,
            DocKind::Test => GuidedAuthoringPathKind::Test,
        };
    };

    let package_root = workspace_state
        .and_then(|workspace_state| workspace_state.package_for_path(path))
        .map(|package_state| package_state.package.root.as_path());

    if matches!(
        package_overview_kind_for_path(path, package_root, config),
        Some(PackageOverviewKind::Source)
    ) {
        return GuidedAuthoringPathKind::SourceOverview;
    }

    if is_overview_markdown_path(path) {
        return GuidedAuthoringPathKind::Overview;
    }

    if with_descriptor_root_for_path(Some(path), workspace_state, || {
        is_root_module_markdown_path(path)
    }) {
        return GuidedAuthoringPathKind::RootModule;
    }

    match doc_kind_for_path(Some(path), config, workspace_state) {
        DocKind::Source => GuidedAuthoringPathKind::Source,
        DocKind::Test => GuidedAuthoringPathKind::Test,
    }
}

pub fn doc_kind_for_path(
    path: Option<&Path>,
    config: &Config,
    workspace_state: Option<&WorkspaceState>,
) -> DocKind {
    let Some(path) = path else {
        return DocKind::Source;
    };

    if let Some(workspace_state) = workspace_state {
        if let Some(package_state) = workspace_state.package_for_path(path) {
            let source_root = package_state
                .package
                .root
                .join(&package_state.package.config.roots.source_md);
            let test_root = package_state
                .package
                .root
                .join(&package_state.package.config.roots.test_md);
            if path.starts_with(&test_root) {
                return DocKind::Test;
            }
            if path.starts_with(&source_root) {
                return DocKind::Source;
            }
        }
    }

    if path_contains_relative_root(path, &config.roots.test_md) {
        return DocKind::Test;
    }
    if path_contains_relative_root(path, &config.roots.source_md) {
        return DocKind::Source;
    }
    if resolve_active_language(path, workspace_state).is_some() {
        return DocKind::Source;
    }
    if path.extension().is_some_and(|extension| extension == "md")
        && !is_overview_markdown_path(path)
    {
        return DocKind::Test;
    }

    DocKind::Source
}

pub fn sections_with_labels_for_doc(
    text: &str,
    label_overrides: &HashMap<String, String>,
    doc_kind: DocKind,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut current: Option<String> = None;
    let mut body = String::new();
    let mut fence_len: Option<usize> = None;

    for line in text.lines() {
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if is_closing_fence(marker_len, suffix, open_len) {
                    fence_len = None;
                }
            } else {
                fence_len = Some(marker_len);
            }
        }

        if fence_len.is_none() && line.starts_with("## ") {
            let title = line.strip_prefix("## ").unwrap_or_default();
            let title = canonical_section_title_for_doc(title.trim(), label_overrides, doc_kind);
            if let Some(name) = current.replace(title) {
                result.insert(name, body.trim_matches('\n').to_string());
                body.clear();
            }
        } else if current.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }

    if let Some(name) = current {
        result.insert(name, body.trim_matches('\n').to_string());
    }

    result
}

pub fn contains_markdown_table(section: &str) -> bool {
    let lines = section.lines().collect::<Vec<_>>();
    for index in 0..lines.len().saturating_sub(1) {
        if lines[index].trim_start().starts_with('|') && lines[index + 1].contains("---") {
            return true;
        }
    }
    false
}

pub fn text_without_code_blocks(text: &str) -> String {
    let mut output = String::new();
    let mut fence_len: Option<usize> = None;

    for line in text.lines() {
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if is_closing_fence(marker_len, suffix, open_len) {
                    fence_len = None;
                }
            } else {
                fence_len = Some(marker_len);
            }
            continue;
        }
        if fence_len.is_none() {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}

pub(crate) fn position_in_code_fence(text: &str, position: Position) -> bool {
    let Ok(target_line) = usize::try_from(position.line) else {
        return false;
    };

    let mut fence_len: Option<usize> = None;

    for (idx, line) in text.lines().enumerate() {
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if is_closing_fence(marker_len, suffix, open_len) {
                    if idx == target_line {
                        return false;
                    }
                    fence_len = None;
                    continue;
                }
            } else {
                if idx == target_line {
                    return false;
                }
                fence_len = Some(marker_len);
                continue;
            }
        }

        if idx == target_line {
            return fence_len.is_some();
        }
    }

    false
}

pub fn wikilinks(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = text;

    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else {
            break;
        };
        let target = rest[..end]
            .split('|')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        links.push(target);
        rest = &rest[end + 2..];
    }

    links
}

fn path_contains_relative_root(path: &Path, relative_root: &Path) -> bool {
    let needle = relative_root
        .components()
        .filter_map(normal_component)
        .collect::<Vec<_>>();
    if needle.is_empty() {
        return false;
    }

    let haystack = path
        .components()
        .filter_map(normal_component)
        .collect::<Vec<_>>();
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_slice())
}

fn normal_component(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(value) => Some(value.to_string_lossy().to_string()),
        _ => None,
    }
}

fn canonical_section_title_for_doc(
    title: &str,
    label_overrides: &HashMap<String, String>,
    doc_kind: DocKind,
) -> String {
    for (canonical, aliases) in [
        ("Purpose", &["Purpose", "Overview", "概要", "目的"] as &[_]),
        ("Contract", &["Contract", "仕様", "契約"]),
        (
            "Exports",
            &[
                "Exports",
                "API",
                "公開API",
                "Interface",
                "Expose",
                "Exposes",
            ],
        ),
        ("Imports", &["Imports", "Uses"]),
        ("Source", &["Source"]),
        ("Cases", &["Cases", "ケース"]),
        ("Test", &["Test", "Verification", "検証", "テスト"]),
        ("Covers", &["Covers", "対象"]),
        ("Architecture", &["Architecture"]),
        ("Rules", &["Rules"]),
    ] {
        if aliases.iter().any(|alias| title == *alias) {
            return canonical.to_string();
        }
        let key = canonical.to_ascii_lowercase();
        if label_overrides
            .get(&key)
            .is_some_and(|override_label| override_label.trim() == title)
        {
            return canonical.to_string();
        }
    }

    if matches!(title, "Implementation" | "実装") {
        return match doc_kind {
            DocKind::Source => "Source".to_string(),
            DocKind::Test => "Test".to_string(),
        };
    }

    title.to_string()
}

fn backtick_fence(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let count = trimmed
        .chars()
        .take_while(|character| *character == '`')
        .count();
    (count >= 3).then_some((count, &trimmed[count..]))
}

fn is_closing_fence(marker_len: usize, suffix: &str, open_len: usize) -> bool {
    marker_len >= open_len && suffix.trim().is_empty()
}
