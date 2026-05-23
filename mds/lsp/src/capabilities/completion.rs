use crate::capabilities::authoring::{
    doc_kind_for_path, guided_authoring_path_kind, GuidedAuthoringPathKind,
};
use crate::convert::line_at;
use crate::labels::resolve_label;
use crate::state::{
    resolve_active_language, resolve_fence_labels_for_path, with_descriptor_root_for_path,
    WorkspaceState,
};
use mds_core::descriptor::all_fence_label_completions;
use mds_core::descriptor::fence_labels_for_lang;
use mds_core::model::{Config, DocKind, Lang};
use std::path::Path;
use tower_lsp::lsp_types::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkdownDocKind {
    Source,
    Test,
    Unknown,
}

const SOURCE_SECTION_COMPLETIONS: [(&str, &str); 5] = [
    ("purpose", "Module purpose and intent"),
    ("contract", "Readable contract and invariants"),
    ("api", "Public API prose"),
    ("source", "Implementation code"),
    ("cases", "Verification notes"),
];

const TEST_SECTION_COMPLETIONS: [(&str, &str); 4] = [
    ("purpose", "Verification goal and scope"),
    ("covers", "Covered source modules"),
    ("cases", "Verification cases"),
    ("test", "Verification code"),
];

const SOURCE_OVERVIEW_SECTION_COMPLETIONS: [(&str, &str); 3] = [
    ("purpose", "Package-level source authoring purpose"),
    (
        "architecture",
        "Source hierarchy with managed Package Summary / Dependencies / Dev Dependencies headings",
    ),
    (
        "rules",
        "Package-level authoring rules and root-module boundaries",
    ),
];

const OVERVIEW_SECTION_COMPLETIONS: [(&str, &str); 3] = [
    ("purpose", "Directory or verification authoring purpose"),
    ("architecture", "Document hierarchy and organization"),
    ("rules", "Authoring rules and linkage boundaries"),
];

const ROOT_MODULE_SECTION_COMPLETIONS: [(&str, &str); 4] = [
    ("purpose", "Package or directory root module purpose"),
    (
        "contract",
        "Stable exports, re-exports, and entrypoint invariants",
    ),
    ("api", "Public exports and entrypoint behavior in prose"),
    (
        "source",
        "Optional runtime code when the root module owns behavior",
    ),
];

const UNKNOWN_SECTION_COMPLETIONS: [(&str, &str); 7] = [
    ("purpose", "Module purpose and intent"),
    ("contract", "Readable contract and invariants"),
    ("api", "Public API prose"),
    ("source", "Implementation code"),
    ("covers", "Covered source modules"),
    ("cases", "Verification cases"),
    ("test", "Verification code"),
];

const SOURCE_OVERVIEW_MANAGED_HEADING_COMPLETIONS: [(&str, &str); 3] = [
    (
        "Package Summary",
        "Managed package summary heading for exact source overview",
    ),
    (
        "Dependencies",
        "Managed runtime dependency heading for exact source overview",
    ),
    (
        "Dev Dependencies",
        "Managed development dependency heading for exact source overview",
    ),
];

const EMPTY_HEADING_COMPLETIONS: [(&str, &str); 0] = [];

