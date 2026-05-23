use crate::descriptor::resolve_fence_label;
use crate::descriptor::resolve_markdown_lang;
use crate::descriptor::resolve_markdown_path_lang;
use crate::descriptor::with_workspace_descriptor_root;
use crate::descriptor::FenceLabelResolution;
use crate::descriptor::MarkdownPathLangResolution;
use crate::descriptor::{
    is_overview_markdown_path, is_root_module_markdown_path, markdown_module_id,
    markdown_module_id_for_lang, markdown_module_path_for_lang,
};
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::fs_utils::collect_files;
use crate::fs_utils::is_excluded;
use crate::model::CheckDiagnosticPolicy;
use crate::model::CodeFenceBlock;
use crate::model::DocKind;
use crate::model::DocProfile;
use crate::model::ImplDoc;
use crate::model::Lang;
use crate::model::LinkPolicy;
use crate::model::Package;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum SourceDocLangDiscovery {
    Resolved(Lang),
    NoSignal,
    Rejected(String),
}

pub fn discover_source_doc_lang(package: &Package, path: &Path) -> Option<Lang> {
    match inspect_source_doc_lang(package, path) {
        SourceDocLangDiscovery::Resolved(lang) => Some(lang),
        SourceDocLangDiscovery::NoSignal | SourceDocLangDiscovery::Rejected(_) => None,
    }
}

pub fn discover_source_doc_lang_from_text(
    path: &Path,
    text: &str,
    label_overrides: &HashMap<String, String>,
    implementation_section_only: bool,
) -> Option<Lang> {
    match inspect_source_doc_lang_from_text(
        path,
        text,
        label_overrides,
        implementation_section_only,
    ) {
        SourceDocLangDiscovery::Resolved(lang) => Some(lang),
        SourceDocLangDiscovery::NoSignal | SourceDocLangDiscovery::Rejected(_) => None,
    }
}

pub fn source_doc_lang_rejection_from_text(
    path: &Path,
    text: &str,
    label_overrides: &HashMap<String, String>,
    implementation_section_only: bool,
) -> Option<String> {
    match inspect_source_doc_lang_from_text(
        path,
        text,
        label_overrides,
        implementation_section_only,
    ) {
        SourceDocLangDiscovery::Rejected(message) => Some(message),
        SourceDocLangDiscovery::Resolved(_) | SourceDocLangDiscovery::NoSignal => None,
    }
}

pub(crate) fn inspect_source_doc_lang(package: &Package, path: &Path) -> SourceDocLangDiscovery {
    with_workspace_descriptor_root(Some(&package.root), || {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => return SourceDocLangDiscovery::NoSignal,
        };
        inspect_source_doc_lang_from_text(
            path,
            &text,
            &package.config.label_overrides,
            package.config.check.implementation_section_only,
        )
    })
}

