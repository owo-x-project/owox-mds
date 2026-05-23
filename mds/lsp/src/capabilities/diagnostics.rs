use std::collections::HashMap;
use std::path::Path;

use mds_core::config::merge_config_text;
use mds_core::descriptor::{
    fence_labels_for_lang, is_overview_markdown_path, is_root_module_markdown_path,
    package_overview_kind_for_path, PackageOverviewKind,
};
use mds_core::diagnostics::RunState;
use mds_core::markdown::{
    doc_profile, extract_all_code_blocks, is_authoring_doc_candidate_path, parse_impl_doc_text,
    source_doc_lang_rejection_from_text, source_markdown_root, validate_link_policy_text,
    validate_markdown_links,
};
use mds_core::model::{CheckDiagnosticPolicy, Config, DocKind, ImplDoc, Lang, Package};
use tower_lsp::lsp_types;

use crate::capabilities::authoring;
use crate::convert::to_lsp_diagnostic;

use crate::state::PackageState;
use crate::state::{
    resolve_active_language_for_text, with_descriptor_root_for_path, WorkspaceState,
};

pub fn validate_impl_md_text(
    path: &Path,
    text: &str,
    config: &Config,
) -> Vec<lsp_types::Diagnostic> {
    validate_impl_md_text_with_state(path, text, config, None)
}

pub fn validate_impl_md_text_with_state(
    path: &Path,
    text: &str,
    config: &Config,
    workspace_state: Option<&WorkspaceState>,
) -> Vec<lsp_types::Diagnostic> {
    with_descriptor_root_for_path(Some(path), workspace_state, || {
        let mut state = RunState::default();
        let package_state =
            workspace_state.and_then(|workspace_state| workspace_state.package_for_path(path));
        let effective_package =
            package_state.map(|package_state| package_with_config(&package_state.package, config));
        if let Some(message) = effective_package
            .as_ref()
            .and_then(|package| source_doc_lang_rejection(path, text, package))
        {
            state.diagnostics.push(mds_core::Diagnostic::error(
                Some(path.to_path_buf()),
                message,
            ));
            return state.diagnostics.iter().map(to_lsp_diagnostic).collect();
        }
        let doc_kind = authoring::doc_kind_for_path(Some(path), config, workspace_state);
        let is_source_overview_doc = is_source_overview_doc_path(path, config, package_state);
        let active_lang = resolve_active_language_for_text(path, text, workspace_state);

        if is_source_overview_doc {
            if is_source_overview_path(path, config, package_state) {
                if let Some(package) = effective_package.as_ref() {
                    mds_core::validate_source_overview_text(package, path, text, &mut state);
                } else {
                    mds_core::validate_source_overview_required_sections(
                        path,
                        text,
                        &config.label_overrides,
                        &mut state,
                    );
                }
            } else {
                mds_core::validate_source_overview_required_sections(
                    path,
                    text,
                    &config.label_overrides,
                    &mut state,
                );
            }
        }
        if is_test_overview_path(path, config, package_state) {
            return Vec::new();
        }

        if config.check.markdown_links {
            validate_markdown_links(path, text, &mut state);
        }

        if let Some(package_state) = package_state {
            let package = effective_package
                .as_ref()
                .expect("effective package should exist when package state exists");
            let docs = link_policy_docs(package_state);
            let doc = package_state
                .index
                .docs
                .get(path)
                .cloned()
                .unwrap_or_else(|| {
                    link_policy_doc(path, text, doc_kind, package, active_lang.clone())
                });
            validate_link_policy_text(package, &docs, &doc, text, &mut state);
        }

        let sections =
            authoring::sections_with_labels_for_doc(text, &config.label_overrides, doc_kind);

        if is_source_overview_doc {
            if config.check.documented_sections {
                validate_overview_documentation_rules(path, &sections, &mut state);
            }

            validate_split_source_and_test(
                path,
                doc_kind,
                sections.get("Source"),
                sections.get("Test"),
                &config.check,
                &mut state,
            );

            if let Some(ref lang) = active_lang {
                validate_code_block_languages(text, lang, path, &mut state);
            }

            return state.diagnostics.iter().map(to_lsp_diagnostic).collect();
        }

        if config.check.documented_sections {
            validate_documented_sections(path, doc_kind, &sections, &mut state);
        }
        if config.check.documented_exports {
            validate_documented_exports(path, text, &sections, &mut state);
            validate_import_documentation(path, &sections, &mut state);
        }

        validate_legacy_table_sections(path, &sections, &config.check, &mut state);
        validate_split_source_and_test(
            path,
            doc_kind,
            sections.get("Source"),
            sections.get("Test"),
            &config.check,
            &mut state,
        );

        if let Some(ref lang) = active_lang {
            validate_code_block_languages(text, lang, path, &mut state);
        }

        state.diagnostics.iter().map(to_lsp_diagnostic).collect()
    })
}