pub fn provide_completions(
    text: &str,
    position: Position,
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let effective_config = completion_config(path, config, state);

    let Some(line_text) = line_at(text, position.line) else {
        return items;
    };

    let col = position.character as usize;
    let prefix = if col <= line_text.len() {
        &line_text[..col]
    } else {
        line_text
    };

    // Section name completion: `## ` prefix
    if prefix.starts_with("## ") || prefix == "##" {
        items.extend(section_completions(path, &effective_config, state));
        return items;
    }

    if prefix.starts_with("### ") || prefix == "###" {
        let managed_headings = managed_heading_completions(path, &effective_config, state);
        if !managed_headings.is_empty() {
            items.extend(managed_headings);
            return items;
        }
    }

    // Shared definition completion: `##### ` prefix
    if prefix.starts_with("##### ") || prefix == "#####" {
        items.push(CompletionItem {
            label: "mds: Shared Definition Section".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("Add a H5 shared definition section".to_string()),
            insert_text: Some("${1:symbolName}\n\n${2:Describe the shared symbol.}\n".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            sort_text: Some("0_h5".to_string()),
            ..Default::default()
        });
        return items;
    }

    if let Some(wiki_prefix) = wiki_link_prefix(prefix) {
        items.extend(wiki_link_completions(wiki_prefix, state));
        return items;
    }

    // Table column completion: inside `| ... |` row
    if prefix.trim_start().starts_with('|') {
        items.extend(table_column_completions(&effective_config));
    }

    // Code block language completion: any opening backtick fence prefix
    if is_backtick_fence_prefix(prefix.trim_start()) {
        items.extend(code_block_language_completions(
            path,
            &effective_config,
            state,
        ));
    }

    // Snippet completions
    items.extend(snippet_completions(path, &effective_config, state));

    items
}

fn completion_config(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> Config {
    path.and_then(|path| state.and_then(|state| state.effective_config_for_path(path)))
        .unwrap_or_else(|| config.clone())
}

fn is_backtick_fence_prefix(prefix: &str) -> bool {
    let marker_len = prefix
        .chars()
        .take_while(|character| *character == '`')
        .count();
    marker_len >= 3 && prefix[marker_len..].trim().is_empty()
}

fn wiki_link_prefix(prefix: &str) -> Option<&str> {
    let start = prefix.rfind("[[")?;
    let candidate = &prefix[start + 2..];
    (!candidate.contains("]]")).then_some(candidate)
}

fn wiki_link_completions(prefix: &str, state: Option<&WorkspaceState>) -> Vec<CompletionItem> {
    let Some(state) = state else {
        return Vec::new();
    };
    let (module_prefix, symbol_prefix) = prefix.split_once('#').unwrap_or((prefix, ""));
    let mut items = Vec::new();
    for pkg in &state.packages {
        if prefix.contains('#') {
            for (module, symbol) in pkg.index.symbol_index.keys() {
                if module == module_prefix && symbol.starts_with(symbol_prefix) {
                    items.push(CompletionItem {
                        label: symbol.clone(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(format!("Exported symbol from {module}")),
                        insert_text: Some(format!("{symbol}]]")),
                        sort_text: Some(format!("0_{symbol}")),
                        ..Default::default()
                    });
                }
            }
        } else {
            let mut modules = pkg.index.module_index.keys().cloned().collect::<Vec<_>>();
            modules.sort();
            modules.dedup();
            for module in modules {
                if module.starts_with(module_prefix) {
                    items.push(CompletionItem {
                        label: module.clone(),
                        kind: Some(CompletionItemKind::MODULE),
                        detail: Some("mds module".to_string()),
                        insert_text: Some(format!("{module}]]")),
                        sort_text: Some(format!("0_{module}")),
                        ..Default::default()
                    });
                }
            }
        }
    }
    items
}

fn section_completions(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> Vec<CompletionItem> {
    section_completion_entries(path, config, state)
        .iter()
        .enumerate()
        .map(|(index, (key, detail))| {
            let label = section_completion_label(key, config);
            CompletionItem {
                label: label.clone(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(detail.to_string()),
                insert_text: Some(format!("{label}\n\n")),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("{index:02}_{key}")),
                ..Default::default()
            }
        })
        .collect()
}

fn section_completion_entries(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> &'static [(&'static str, &'static str)] {
    let Some(path) = path else {
        return &UNKNOWN_SECTION_COMPLETIONS;
    };

    match guided_authoring_path_kind(Some(path), config, state) {
        GuidedAuthoringPathKind::SourceOverview => &SOURCE_OVERVIEW_SECTION_COMPLETIONS,
        GuidedAuthoringPathKind::Overview => &OVERVIEW_SECTION_COMPLETIONS,
        GuidedAuthoringPathKind::RootModule => &ROOT_MODULE_SECTION_COMPLETIONS,
        GuidedAuthoringPathKind::Source => &SOURCE_SECTION_COMPLETIONS,
        GuidedAuthoringPathKind::Test => &TEST_SECTION_COMPLETIONS,
    }
}

fn managed_heading_completions(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> Vec<CompletionItem> {
    managed_heading_completion_entries(path, config, state)
        .iter()
        .enumerate()
        .map(|(index, (label, detail))| CompletionItem {
            label: (*label).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(detail.to_string()),
            insert_text: Some(format!("{label}\n\n")),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            sort_text: Some(format!(
                "{index:02}_{}",
                label.to_ascii_lowercase().replace(' ', "_")
            )),
            ..Default::default()
        })
        .collect()
}

fn managed_heading_completion_entries(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> &'static [(&'static str, &'static str)] {
    let Some(path) = path else {
        return &EMPTY_HEADING_COMPLETIONS;
    };

    match guided_authoring_path_kind(Some(path), config, state) {
        GuidedAuthoringPathKind::SourceOverview => &SOURCE_OVERVIEW_MANAGED_HEADING_COMPLETIONS,
        _ => &EMPTY_HEADING_COMPLETIONS,
    }
}

fn section_completion_label(key: &str, config: &Config) -> String {
    resolve_label(key, config)
}

fn table_column_completions(config: &Config) -> Vec<CompletionItem> {
    let imports_columns = [
        (
            "From",
            "Import source: builtin, external, package, workspace, internal",
        ),
        ("Target", "Import target module or package"),
        ("Symbols", "Imported symbols from the target"),
        ("Via", "Import mechanism or adapter-specific path"),
        ("Summary", "Brief description of the import"),
        ("Reference", "Markdown reference for navigation"),
    ];

    let export_columns = [
        ("Name", "Exported symbol name"),
        ("Visibility", "Public or internal visibility"),
        ("Summary", "Brief description of the exported symbol"),
    ];

    let mut items: Vec<CompletionItem> = imports_columns
        .iter()
        .map(|(name, detail)| {
            let label = resolve_label(&name.to_lowercase(), config);
            CompletionItem {
                label,
                kind: Some(CompletionItemKind::FIELD),
                detail: Some(detail.to_string()),
                ..Default::default()
            }
        })
        .collect();

    items.extend(export_columns.iter().map(|(name, detail)| {
        let label = resolve_label(&name.to_lowercase(), config);
        CompletionItem {
            label,
            kind: Some(CompletionItemKind::FIELD),
            detail: Some(detail.to_string()),
            ..Default::default()
        }
    }));

    items
}

fn configured_languages(config: &Config) -> Vec<Lang> {
    let mut languages: Vec<Lang> = config
        .adapters
        .keys()
        .chain(config.quality.keys())
        .cloned()
        .collect();
    languages.sort_by(|left, right| left.key().cmp(right.key()));
    languages.dedup_by(|left, right| left.key() == right.key());
    languages
}

fn push_fence_label_entries(
    entries: &mut Vec<(String, String)>,
    lang: &Lang,
    path: Option<&Path>,
    state: Option<&WorkspaceState>,
) {
    for label in with_descriptor_root_for_path(path, state, || fence_labels_for_lang(lang)) {
        entries.push((label, lang.key().to_string()));
    }
}

fn fence_label_entries(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> Vec<(String, String)> {
    let mut entries = with_descriptor_root_for_path(path, state, || all_fence_label_completions());

    if let Some(path) = path {
        if let Some(lang) = resolve_active_language(path, state) {
            push_fence_label_entries(&mut entries, &lang, Some(path), state);
        }
    }

    for lang in configured_languages(config) {
        push_fence_label_entries(&mut entries, &lang, path, state);
    }

    entries.sort();
    entries.dedup();
    entries
}

fn code_block_language_completions(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> Vec<CompletionItem> {
    let recommended = path
        .map(|path| resolve_fence_labels_for_path(path, state))
        .unwrap_or_default();

    fence_label_entries(path, config, state)
        .into_iter()
        .map(|(label, detail)| {
            let is_recommended = recommended.contains(&label);

            CompletionItem {
                label: label.to_string(),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                detail: Some(detail),
                sort_text: Some(if is_recommended {
                    format!("0_{label}")
                } else {
                    format!("1_{label}")
                }),
                preselect: Some(is_recommended),
                ..Default::default()
            }
        })
        .collect()
}

fn markdown_doc_kind(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> MarkdownDocKind {
    let Some(path) = path else {
        return MarkdownDocKind::Unknown;
    };

    match doc_kind_for_path(Some(path), config, state) {
        DocKind::Source => MarkdownDocKind::Source,
        DocKind::Test => MarkdownDocKind::Test,
    }
}

fn fence_opening(lang_label: Option<&str>) -> String {
    match lang_label {
        Some(label) if !label.is_empty() => format!("```{label}"),
        _ => "```".to_string(),
    }
}

fn snippet_completions(
    path: Option<&Path>,
    config: &Config,
    state: Option<&WorkspaceState>,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let lang = path.and_then(|path| resolve_active_language(path, state));
    let doc_kind = markdown_doc_kind(path, config, state);
    let authoring_kind = guided_authoring_path_kind(path, config, state);

    let lang_label = lang.as_ref().and_then(|lang| {
        path.and_then(|path| {
            resolve_fence_labels_for_path(path, state)
                .into_iter()
                .next()
        })
        .or_else(|| {
            with_descriptor_root_for_path(path, state, || {
                fence_labels_for_lang(lang).into_iter().next()
            })
        })
    });

    items.push(CompletionItem {
        label: "mds: New Impl Document".to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Phase 07 `mds new impl` template".to_string()),
        insert_text: Some(format!(
            "## {purpose}\n\n${{1:Describe the module purpose}}\n\n\
             ## {contract}\n\n${{2:Define behavior, constraints, and boundaries}}\n\n\
             ## {api}\n\n${{3:Describe the public API in prose. Keep import/export details in the source block.}}\n\n\
             ## {source}\n\n\
             {source_fence}\n${{4:// implementation}}\n```\n\n\
             ## {cases}\n\n- ${{5:Describe expected behavior}}\n",
            purpose = resolve_label("purpose", config),
            contract = resolve_label("contract", config),
            api = resolve_label("api", config),
            source = resolve_label("source", config),
            cases = resolve_label("cases", config),
            source_fence = fence_opening(lang_label.as_deref()),
        )),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some(
            match doc_kind {
                MarkdownDocKind::Source => "8_impl_template",
                _ => "9_impl_template",
            }
            .to_string(),
        ),
        ..Default::default()
    });

    items.push(CompletionItem {
        label: "mds: New Test Document".to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Tableless authoring-v2 test document".to_string()),
        insert_text: Some(format!(
            "## {purpose}\n\n${{1:Describe the verification goal}}\n\n\
             ## {covers}\n\n- ${{2:module.id}}\n\n\
             ## {cases}\n\n- ${{3:Describe the test case}}\n\n\
             ## {test}\n\n\
             {test_fence}\n${{4:// test code}}\n```\n",
            purpose = resolve_label("purpose", config),
            covers = resolve_label("covers", config),
            cases = resolve_label("cases", config),
            test = resolve_label("test", config),
            test_fence = fence_opening(lang_label.as_deref()),
        )),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some(
            match doc_kind {
                MarkdownDocKind::Test => "8_test_template",
                _ => "9_test_template",
            }
            .to_string(),
        ),
        ..Default::default()
    });

    items.push(CompletionItem {
        label: "mds: New Overview Document".to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Phase 07 `mds new overview` template".to_string()),
        insert_text: Some(match authoring_kind {
            GuidedAuthoringPathKind::SourceOverview => source_overview_snippet(config),
            _ => generic_overview_snippet(config),
        }),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some("9_overview_template".to_string()),
        ..Default::default()
    });

    items.push(CompletionItem {
        label: "mds: New Root Module Document".to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Phase 07 `mds new root-module` template".to_string()),
        insert_text: Some(format!(
            "## {purpose}\n\n${{1:Describe the package or directory root module surface.}}\n\n\
             ## {contract}\n\n- ${{2:Describe stable exports, re-exports, and entrypoint constraints.}}\n\n\
             ## {api}\n\n${{3:Describe public exports, re-exports, and package entrypoint behavior in prose.}}\n\n\
             ${{4:Add a `{source}` section only when this root module owns runtime behavior, and keep import or export declarations in that code block instead of duplicate metadata tables.}}\n",
            purpose = resolve_label("purpose", config),
            contract = resolve_label("contract", config),
            api = resolve_label("api", config),
            source = resolve_label("source", config),
        )),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some("9_root_module_template".to_string()),
        ..Default::default()
    });

    items.push(CompletionItem {
        label: "mds: Code Block".to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Add a new code block".to_string()),
        insert_text: Some(format!(
            "{}\n${{1:// code}}\n```\n",
            fence_opening(lang_label.as_deref())
        )),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some("9_code_block".to_string()),
        ..Default::default()
    });

    let section_choices = section_completion_entries(path, config, state)
        .iter()
        .map(|(key, _)| section_completion_label(key, config))
        .collect::<Vec<_>>()
        .join(",");

    items.push(CompletionItem {
        label: "mds: New Section".to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Add a new authoring-v2 section".to_string()),
        insert_text: Some(format!("## ${{1|{section_choices}|}}\n\n${{2:Content}}\n")),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some("9_section".to_string()),
        ..Default::default()
    });

    items
}

fn source_overview_snippet(config: &Config) -> String {
    format!(
        "## {purpose}\n\n${{1:Describe the package-level source hierarchy and authoring intent.}}\n\n\
         ## {architecture}\n\n${{2:Markdown files in this directory describe package and source hierarchy rules.}}\n\n\
         ### Package Summary\n\n\
         | Name | Version |\n\
         | --- | --- |\n\
         | ${{3:package-name}} | ${{4:0.1.0}} |\n\n\
         ### Dependencies\n\n\
         | Name | Version | Summary |\n\
         | --- | --- | --- |\n\n\
         ### Dev Dependencies\n\n\
         | Name | Version | Summary |\n\
         | --- | --- | --- |\n\n\
         ## {rules}\n\n\
         - ${{5:Keep package or directory root API notes in the root module doc.}}\n\
         - ${{6:Add source code blocks there only when the root module owns runtime behavior.}}\n\
         - ${{7:Keep one source md per feature.}}\n",
        purpose = resolve_label("purpose", config),
        architecture = resolve_label("architecture", config),
        rules = resolve_label("rules", config),
    )
}

fn generic_overview_snippet(config: &Config) -> String {
    format!(
        "## {purpose}\n\n${{1:Describe the directory-level hierarchy and authoring intent.}}\n\n\
         ## {architecture}\n\n${{2:Summarize how markdown files in this directory are organized.}}\n\n\
         ## {rules}\n\n\
         - ${{3:Keep one markdown document per feature or verification target.}}\n\
         - ${{4:Link to nearby source or test docs instead of duplicating detail.}}\n",
        purpose = resolve_label("purpose", config),
        architecture = resolve_label("architecture", config),
        rules = resolve_label("rules", config),
    )
}