fn inspect_source_doc_lang_from_text(
    path: &Path,
    text: &str,
    label_overrides: &HashMap<String, String>,
    implementation_section_only: bool,
) -> SourceDocLangDiscovery {
    match resolve_markdown_path_lang(path) {
        MarkdownPathLangResolution::Resolved { lang, .. } => {
            return SourceDocLangDiscovery::Resolved(lang);
        }
        MarkdownPathLangResolution::Ambiguous(ids) => {
            return SourceDocLangDiscovery::Rejected(format!(
                "source md path suffix resolves ambiguously across languages: {}",
                format_language_keys(&ids)
            ));
        }
        MarkdownPathLangResolution::Unknown => {}
    }

    let fence_labels = code_fence_labels_by_section_for_doc(text, label_overrides, DocKind::Source);
    let source_labels = source_section_fence_labels(&fence_labels);
    let source_resolution = inspect_source_doc_lang_from_fence_labels(&source_labels);
    match source_resolution {
        SourceDocLangFenceDiscovery::Resolved(lang) => {
            return SourceDocLangDiscovery::Resolved(lang);
        }
        SourceDocLangFenceDiscovery::Ambiguous(labels) => {
            return SourceDocLangDiscovery::Rejected(format!(
                "source md Source fence labels resolve ambiguously: {}",
                format_fence_labels(&labels)
            ));
        }
        SourceDocLangFenceDiscovery::Unknown(labels) if implementation_section_only => {
            return SourceDocLangDiscovery::Rejected(format!(
                "source md Source fence labels do not resolve to a known language: {}",
                format_fence_labels(&labels)
            ));
        }
        SourceDocLangFenceDiscovery::NoSignal | SourceDocLangFenceDiscovery::Unknown(_) => {}
    }

    if implementation_section_only {
        return SourceDocLangDiscovery::NoSignal;
    }

    let relevant_labels = relevant_source_doc_fence_labels(&fence_labels);
    match inspect_source_doc_lang_from_fence_labels(&relevant_labels) {
        SourceDocLangFenceDiscovery::Resolved(lang) => SourceDocLangDiscovery::Resolved(lang),
        SourceDocLangFenceDiscovery::NoSignal => match source_resolution {
            SourceDocLangFenceDiscovery::Unknown(labels) => {
                SourceDocLangDiscovery::Rejected(format!(
                    "source md Source fence labels do not resolve to a known language: {}",
                    format_fence_labels(&labels)
                ))
            }
            SourceDocLangFenceDiscovery::Resolved(_)
            | SourceDocLangFenceDiscovery::NoSignal
            | SourceDocLangFenceDiscovery::Ambiguous(_) => SourceDocLangDiscovery::NoSignal,
        },
        SourceDocLangFenceDiscovery::Unknown(labels) => {
            let has_noncanonical_labels = has_noncanonical_source_doc_fence_labels(&fence_labels);
            if let SourceDocLangFenceDiscovery::Unknown(source_labels) = source_resolution {
                if !has_noncanonical_labels {
                    return SourceDocLangDiscovery::Rejected(format!(
                        "source md Source fence labels do not resolve to a known language: {}",
                        format_fence_labels(&source_labels)
                    ));
                }
            }
            SourceDocLangDiscovery::Rejected(format!(
                "source md relevant fence labels do not resolve to a known language: {}",
                format_fence_labels(&labels)
            ))
        }
        SourceDocLangFenceDiscovery::Ambiguous(labels) => {
            SourceDocLangDiscovery::Rejected(format!(
                "source md relevant fence labels resolve ambiguously: {}",
                format_fence_labels(&labels)
            ))
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum SourceDocLangFenceDiscovery {
    Resolved(Lang),
    NoSignal,
    Unknown(Vec<String>),
    Ambiguous(Vec<String>),
}

fn source_section_fence_labels(fence_labels: &HashMap<String, Vec<String>>) -> Vec<String> {
    fence_labels.get("Source").cloned().unwrap_or_default()
}

fn relevant_source_doc_fence_labels(fence_labels: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut sections = fence_labels
        .iter()
        .filter(|(section, _)| section.as_str() != "Test")
        .collect::<Vec<_>>();
    sections.sort_by(|(left, _), (right, _)| left.cmp(right));
    sections
        .into_iter()
        .flat_map(|(_, labels)| labels.iter().cloned())
        .collect()
}

fn has_noncanonical_source_doc_fence_labels(fence_labels: &HashMap<String, Vec<String>>) -> bool {
    fence_labels.iter().any(|(section, labels)| {
        section.as_str() != "Source"
            && section.as_str() != "Test"
            && labels
                .iter()
                .map(String::as_str)
                .map(str::trim)
                .any(|label| !label.is_empty())
    })
}

fn inspect_source_doc_lang_from_fence_labels(labels: &[String]) -> SourceDocLangFenceDiscovery {
    let labels = labels
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    if labels.is_empty() {
        return SourceDocLangFenceDiscovery::NoSignal;
    }

    let mut resolved_lang = None;
    let mut resolved_labels = Vec::new();
    let mut ambiguous_labels = Vec::new();
    let mut unknown_labels = Vec::new();
    for label in labels {
        match resolve_fence_label(label) {
            FenceLabelResolution::Resolved(candidate) => {
                resolved_labels.push(label.to_string());
                match &resolved_lang {
                    None => resolved_lang = Some(candidate),
                    Some(current) if *current == candidate => {}
                    Some(_) => ambiguous_labels.push(label.to_string()),
                }
            }
            FenceLabelResolution::Unknown => unknown_labels.push(label.to_string()),
            FenceLabelResolution::Ambiguous(_) => ambiguous_labels.push(label.to_string()),
        }
    }

    if !ambiguous_labels.is_empty() {
        resolved_labels.extend(ambiguous_labels);
        normalize_fence_labels(&mut resolved_labels);
        return SourceDocLangFenceDiscovery::Ambiguous(resolved_labels);
    }

    if let Some(lang) = resolved_lang {
        return SourceDocLangFenceDiscovery::Resolved(lang);
    }

    normalize_fence_labels(&mut unknown_labels);
    SourceDocLangFenceDiscovery::Unknown(unknown_labels)
}

pub fn load_implementation_docs(
    package: &Package,
    state: &mut RunState,
) -> Result<Vec<ImplDoc>, String> {
    with_workspace_descriptor_root(Some(&package.root), || {
        let markdown_root = source_markdown_root(package);
        if !markdown_root.exists() {
            state.diagnostics.push(Diagnostic::error(
                Some(markdown_root),
                "markdown root does not exist",
            ));
            return Ok(Vec::new());
        }

        let mut docs = Vec::new();
        for path in collect_files(&markdown_root, false)? {
            if is_excluded(&package.root, &path, &package.config.excludes) {
                continue;
            }
            if is_template_asset_markdown(&path) {
                continue;
            }
            let lang = match inspect_source_doc_lang(package, &path) {
                SourceDocLangDiscovery::Resolved(lang) => lang,
                SourceDocLangDiscovery::NoSignal => continue,
                SourceDocLangDiscovery::Rejected(message) => {
                    state
                        .diagnostics
                        .push(Diagnostic::error(Some(path.clone()), message));
                    continue;
                }
            };
            if !package.config.adapters.get(&lang).copied().unwrap_or(true) {
                continue;
            }
            if let Some(doc) = parse_impl_doc(package, DocKind::Source, lang, &path, state) {
                docs.push(doc);
            }
        }
        docs.extend(load_test_docs(package, &docs, state)?);
        validate_wiki_link_targets(package, &docs, state);
        docs.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(docs)
    })
}

pub fn load_workspace_index_docs(
    package: &Package,
    state: &mut RunState,
) -> Result<Vec<ImplDoc>, String> {
    with_workspace_descriptor_root(Some(&package.root), || {
        let mut docs = load_implementation_docs(package, state)?;
        docs.extend(load_overview_docs(
            package,
            DocKind::Source,
            &source_markdown_root(package),
            state,
        )?);
        docs.extend(load_overview_docs(
            package,
            DocKind::Test,
            &test_markdown_root(package),
            state,
        )?);
        docs.sort_by(|left, right| left.path.cmp(&right.path));
        docs.dedup_by(|left, right| left.path == right.path);
        Ok(docs)
    })
}

pub fn is_authoring_doc_candidate_path(package: &Package, path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
        && !is_excluded(&package.root, path, &package.config.excludes)
        && !is_template_asset_markdown(path)
        && (path.starts_with(&source_markdown_root(package))
            || path.starts_with(&test_markdown_root(package)))
}

pub fn is_discoverable_authoring_doc_path(package: &Package, path: &Path) -> bool {
    if !is_authoring_doc_candidate_path(package, path) {
        return false;
    }

    if path.starts_with(&source_markdown_root(package)) {
        return is_overview_markdown_path(path)
            || match inspect_source_doc_lang(package, path) {
                SourceDocLangDiscovery::Resolved(lang) => {
                    package.config.adapters.get(&lang).copied().unwrap_or(true)
                }
                SourceDocLangDiscovery::NoSignal | SourceDocLangDiscovery::Rejected(_) => false,
            };
    }

    path.starts_with(&test_markdown_root(package))
        && (is_overview_markdown_path(path) || is_test_doc(path))
}

fn load_overview_docs(
    package: &Package,
    doc_kind: DocKind,
    markdown_root: &Path,
    state: &mut RunState,
) -> Result<Vec<ImplDoc>, String> {
    if !markdown_root.exists() {
        return Ok(Vec::new());
    }

    let mut docs = Vec::new();
    for path in collect_files(markdown_root, false)? {
        if !is_overview_markdown_path(&path) {
            continue;
        }
        if !is_discoverable_authoring_doc_path(package, &path) {
            continue;
        }
        if let Some(doc) = parse_impl_doc(
            package,
            doc_kind,
            Lang::Other("md".to_string()),
            &path,
            state,
        ) {
            docs.push(doc);
        }
    }
    Ok(docs)
}

fn load_test_docs(
    package: &Package,
    source_docs: &[ImplDoc],
    state: &mut RunState,
) -> Result<Vec<ImplDoc>, String> {
    let markdown_root = test_markdown_root(package);
    if !markdown_root.exists() {
        return Ok(Vec::new());
    }

    let mut docs = Vec::new();
    for path in collect_files(&markdown_root, false)? {
        if is_excluded(&package.root, &path, &package.config.excludes) || !is_test_doc(&path) {
            continue;
        }
        let Some(lang) = resolve_test_doc_lang(package, &path, source_docs, state) else {
            continue;
        };
        if let Some(doc) = parse_impl_doc(package, DocKind::Test, lang, &path, state) {
            docs.push(doc);
        }
    }
    Ok(docs)
}

fn is_template_asset_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
        && path
            .components()
            .any(|component| component.as_os_str() == "templates")
}

pub fn parse_impl_doc(
    package: &Package,
    doc_kind: DocKind,
    lang: Lang,
    path: &Path,
    state: &mut RunState,
) -> Option<ImplDoc> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to read implementation md: {error}"),
            ));
            return None;
        }
    };

    parse_impl_doc_text(package, doc_kind, lang, path, &text, state)
}

pub fn parse_impl_doc_text(
    package: &Package,
    doc_kind: DocKind,
    lang: Lang,
    path: &Path,
    text: &str,
    state: &mut RunState,
) -> Option<ImplDoc> {
    validate_doc_text(package, doc_kind, &lang, path, &text, state);

    let sections = sections_with_labels_for_doc(&text, &package.config.label_overrides, doc_kind);
    let section_blocks =
        code_fence_blocks_by_section_for_doc(&text, &package.config.label_overrides, doc_kind);
    let source_blocks = section_blocks.get("Source").cloned().unwrap_or_default();
    let test_blocks = section_blocks.get("Test").cloned().unwrap_or_default();
    let source_code = if matches!(doc_kind, DocKind::Source) {
        generated_code_for_doc(
            doc_kind,
            sections.get("Source"),
            &source_blocks,
            &section_blocks,
            &package.config.check,
        )
    } else {
        String::new()
    };
    let test_code = if matches!(doc_kind, DocKind::Test) {
        generated_code_for_doc(
            doc_kind,
            sections.get("Test"),
            &test_blocks,
            &section_blocks,
            &package.config.check,
        )
    } else {
        String::new()
    };
    let covers = covers_from_section(package, path, sections.get("Covers"));

    let code = extract_all_code_blocks(&text);
    let profile = doc_profile(doc_kind, path, &source_blocks, &source_code);

    let package_relative_path = match path.strip_prefix(&package.root) {
        Ok(path) => path.to_path_buf(),
        Err(_) => path.to_path_buf(),
    };
    let markdown_relative_path = match path.strip_prefix(markdown_root_for(package, doc_kind)) {
        Ok(path) => path.to_path_buf(),
        Err(_) => path.to_path_buf(),
    };
    let normalized_input = normalized_input(path, &text);

    Some(ImplDoc {
        doc_kind,
        profile,
        lang,
        path: path.to_path_buf(),
        package_relative_path,
        markdown_relative_path,
        code,
        source_code,
        test_code,
        source_blocks,
        test_blocks,
        covers,
        normalized_input,
    })
}