fn package_with_config(package: &Package, config: &Config) -> Package {
    let mut package = package.clone();
    package.config = config.clone();
    package
}

fn source_doc_lang_rejection(path: &Path, text: &str, package: &Package) -> Option<String> {
    if is_overview_markdown_path(path) {
        return None;
    }
    if !path.starts_with(source_markdown_root(package).as_path()) {
        return None;
    }
    if !is_authoring_doc_candidate_path(package, path) {
        return None;
    }

    source_doc_lang_rejection_from_text(
        path,
        text,
        &package.config.label_overrides,
        package.config.check.implementation_section_only,
    )
}

fn link_policy_docs(package_state: &PackageState) -> Vec<ImplDoc> {
    package_state
        .index
        .docs
        .values()
        .filter(|doc| !is_overview_markdown_path(&doc.path))
        .cloned()
        .collect()
}

fn is_source_overview_path(
    path: &Path,
    config: &Config,
    package_state: Option<&PackageState>,
) -> bool {
    matches!(
        package_overview_kind_for_path(
            path,
            package_state.map(|package_state| package_state.package.root.as_path()),
            config,
        ),
        Some(PackageOverviewKind::Source)
    )
}

fn is_source_overview_doc_path(
    path: &Path,
    config: &Config,
    package_state: Option<&PackageState>,
) -> bool {
    if !is_overview_markdown_path(path) {
        return false;
    }

    if let Some(package_state) = package_state {
        return path.starts_with(&package_state.package.root.join(&config.roots.source_md));
    }

    matches!(
        package_overview_kind_for_path(path, None, config),
        Some(PackageOverviewKind::Source)
    )
}

fn is_test_overview_path(
    path: &Path,
    config: &Config,
    package_state: Option<&PackageState>,
) -> bool {
    matches!(
        package_overview_kind_for_path(
            path,
            package_state.map(|package_state| package_state.package.root.as_path()),
            config,
        ),
        Some(PackageOverviewKind::Test)
    )
}

fn link_policy_doc(
    path: &Path,
    text: &str,
    doc_kind: DocKind,
    package: &Package,
    active_lang: Option<Lang>,
) -> ImplDoc {
    let lang = active_lang.unwrap_or_else(|| Lang::Other("md".to_string()));
    let mut scratch = RunState::default();
    if let Some(doc) =
        parse_impl_doc_text(package, doc_kind, lang.clone(), path, text, &mut scratch)
    {
        return doc;
    }

    let package_relative_path = path
        .strip_prefix(&package.root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf());
    ImplDoc {
        doc_kind,
        profile: doc_profile(doc_kind, path, &[], ""),
        lang,
        path: path.to_path_buf(),
        package_relative_path: package_relative_path.clone(),
        markdown_relative_path: package_relative_path,
        code: String::new(),
        source_code: String::new(),
        test_code: String::new(),
        source_blocks: Vec::new(),
        test_blocks: Vec::new(),
        covers: Vec::new(),
        normalized_input: String::new(),
    }
}

fn validate_overview_documentation_rules(
    path: &Path,
    sections: &HashMap<String, String>,
    state: &mut RunState,
) {
    for forbidden in ["Imports", "Exports", "Expose", "Exposes"] {
        if sections.contains_key(forbidden) {
            state.diagnostics.push(mds_core::Diagnostic::error(
                Some(path.to_path_buf()),
                format!("overview md must not contain ## {forbidden}"),
            ));
        }
    }
}

fn validate_documented_sections(
    path: &Path,
    doc_kind: DocKind,
    sections: &HashMap<String, String>,
    state: &mut RunState,
) {
    if !sections.contains_key("Purpose") {
        state.diagnostics.push(mds_core::Diagnostic::error(
            Some(path.to_path_buf()),
            format!("{} md requires ## Purpose", doc_kind.key()),
        ));
    }

    match doc_kind {
        DocKind::Source
            if has_generated_code(sections.get("Source")) && !sections.contains_key("Contract") =>
        {
            state.diagnostics.push(mds_core::Diagnostic::error(
                Some(path.to_path_buf()),
                "source md requires ## Contract",
            ));
        }
        DocKind::Test if has_generated_code(sections.get("Test")) => {
            for section in ["Covers", "Cases"] {
                if !sections.contains_key(section) {
                    state.diagnostics.push(mds_core::Diagnostic::error(
                        Some(path.to_path_buf()),
                        format!("test md requires ## {section}"),
                    ));
                }
            }
        }
        _ => {}
    }
}

