use crate::descriptor::{
    is_overview_markdown_path, resolve_output_rule_for_markdown, with_workspace_descriptor_root,
    OutputRootKind, ResolvedOutputRuleOrigin,
};
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::fs_utils::collect_files;
use crate::fs_utils::glob_match;
use crate::fs_utils::is_excluded;
use crate::fs_utils::is_mds_managed_file;
use crate::fs_utils::path_within;
use crate::hash::sha256;
use crate::manifest::plan_manifest;
use crate::markdown::{inspect_source_doc_lang, source_markdown_root, SourceDocLangDiscovery};
use crate::model::CodeFenceBlock;
use crate::model::DocKind;
use crate::model::GeneratedFile;
use crate::model::GeneratedKind;
use crate::model::ImplDoc;
use crate::model::OutputKind;
use crate::model::Package;
use crate::model::SourceMap;
use crate::model::SourceSpan;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct GenerationPlan {
    pub generated: Vec<GeneratedFile>,
    pub source_map: SourceMap,
}

struct OutputPatternContext {
    source_out: String,
    test_out: String,
    module: String,
    ext: String,
}

pub fn plan_generation_with_source_map(
    package: &Package,
    docs: &[ImplDoc],
    state: &mut RunState,
) -> GenerationPlan {
    with_workspace_descriptor_root(Some(&package.root), || {
        let mut plan = GenerationPlan::default();
        for doc in docs {
            let doc_plan = plan_outputs(package, doc, state);
            plan.generated.extend(doc_plan.generated);
            plan.source_map.extend(doc_plan.source_map.spans);
        }
        plan.generated.extend(plan_source_assets(package, state));
        plan.generated
            .push(plan_manifest(package, docs, &plan.generated));
        plan
    })
}

pub(crate) fn plan_generation(
    package: &Package,
    docs: &[ImplDoc],
    state: &mut RunState,
) -> Vec<GeneratedFile> {
    plan_generation_with_source_map(package, docs, state).generated
}

fn plan_outputs(package: &Package, doc: &ImplDoc, state: &mut RunState) -> GenerationPlan {
    let mut plan = GenerationPlan::default();

    if let Some(file) = plan_output(package, doc, OutputKind::Source, source_body(doc), state) {
        plan.generated.push(file.file);
        plan.source_map.extend(file.source_spans);
    }
    if let Some(file) = plan_output(package, doc, OutputKind::Test, &doc.test_code, state) {
        plan.generated.push(file.file);
        plan.source_map.extend(file.source_spans);
    }

    plan
}

fn source_body(doc: &ImplDoc) -> &str {
    if matches!(doc.doc_kind, DocKind::Source) {
        doc.source_code.as_str()
    } else {
        ""
    }
}

fn plan_source_assets(package: &Package, state: &mut RunState) -> Vec<GeneratedFile> {
    if !package.config.copy_source_assets {
        return Vec::new();
    }
    let source_root = source_markdown_root(package);
    if !source_root.exists() {
        return Vec::new();
    }
    let Ok(files) = collect_files(&source_root, false) else {
        return Vec::new();
    };
    let mut generated = Vec::new();
    for path in files
        .into_iter()
        .filter(|path| !is_excluded(&package.root, path, &package.config.excludes))
    {
        let Ok(relative) = path.strip_prefix(&source_root) else {
            continue;
        };
        if is_overview_markdown_path(relative) {
            continue;
        }
        if path.extension() == Some(OsStr::new("md")) && !is_template_asset_markdown(relative) {
            match inspect_source_doc_lang(package, &path) {
                SourceDocLangDiscovery::Resolved(_) => {
                    continue;
                }
                SourceDocLangDiscovery::Rejected(message) => {
                    push_unique_error(state, &path, &message);
                    continue;
                }
                SourceDocLangDiscovery::NoSignal => {}
            }
        }

        let package_relative_path = path
            .strip_prefix(&package.root)
            .unwrap_or(&path)
            .to_path_buf();
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.clone()),
                    format!("failed to read copied asset {}: {error}", path.display()),
                ));
                continue;
            }
        };
        let output_path = package
            .root
            .join(&package.config.roots.source_out)
            .join(relative);
        if !path_within(&package.root, &output_path) {
            state.diagnostics.push(Diagnostic::error(
                Some(output_path),
                "copied asset path must stay inside package root",
            ));
            continue;
        }

        generated.push(GeneratedFile {
            path: output_path,
            content,
            kind: GeneratedKind::Asset,
            source_path: Some(package_relative_path),
        });
    }
    generated
}

