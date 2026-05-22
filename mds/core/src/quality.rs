use crate::adapter::path_variants;
use crate::adapter::run_toolchain_command;
use crate::adapter::tool_available;
use crate::adapter::tool_output_detail;
use crate::adapter::ToolInvocation;
use crate::adapter::ToolRunOutput;
use crate::adapter::ToolRunStatus;
use crate::descriptor::ToolBehavior;
use crate::descriptor::ToolInputMode;
use crate::descriptor::ToolOutputMode;
use crate::descriptor::{self};
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::diff::unified_diff;
use crate::fs_utils::is_excluded;
use crate::markdown::{
    normalize_link_policy_text, validate_fix_result_text, validate_link_policy,
    validate_link_policy_text, validate_markdown_links, validate_wiki_link_targets,
};
use crate::model::DocKind;
use crate::model::DocProfile;
use crate::model::ImplDoc;
use crate::model::Lang;
use crate::model::OutputKind;
use crate::model::Package;
use crate::model::SourceMap;
use crate::model::SourceSpan;
use crate::package_sync::validate_source_overview_text;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

struct PreparedQualityInput {
    source: String,
    source_map: SourceMap,
}

#[derive(Debug, Clone, Copy)]

pub(crate) enum QualityOperation {
    Typecheck,
    Lint,
    Fix { check: bool },
    Test,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum QualitySlot {
    Typecheck,
    Lint,
    Fix,
    Test,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum QualityCommandSource {
    Config,
    Disabled,
    PackageManagerScript,
    PackageManager,
    DescriptorDefault,
    None,
}

impl Default for QualityCommandSource {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct QualityCommandResolution {
    pub command: Option<String>,
    pub source: QualityCommandSource,
}

#[derive(Debug, Clone, Default)]
struct ResolvedQualityCommand {
    command: Option<String>,
    config: crate::model::QualityConfig,
    source: QualityCommandSource,
}

impl QualityOperation {
    fn slot(self) -> QualitySlot {
        match self {
            Self::Typecheck => QualitySlot::Typecheck,
            Self::Lint => QualitySlot::Lint,
            Self::Fix { .. } => QualitySlot::Fix,
            Self::Test => QualitySlot::Test,
        }
    }
}

impl QualitySlot {
    fn command_kind(self) -> &'static str {
        match self {
            Self::Typecheck => "typecheck",
            Self::Lint => "lint",
            Self::Fix => "fix",
            Self::Test => "test",
        }
    }

    fn command<'a>(self, config: &'a crate::model::QualityConfig) -> Option<&'a str> {
        match self {
            Self::Typecheck => config.type_check.as_deref(),
            Self::Lint => config.lint.as_deref(),
            Self::Fix => config.fix.as_deref(),
            Self::Test => config.test.as_deref(),
        }
    }

    fn disabled(self, config: &crate::model::QualityConfig) -> bool {
        match self {
            Self::Typecheck => config.type_check_disabled,
            Self::Lint => config.lint_disabled,
            Self::Fix => config.fix_disabled,
            Self::Test => config.test_disabled,
        }
    }

    fn descriptor_default<'a>(self, descriptor: &'a descriptor::Descriptor) -> Option<&'a str> {
        match self {
            Self::Typecheck => descriptor.default_typecheck_command(),
            Self::Lint => descriptor.default_lint_command(),
            Self::Fix => descriptor.default_fix_command(),
            Self::Test => descriptor.default_test_command(),
        }
    }

    fn behavior(self, descriptor: &descriptor::Descriptor) -> ToolBehavior {
        match self {
            Self::Typecheck => descriptor.typecheck_behavior().clone(),
            Self::Lint => descriptor.lint_behavior().clone(),
            Self::Fix => descriptor.fix_behavior().clone(),
            Self::Test => descriptor.test_behavior().clone(),
        }
    }
}

impl ResolvedQualityCommand {
    fn is_declared(&self) -> bool {
        !matches!(self.source, QualityCommandSource::None)
    }
}