fn validate_doc_text(
    package: &Package,
    doc_kind: DocKind,
    lang: &Lang,
    path: &Path,
    text: &str,
    state: &mut RunState,
) -> Vec<String> {
    validate_impl_doc_structure(
        path,
        doc_kind,
        text,
        &package.config.label_overrides,
        &package.config.check,
        state,
    );
    if package.config.check.markdown_links {
        validate_markdown_links(path, text, state);
    }
    validate_code_block_boundaries(
        path,
        lang,
        text,
        &package.config.label_overrides,
        &package.config.check,
        state,
    );

    let sections = sections_with_labels_for_doc(text, &package.config.label_overrides, doc_kind);
    let section_blocks =
        code_fence_blocks_by_section_for_doc(text, &package.config.label_overrides, doc_kind);
    let source_blocks = section_blocks.get("Source").cloned().unwrap_or_default();
    let test_blocks = section_blocks.get("Test").cloned().unwrap_or_default();
    validate_legacy_table_sections(path, &sections, &package.config.check, state);
    validate_split_source_and_test(
        path,
        doc_kind,
        sections.get("Source"),
        &source_blocks,
        sections.get("Test"),
        &test_blocks,
        &package.config.check,
        state,
    );
    let source_code = if matches!(doc_kind, DocKind::Source) {
        generated_code_for_doc(
            doc_kind,
            sections.get("Source"),
            &source_blocks,
            &section_blocks,
            &package.config.check,
        )
    } else {
        String::new()
    };
    let covers = covers_from_section(package, path, sections.get("Covers"));

    let code = extract_all_code_blocks(text);
    let profile = doc_profile(doc_kind, path, &source_blocks, &source_code);
    if package.config.check.documented_sections {
        validate_documented_sections(path, profile, &sections, state);
    }
    if package.config.check.documented_exports {
        validate_documented_exports(path, profile, &sections, text, state);
        validate_documented_imports(path, &sections, state);
    }

    if package.config.check.code_blocks_required
        && code.trim().is_empty()
        && requires_code_block(profile)
    {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            "implementation md requires at least one code block",
        ));
    }

    covers
}

pub(crate) fn validate_fix_result_text(
    package: &Package,
    docs: &[ImplDoc],
    doc: &ImplDoc,
    text: &str,
    state: &mut RunState,
) {
    let covers = validate_doc_text(package, doc.doc_kind, &doc.lang, &doc.path, text, state);
    let mut candidate_doc = doc.clone();
    let sections =
        sections_with_labels_for_doc(text, &package.config.label_overrides, doc.doc_kind);
    let section_blocks =
        code_fence_blocks_by_section_for_doc(text, &package.config.label_overrides, doc.doc_kind);
    let source_blocks = section_blocks.get("Source").cloned().unwrap_or_default();
    let source_code = if matches!(doc.doc_kind, DocKind::Source) {
        generated_code_for_doc(
            doc.doc_kind,
            sections.get("Source"),
            &source_blocks,
            &section_blocks,
            &package.config.check,
        )
    } else {
        String::new()
    };
    candidate_doc.covers = covers;
    candidate_doc.profile = doc_profile(doc.doc_kind, &doc.path, &source_blocks, &source_code);
    candidate_doc.normalized_input = normalized_input(&candidate_doc.path, text);
    validate_link_policy_text(package, docs, &candidate_doc, text, state);
}

fn is_module_root_metadata_doc(path: &Path) -> bool {
    is_root_module_markdown_path(path)
}

pub fn doc_profile(
    doc_kind: DocKind,
    path: &Path,
    source_blocks: &[CodeFenceBlock],
    source_code: &str,
) -> DocProfile {
    if is_overview_markdown_path(path) {
        return DocProfile::Overview;
    }
    match doc_kind {
        DocKind::Test => DocProfile::Test,
        DocKind::Source => {
            if !source_blocks.is_empty() || !source_code.trim().is_empty() {
                DocProfile::Impl
            } else {
                DocProfile::Spec
            }
        }
    }
}

fn normalize_fence_labels(labels: &mut Vec<String>) {
    labels.sort();
    labels.dedup();
}

fn format_fence_labels(labels: &[String]) -> String {
    labels
        .iter()
        .map(|label| format!("`{label}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_language_keys(ids: &[String]) -> String {
    ids.iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn requires_code_block(profile: DocProfile) -> bool {
    matches!(profile, DocProfile::Impl | DocProfile::Test)
}

fn validate_legacy_table_sections(
    path: &Path,
    sections: &HashMap<String, String>,
    check: &crate::model::CheckConfig,
    state: &mut RunState,
) {
    for section_name in ["Imports", "Exports"] {
        let Some(section) = sections.get(section_name) else {
            continue;
        };
        if !contains_markdown_table(section) {
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
    source_blocks: &[CodeFenceBlock],
    test_section: Option<&String>,
    test_blocks: &[CodeFenceBlock],
    check: &crate::model::CheckConfig,
    state: &mut RunState,
) {
    if !check.split_source_and_test {
        return;
    }

    match doc_kind {
        DocKind::Source if section_has_generated_code(test_section, test_blocks) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "source md must not contain generated test code in ## Test",
            ));
        }
        DocKind::Test if section_has_generated_code(source_section, source_blocks) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "test md must not contain generated source code in ## Source",
            ));
        }
        _ => {}
    }
}

fn section_has_generated_code(section: Option<&String>, blocks: &[CodeFenceBlock]) -> bool {
    !code_from_section(section, blocks).trim().is_empty()
}

fn generated_code_for_doc(
    doc_kind: DocKind,
    canonical_section: Option<&String>,
    canonical_blocks: &[CodeFenceBlock],
    section_blocks: &HashMap<String, Vec<CodeFenceBlock>>,
    check: &crate::model::CheckConfig,
) -> String {
    let canonical_code = code_from_section(canonical_section, canonical_blocks);
    if check.implementation_section_only {
        return canonical_code;
    }

    let extra_blocks = noncanonical_output_blocks(doc_kind, section_blocks);
    let extra_code = code_from_fence_blocks(&extra_blocks);
    join_code_segments([canonical_code, extra_code])
}

fn noncanonical_output_blocks(
    doc_kind: DocKind,
    section_blocks: &HashMap<String, Vec<CodeFenceBlock>>,
) -> Vec<CodeFenceBlock> {
    let mut blocks = section_blocks
        .iter()
        .filter(|(section, _)| match doc_kind {
            DocKind::Source => section.as_str() != "Source" && section.as_str() != "Test",
            DocKind::Test => section.as_str() != "Test" && section.as_str() != "Source",
        })
        .flat_map(|(_, blocks)| blocks.iter().cloned())
        .collect::<Vec<_>>();
    blocks.sort_by_key(|block| block.fence_index);
    blocks
}

fn join_code_segments<const N: usize>(segments: [String; N]) -> String {
    let segments = segments
        .into_iter()
        .map(|segment| segment.trim_end_matches('\n').to_string())
        .filter(|segment| !segment.trim().is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        String::new()
    } else {
        segments.join("\n\n") + "\n"
    }
}