fn has_generated_code(section: Option<&String>) -> bool {
    section.is_some_and(|section| !extract_all_code_blocks(section).trim().is_empty())
}

fn has_generated_source(sections: &HashMap<String, String>) -> bool {
    has_generated_code(sections.get("Source"))
}

fn validate_documented_exports(
    path: &Path,
    text: &str,
    sections: &HashMap<String, String>,
    state: &mut RunState,
) {
    let Some(section) = sections
        .get("Exports")
        .or_else(|| sections.get("Expose"))
        .or_else(|| sections.get("Exposes"))
    else {
        return;
    };
    let Some(rows) = table_rows(section) else {
        return;
    };
    let h5 = h5_sections(text);
    for row in rows {
        let name = row
            .get("name")
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        if is_blank_cell(name) {
            continue;
        }
        let summary = row.get("summary").map(String::as_str).unwrap_or_default();
        if is_blank_cell(summary) {
            state.diagnostics.push(mds_core::Diagnostic::error(
                Some(path.to_path_buf()),
                format!("export `{name}` requires a non-empty Summary"),
            ));
        }
        if !is_root_module_markdown_path(path) {
            match h5.get(&slugify_heading(name)) {
                Some(section) if is_blank_cell(&section.prose) => state.diagnostics.push(mds_core::Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("export `{name}` H5 shared definition requires explanatory prose"),
                )),
                Some(section) if has_generated_source(sections) && section.parent_section.as_deref() != Some("Source") => state.diagnostics.push(mds_core::Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("export `{name}` H5 shared definition must be in ## Source before its code block"),
                )),
                Some(section) if has_generated_source(sections) && !section.before_code_block => state.diagnostics.push(mds_core::Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("export `{name}` H5 shared definition must be followed by its Source code block"),
                )),
                Some(_) => {}
                None => state.diagnostics.push(mds_core::Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("export `{name}` requires a matching H5 shared definition"),
                )),
            }
        }
    }
}

fn validate_import_documentation(
    path: &Path,
    sections: &HashMap<String, String>,
    state: &mut RunState,
) {
    let Some(section) = sections.get("Imports").or_else(|| sections.get("Uses")) else {
        return;
    };
    let Some(rows) = table_rows(section) else {
        return;
    };
    for row in rows {
        let from = row
            .get("from")
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        let target = row
            .get("target")
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        let reference = row.get("reference").map(String::as_str).unwrap_or_default();
        if is_blank_cell(from) && is_blank_cell(target) {
            continue;
        }
        let requires_reference = matches!(from, "internal" | "workspace" | "package");
        if requires_reference && is_blank_cell(reference) {
            state.diagnostics.push(mds_core::Diagnostic::error(
                Some(path.to_path_buf()),
                format!("import `{target}` requires a Markdown Reference link"),
            ));
        }
    }
}

fn validate_legacy_table_sections(
    path: &Path,
    sections: &HashMap<String, String>,
    check: &mds_core::model::CheckConfig,
    state: &mut RunState,
) {
    for section_name in ["Imports", "Exports"] {
        let Some(section) = sections.get(section_name) else {
            continue;
        };
        if !authoring::contains_markdown_table(section) {
            continue;
        }
        push_policy_diagnostic(
            check.legacy_tables,
            path,
            format!("legacy table metadata in ## {section_name} is deprecated"),
            state,
        );
    }
}

fn validate_split_source_and_test(
    path: &Path,
    doc_kind: DocKind,
    source_section: Option<&String>,
    test_section: Option<&String>,
    check: &mds_core::model::CheckConfig,
    state: &mut RunState,
) {
    if !check.split_source_and_test {
        return;
    }

    match doc_kind {
        DocKind::Source if has_generated_code(test_section) => {
            state.diagnostics.push(mds_core::Diagnostic::error(
                Some(path.to_path_buf()),
                "source md must not contain generated test code in ## Test",
            ))
        }
        DocKind::Test if has_generated_code(source_section) => {
            state.diagnostics.push(mds_core::Diagnostic::error(
                Some(path.to_path_buf()),
                "test md must not contain generated source code in ## Source",
            ))
        }
        _ => {}
    }
}