pub(crate) fn run_quality(
    package: &Package,
    docs: &[ImplDoc],
    operation: QualityOperation,
    state: &mut RunState,
) -> Result<(), String> {
    let package_manager = descriptor::package_manager_for_id(&package.package_manager_id);
    let package_manager_scripts = package_manager
        .as_ref()
        .map(|manager| descriptor::load_package_manager_scripts(manager, &package.root))
        .unwrap_or_default();
    match operation {
        QualityOperation::Typecheck | QualityOperation::Lint | QualityOperation::Test => {
            if matches!(operation, QualityOperation::Lint) {
                validate_link_policy(package, docs, state);
                validate_source_overview_link_policy(package, docs, state);
            }
            let mut any_resolved = false;
            for doc in docs {
                let resolved = resolve_quality_command(
                    package,
                    &doc.lang,
                    operation.slot(),
                    package_manager.as_ref(),
                    &package_manager_scripts,
                );
                if resolved.is_declared() {
                    any_resolved = true;
                }
                run_doc_quality(package, doc, operation, &resolved, state)?;
            }
            if !any_resolved && !docs.is_empty() {
                match operation {
                    QualityOperation::Lint => state.stdout.push_str(
                        "warning: no lint command resolved for the active runtime slot; running mds markdown validation only\n",
                    ),
                    QualityOperation::Typecheck => state.stdout.push_str(
                        "warning: no typecheck command resolved for the active runtime slot; skipping\n",
                    ),
                    QualityOperation::Test => state.stdout.push_str(
                        "warning: no test command resolved for the active runtime slot; skipping\n",
                    ),
                    QualityOperation::Fix { .. } => unreachable!(),
                }
            }
            if !state.has_errors() && !state.environment_missing {
                let name = match operation {
                    QualityOperation::Typecheck => "typecheck",
                    QualityOperation::Lint => "lint",
                    QualityOperation::Test => "test",
                    QualityOperation::Fix { .. } => unreachable!(),
                };
                state.stdout.push_str(&format!("{name} ok\n"));
            }
        }
        QualityOperation::Fix { check } => {
            fix_source_overview(package, docs, check, state)?;
            let mut any_resolved = false;
            for doc in docs {
                let resolved = resolve_quality_command(
                    package,
                    &doc.lang,
                    QualitySlot::Fix,
                    package_manager.as_ref(),
                    &package_manager_scripts,
                );
                if resolved.is_declared() {
                    any_resolved = true;
                }
                fix_doc(package, docs, doc, check, &resolved, state)?;
            }
            if !any_resolved && !docs.is_empty() {
                state.stdout.push_str(
                    "warning: no fix command resolved for the active runtime slot; applying mds markdown normalization only\n",
                );
            }
            if !check {
                validate_link_policy(package, docs, state);
                validate_wiki_link_targets(package, docs, state);
                validate_source_overview_link_policy(package, docs, state);
            }
            if !state.has_errors() && !state.environment_missing {
                state.stdout.push_str("lint --fix ok\n");
            }
        }
    }
    Ok(())
}

fn validate_source_overview_link_policy(package: &Package, docs: &[ImplDoc], state: &mut RunState) {
    let path = source_overview_path(package);
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    validate_source_overview_link_policy_text(package, docs, &path, &text, state);
}

fn validate_source_overview_link_policy_text(
    package: &Package,
    docs: &[ImplDoc],
    path: &Path,
    text: &str,
    state: &mut RunState,
) {
    if !package.config.check.markdown_links {
        return;
    }

    validate_markdown_links(path, text, state);
    let doc = source_overview_doc(package, path);
    validate_link_policy_text(package, docs, &doc, text, state);
}

