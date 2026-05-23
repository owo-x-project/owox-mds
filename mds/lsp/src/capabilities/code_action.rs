use std::path::Path;

use crate::capabilities::authoring;
use crate::labels::resolve_label;
use crate::state::{resolve_active_language, WorkspaceState};
use mds_core::model::{Config, DocKind};
use tower_lsp::lsp_types::*;

pub fn provide_code_actions(uri: &Url, text: &str, config: &Config) -> CodeActionResponse {
    provide_code_actions_with_state(uri, text, config, None)
}

pub fn provide_code_actions_with_state(
    uri: &Url,
    text: &str,
    config: &Config,
    workspace_state: Option<&WorkspaceState>,
) -> CodeActionResponse {
    let mut actions = Vec::new();
    let path = uri.to_file_path().ok();
    let path = path.as_deref();
    let effective_config = code_action_config(path, config, workspace_state);
    let doc_kind = authoring::doc_kind_for_path(path, &effective_config, workspace_state);
    let authoring_kind =
        authoring::guided_authoring_path_kind(path, &effective_config, workspace_state);

    let sections =
        authoring::sections_with_labels_for_doc(text, &effective_config.label_overrides, doc_kind);
    let required_sections = required_sections(authoring_kind, doc_kind);
    let missing: Vec<&str> = required_sections
        .iter()
        .copied()
        .filter(|section| !sections.contains_key(*section))
        .collect();
    let missing_labels = missing
        .iter()
        .map(|section| section_label(section, &effective_config))
        .collect::<Vec<_>>();

    if !missing.is_empty() {
        let line_count = text.lines().count() as u32;

        let mut new_text = String::new();
        if !text.ends_with('\n') {
            new_text.push('\n');
        }
        new_text.push('\n');

        for section in &missing {
            append_section(
                &mut new_text,
                section,
                &effective_config,
                path,
                workspace_state,
                authoring_kind,
                doc_kind,
            );
        }

        actions.push(insert_action(
            uri,
            line_count,
            format!("Add missing sections: {}", missing_labels.join(", ")),
            new_text,
        ));

        for (section, label) in missing.iter().zip(missing_labels.iter()) {
            let mut section_text = String::new();
            if !text.ends_with('\n') {
                section_text.push('\n');
            }
            section_text.push('\n');
            append_section(
                &mut section_text,
                section,
                &effective_config,
                path,
                workspace_state,
                authoring_kind,
                doc_kind,
            );
            actions.push(insert_action(
                uri,
                line_count,
                format!("Add missing ## {label} section"),
                section_text,
            ));
        }
    }

    actions
}

fn code_action_config(
    path: Option<&Path>,
    config: &Config,
    workspace_state: Option<&WorkspaceState>,
) -> Config {
    path.and_then(|path| {
        workspace_state.and_then(|workspace_state| workspace_state.effective_config_for_path(path))
    })
    .unwrap_or_else(|| config.clone())
}

fn required_sections(
    authoring_kind: authoring::GuidedAuthoringPathKind,
    doc_kind: DocKind,
) -> &'static [&'static str] {
    match authoring_kind {
        authoring::GuidedAuthoringPathKind::SourceOverview
        | authoring::GuidedAuthoringPathKind::Overview => &["Purpose", "Architecture", "Rules"],
        authoring::GuidedAuthoringPathKind::RootModule => &["Purpose", "Contract"],
        authoring::GuidedAuthoringPathKind::Source => &["Purpose", "Contract", "Source"],
        authoring::GuidedAuthoringPathKind::Test => match doc_kind {
            DocKind::Source => &["Purpose", "Contract", "Source"],
            DocKind::Test => &["Purpose", "Covers", "Cases", "Test"],
        },
    }
}

fn section_label(section: &str, config: &Config) -> String {
    resolve_label(&section.to_lowercase(), config)
}

fn append_section(
    buffer: &mut String,
    section: &str,
    config: &Config,
    path: Option<&Path>,
    workspace_state: Option<&WorkspaceState>,
    authoring_kind: authoring::GuidedAuthoringPathKind,
    doc_kind: DocKind,
) {
    let label = section_label(section, config);
    buffer.push_str(&format!("## {label}\n\n"));

    match section {
        "Purpose" | "Contract" | "Cases" => buffer.push_str("Fill in this section.\n\n"),
        "Architecture" => {
            append_architecture_section(buffer, authoring_kind, doc_kind);
        }
        "Rules" => {
            append_rules_section(buffer, authoring_kind, doc_kind);
        }
        "Covers" => buffer.push_str("Add covered module wiki links.\n\n"),
        "Source" | "Test" => {
            append_code_block(buffer, path, workspace_state);
        }
        _ => buffer.push_str("Fill in this section.\n\n"),
    }
}

fn append_architecture_section(
    buffer: &mut String,
    authoring_kind: authoring::GuidedAuthoringPathKind,
    doc_kind: DocKind,
) {
    match authoring_kind {
        authoring::GuidedAuthoringPathKind::SourceOverview => buffer.push_str(
            "Markdown files in this directory describe package and source hierarchy rules.\n\n\
             ### Package Summary\n\n\
             | Name | Version |\n\
             | --- | --- |\n\
             | package-name | 0.1.0 |\n\n\
             ### Dependencies\n\n\
             | Name | Version | Summary |\n\
             | --- | --- | --- |\n\n\
             ### Dev Dependencies\n\n\
             | Name | Version | Summary |\n\
             | --- | --- | --- |\n\n",
        ),
        _ => match doc_kind {
            DocKind::Source => {
                buffer.push_str("Describe directory structure and authoring intent.\n\n")
            }
            DocKind::Test => {
                buffer.push_str("Describe test structure and verification intent.\n\n")
            }
        },
    }
}

fn append_rules_section(
    buffer: &mut String,
    authoring_kind: authoring::GuidedAuthoringPathKind,
    doc_kind: DocKind,
) {
    match authoring_kind {
        authoring::GuidedAuthoringPathKind::SourceOverview => buffer.push_str(
            "- Keep package or directory root API notes in the root module doc.\n\
             - Add source code blocks there only when the root module owns runtime behavior.\n\
             - Keep one source md per feature.\n\n",
        ),
        _ => match doc_kind {
            DocKind::Source => buffer.push_str("- Describe source authoring rules.\n\n"),
            DocKind::Test => buffer.push_str("- Describe test authoring rules.\n\n"),
        },
    }
}

fn append_code_block(
    buffer: &mut String,
    path: Option<&Path>,
    workspace_state: Option<&WorkspaceState>,
) {
    if let Some(lang) = path.and_then(|path| resolve_active_language(path, workspace_state)) {
        buffer.push_str(&format!("```{}\n\n```\n\n", lang.key()));
    } else {
        buffer.push_str("```\n\n```\n\n");
    }
}

fn insert_action(
    uri: &Url,
    line_count: u32,
    title: String,
    new_text: String,
) -> CodeActionOrCommand {
    let edit = TextEdit {
        range: Range {
            start: Position {
                line: line_count,
                character: 0,
            },
            end: Position {
                line: line_count,
                character: 0,
            },
        },
        new_text,
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        ..Default::default()
    })
}