fn is_template_asset_markdown(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("md"))
        && path
            .components()
            .any(|component| component.as_os_str() == OsStr::new("templates"))
}

fn push_unique_error(state: &mut RunState, path: &Path, message: &str) {
    if state
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.path.as_deref() == Some(path) && diagnostic.message == message)
    {
        return;
    }

    state.diagnostics.push(Diagnostic::error(
        Some(path.to_path_buf()),
        message.to_string(),
    ));
}

struct PlannedOutput {
    file: GeneratedFile,
    source_spans: Vec<SourceSpan>,
}

fn plan_output(
    package: &Package,
    doc: &ImplDoc,
    kind: OutputKind,
    body: &str,
    state: &mut RunState,
) -> Option<PlannedOutput> {
    if body.trim().is_empty() {
        return None;
    }
    let source_hash = sha256(&doc.normalized_input);
    let Some(path) = planned_output_path(package, doc, kind, state) else {
        return None;
    };
    if is_excluded(&package.root, &path, &package.config.excludes) {
        return None;
    }
    if !path_within(&package.root, &path) {
        state.diagnostics.push(Diagnostic::error(
            Some(path),
            "output path must stay inside package root",
        ));
        return None;
    }
    if path.exists() && !is_mds_managed_file(&path) {
        state.diagnostics.push(Diagnostic::error(
            Some(path),
            "refusing to overwrite file without mds generated header",
        ));
        return None;
    }

    let header = format!(
        "{} Generated by mds. Do not edit. Source: {}. Source-Hash: {}.\n",
        doc.lang.header_prefix(),
        doc.package_relative_path.display(),
        source_hash
    );
    let generated_body_start_line = header.lines().count() + 2;
    let content = format!("{header}\n{body}");
    let source_spans = source_spans_for_output(doc, kind, body, &path, generated_body_start_line);

    Some(PlannedOutput {
        file: GeneratedFile {
            path,
            content,
            kind: GeneratedKind::Output(kind),
            source_path: Some(doc.package_relative_path.clone()),
        },
        source_spans,
    })
}

fn planned_output_path(
    package: &Package,
    doc: &ImplDoc,
    kind: OutputKind,
    state: &mut RunState,
) -> Option<std::path::PathBuf> {
    let resolved_rule =
        match resolve_output_rule_for_markdown(&doc.lang, &doc.markdown_relative_path, kind) {
            Ok(Some(rule)) => rule,
            Ok(None) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(doc.path.clone()),
                    format!(
                        "cannot resolve output rule for language `{}`",
                        doc.lang.key()
                    ),
                ));
                return None;
            }
            Err(message) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(doc.path.clone()),
                    format!("{message} in output rule resolution"),
                ));
                return None;
            }
        };
    let context = OutputPatternContext {
        source_out: path_pattern_value(&package.config.roots.source_out),
        test_out: path_pattern_value(&package.config.roots.test_out),
        module: resolved_rule.module_id.clone(),
        ext: resolved_rule.extension.clone(),
    };
    if let Some(pattern) = output_override_for(package, kind, &resolved_rule.module_id) {
        let relative = match expand_output_pattern(&pattern, &context) {
            Ok(relative) => Path::new(&relative).to_path_buf(),
            Err(message) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(doc.path.clone()),
                    format!("{message} in output path pattern `{pattern}`"),
                ));
                return None;
            }
        };

        return Some(package.root.join(relative));
    }

    if resolved_rule.origin == ResolvedOutputRuleOrigin::DescriptorSpecialFile {
        return Some(
            output_root_path(package, resolved_rule.root).join(&resolved_rule.relative_path),
        );
    }

    if let Some(pattern) = output_pattern_for(package, kind) {
        let relative = match expand_output_pattern(&pattern, &context) {
            Ok(relative) => Path::new(&relative).to_path_buf(),
            Err(message) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(doc.path.clone()),
                    format!("{message} in output path pattern `{pattern}`"),
                ));
                return None;
            }
        };

        return Some(package.root.join(relative));
    }

    Some(output_root_path(package, resolved_rule.root).join(&resolved_rule.relative_path))
}