fn fix_source_overview(
    package: &Package,
    docs: &[ImplDoc],
    check: bool,
    state: &mut RunState,
) -> Result<(), String> {
    if !package.config.check.markdown_links {
        return Ok(());
    }

    let path = source_overview_path(package);
    let old = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Ok(()),
    };
    let new = normalize_link_policy_text(package, &path, &old, package.config.link_policy);

    let mut validation_failed = false;
    if check || old != new {
        let mut validation_state = RunState::default();
        validate_source_overview_text(package, &path, &new, &mut validation_state);
        validate_source_overview_link_policy_text(
            package,
            docs,
            &path,
            &new,
            &mut validation_state,
        );
        validation_failed = validation_state.has_errors();
        if check || validation_failed {
            state.diagnostics.extend(validation_state.diagnostics);
        }
    }

    if old != new {
        state.stdout.push_str(&unified_diff(&path, &old, &new));
        if check {
            state.diagnostics.push(Diagnostic::error(
                Some(path.clone()),
                "lint --fix --check found markdown changes",
            ));
        } else if !validation_failed {
            fs::write(&path, new)
                .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
            state.generated.push(path);
        }
    }

    Ok(())
}

fn source_overview_path(package: &Package) -> PathBuf {
    descriptor::package_source_overview_markdown_path(&package.root, &package.config)
}

fn source_overview_doc(package: &Package, path: &Path) -> ImplDoc {
    let package_relative_path = path
        .strip_prefix(&package.root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf());
    let markdown_relative_path = path
        .strip_prefix(package.root.join(&package.config.roots.source_md))
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| package_relative_path.clone());

    ImplDoc {
        doc_kind: DocKind::Source,
        profile: DocProfile::Overview,
        lang: Lang::Other("md".to_string()),
        path: path.to_path_buf(),
        package_relative_path,
        markdown_relative_path,
        code: String::new(),
        source_code: String::new(),
        test_code: String::new(),
        source_blocks: Vec::new(),
        test_blocks: Vec::new(),
        covers: Vec::new(),
        normalized_input: String::new(),
    }
}

fn resolve_quality_command(
    package: &Package,
    lang: &Lang,
    slot: QualitySlot,
    package_manager: Option<&descriptor::PackageManagerManifest>,
    package_manager_scripts: &HashMap<String, String>,
) -> ResolvedQualityCommand {
    let config = package
        .config
        .quality
        .get(lang)
        .cloned()
        .unwrap_or_default();

    let resolution = resolve_quality_command_for_config(
        &config,
        lang,
        slot,
        package_manager,
        package_manager_scripts,
    );

    ResolvedQualityCommand {
        command: resolution.command,
        config,
        source: resolution.source,
    }
}

pub(crate) fn resolve_quality_command_for_config(
    config: &crate::model::QualityConfig,
    lang: &Lang,
    slot: QualitySlot,
    package_manager: Option<&descriptor::PackageManagerManifest>,
    package_manager_scripts: &HashMap<String, String>,
) -> QualityCommandResolution {
    let descriptor = descriptor::descriptor_for_key(lang.key());

    resolve_quality_command_for_config_with_descriptor(
        config,
        descriptor.as_ref(),
        lang,
        slot,
        package_manager,
        package_manager_scripts,
    )
}