fn validate_documented_sections(
    path: &Path,
    profile: DocProfile,
    sections: &HashMap<String, String>,
    state: &mut RunState,
) {
    let required: &[&str] = match profile {
        DocProfile::Overview => &["Purpose", "Architecture", "Rules"],
        DocProfile::Spec => &["Contract"],
        DocProfile::Impl => &["Contract"],
        DocProfile::Test => &["Covers", "Cases"],
    };
    for section in required {
        if !sections.contains_key(*section) {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("{:?} md requires ## {section}", profile).to_ascii_lowercase(),
            ));
        }
    }
    if profile == DocProfile::Overview {
        for forbidden in ["Imports", "Exports", "Expose", "Exposes"] {
            if sections.contains_key(forbidden) {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("overview md must not contain ## {forbidden}"),
                ));
            }
        }
    }
}

fn validate_documented_exports(
    path: &Path,
    profile: DocProfile,
    sections: &HashMap<String, String>,
    text: &str,
    state: &mut RunState,
) {
    if profile == DocProfile::Overview {
        return;
    }
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
    let h5_sections = h5_sections(text);
    for row in rows {
        let Some(name) = row.get("name").map(String::as_str).map(str::trim) else {
            continue;
        };
        if is_blank_cell(name) {
            continue;
        }
        let summary = row.get("summary").map(String::as_str).unwrap_or_default();
        if is_blank_cell(summary) {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("export `{name}` requires a non-empty Summary"),
            ));
        }
        if !matches!(profile, DocProfile::Spec | DocProfile::Impl)
            || is_module_root_metadata_doc(path)
        {
            continue;
        }
        let anchor = slugify_heading(name);
        match h5_sections.get(&anchor) {
            Some(section) if section.prose.trim().is_empty() => state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("export `{name}` H5 shared definition requires explanatory prose"),
            )),
            Some(section) if profile == DocProfile::Impl && section.parent_section.as_deref() != Some("Source") => state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("export `{name}` H5 shared definition must be in ## Source before its code block"),
            )),
            Some(section) if profile == DocProfile::Impl && !section.before_code_block => state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("export `{name}` H5 shared definition must be followed by its Source code block"),
            )),
            Some(_) => {}
            None => state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("export `{name}` requires a matching H5 shared definition"),
            )),
        }
    }
}

fn validate_documented_imports(
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
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("import `{target}` requires a Markdown Reference link"),
            ));
        }
    }
}

fn table_rows(section: &str) -> Option<Vec<HashMap<String, String>>> {
    let lines = section.lines().collect::<Vec<_>>();
    for index in 0..lines.len().saturating_sub(1) {
        if !lines[index].trim_start().starts_with('|') || !lines[index + 1].contains("---") {
            continue;
        }
        let headers = split_markdown_row(lines[index])
            .into_iter()
            .map(|header| header.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for row_line in lines.iter().skip(index + 2) {
            if !row_line.trim_start().starts_with('|') {
                break;
            }
            let cells = split_markdown_row(row_line);
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
            if let Some(name) = current.replace(slugify_heading(title.trim())) {
                sections.insert(name, h5_section(&body, current_parent.take()));
                body.clear();
            }
            current_parent = parent_section.clone();
        } else if line.starts_with("## ") || line.starts_with("### ") || line.starts_with("#### ") {
            if let Some(name) = current.take() {
                sections.insert(name, h5_section(&body, current_parent.take()));
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
    if let Some(name) = current {
        sections.insert(name, h5_section(&body, current_parent));
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
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('|')
                && !line.starts_with("```")
                && !line.starts_with("````")
        })
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

pub fn source_markdown_root(package: &Package) -> PathBuf {
    package.root.join(&package.config.roots.source_md)
}

pub fn test_markdown_root(package: &Package) -> PathBuf {
    package.root.join(&package.config.roots.test_md)
}

fn markdown_root_for(package: &Package, doc_kind: DocKind) -> PathBuf {
    match doc_kind {
        DocKind::Source => source_markdown_root(package),
        DocKind::Test => test_markdown_root(package),
    }
}

#[derive(Debug, Default)]
struct ModuleReferenceIndex {
    symbols: HashSet<String>,
}

pub(crate) fn validate_wiki_link_targets(
    package: &Package,
    docs: &[ImplDoc],
    state: &mut RunState,
) {
    if !package.config.check.markdown_links {
        return;
    }

    let module_index = build_module_reference_index(package, docs, state);
    for doc in docs {
        let text = match fs::read_to_string(&doc.path) {
            Ok(text) => text,
            Err(error) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(doc.path.clone()),
                    format!("failed to read implementation md: {error}"),
                ));
                continue;
            }
        };
        validate_wiki_link_targets_for_doc_text(package, doc, &text, &module_index, state);
    }
}

fn build_module_reference_index(
    package: &Package,
    docs: &[ImplDoc],
    state: &mut RunState,
) -> HashMap<String, Vec<ModuleReferenceIndex>> {
    let mut module_index = HashMap::new();
    for doc in docs {
        let text = match fs::read_to_string(&doc.path) {
            Ok(text) => text,
            Err(error) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(doc.path.clone()),
                    format!("failed to read implementation md: {error}"),
                ));
                continue;
            }
        };
        let symbols = documented_symbols(&text, &package.config.label_overrides);
        module_index
            .entry(logical_module_id_for_doc(doc))
            .or_insert_with(Vec::new)
            .push(ModuleReferenceIndex { symbols });
    }
    module_index
}

fn validate_package_link_target(
    path: &Path,
    target: &str,
    rendered_target: &str,
    kind: &str,
    module_index: &HashMap<String, Vec<ModuleReferenceIndex>>,
    check: &crate::model::CheckConfig,
    state: &mut RunState,
) {
    let Some((module_id, symbol)) = target.split_once('#') else {
        match module_index.get(target).map(Vec::as_slice) {
            Some([_]) => {}
            Some([]) | None => state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("{kind} target `{rendered_target}` does not resolve to a module"),
            )),
            Some(_) => state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("{kind} target `{rendered_target}` resolves ambiguously"),
            )),
        }
        return;
    };

    let symbol = symbol.trim();
    let symbol_resolves = module_index
        .get(module_id)
        .and_then(|entries| match entries.as_slice() {
            [entry] => Some(symbol_matches(&entry.symbols, symbol)),
            _ => None,
        })
        .unwrap_or(false);
    if symbol_resolves {
        return;
    }

    push_policy_diagnostic(
        check.unresolved_module_symbols,
        path,
        format!("{kind} target `{rendered_target}` does not resolve to a documented symbol"),
        state,
    );
}

fn symbol_matches(symbols: &HashSet<String>, symbol: &str) -> bool {
    symbols.contains(symbol) || symbols.contains(&slugify_heading(symbol))
}

fn documented_symbols(text: &str, label_overrides: &HashMap<String, String>) -> HashSet<String> {
    let mut symbols = HashSet::new();
    for heading in h5_symbol_headings(text) {
        insert_symbol_variants(&mut symbols, &heading);
    }

    let sections = sections_with_labels(text, label_overrides);
    if let Some(section) = sections.get("Exports") {
        if let Some(rows) = table_rows(section) {
            for row in rows {
                if let Some(name) = row.get("name") {
                    insert_symbol_variants(&mut symbols, name);
                }
            }
        }
    }

    for code_span in inline_code_spans(&text_without_code_blocks(text)) {
        insert_symbol_variants(&mut symbols, &code_span);
    }

    symbols
}

fn h5_symbol_headings(text: &str) -> Vec<String> {
    let mut headings = Vec::new();
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
            if let Some(title) = line.strip_prefix("##### ") {
                let title = title.trim();
                if !title.is_empty() {
                    headings.push(title.to_string());
                }
            }
        }
    }
    headings
}