fn table_rows(section: &str) -> Option<Vec<HashMap<String, String>>> {
    let lines = section.lines().collect::<Vec<_>>();
    for index in 0..lines.len().saturating_sub(1) {
        if !lines[index].trim_start().starts_with('|') || !lines[index + 1].contains("---") {
            continue;
        }
        let headers = split_row(lines[index])
            .into_iter()
            .map(|header| header.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for row_line in lines.iter().skip(index + 2) {
            if !row_line.trim_start().starts_with('|') {
                break;
            }
            let cells = split_row(row_line);
            let mut row = HashMap::new();
            for (cell_index, header) in headers.iter().enumerate() {
                row.insert(
                    header.clone(),
                    cells.get(cell_index).cloned().unwrap_or_default(),
                );
            }
            rows.push(row);
        }
        return Some(rows);
    }
    None
}

fn split_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

struct H5Section {
    prose: String,
    parent_section: Option<String>,
    before_code_block: bool,
}

fn h5_sections(text: &str) -> HashMap<String, H5Section> {
    let mut sections = HashMap::new();
    let mut current: Option<String> = None;
    let mut current_parent: Option<String> = None;
    let mut body = String::new();
    let mut parent_section: Option<String> = None;
    for line in text.lines() {
        if current.is_none() {
            if let Some(title) = line.strip_prefix("## ") {
                parent_section = Some(title.trim().to_string());
            }
        }
        if let Some(title) = line.strip_prefix("##### ") {
            if let Some(anchor) = current.replace(slugify_heading(title.trim())) {
                sections.insert(anchor, h5_section(&body, current_parent.take()));
                body.clear();
            }
            current_parent = parent_section.clone();
        } else if line.starts_with("## ") || line.starts_with("### ") || line.starts_with("#### ") {
            if let Some(anchor) = current.take() {
                sections.insert(anchor, h5_section(&body, current_parent.take()));
                body.clear();
            }
            if let Some(title) = line.strip_prefix("## ") {
                parent_section = Some(title.trim().to_string());
            }
        } else if current.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(anchor) = current {
        sections.insert(anchor, h5_section(&body, current_parent));
    }
    sections
}

fn h5_section(body: &str, parent_section: Option<String>) -> H5Section {
    H5Section {
        prose: prose_body(body),
        parent_section,
        before_code_block: body
            .lines()
            .map(str::trim_start)
            .any(|line| line.starts_with("```")),
    }
}

fn prose_body(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('|') && !line.starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_blank_cell(value: &str) -> bool {
    let trimmed = value.trim().trim_matches('`');
    trimmed.is_empty() || trimmed == "-"
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

fn push_policy_diagnostic(
    policy: CheckDiagnosticPolicy,
    path: &Path,
    message: impl Into<String>,
    state: &mut RunState,
) {
    let message = message.into();
    match policy {
        CheckDiagnosticPolicy::Warn => state.diagnostics.push(mds_core::Diagnostic::warning(
            Some(path.to_path_buf()),
            message,
        )),
        CheckDiagnosticPolicy::Error => state.diagnostics.push(mds_core::Diagnostic::error(
            Some(path.to_path_buf()),
            message,
        )),
        CheckDiagnosticPolicy::Allow => {}
    }
}

pub fn validate_config_text(path: &Path, text: &str) -> Vec<lsp_types::Diagnostic> {
    with_descriptor_root_for_path(Some(path), None, || {
        let mut state = RunState::default();
        let mut config = Config::default();

        // Try to parse and merge the config
        match text.parse::<toml::Value>() {
            Ok(_value) => {
                merge_config_text(&mut config, path, text, &mut state);
            }
            Err(error) => {
                // Extract line information from TOML parse error
                let msg = format!("TOML parse error: {error}");
                state
                    .diagnostics
                    .push(mds_core::Diagnostic::error(Some(path.to_path_buf()), msg));
            }
        }

        state.diagnostics.iter().map(to_lsp_diagnostic).collect()
    })
}

fn validate_code_block_languages(
    text: &str,
    expected_lang: &Lang,
    path: &Path,
    state: &mut RunState,
) {
    let expected_labels = fence_labels_for_lang(expected_lang);

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(label) = opening_fence_label(trimmed) {
            if !label.is_empty()
                && !expected_labels.contains(&label)
                && label != "text"
                && label != "txt"
                && label != "markdown"
                && label != "md"
                && label != "toml"
                && label != "json"
                && label != "yaml"
                && label != "yml"
                && label != "sh"
                && label != "bash"
                && label != "shell"
            {
                state.diagnostics.push(
                    mds_core::Diagnostic::warning(
                        Some(path.to_path_buf()),
                        format!(
                            "code block language `{label}` may not match file language `{}`",
                            expected_lang.key()
                        ),
                    )
                    .at_line(idx + 1),
                );
            }
        }
    }
}

fn opening_fence_label(trimmed: &str) -> Option<String> {
    let marker_len = trimmed
        .chars()
        .take_while(|character| *character == '`')
        .count();
    if marker_len < 3 {
        return None;
    }
    let label = trimmed[marker_len..]
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    (!label.is_empty()).then_some(label)
}