pub(crate) fn resolve_quality_command_for_config_with_descriptor(
    config: &crate::model::QualityConfig,
    descriptor: Option<&descriptor::Descriptor>,
    lang: &Lang,
    slot: QualitySlot,
    package_manager: Option<&descriptor::PackageManagerManifest>,
    package_manager_scripts: &HashMap<String, String>,
) -> QualityCommandResolution {
    if slot.disabled(config) {
        return QualityCommandResolution {
            command: None,
            source: QualityCommandSource::Disabled,
        };
    }

    if let Some(command) = slot.command(config) {
        return QualityCommandResolution {
            command: Some(command.to_string()),
            source: QualityCommandSource::Config,
        };
    }

    let manager_applies = package_manager
        .is_some_and(|manager| descriptor::langs_match(&manager.resolved_lang(), lang));
    if manager_applies {
        if let Some(manager) = package_manager {
            if let Some(command) = descriptor::quality_script_command_from_scripts(
                manager,
                package_manager_scripts,
                slot.command_kind(),
            ) {
                return QualityCommandResolution {
                    command: Some(command),
                    source: QualityCommandSource::PackageManagerScript,
                };
            }
            if let Some(command) = manager.command(slot.command_kind()) {
                return QualityCommandResolution {
                    command: Some(command.to_string()),
                    source: QualityCommandSource::PackageManager,
                };
            }
        }
        if let Some(command) = descriptor.and_then(|descriptor| slot.descriptor_default(descriptor))
        {
            return QualityCommandResolution {
                command: Some(command.to_string()),
                source: QualityCommandSource::DescriptorDefault,
            };
        }
    }

    QualityCommandResolution {
        command: None,
        source: QualityCommandSource::None,
    }
}

fn run_doc_quality(
    package: &Package,
    doc: &ImplDoc,
    operation: QualityOperation,
    resolved: &ResolvedQualityCommand,
    state: &mut RunState,
) -> Result<(), String> {
    let Some(command) = resolved.command.as_deref() else {
        return Ok(());
    };
    let Some(descriptor) = quality_descriptor_for_doc(doc, state) else {
        return Ok(());
    };
    let path = temp_code_path(package, &doc.lang);
    let input = quality_input_from_markdown(doc, &path)?;
    let behavior = resolve_tool_behavior(&descriptor, operation.slot(), command, state);
    let _ = execute_quality_command(
        package,
        doc,
        command,
        &resolved.config,
        &behavior,
        &path,
        &input.source,
        Some(&input.source_map),
        state,
    )?;
    Ok(())
}

fn fix_doc(
    package: &Package,
    docs: &[ImplDoc],
    doc: &ImplDoc,
    check: bool,
    resolved: &ResolvedQualityCommand,
    state: &mut RunState,
) -> Result<(), String> {
    let old = fs::read_to_string(&doc.path)
        .map_err(|error| format!("failed to read {}: {error}", doc.path.display()))?;
    let mut replacements = Vec::new();
    if let Some(command) = resolved.command.as_deref() {
        let Some(descriptor) = quality_descriptor_for_doc(doc, state) else {
            return Ok(());
        };
        let behavior = resolve_tool_behavior(&descriptor, QualitySlot::Fix, command, state);
        let path = temp_code_path(package, &doc.lang);
        for block in code_block_ranges(&old) {
            let source_map = source_map_for_code_block(doc, &path, &block);
            if let Some(fixed) = execute_quality_command(
                package,
                doc,
                command,
                &resolved.config,
                &behavior,
                &path,
                block.content,
                Some(&source_map),
                state,
            )? {
                replacements.push((block.start, block.end, fixed));
            }
        }
    }
    let new = normalize_link_policy_text(
        package,
        &doc.path,
        &apply_replacements(&old, &replacements),
        package.config.link_policy,
    );
    let new = if package.config.check.markdown_links {
        new
    } else {
        apply_replacements(&old, &replacements)
    };
    let mut validation_failed = false;
    if check || old != new {
        let mut validation_state = RunState::default();
        validate_fix_result_text(package, docs, doc, &new, &mut validation_state);
        validation_failed = validation_state.has_errors();
        if check || validation_failed {
            state.diagnostics.extend(validation_state.diagnostics);
        }
    }
    if old != new {
        state.stdout.push_str(&unified_diff(&doc.path, &old, &new));
        if check {
            state.diagnostics.push(Diagnostic::error(
                Some(doc.path.clone()),
                "lint --fix --check found markdown changes",
            ));
        } else if !validation_failed {
            fs::write(&doc.path, new)
                .map_err(|error| format!("failed to write {}: {error}", doc.path.display()))?;
            state.generated.push(doc.path.clone());
        }
    }
    Ok(())
}