fn inline_code_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('`') else {
            break;
        };
        let value = rest[..end].trim();
        if !value.is_empty() {
            spans.push(value.to_string());
        }
        rest = &rest[end + 1..];
    }
    spans
}

fn insert_symbol_variants(symbols: &mut HashSet<String>, value: &str) {
    let value = value.trim().trim_matches('`');
    if value.is_empty() {
        return;
    }
    symbols.insert(value.to_string());
    symbols.insert(slugify_heading(value));
}

fn contains_markdown_table(section: &str) -> bool {
    let lines = section.lines().collect::<Vec<_>>();
    for index in 0..lines.len().saturating_sub(1) {
        if lines[index].trim_start().starts_with('|') && lines[index + 1].contains("---") {
            return true;
        }
    }
    false
}

fn push_policy_diagnostic(
    policy: CheckDiagnosticPolicy,
    path: &Path,
    message: impl Into<String>,
    state: &mut RunState,
) {
    let message = message.into();
    match policy {
        CheckDiagnosticPolicy::Warn => state
            .diagnostics
            .push(Diagnostic::warning(Some(path.to_path_buf()), message)),
        CheckDiagnosticPolicy::Error => state
            .diagnostics
            .push(Diagnostic::error(Some(path.to_path_buf()), message)),
        CheckDiagnosticPolicy::Allow => {}
    }
}

fn is_test_doc(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("md"))
        && !is_overview_markdown_path(path)
}

pub fn discover_test_doc_lang_from_text(
    package: &Package,
    path: &Path,
    text: &str,
    source_docs: &[ImplDoc],
    state: &mut RunState,
) -> Option<Lang> {
    with_workspace_descriptor_root(Some(&package.root), || {
        let sections =
            sections_with_labels_for_doc(&text, &package.config.label_overrides, DocKind::Test);
        let fence_labels = code_fence_labels_by_section_for_doc(
            &text,
            &package.config.label_overrides,
            DocKind::Test,
        );
        let covers = covers_from_section(package, path, sections.get("Covers"));
        if covers.is_empty() {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "test md requires at least one Covers entry",
            ));
            return None;
        }

        let preferred_lang = fence_labels
            .get("Test")
            .cloned()
            .filter(|labels| !labels.is_empty())
            .or_else(|| {
                let labels = fence_labels
                    .values()
                    .flat_map(|labels| labels.iter().cloned())
                    .collect::<Vec<_>>();
                (!labels.is_empty()).then_some(labels)
            })
            .and_then(|labels| resolve_markdown_lang(path, labels.iter().map(String::as_str)));

        let mut lang = None;
        for cover in covers {
            let matches = source_docs
                .iter()
                .filter(|doc| cover_matches(doc, &cover))
                .collect::<Vec<_>>();
            let preferred_matches = preferred_lang
                .as_ref()
                .map(|preferred| {
                    matches
                        .iter()
                        .copied()
                        .filter(|doc| &doc.lang == preferred)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let matches = if preferred_matches.is_empty() {
                matches
            } else {
                preferred_matches
            };
            match matches.as_slice() {
                [] => {
                    state.diagnostics.push(Diagnostic::error(
                        Some(path.to_path_buf()),
                        format!("test md Covers entry `{cover}` does not resolve to a source md"),
                    ));
                    return None;
                }
                [doc] => {
                    if let Some(current) = &lang {
                        if current != &doc.lang {
                            state.diagnostics.push(Diagnostic::error(
                                Some(path.to_path_buf()),
                                "test md Covers entries must resolve to source docs of the same language",
                            ));
                            return None;
                        }
                    } else {
                        lang = Some(doc.lang.clone());
                    }
                }
                _ => {
                    state.diagnostics.push(Diagnostic::error(
                        Some(path.to_path_buf()),
                        format!("test md Covers entry `{cover}` resolves ambiguously"),
                    ));
                    return None;
                }
            }
        }

        lang
    })
}

fn resolve_test_doc_lang(
    package: &Package,
    path: &Path,
    source_docs: &[ImplDoc],
    state: &mut RunState,
) -> Option<Lang> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to read test md: {error}"),
            ));
            return None;
        }
    };
    discover_test_doc_lang_from_text(package, path, &text, source_docs, state)
}

fn cover_matches(doc: &ImplDoc, cover: &str) -> bool {
    let cover = cover.trim();
    let cover = cover.split('#').next().unwrap_or(&cover).trim();
    cover == logical_module_id_for_doc(doc)
        || cover == legacy_slash_module_id_for_doc(doc)
        || cover == doc.markdown_relative_path.to_string_lossy()
}

fn logical_module_id_for_doc(doc: &ImplDoc) -> String {
    let lang = match resolve_markdown_path_lang(&doc.path) {
        MarkdownPathLangResolution::Resolved { lang, .. } => lang,
        MarkdownPathLangResolution::Unknown | MarkdownPathLangResolution::Ambiguous(_) => {
            doc.lang.clone()
        }
    };
    markdown_module_id_for_lang(&lang, &doc.markdown_relative_path)
}

fn legacy_slash_module_id_for_doc(doc: &ImplDoc) -> String {
    let lang = match resolve_markdown_path_lang(&doc.path) {
        MarkdownPathLangResolution::Resolved { lang, .. } => lang,
        MarkdownPathLangResolution::Unknown | MarkdownPathLangResolution::Ambiguous(_) => {
            doc.lang.clone()
        }
    };
    markdown_module_path_for_lang(&lang, &doc.markdown_relative_path)
}

fn logical_module_id(path: &Path) -> String {
    markdown_module_id(path)
}

fn validate_impl_doc_structure(
    path: &Path,
    doc_kind: DocKind,
    text: &str,
    label_overrides: &HashMap<String, String>,
    check: &crate::model::CheckConfig,
    state: &mut RunState,
) {
    if check.code_fence_integrity {
        validate_code_fence_integrity(path, text, state);
    }
    if check.duplicate_h2_sections {
        validate_duplicate_h2_sections(path, doc_kind, text, label_overrides, state);
    }
}

fn validate_code_fence_integrity(path: &Path, text: &str, state: &mut RunState) {
    let mut fence_len: Option<usize> = None;
    let mut fence_start_line = 0usize;
    for (line_index, line) in text.lines().enumerate() {
        let Some((marker_len, suffix)) = backtick_fence(line) else {
            continue;
        };
        if let Some(open_len) = fence_len {
            if is_closing_fence(marker_len, suffix, open_len) {
                fence_len = None;
                fence_start_line = 0;
            } else if marker_len >= open_len {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!(
                        "code fence opened at line {} is not closed before a new fence opener at line {}",
                        fence_start_line,
                        line_index + 1
                    ),
                ));
                fence_len = Some(marker_len);
                fence_start_line = line_index + 1;
            }
        } else {
            fence_len = Some(marker_len);
            fence_start_line = line_index + 1;
        }
    }
    if fence_len.is_some() {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("unterminated code fence opened at line {fence_start_line}"),
        ));
    }
}

fn validate_duplicate_h2_sections(
    path: &Path,
    doc_kind: DocKind,
    text: &str,
    label_overrides: &HashMap<String, String>,
    state: &mut RunState,
) {
    let mut first_seen = HashMap::new();
    let mut fence_len: Option<usize> = None;
    for (line_index, line) in text.lines().enumerate() {
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
        if fence_len.is_some() {
            continue;
        }
        let Some(title) = line.strip_prefix("## ") else {
            continue;
        };
        let canonical =
            canonical_section_title_for_doc(title.trim(), label_overrides, Some(doc_kind));
        if let Some(first_line) = first_seen.insert(canonical.clone(), line_index + 1) {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!(
                    "duplicate H2 section `{canonical}`: first defined at line {first_line}, repeated at line {}",
                    line_index + 1
                ),
            ));
        }
    }
}