fn output_override_for(package: &Package, kind: OutputKind, module_id: &str) -> Option<String> {
    package
        .config
        .output
        .overrides
        .iter()
        .find(|rule| rule.kind == kind && glob_match(&rule.match_pattern, module_id))
        .map(|rule| rule.path.clone())
}

fn output_pattern_for(package: &Package, kind: OutputKind) -> Option<String> {
    match kind {
        OutputKind::Source => package.config.output.source.clone(),
        OutputKind::Test => package.config.output.test.clone(),
    }
}

fn output_root_path(package: &Package, root: OutputRootKind) -> PathBuf {
    match root {
        OutputRootKind::Package => package.root.clone(),
        OutputRootKind::SourceOut => package.root.join(&package.config.roots.source_out),
        OutputRootKind::TestOut => package.root.join(&package.config.roots.test_out),
    }
}

fn path_pattern_value(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn expand_output_pattern(pattern: &str, context: &OutputPatternContext) -> Result<String, String> {
    let mut expanded = String::new();
    let mut chars = pattern.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    expanded.push('{');
                    continue;
                }

                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(value) => name.push(value),
                        None => return Err("unterminated output path placeholder".to_string()),
                    }
                }

                let value = match name.as_str() {
                    "source_out" => context.source_out.as_str(),
                    "test_out" => context.test_out.as_str(),
                    "module" => context.module.as_str(),
                    "ext" => context.ext.as_str(),
                    _ => return Err(format!("unknown output path placeholder `{{{name}}}`")),
                };
                expanded.push_str(value);
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    expanded.push('}');
                } else {
                    return Err("unescaped `}` in output path pattern".to_string());
                }
            }
            value => expanded.push(value),
        }
    }

    Ok(expanded)
}

fn source_spans_for_output(
    doc: &ImplDoc,
    kind: OutputKind,
    body: &str,
    generated_path: &Path,
    generated_body_start_line: usize,
) -> Vec<SourceSpan> {
    let blocks = contributing_blocks(doc, kind, body);
    let mut spans = Vec::with_capacity(blocks.len());
    let mut generated_line = generated_body_start_line;

    for block in blocks {
        let generated_line_count = rendered_block_line_count(&block.content);
        spans.push(SourceSpan {
            markdown_path: doc.path.clone(),
            markdown_start_line: block.content_start_line,
            markdown_end_line: block.content_end_line,
            generated_path: generated_path.to_path_buf(),
            generated_start_line: generated_line,
            generated_end_line: generated_line + generated_line_count - 1,
            output_kind: kind,
            extension_key: doc.lang.key().to_string(),
            fence_index: block.fence_index,
        });
        generated_line += generated_line_count + 1;
    }

    spans
}

fn contributing_blocks<'a>(doc: &'a ImplDoc, kind: OutputKind, body: &str) -> &'a [CodeFenceBlock] {
    let source_blocks = doc.source_blocks.as_slice();
    let test_blocks = doc.test_blocks.as_slice();

    match kind {
        OutputKind::Source => {
            if code_from_blocks(source_blocks) == body {
                source_blocks
            } else {
                &[]
            }
        }
        OutputKind::Test => {
            if code_from_blocks(test_blocks) == body {
                test_blocks
            } else if code_from_blocks(source_blocks) == body {
                source_blocks
            } else {
                &[]
            }
        }
    }
}

fn code_from_blocks(blocks: &[CodeFenceBlock]) -> String {
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

fn rendered_block_line_count(content: &str) -> usize {
    if content.is_empty() {
        1
    } else {
        content.lines().count()
    }
}