fn resolve_tool_behavior(
    descriptor: &descriptor::Descriptor,
    slot: QualitySlot,
    command: &str,
    state: &mut RunState,
) -> ToolBehavior {
    let slot_behavior = slot.behavior(descriptor);
    if slot_behavior_is_specific(&slot_behavior) {
        slot_behavior
    } else {
        descriptor::clear_descriptor_diagnostics();
        let behavior = descriptor::tool_behavior_for_command(command).unwrap_or(slot_behavior);
        state
            .diagnostics
            .extend(descriptor::drain_descriptor_diagnostics());
        behavior
    }
}

fn slot_behavior_is_specific(behavior: &ToolBehavior) -> bool {
    behavior.has_explicit_contract()
}

fn quality_descriptor_for_doc(
    doc: &ImplDoc,
    state: &mut RunState,
) -> Option<descriptor::Descriptor> {
    let descriptor = descriptor::descriptor_for_key(doc.lang.key());
    if descriptor.is_none() {
        state.diagnostics.push(Diagnostic::error(
            Some(doc.path.clone()),
            format!(
                "quality runtime requires a package-local descriptor for language `{}` under `.mds/descriptors/languages`",
                doc.lang.key()
            ),
        ));
    }
    descriptor
}

fn execute_quality_command(
    package: &Package,
    doc: &ImplDoc,
    command: &str,
    config: &crate::model::QualityConfig,
    behavior: &ToolBehavior,
    input_path: &Path,
    source: &str,
    source_map: Option<&SourceMap>,
    state: &mut RunState,
) -> Result<Option<String>, String> {
    if needs_tempfile(behavior) || behavior.append_file_arg() {
        if let Some(parent) = input_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(input_path, source)
            .map_err(|error| format!("failed to write {}: {error}", input_path.display()))?;
    }

    let file_arg = behavior.append_file_arg().then_some(input_path);
    let stdin = match behavior.input_mode() {
        ToolInputMode::Stdin => Some(source),
        ToolInputMode::TempFile | ToolInputMode::Inline => None,
    };
    let inline_arg = match behavior.input_mode() {
        ToolInputMode::Inline => Some(source),
        ToolInputMode::TempFile | ToolInputMode::Stdin => None,
    };

    let status = run_toolchain_command(ToolInvocation {
        command,
        file_arg,
        cwd: &package.root,
        stdin,
        inline_arg,
    })?;

    warn_missing_optional_tools(config, &doc.path, state);

    match status {
        ToolRunStatus::EmptyCommand => Ok(None),
        ToolRunStatus::MissingTool(program) => {
            state.environment_missing = true;
            state.diagnostics.push(Diagnostic::error(
                Some(doc.path.clone()),
                format!(
                    "LINT001_TOOLCHAIN_FAILED: required toolchain `{program}` is not available"
                ),
            ));
            Ok(None)
        }
        ToolRunStatus::Completed(output) => {
            if !output.success {
                report_tool_failure(
                    &output,
                    behavior,
                    &doc.path,
                    input_path,
                    source_map,
                    &package.root,
                    state,
                )?;
                return Ok(None);
            }
            let fixed = match behavior.output_mode() {
                ToolOutputMode::None => None,
                ToolOutputMode::Stdout => Some(output.stdout),
                ToolOutputMode::TempFile => {
                    Some(fs::read_to_string(input_path).map_err(|error| {
                        format!("failed to read {}: {error}", input_path.display())
                    })?)
                }
            };
            Ok(fixed)
        }
    }
}

fn warn_missing_optional_tools(
    config: &crate::model::QualityConfig,
    path: &std::path::Path,
    state: &mut RunState,
) {
    for optional in &config.optional {
        if !tool_available(optional) {
            state.diagnostics.push(Diagnostic::warning(
                Some(path.to_path_buf()),
                format!("optional toolchain `{optional}` is not available"),
            ));
        }
    }
}