pub fn extract_all_code_blocks(text: &str) -> String {
    let mut fence_len: Option<usize> = None;
    let mut current = String::new();
    let mut blocks = Vec::new();
    for line in text.lines() {
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if !is_closing_fence(marker_len, suffix, open_len) {
                    current.push_str(line);
                    current.push('\n');
                    continue;
                }
                blocks.push(current.trim_end_matches(['\r', '\n']).to_string());
                current.clear();
                fence_len = None;
            } else {
                fence_len = Some(marker_len);
            }
            continue;
        }
        if fence_len.is_some() {
            current.push_str(line);
            current.push('\n');
        }
    }
    if blocks.is_empty() {
        String::new()
    } else {
        blocks.join("\n\n") + "\n"
    }
}

fn code_fence_blocks_by_section_for_doc(
    text: &str,
    label_overrides: &HashMap<String, String>,
    doc_kind: DocKind,
) -> HashMap<String, Vec<CodeFenceBlock>> {
    let mut result = HashMap::new();
    let mut current_section: Option<String> = None;
    let mut fence_len: Option<usize> = None;
    let mut next_fence_index = 0usize;
    let mut current_fence_index = 0usize;
    let mut current_content = String::new();
    let mut current_content_start_line = 1usize;
    let mut current_content_end_line = 1usize;
    let mut line_number = 1usize;

    for line in text.lines() {
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if is_closing_fence(marker_len, suffix, open_len) {
                    if let Some(section) = current_section.as_ref() {
                        result.entry(section.clone()).or_insert_with(Vec::new).push(
                            CodeFenceBlock {
                                fence_index: current_fence_index,
                                content_start_line: current_content_start_line,
                                content_end_line: current_content_end_line,
                                content: current_content.trim_end_matches(['\r', '\n']).to_string(),
                            },
                        );
                    }
                    current_content.clear();
                    fence_len = None;
                } else {
                    current_content.push_str(line);
                    current_content.push('\n');
                    current_content_end_line = line_number;
                }
            } else {
                fence_len = Some(marker_len);
                current_fence_index = next_fence_index;
                next_fence_index += 1;
                current_content.clear();
                current_content_start_line = line_number + 1;
                current_content_end_line = current_content_start_line;
            }
            line_number += 1;
            continue;
        }

        if fence_len.is_none() && line.starts_with("## ") {
            let title = line.strip_prefix("## ").unwrap();
            current_section = Some(canonical_section_title_for_doc(
                title.trim(),
                label_overrides,
                Some(doc_kind),
            ));
        } else if fence_len.is_some() {
            current_content.push_str(line);
            current_content.push('\n');
            current_content_end_line = line_number;
        }
        line_number += 1;
    }

    result
}

fn code_fence_labels_by_section_for_doc(
    text: &str,
    label_overrides: &HashMap<String, String>,
    doc_kind: DocKind,
) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    let mut current_section: Option<String> = None;
    let mut fence_len: Option<usize> = None;

    for line in text.lines() {
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if is_closing_fence(marker_len, suffix, open_len) {
                    fence_len = None;
                }
            } else {
                fence_len = Some(marker_len);
                if let Some(section) = current_section.as_ref() {
                    if let Some(label) = fence_label(suffix) {
                        result
                            .entry(section.clone())
                            .or_insert_with(Vec::new)
                            .push(label.to_string());
                    }
                }
            }
            continue;
        }

        if fence_len.is_none() && line.starts_with("## ") {
            let title = line.strip_prefix("## ").unwrap();
            current_section = Some(canonical_section_title_for_doc(
                title.trim(),
                label_overrides,
                Some(doc_kind),
            ));
        }
    }

    result
}

fn code_from_fence_blocks(blocks: &[CodeFenceBlock]) -> String {
    if blocks.is_empty() {
        String::new()
    } else {
        blocks
            .iter()
            .map(|block| block.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
            + "\n"
    }
}

fn code_from_section(section: Option<&String>, blocks: &[CodeFenceBlock]) -> String {
    if !blocks.is_empty() {
        code_from_fence_blocks(blocks)
    } else {
        section
            .map(|section| code_from_table(section))
            .unwrap_or_default()
    }
}

fn code_from_table(section: &str) -> String {
    let lines: Vec<&str> = section.lines().collect();
    for index in 0..lines.len().saturating_sub(1) {
        if !lines[index].trim_start().starts_with('|') || !lines[index + 1].contains("---") {
            continue;
        }
        let headers = split_markdown_row(lines[index]);
        let Some(code_index) = headers
            .iter()
            .position(|header| header.trim().eq_ignore_ascii_case("statement"))
        else {
            continue;
        };
        let mut output = String::new();
        for row_line in lines.iter().skip(index + 2) {
            if !row_line.trim_start().starts_with('|') {
                break;
            }
            let cells = split_markdown_row(row_line);
            let value = cells
                .get(code_index)
                .map(String::as_str)
                .unwrap_or_default()
                .trim()
                .trim_matches('`');
            if value.is_empty() {
                continue;
            }
            output.push_str(value);
            output.push('\n');
        }
        return output;
    }
    String::new()
}

fn split_markdown_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn covers_from_section(
    package: &Package,
    current_path: &Path,
    section: Option<&String>,
) -> Vec<String> {
    section
        .map(|section| {
            section
                .lines()
                .map(|line| line.trim().trim_start_matches(['-', '*']).trim())
                .filter(|line| !line.is_empty())
                .map(|line| normalize_reference_value(package, current_path, line))
                .collect()
        })
        .unwrap_or_default()
}

fn wiki_link_target(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        inner
            .split('|')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        value.to_string()
    }
}

fn validate_code_block_boundaries(
    path: &Path,
    lang: &Lang,
    text: &str,
    label_overrides: &HashMap<String, String>,
    _check: &crate::model::CheckConfig,
    state: &mut RunState,
) {
    let mut previous_block: Option<CodeBlock<'_>> = None;
    for block in code_blocks(text, label_overrides) {
        if let Some(previous) = previous_block {
            if is_unnecessary_code_block_split(lang, previous.content, block.content) {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!(
                        "code block starting at line {} appears to continue the previous code block; merge the blocks or split at a top-level logical boundary",
                        block.start_line
                    ),
                ));
            }
        }
        previous_block = Some(block);
    }
}

fn is_unnecessary_code_block_split(lang: &Lang, previous: &str, current: &str) -> bool {
    let previous_last = previous.lines().rev().find(|line| !line.trim().is_empty());
    let current_first = current.lines().find(|line| !line.trim().is_empty());
    let Some(previous_last) = previous_last.map(str::trim_end) else {
        return false;
    };
    let Some(current_first_raw) = current_first else {
        return false;
    };
    let current_first = current_first_raw.trim_start();
    let merges_with_descriptor = crate::descriptor::descriptor_for_key(lang.key())
        .is_some_and(|descriptor| descriptor.matches_code_block_merge_start(current_first));
    previous_last.ends_with(['{', '(', '[', ',', '\\'])
        || current_first.starts_with(['}', ')', ']', ',', '.'])
        || (previous_last.ends_with(';') && merges_with_descriptor)
        || current_first_raw.starts_with(' ')
        || current_first_raw.starts_with('\t')
}

#[derive(Debug)]
struct CodeBlock<'a> {
    content: &'a str,
    start_line: usize,
}

fn code_blocks<'a>(
    text: &'a str,
    _label_overrides: &HashMap<String, String>,
) -> Vec<CodeBlock<'a>> {
    let mut blocks = Vec::new();
    let mut fence_len: Option<usize> = None;
    let mut content_start = 0;
    let mut content_start_line = 1;
    let mut cursor = 0;
    let mut line_number = 1;
    for line in text.split_inclusive('\n') {
        let line_start = cursor;
        cursor += line.len();
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if is_closing_fence(marker_len, suffix, open_len) {
                    blocks.push(CodeBlock {
                        content: &text[content_start..line_start],
                        start_line: content_start_line,
                    });
                    fence_len = None;
                }
            } else {
                fence_len = Some(marker_len);
                content_start = cursor;
                content_start_line = line_number + 1;
            }
        }
        line_number += 1;
    }
    blocks
}