fn report_tool_failure(
    output: &ToolRunOutput,
    behavior: &ToolBehavior,
    markdown_path: &std::path::Path,
    input_path: &std::path::Path,
    source_map: Option<&SourceMap>,
    cwd: &std::path::Path,
    state: &mut RunState,
) -> Result<(), String> {
    let detail = tool_output_detail(output);
    let diagnostics = capture_tool_diagnostics(
        &detail,
        behavior,
        markdown_path,
        input_path,
        source_map,
        cwd,
    )?;
    if diagnostics.is_empty() {
        let rendered = source_map
            .and_then(|map| {
                remap_generic_tool_failure(&detail, input_path, markdown_path, map, cwd)
            })
            .unwrap_or_else(|| detail.clone());
        state.diagnostics.push(Diagnostic::error(
            Some(markdown_path.to_path_buf()),
            format!("LINT001_TOOLCHAIN_FAILED: toolchain command failed: {rendered}"),
        ));
        return Ok(());
    }
    state.diagnostics.extend(diagnostics);
    Ok(())
}

fn remap_generic_tool_failure(
    detail: &str,
    input_path: &Path,
    _markdown_path: &Path,
    source_map: &SourceMap,
    cwd: &Path,
) -> Option<String> {
    let regex = Regex::new(r"^(?P<path>.+?):(?P<line>\d+):(?P<column>\d+):(?P<rest>.*)$").ok()?;
    let captures = regex.captures(detail.lines().next()?)?;
    let line = captures.name("line")?.as_str().parse::<usize>().ok()?;
    let column = captures.name("column")?.as_str();
    let rest = captures.name("rest")?.as_str();
    let generated_path = diagnostic_input_path(
        captures.name("path").map(|value| value.as_str()),
        input_path,
        cwd,
    )?;
    let (remapped_path, markdown_line) = source_map.remap_generated_line(generated_path, line)?;
    Some(format!(
        "{}:{}:{}:{}",
        remapped_path.display(),
        markdown_line,
        column,
        rest
    ))
}

fn capture_tool_diagnostics(
    detail: &str,
    behavior: &ToolBehavior,
    markdown_path: &Path,
    input_path: &std::path::Path,
    source_map: Option<&SourceMap>,
    cwd: &std::path::Path,
) -> Result<Vec<Diagnostic>, String> {
    let mut diagnostics = Vec::new();
    for raw_line in detail.lines().filter(|line| !line.trim().is_empty()) {
        for rule in &behavior.diagnostics {
            let regex = Regex::new(&rule.pattern)
                .map_err(|error| format!("invalid diagnostic regex `{}`: {error}", rule.pattern))?;
            let Some(captures) = regex.captures(raw_line) else {
                continue;
            };
            let line = captures
                .name(&rule.line_group)
                .and_then(|value| value.as_str().parse::<usize>().ok())
                .map(|value| apply_line_offset(value, rule.line_offset));
            let captured_path = captures.name(&rule.path_group).map(|value| value.as_str());
            let generated_path =
                line.and_then(|_| diagnostic_input_path(captured_path, input_path, cwd));
            let remapped = line
                .and_then(|generated_line| {
                    generated_path.and_then(|generated_path| {
                        source_map.and_then(|map| {
                            map.remap_generated_line(generated_path, generated_line)
                        })
                    })
                })
                .map(|(path, markdown_line)| (path.to_path_buf(), markdown_line));
            if line.is_some() && generated_path.is_some() && remapped.is_none() {
                diagnostics.push(Diagnostic::error(
                    Some(markdown_path.to_path_buf()),
                    format!("LINT001_TOOLCHAIN_FAILED: toolchain command failed: {raw_line}"),
                ));
                break;
            }
            let mut diagnostic = match rule.severity.as_str() {
                "warning" => Diagnostic::warning(
                    captured_diagnostic_path(captured_path, line, input_path, cwd),
                    capture_message(&captures, rule, raw_line),
                ),
                _ => Diagnostic::error(
                    captured_diagnostic_path(captured_path, line, input_path, cwd),
                    capture_message(&captures, rule, raw_line),
                ),
            };
            if let Some(column) = captures
                .name(&rule.column_group)
                .and_then(|value| value.as_str().parse::<usize>().ok())
            {
                diagnostic = diagnostic.at_column(column);
            }
            let remapped_path = remapped.as_ref().map(|(path, _)| path.clone());
            let remapped_line = remapped.as_ref().map(|(_, markdown_line)| *markdown_line);
            if let (Some(path), Some(markdown_line)) = (remapped_path, remapped_line) {
                diagnostic.path = Some(path);
                diagnostic = diagnostic.at_line(markdown_line);
            } else if let Some(line) = line {
                diagnostic = diagnostic.at_line(line);
            }
            diagnostics.push(diagnostic);
            break;
        }
    }
    Ok(diagnostics)
}

fn captured_diagnostic_path(
    captured_path: Option<&str>,
    line: Option<usize>,
    input_path: &Path,
    cwd: &Path,
) -> Option<PathBuf> {
    match captured_path {
        Some(path) => diagnostic_input_path(Some(path), input_path, cwd)
            .map(Path::to_path_buf)
            .or_else(|| Some(PathBuf::from(path))),
        None => line.map(|_| input_path.to_path_buf()),
    }
}

fn diagnostic_input_path<'a>(
    captured_path: Option<&str>,
    input_path: &'a Path,
    cwd: &Path,
) -> Option<&'a Path> {
    match captured_path {
        Some(path) => path_variants(input_path, cwd)
            .into_iter()
            .any(|variant| variant == path)
            .then_some(input_path),
        None => Some(input_path),
    }
}

fn capture_message(
    captures: &regex::Captures<'_>,
    rule: &crate::descriptor::DiagnosticCaptureRule,
    raw_line: &str,
) -> String {
    captures
        .name(&rule.message_group)
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| raw_line.to_string())
}

fn apply_line_offset(line: usize, offset: isize) -> usize {
    if offset >= 0 {
        line.saturating_add(offset as usize)
    } else {
        line.saturating_sub(offset.unsigned_abs())
    }
}

fn needs_tempfile(behavior: &ToolBehavior) -> bool {
    matches!(behavior.input_mode(), ToolInputMode::TempFile)
        || matches!(behavior.output_mode(), ToolOutputMode::TempFile)
}

fn temp_code_path(package: &Package, lang: &Lang) -> PathBuf {
    let ext = lang.file_ext();
    let file_name = format!("source.{ext}");
    let path = package.root.join(".build/mds/tmp").join(&file_name);
    if is_excluded(&package.root, &path, &package.config.excludes) {
        package.root.join(".build/mds-tmp").join(file_name)
    } else {
        path
    }
}

#[derive(Debug)]
struct CodeBlock<'a> {
    fence_index: usize,
    start: usize,
    end: usize,
    content: &'a str,
    content_start_line: usize,
    content_end_line: usize,
}

fn code_block_ranges(text: &str) -> Vec<CodeBlock<'_>> {
    let mut ranges = Vec::new();
    let mut in_block = false;
    let mut content_start = 0;
    let mut content_start_line = 1;
    let mut fence_index = 0;
    let mut cursor = 0;
    let mut line_number: usize = 1;
    for line in text.split_inclusive('\n') {
        let line_start = cursor;
        cursor += line.len();
        if line.trim_start().starts_with("```") {
            if in_block {
                ranges.push(CodeBlock {
                    fence_index,
                    start: content_start,
                    end: line_start,
                    content: &text[content_start..line_start],
                    content_start_line,
                    content_end_line: line_number.saturating_sub(1),
                });
                fence_index += 1;
                in_block = false;
            } else {
                in_block = true;
                content_start = cursor;
                content_start_line = line_number + 1;
            }
        }
        line_number += 1;
    }
    ranges
}