fn backtick_fence(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let count = trimmed.chars().take_while(|char| *char == '`').count();
    (count >= 3).then_some((count, &trimmed[count..]))
}

fn fence_label(suffix: &str) -> Option<&str> {
    let label = suffix.trim();
    let label = label.split_whitespace().next().unwrap_or_default();
    (!label.is_empty()).then_some(label)
}

fn is_closing_fence(marker_len: usize, suffix: &str, open_len: usize) -> bool {
    marker_len >= open_len && suffix.trim().is_empty()
}

pub fn sections_with_labels(
    text: &str,
    label_overrides: &HashMap<String, String>,
) -> HashMap<String, String> {
    sections_with_labels_for_doc(text, label_overrides, DocKind::Source)
}

fn sections_with_labels_for_doc(
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
            let title = line.strip_prefix("## ").unwrap();
            let title =
                canonical_section_title_for_doc(title.trim(), label_overrides, Some(doc_kind));
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

fn canonical_section_title_for_doc(
    title: &str,
    label_overrides: &HashMap<String, String>,
    doc_kind: Option<DocKind>,
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
            Some(DocKind::Test) => "Test".to_string(),
            _ => "Source".to_string(),
        };
    }
    title.to_string()
}

pub fn normalized_input(path: &Path, text: &str) -> String {
    let mut normalized = path.display().to_string();
    normalized.push('\n');
    normalized.push_str(text.replace("\r\n", "\n").trim_end());
    normalized.push('\n');
    normalized
}

pub fn validate_link_policy(package: &Package, docs: &[ImplDoc], state: &mut RunState) {
    if !package.config.check.markdown_links {
        return;
    }

    let module_index = build_module_reference_index(package, docs, state);
    for doc in docs {
        let text = match fs::read_to_string(&doc.path) {
            Ok(text) => text,
            Err(error) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(doc.path.clone()),
                    format!("failed to read implementation md: {error}"),
                ));
                continue;
            }
        };
        validate_link_policy_for_doc_text(package, doc, &text, &module_index, state);
    }
}

pub fn validate_link_policy_text(
    package: &Package,
    docs: &[ImplDoc],
    doc: &ImplDoc,
    text: &str,
    state: &mut RunState,
) {
    if !package.config.check.markdown_links {
        return;
    }

    let module_index = build_module_reference_index(package, docs, state);
    validate_link_policy_for_doc_text(package, doc, text, &module_index, state);
    validate_wiki_link_targets_for_doc_text(package, doc, text, &module_index, state);
}

fn validate_link_policy_for_doc_text(
    package: &Package,
    doc: &ImplDoc,
    text: &str,
    module_index: &HashMap<String, Vec<ModuleReferenceIndex>>,
    state: &mut RunState,
) {
    let mut seen_markdown_targets = HashSet::new();
    for link in parsed_links(&text_without_code_blocks(text)) {
        if !link_allowed(package.config.link_policy, link.syntax) {
            let required = match package.config.link_policy {
                LinkPolicy::WikiOnly => "wiki links",
                LinkPolicy::MarkdownOnly => "Markdown links",
                LinkPolicy::Mixed => unreachable!(),
            };
            state.diagnostics.push(Diagnostic::error(
                Some(doc.path.clone()),
                format!(
                    "link policy `{}` requires {required}: `{}`",
                    package.config.link_policy.as_str(),
                    link.raw,
                ),
            ));
        }
        if link.syntax != LinkSyntax::Markdown {
            continue;
        }
        let Some(target) = canonical_module_target(package, &doc.path, &link.target) else {
            continue;
        };
        if !should_validate_module_link(&target) || !seen_markdown_targets.insert(target.clone()) {
            continue;
        }
        if matches!(doc.doc_kind, DocKind::Test)
            && !target.contains('#')
            && doc
                .covers
                .iter()
                .any(|cover| cover_targets_module(cover, &target))
        {
            continue;
        }
        validate_package_link_target(
            &doc.path,
            &target,
            &link.raw,
            "link",
            module_index,
            &package.config.check,
            state,
        );
    }
}

fn validate_wiki_link_targets_for_doc_text(
    package: &Package,
    doc: &ImplDoc,
    text: &str,
    module_index: &HashMap<String, Vec<ModuleReferenceIndex>>,
    state: &mut RunState,
) {
    let link_text = text_without_code_blocks(text);
    let mut seen = HashSet::new();
    for link in parsed_links(&link_text)
        .into_iter()
        .filter(|link| link.syntax == LinkSyntax::Wiki)
    {
        let Some(target) = validation_target_for_wikilink(package, &doc.path, &link) else {
            continue;
        };
        if !seen.insert(target.clone()) {
            continue;
        }
        if matches!(doc.doc_kind, DocKind::Test)
            && !target.contains('#')
            && doc
                .covers
                .iter()
                .any(|cover| cover_targets_module(cover, &target))
        {
            continue;
        }
        validate_package_link_target(
            &doc.path,
            &target,
            &link.raw,
            "wiki link",
            module_index,
            &package.config.check,
            state,
        );
    }
}

pub fn normalize_link_policy_text(
    package: &Package,
    current_path: &Path,
    text: &str,
    policy: LinkPolicy,
) -> String {
    if matches!(policy, LinkPolicy::Mixed) {
        return text.to_string();
    }

    let mut output = String::new();
    let mut prose = String::new();
    let mut fence_len: Option<usize> = None;
    for line in text.split_inclusive('\n') {
        if let Some((marker_len, suffix)) = backtick_fence(line) {
            if let Some(open_len) = fence_len {
                if is_closing_fence(marker_len, suffix, open_len) {
                    fence_len = None;
                }
            } else {
                output.push_str(&normalize_links_in_segment(
                    package,
                    current_path,
                    &prose,
                    policy,
                ));
                prose.clear();
                fence_len = Some(marker_len);
            }
            output.push_str(line);
            continue;
        }
        if fence_len.is_some() {
            output.push_str(line);
        } else {
            prose.push_str(line);
        }
    }
    if !prose.is_empty() {
        output.push_str(&normalize_links_in_segment(
            package,
            current_path,
            &prose,
            policy,
        ));
    }
    output
}

pub fn validate_markdown_links(path: &Path, text: &str, state: &mut RunState) {
    for link in parsed_links(&text_without_code_blocks(text)) {
        if !should_validate_local_link(&link.target) {
            continue;
        }
        let clean_target = clean_link_target(&link.target);
        if clean_target.is_empty() {
            continue;
        }
        let target_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(clean_target);
        if !target_path.exists() {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("Markdown link target does not exist: `{}`", link.target),
            ));
        }
    }
}