fn quality_input_from_markdown(
    doc: &ImplDoc,
    input_path: &Path,
) -> Result<PreparedQualityInput, String> {
    let text = fs::read_to_string(&doc.path)
        .map_err(|error| format!("failed to read {}: {error}", doc.path.display()))?;
    let blocks = code_block_ranges(&text);
    if blocks.is_empty() {
        return Ok(PreparedQualityInput {
            source: doc.code.clone(),
            source_map: SourceMap::new(),
        });
    }
    let mut output = String::new();
    let mut source_map = SourceMap::new();
    let mut output_line = 1;
    for block in blocks {
        output.push_str(block.content);
        if let Some(span) = source_span_for_code_block(doc, input_path, &block, output_line) {
            source_map.extend([span]);
        }
        output_line += content_line_count(block.content);
    }
    Ok(PreparedQualityInput {
        source: output,
        source_map,
    })
}

fn source_map_for_code_block(doc: &ImplDoc, input_path: &Path, block: &CodeBlock<'_>) -> SourceMap {
    let mut source_map = SourceMap::new();
    if let Some(span) = source_span_for_code_block(doc, input_path, block, 1) {
        source_map.extend([span]);
    }
    source_map
}

fn source_span_for_code_block(
    doc: &ImplDoc,
    input_path: &Path,
    block: &CodeBlock<'_>,
    generated_start_line: usize,
) -> Option<SourceSpan> {
    let line_count = content_line_count(block.content);
    if line_count == 0 {
        return None;
    }
    Some(SourceSpan {
        markdown_path: doc.path.clone(),
        markdown_start_line: block.content_start_line,
        markdown_end_line: block.content_end_line,
        generated_path: input_path.to_path_buf(),
        generated_start_line,
        generated_end_line: generated_start_line + line_count - 1,
        output_kind: quality_output_kind(doc, block.fence_index),
        extension_key: doc.lang.key().to_string(),
        fence_index: block.fence_index,
    })
}

fn quality_output_kind(doc: &ImplDoc, fence_index: usize) -> OutputKind {
    if doc
        .test_blocks
        .iter()
        .any(|block| block.fence_index == fence_index)
    {
        OutputKind::Test
    } else if doc
        .source_blocks
        .iter()
        .any(|block| block.fence_index == fence_index)
    {
        OutputKind::Source
    } else {
        match doc.doc_kind {
            DocKind::Test => OutputKind::Test,
            DocKind::Source => OutputKind::Source,
        }
    }
}

fn content_line_count(text: &str) -> usize {
    text.lines().count()
}

fn apply_replacements(old: &str, replacements: &[(usize, usize, String)]) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    for (start, end, replacement) in replacements {
        output.push_str(&old[cursor..*start]);
        output.push_str(replacement);
        cursor = *end;
    }
    output.push_str(&old[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::{resolve_quality_command_for_config, QualityCommandSource, QualitySlot};
    use crate::model::{Lang, QualityConfig};
    use std::collections::HashMap;

    #[test]
    fn descriptor_default_is_unresolved_without_active_package_manager() {
        let resolution = resolve_quality_command_for_config(
            &QualityConfig::default(),
            &Lang::Other("ts".to_string()),
            QualitySlot::Lint,
            None,
            &HashMap::new(),
        );

        assert_eq!(resolution.command, None);
        assert_eq!(resolution.source, QualityCommandSource::None);
    }

    #[test]
    fn explicit_config_command_still_applies_without_active_package_manager() {
        let resolution = resolve_quality_command_for_config(
            &QualityConfig {
                lint: Some("custom-lint".to_string()),
                ..QualityConfig::default()
            },
            &Lang::Other("ts".to_string()),
            QualitySlot::Lint,
            None,
            &HashMap::new(),
        );

        assert_eq!(resolution.command.as_deref(), Some("custom-lint"));
        assert_eq!(resolution.source, QualityCommandSource::Config);
    }
}