fn text_without_code_blocks(text: &str) -> String {
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

fn should_validate_local_link(target: &str) -> bool {
    let target = target.trim();
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
    {
        return false;
    }
    let clean = target.split('#').next().unwrap_or_default();
    clean.ends_with(".md") || clean.contains('/')
}

fn should_validate_module_link(target: &str) -> bool {
    let target = target.trim();
    !target.is_empty()
        && !target.starts_with('#')
        && !target.starts_with("http://")
        && !target.starts_with("https://")
        && !target.starts_with("mailto:")
        && !target.split('#').next().unwrap_or_default().contains('/')
        && !target
            .split('#')
            .next()
            .unwrap_or_default()
            .ends_with(".md")
}

fn clean_link_target(target: &str) -> &str {
    target
        .split('#')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('<')
        .trim_matches('>')
}

fn normalized_link_target(target: &str) -> String {
    target
        .trim()
        .trim_matches('<')
        .trim_matches('>')
        .to_string()
}

fn normalize_reference_value(package: &Package, current_path: &Path, value: &str) -> String {
    let target = standalone_reference_target(value).unwrap_or_else(|| wiki_link_target(value));
    canonical_module_target(package, current_path, &target).unwrap_or(target)
}

fn standalone_reference_target(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let links = parsed_links(trimmed);
    match links.as_slice() {
        [link] if link.start == 0 && link.end == trimmed.len() => {
            Some(normalized_link_target(&link.target))
        }
        _ => None,
    }
}

fn canonical_module_target(package: &Package, current_path: &Path, target: &str) -> Option<String> {
    let target = normalized_link_target(target);
    if target.is_empty() || target.starts_with('#') || is_external_link_target(&target) {
        return None;
    }

    let (base, fragment) = split_target_fragment(&target);
    let module_id = if should_validate_module_link(&target) {
        base.to_string()
    } else {
        let relative = resolve_package_markdown_target(package, current_path, base)?;
        logical_module_id(&relative)
    };
    Some(with_target_fragment(module_id, fragment))
}

fn validation_target_for_wikilink(
    package: &Package,
    current_path: &Path,
    link: &ParsedLink,
) -> Option<String> {
    canonical_module_target(package, current_path, &link.target).or_else(|| {
        let target = normalized_link_target(&link.target);
        should_validate_wikilink_target(&target).then_some(target)
    })
}

fn should_validate_wikilink_target(target: &str) -> bool {
    let target = target.trim();
    !target.is_empty() && !target.starts_with('#') && !is_external_link_target(target)
}

fn is_external_link_target(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://") || target.starts_with("mailto:")
}

fn split_target_fragment(target: &str) -> (&str, Option<&str>) {
    match target.split_once('#') {
        Some((base, fragment)) => (base.trim(), Some(fragment.trim())),
        None => (target.trim(), None),
    }
}

fn with_target_fragment(module_id: String, fragment: Option<&str>) -> String {
    match fragment.filter(|fragment| !fragment.is_empty()) {
        Some(fragment) => format!("{module_id}#{fragment}"),
        None => module_id,
    }
}

fn resolve_package_markdown_target(
    package: &Package,
    current_path: &Path,
    target: &str,
) -> Option<PathBuf> {
    if !target.ends_with(".md") {
        return None;
    }

    let parent = current_path.parent().unwrap_or_else(|| Path::new("."));
    let resolved = lexical_normalize_path(parent.join(target));
    if !resolved.exists() {
        return None;
    }

    resolved
        .strip_prefix(source_markdown_root(package))
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            resolved
                .strip_prefix(test_markdown_root(package))
                .map(PathBuf::from)
                .ok()
        })
}

fn lexical_normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

fn cover_targets_module(cover: &str, target: &str) -> bool {
    let cover = cover.split('#').next().unwrap_or(cover).trim();
    let target = target.split('#').next().unwrap_or(target).trim();
    cover == target || cover == target.replace('.', "/") || target == cover.replace('.', "/")
}

fn link_allowed(policy: LinkPolicy, syntax: LinkSyntax) -> bool {
    match syntax {
        LinkSyntax::Markdown => policy.allows_markdown(),
        LinkSyntax::Wiki => policy.allows_wiki(),
    }
}

fn normalize_links_in_segment(
    package: &Package,
    current_path: &Path,
    text: &str,
    policy: LinkPolicy,
) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    for link in parsed_links(text) {
        output.push_str(&text[cursor..link.start]);
        let replacement = match (policy, link.syntax) {
            (LinkPolicy::WikiOnly, LinkSyntax::Markdown) => {
                wiki_link_from_markdown(package, current_path, &link)
            }
            (LinkPolicy::MarkdownOnly, LinkSyntax::Wiki) => {
                markdown_link_from_wiki(package, current_path, &link)
            }
            _ => link.raw.clone(),
        };
        output.push_str(&replacement);
        cursor = link.end;
    }
    output.push_str(&text[cursor..]);
    output
}

fn wiki_link_from_markdown(package: &Package, current_path: &Path, link: &ParsedLink) -> String {
    let Some(target) = canonical_module_target(package, current_path, &link.target) else {
        return link.raw.clone();
    };
    let display = link.display.as_deref().unwrap_or_default().trim();
    if display.is_empty() || display == target {
        format!("[[{target}]]")
    } else {
        format!("[[{target}|{display}]]")
    }
}

fn markdown_link_from_wiki(package: &Package, current_path: &Path, link: &ParsedLink) -> String {
    let target = canonical_module_target(package, current_path, &link.target)
        .unwrap_or_else(|| normalized_link_target(&link.target));
    let display = link
        .display
        .as_deref()
        .map(str::trim)
        .filter(|display| !display.is_empty())
        .unwrap_or(&target);
    format!("[{display}]({target})")
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum LinkSyntax {
    Markdown,
    Wiki,
}

#[derive(Debug, Clone)]
struct ParsedLink {
    syntax: LinkSyntax,
    start: usize,
    end: usize,
    target: String,
    display: Option<String>,
    raw: String,
}

fn parsed_links(text: &str) -> Vec<ParsedLink> {
    let mut links = Vec::new();
    let bytes = text.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'`' {
            idx = skip_inline_code_span(text, idx);
            continue;
        }
        if bytes[idx] == b'[' && bytes.get(idx + 1) == Some(&b'[') {
            let Some(close) = text[idx + 2..].find("]]") else {
                break;
            };
            let inner_start = idx + 2;
            let inner_end = inner_start + close;
            let end = inner_end + 2;
            let inner = &text[inner_start..inner_end];
            let mut parts = inner.splitn(2, '|');
            let target = parts.next().unwrap_or_default().trim().to_string();
            let display = parts
                .next()
                .map(str::trim)
                .filter(|display| !display.is_empty())
                .map(str::to_string);
            links.push(ParsedLink {
                syntax: LinkSyntax::Wiki,
                start: idx,
                end,
                target,
                display,
                raw: text[idx..end].to_string(),
            });
            idx = end;
            continue;
        }
        if bytes[idx] == b'[' {
            if idx > 0 && bytes[idx - 1] == b'!' {
                idx += 1;
                continue;
            }
            let Some(close_text) = text[idx + 1..].find(']') else {
                break;
            };
            let close_text = idx + 1 + close_text;
            if bytes.get(close_text + 1) != Some(&b'(') {
                idx += 1;
                continue;
            }
            let Some(close_target) = text[close_text + 2..].find(')') else {
                break;
            };
            let close_target = close_text + 2 + close_target;
            let end = close_target + 1;
            links.push(ParsedLink {
                syntax: LinkSyntax::Markdown,
                start: idx,
                end,
                target: text[close_text + 2..close_target].trim().to_string(),
                display: Some(text[idx + 1..close_text].trim().to_string()),
                raw: text[idx..end].to_string(),
            });
            idx = end;
            continue;
        }
        idx += 1;
    }
    links
}

fn skip_inline_code_span(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let marker_len = backtick_run_length(bytes, start);
    let mut idx = start + marker_len;
    while idx < bytes.len() {
        if bytes[idx] != b'`' {
            idx += 1;
            continue;
        }
        let marker_end = backtick_run_length(bytes, idx);
        if marker_end == marker_len {
            return idx + marker_len;
        }
        idx += marker_end;
    }
    start + marker_len
}

fn backtick_run_length(bytes: &[u8], start: usize) -> usize {
    let mut len = 0;
    while bytes.get(start + len) == Some(&b'`') {
        len += 1;
    }
    len
}
