use crate::diagnostics::Diagnostic;
use crate::fs_utils::glob_match;
use crate::model::{Config, InitQualityCommands, Lang, OutputKind};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Descriptor {
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub match_suffixes: Vec<String>,
    pub language: LanguageSection,
    pub files: FileRules,
    #[serde(default)]
    pub special_files: Vec<SpecialFileRule>,
    #[serde(default)]
    pub imports: ImportSection,
    #[serde(default)]
    pub syntax: SyntaxSection,
    #[serde(default)]
    pub scaffold: ScaffoldSection,
    #[serde(default)]
    pub tooling: ToolingSection,
    #[serde(default)]
    pub quality_defaults: QualityDefaults,
    #[serde(default)]
    pub tool_profiles: ToolProfiles,
}

#[derive(Debug, Clone)]
struct DescriptorRegistry {
    by_id: HashMap<String, Descriptor>,
    origins: HashMap<String, DescriptorOrigin>,
    alias_to_id: HashMap<String, String>,
    suffix_to_id: Vec<(String, String)>,
    path_suffix_to_id: Vec<(String, String)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum FenceLabelResolution {
    Resolved(Lang),
    Unknown,
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum MarkdownPathLangResolution {
    Resolved { lang: Lang, matched_suffix: String },
    Unknown,
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ToolManifest {
    pub id: String,
    #[serde(default)]
    pub match_prefixes: Vec<String>,
    #[serde(default)]
    pub behavior: ToolBehavior,
}

#[derive(Debug, Clone)]
struct ToolRegistry {
    by_id: HashMap<String, ToolManifest>,
    origins: HashMap<String, DescriptorOrigin>,
    prefix_to_id: Vec<(String, String)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageManagerManifest {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub display_name: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub metadata_files: Vec<String>,
    #[serde(default)]
    pub lockfiles: Vec<String>,
    #[serde(default)]
    pub metadata_reader: String,
    #[serde(default)]
    pub commands: PackageManagerCommands,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PackageManagerCommands {
    #[serde(default)]
    pub install: Option<String>,
    #[serde(default)]
    pub build: Option<String>,
    #[serde(default)]
    pub typecheck: Option<String>,
    #[serde(default)]
    pub lint: Option<String>,
    #[serde(default)]
    pub test: Option<String>,
}

#[derive(Debug, Clone)]
struct PackageManagerRegistry {
    by_id: HashMap<String, PackageManagerManifest>,
    origins: HashMap<String, DescriptorOrigin>,
    alias_to_id: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DescriptorSourceKind {
    PackageLocal,
    Workspace,
    Local,
    Git,
    Global,
    Unknown,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DescriptorKind {
    Language,
    Tool,
    PackageManager,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DescriptorOrigin {
    pub source_id: String,
    pub source_kind: DescriptorSourceKind,
    pub root: Option<PathBuf>,
    pub file: Option<PathBuf>,
    pub priority: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DescriptorRegistryEntry {
    pub kind: DescriptorKind,
    pub id: String,
    pub origin: DescriptorOrigin,
}

#[derive(Debug, Clone)]
pub struct DescriptorRegistryReport {
    pub entries: Vec<DescriptorRegistryEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedLanguageDescriptor {
    pub id: String,
    pub aliases: Vec<String>,
    pub match_suffixes: Vec<String>,
    pub primary_ext: String,
    pub vscode_id: Option<String>,
    pub origin: DescriptorOrigin,
}

#[derive(Debug, Clone)]
struct DescriptorSource {
    id: String,
    kind: DescriptorSourceKind,
    root: Option<PathBuf>,
    priority: usize,
}

#[derive(Debug, Clone)]
struct LoadedDescriptor<T> {
    value: T,
    origin: DescriptorOrigin,
}

#[derive(Debug, Deserialize)]
struct DescriptorSourcesFile {
    #[serde(default)]
    sources: Vec<DescriptorSourceConfig>,
}

#[derive(Debug, Deserialize)]
struct DescriptorSourceConfig {
    #[serde(default, alias = "name")]
    id: Option<String>,
    #[serde(default, rename = "type", alias = "kind")]
    source_type: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DescriptorSourceLockFile {
    #[serde(default)]
    sources: Vec<DescriptorSourceLockEntry>,
}

#[derive(Debug, Deserialize)]
struct DescriptorSourceLockEntry {
    #[serde(default, alias = "name")]
    id: Option<String>,
    #[serde(default)]
    resolved_path: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SpecialFileRule {
    #[serde(rename = "match")]
    pub match_path: String,
    pub kind: String,
    pub output: String,
    #[serde(default)]
    pub root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LanguageSection {
    pub primary_ext: String,
    #[serde(default)]
    pub vscode_id: Option<String>,
    #[serde(default)]
    pub root_module_markdown_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FileRules {
    pub source: FileRule,
    pub test: FileRule,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FileRule {
    #[serde(default)]
    pub strip_lang_ext: bool,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
    pub extension: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OutputRootKind {
    Package,
    SourceOut,
    TestOut,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ResolvedOutputRuleOrigin {
    DescriptorSpecialFile,
    DescriptorDefault,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ResolvedOutputRule {
    pub module_id: String,
    pub extension: String,
    pub relative_path: PathBuf,
    pub root: OutputRootKind,
    pub origin: ResolvedOutputRuleOrigin,
}

pub const OVERVIEW_MARKDOWN_NAME: &str = "overview.md";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PackageOverviewKind {
    Source,
    Test,
}

impl PackageOverviewKind {
    fn markdown_root(self, config: &Config) -> &Path {
        match self {
            Self::Source => config.roots.source_md.as_path(),
            Self::Test => config.roots.test_md.as_path(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ImportSection {
    #[serde(default)]
    pub style: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct SyntaxSection {
    #[serde(default)]
    pub imports: Vec<LinePattern>,
    #[serde(default)]
    pub top_level_keywords: Vec<String>,
    #[serde(default)]
    pub code_block_merge_prefixes: Vec<String>,
    #[serde(default)]
    pub comment_prefixes: Vec<String>,
    #[serde(default)]
    pub doc_comment_prefixes: Vec<String>,
    #[serde(default)]
    pub doc_string_delimiters: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct LinePattern {
    pub starts_with: String,
    #[serde(default)]
    pub contains: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ScaffoldSection {
    #[serde(default)]
    pub fence_lang: Option<String>,
    #[serde(default)]
    pub source_body: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ToolingSection {
    #[serde(default)]
    pub typecheck: ToolBehavior,
    #[serde(default)]
    pub lint: ToolBehavior,
    #[serde(default)]
    pub fix: ToolBehavior,
    #[serde(default)]
    pub test: ToolBehavior,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct QualityDefaults {
    #[serde(default)]
    pub typecheck: Option<String>,
    #[serde(default)]
    pub lint: Option<String>,
    #[serde(default)]
    pub fix: Option<String>,
    #[serde(default)]
    pub test: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ToolProfiles {
    #[serde(default)]
    pub typecheck: HashMap<String, ToolProfile>,
    #[serde(default)]
    pub lint: HashMap<String, ToolProfile>,
    #[serde(default)]
    pub fix: HashMap<String, ToolProfile>,
    #[serde(default)]
    pub test: HashMap<String, ToolProfile>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ToolProfile {
    pub command: String,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub optional: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolBehavior {
    pub input: String,
    pub output: String,
    pub append_file_arg: Option<bool>,
    pub diagnostics: Vec<DiagnosticCaptureRule>,
    input_explicit: bool,
    output_explicit: bool,
    append_file_arg_explicit: bool,
    diagnostics_explicit: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawToolBehavior {
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    append_file_arg: Option<bool>,
    #[serde(default)]
    diagnostics: Option<Vec<DiagnosticCaptureRule>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DiagnosticCaptureRule {
    pub pattern: String,
    #[serde(default = "default_path_group")]
    pub path_group: String,
    #[serde(default = "default_line_group")]
    pub line_group: String,
    #[serde(default = "default_column_group")]
    pub column_group: String,
    #[serde(default = "default_message_group")]
    pub message_group: String,
    #[serde(default = "default_severity_value")]
    pub severity: String,
    #[serde(default)]
    pub line_offset: isize,
}

impl Default for ToolBehavior {
    fn default() -> Self {
        Self {
            input: default_input_mode(),
            output: default_output_mode(),
            append_file_arg: None,
            diagnostics: Vec::new(),
            input_explicit: false,
            output_explicit: false,
            append_file_arg_explicit: false,
            diagnostics_explicit: false,
        }
    }
}

impl<'de> Deserialize<'de> for ToolBehavior {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawToolBehavior::deserialize(deserializer)?;
        let input_explicit = raw.input.is_some();
        let output_explicit = raw.output.is_some();
        let append_file_arg_explicit = raw.append_file_arg.is_some();
        let diagnostics_explicit = raw.diagnostics.is_some();
        Ok(Self {
            input: raw.input.unwrap_or_else(default_input_mode),
            output: raw.output.unwrap_or_else(default_output_mode),
            append_file_arg: raw.append_file_arg,
            diagnostics: raw.diagnostics.unwrap_or_default(),
            input_explicit,
            output_explicit,
            append_file_arg_explicit,
            diagnostics_explicit,
        })
    }
}

impl Descriptor {
    pub fn matches_code_block_merge_start(&self, line: &str) -> bool {
        self.syntax
            .code_block_merge_prefixes
            .iter()
            .any(|prefix| line.starts_with(prefix))
    }

    pub fn is_root_module_markdown_name(&self, name: &str) -> bool {
        let normalized_path = self.normalized_markdown_module_path(Path::new(name));
        let normalized_name = normalized_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        self.language
            .root_module_markdown_names
            .iter()
            .any(|candidate| {
                candidate == name || (!candidate.ends_with(".md") && candidate == normalized_name)
            })
    }

    pub fn default_root_module_markdown_name(&self) -> String {
        self.language
            .root_module_markdown_names
            .first()
            .cloned()
            .map(|candidate| {
                if candidate.ends_with(".md") {
                    candidate
                } else {
                    format!("{candidate}.{}.md", self.preferred_markdown_suffix())
                }
            })
            .unwrap_or_else(|| format!("index.{}.md", self.preferred_markdown_suffix()))
    }

    fn file_rule(&self, kind: OutputKind) -> &FileRule {
        match kind {
            OutputKind::Source => &self.files.source,
            OutputKind::Test => &self.files.test,
        }
    }

    fn normalized_markdown_module_path(&self, markdown_relative_path: &Path) -> PathBuf {
        let relative = markdown_relative_without_md(markdown_relative_path);
        let parent = relative.parent().map(Path::to_path_buf).unwrap_or_default();
        let name = relative
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let stripped = self
            .markdown_path_suffixes()
            .into_iter()
            .filter_map(|suffix| {
                name.strip_suffix(&format!(".{suffix}"))
                    .map(|value| (suffix.len(), value))
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, value)| value)
            .unwrap_or(name);
        if parent.as_os_str().is_empty() {
            PathBuf::from(stripped)
        } else {
            parent.join(stripped)
        }
    }

    fn markdown_module_path(&self, markdown_relative_path: &Path) -> String {
        self.normalized_markdown_module_path(markdown_relative_path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn markdown_module_id(&self, markdown_relative_path: &Path) -> String {
        self.markdown_module_path(markdown_relative_path)
            .replace('/', ".")
    }

    fn special_file_rule(
        &self,
        markdown_relative_path: &Path,
        kind: OutputKind,
    ) -> Result<Option<ResolvedOutputRule>, String> {
        let relative = markdown_relative_path.to_string_lossy().replace('\\', "/");
        let module_id = self.markdown_module_path(markdown_relative_path);
        for rule in &self.special_files {
            if !matches_special_file_kind(&rule.kind, kind)
                || !glob_match(&rule.match_path, &relative)
            {
                continue;
            }
            return Ok(Some(ResolvedOutputRule {
                module_id,
                extension: self.file_rule(kind).extension.clone(),
                relative_path: PathBuf::from(&rule.output),
                root: parse_output_root(rule.root.as_deref(), kind)?,
                origin: ResolvedOutputRuleOrigin::DescriptorSpecialFile,
            }));
        }
        Ok(None)
    }

    fn default_output_rule(
        &self,
        markdown_relative_path: &Path,
        kind: OutputKind,
    ) -> ResolvedOutputRule {
        let file_rule = self.file_rule(kind);
        ResolvedOutputRule {
            module_id: self.markdown_module_path(markdown_relative_path),
            extension: file_rule.extension.clone(),
            relative_path: output_relative_path_for_rule(markdown_relative_path, file_rule),
            root: match kind {
                OutputKind::Source => OutputRootKind::SourceOut,
                OutputKind::Test => OutputRootKind::TestOut,
            },
            origin: ResolvedOutputRuleOrigin::DescriptorDefault,
        }
    }

    fn resolve_output_rule(
        &self,
        markdown_relative_path: &Path,
        kind: OutputKind,
    ) -> Result<ResolvedOutputRule, String> {
        if let Some(rule) = self.special_file_rule(markdown_relative_path, kind)? {
            return Ok(rule);
        }
        Ok(self.default_output_rule(markdown_relative_path, kind))
    }

    pub fn render_source_block(&self, matched_suffix: Option<&str>) -> String {
        let fence_lang = match self.scaffold.fence_lang.as_deref() {
            Some("matched-suffix") => matched_suffix.unwrap_or(&self.language.primary_ext),
            Some(value) if !value.is_empty() => value,
            _ => &self.language.primary_ext,
        };
        let body = if self.scaffold.source_body.trim().is_empty() {
            let prefix = self
                .syntax
                .comment_prefixes
                .first()
                .map(String::as_str)
                .unwrap_or("//");
            format!("{prefix} Implement your feature here.")
        } else {
            self.scaffold.source_body.trim_end().to_string()
        };
        format!("```{fence_lang}\n{body}\n```")
    }

    pub fn render_import(&self, row: &HashMap<String, String>) -> Option<String> {
        let style = self.imports.style.as_str();
        if style.is_empty() || style == "none" {
            return None;
        }

        let from = normalize_import_cell(row.get("from").map(String::as_str));
        let target = normalize_import_cell(row.get("target").map(String::as_str));
        let symbols = split_import_symbols(row.get("symbols").map(String::as_str));
        let via = normalize_import_cell(row.get("via").map(String::as_str));
        let via = (!via.is_empty()).then_some(via.as_str());

        match style {
            "typescript" if !target.is_empty() => render_typescript_import(&target, &symbols, via),
            "python" if !target.is_empty() => Some(render_python_import(&target, &symbols)),
            "rust" => render_rust_import(&target, &symbols),
            "go" if !target.is_empty() => Some(format!("import \"{target}\"")),
            "java" if !target.is_empty() => render_java_import(&target, &symbols),
            "csharp" if !target.is_empty() => Some(format!("using {target};")),
            "c-include" if !target.is_empty() => Some(render_c_include(from.as_str(), &target)),
            "dart" if !target.is_empty() => Some(render_dart_import(&target, &symbols)),
            "ruby" if !target.is_empty() => Some(render_ruby_import(from.as_str(), &target)),
            "scss" if !target.is_empty() => Some(render_scss_import(&target, via)),
            "zig" if !target.is_empty() => Some(render_zig_import(&target, &symbols, via)),
            "mojo" if !target.is_empty() => Some(render_python_import(&target, &symbols)),
            _ => None,
        }
    }

    pub fn lint_behavior(&self) -> &ToolBehavior {
        &self.tooling.lint
    }

    pub fn typecheck_behavior(&self) -> &ToolBehavior {
        &self.tooling.typecheck
    }

    pub fn fix_behavior(&self) -> &ToolBehavior {
        &self.tooling.fix
    }

    pub fn test_behavior(&self) -> &ToolBehavior {
        &self.tooling.test
    }

    pub fn default_lint_command(&self) -> Option<&str> {
        self.quality_defaults.lint.as_deref()
    }

    pub fn default_typecheck_command(&self) -> Option<&str> {
        self.quality_defaults.typecheck.as_deref()
    }

    pub fn default_fix_command(&self) -> Option<&str> {
        self.quality_defaults.fix.as_deref()
    }

    pub fn default_test_command(&self) -> Option<&str> {
        self.quality_defaults.test.as_deref()
    }

    #[allow(dead_code)]
    pub fn typecheck_profile(&self, key: &str) -> Option<&ToolProfile> {
        self.tool_profiles.typecheck.get(key)
    }

    fn all_match_suffixes(&self) -> Vec<&str> {
        let mut suffixes: Vec<&str> = self.match_suffixes.iter().map(String::as_str).collect();
        if !suffixes.contains(&self.language.primary_ext.as_str()) {
            suffixes.push(&self.language.primary_ext);
        }
        suffixes
    }

    fn markdown_path_suffixes(&self) -> Vec<&str> {
        if self.match_suffixes.is_empty() {
            vec![self.language.primary_ext.as_str()]
        } else {
            self.match_suffixes.iter().map(String::as_str).collect()
        }
    }

    fn preferred_markdown_suffix(&self) -> &str {
        self.markdown_path_suffixes()
            .into_iter()
            .next()
            .unwrap_or(self.language.primary_ext.as_str())
    }
}

fn normalize_import_cell(value: Option<&str>) -> String {
    let value = value.unwrap_or_default().trim();
    if value.is_empty() || value == "-" {
        return String::new();
    }
    let value = value.trim_matches('`');
    if let Some((label, _link)) = markdown_link_parts(value) {
        return label.to_string();
    }
    value.to_string()
}

fn split_import_symbols(value: Option<&str>) -> Vec<String> {
    normalize_import_cell(value)
        .trim_matches(['{', '}'])
        .split(',')
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty() && *symbol != "-")
        .map(|symbol| symbol.to_string())
        .collect()
}

fn markdown_link_parts(value: &str) -> Option<(&str, &str)> {
    if !value.starts_with('[') || !value.ends_with(')') {
        return None;
    }
    let middle = value.find("](")?;
    Some((&value[1..middle], &value[middle + 2..value.len() - 1]))
}

fn render_typescript_import(target: &str, symbols: &[String], via: Option<&str>) -> Option<String> {
    if symbols.is_empty() {
        return Some(format!("import '{target}';"));
    }
    match via.unwrap_or_default() {
        "default" if symbols.len() == 1 => Some(format!("import {} from '{target}';", symbols[0])),
        "namespace" if symbols.len() == 1 => {
            Some(format!("import * as {} from '{target}';", symbols[0]))
        }
        "type" => Some(format!(
            "import type {{ {} }} from '{target}';",
            symbols.join(", ")
        )),
        _ => Some(format!(
            "import {{ {} }} from '{target}';",
            symbols.join(", ")
        )),
    }
}

fn render_python_import(target: &str, symbols: &[String]) -> String {
    if symbols.is_empty() {
        format!("import {target}")
    } else {
        format!("from {target} import {}", symbols.join(", "))
    }
}

fn render_rust_import(target: &str, symbols: &[String]) -> Option<String> {
    if target.is_empty() && symbols.is_empty() {
        return None;
    }
    if symbols.is_empty() {
        return Some(format!("use {target};"));
    }
    Some(format!("use {target}::{{{}}};", symbols.join(", ")))
}

fn render_java_import(target: &str, symbols: &[String]) -> Option<String> {
    if symbols.is_empty() {
        return Some(format!("import {target};"));
    }
    Some(
        symbols
            .iter()
            .map(|symbol| format!("import {target}.{symbol};"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn render_c_include(from: &str, target: &str) -> String {
    if matches!(from, "internal" | "workspace") {
        format!("#include \"{target}\"")
    } else {
        format!("#include <{target}>")
    }
}

fn render_dart_import(target: &str, symbols: &[String]) -> String {
    if symbols.is_empty() {
        format!("import '{target}';")
    } else {
        format!("import '{target}' show {};", symbols.join(", "))
    }
}

fn render_ruby_import(from: &str, target: &str) -> String {
    if matches!(from, "internal" | "workspace") {
        format!("require_relative '{target}'")
    } else {
        format!("require '{target}'")
    }
}

fn render_scss_import(target: &str, via: Option<&str>) -> String {
    match via.unwrap_or_default() {
        alias if alias.starts_with("as ") => format!("@use '{target}' {alias};"),
        _ => format!("@use '{target}';"),
    }
}

fn render_zig_import(target: &str, symbols: &[String], via: Option<&str>) -> String {
    let binding = via
        .filter(|value| !value.is_empty() && *value != "-")
        .map(str::to_string)
        .or_else(|| symbols.first().cloned())
        .unwrap_or_else(|| {
            target
                .rsplit('/')
                .next()
                .unwrap_or("dep")
                .trim_end_matches(".zig")
                .to_string()
        });
    format!("const {binding} = @import(\"{target}\");")
}

impl ToolBehavior {
    pub fn has_explicit_contract(&self) -> bool {
        self.input_explicit
            || self.output_explicit
            || self.append_file_arg_explicit
            || self.diagnostics_explicit
    }

    pub fn input_mode(&self) -> ToolInputMode {
        match self.input.as_str() {
            "stdin" => ToolInputMode::Stdin,
            "inline" => ToolInputMode::Inline,
            _ => ToolInputMode::TempFile,
        }
    }

    pub fn output_mode(&self) -> ToolOutputMode {
        match self.output.as_str() {
            "stdout" => ToolOutputMode::Stdout,
            "tempfile" => ToolOutputMode::TempFile,
            _ => ToolOutputMode::None,
        }
    }

    pub fn append_file_arg(&self) -> bool {
        self.append_file_arg
            .unwrap_or(matches!(self.input_mode(), ToolInputMode::TempFile))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ToolInputMode {
    TempFile,
    Stdin,
    Inline,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ToolOutputMode {
    None,
    Stdout,
    TempFile,
}

fn default_input_mode() -> String {
    "tempfile".to_string()
}

fn default_output_mode() -> String {
    "none".to_string()
}

fn default_path_group() -> String {
    "path".to_string()
}

fn default_line_group() -> String {
    "line".to_string()
}

fn default_message_group() -> String {
    "message".to_string()
}

fn default_column_group() -> String {
    "column".to_string()
}

fn default_severity_value() -> String {
    "error".to_string()
}

fn matches_special_file_kind(value: &str, kind: OutputKind) -> bool {
    matches!(
        (value.trim(), kind),
        ("source", OutputKind::Source) | ("test", OutputKind::Test)
    )
}

fn parse_output_root(value: Option<&str>, kind: OutputKind) -> Result<OutputRootKind, String> {
    match value.unwrap_or_default().trim() {
        "" => Ok(match kind {
            OutputKind::Source => OutputRootKind::SourceOut,
            OutputKind::Test => OutputRootKind::TestOut,
        }),
        "package" => Ok(OutputRootKind::Package),
        "source" | "source_out" => Ok(OutputRootKind::SourceOut),
        "test" | "test_out" => Ok(OutputRootKind::TestOut),
        other => Err(format!("unsupported special file root `{other}`")),
    }
}

fn normalized_markdown_module_path(markdown_relative_path: &Path) -> PathBuf {
    strip_matched_markdown_suffix(&markdown_relative_without_md(markdown_relative_path))
}

pub fn markdown_module_path(markdown_relative_path: &Path) -> String {
    normalized_markdown_module_path(markdown_relative_path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn markdown_module_id(markdown_relative_path: &Path) -> String {
    markdown_module_path(markdown_relative_path).replace('/', ".")
}

pub fn markdown_module_path_for_lang(lang: &Lang, markdown_relative_path: &Path) -> String {
    descriptor_for_key(lang.key())
        .map(|descriptor| descriptor.markdown_module_path(markdown_relative_path))
        .unwrap_or_else(|| markdown_module_path(markdown_relative_path))
}

pub fn markdown_module_id_for_lang(lang: &Lang, markdown_relative_path: &Path) -> String {
    descriptor_for_key(lang.key())
        .map(|descriptor| descriptor.markdown_module_id(markdown_relative_path))
        .unwrap_or_else(|| markdown_module_id(markdown_relative_path))
}

fn output_relative_path_for_rule(markdown_relative_path: &Path, rule: &FileRule) -> PathBuf {
    let relative = markdown_relative_without_md(markdown_relative_path);
    let relative = if rule.strip_lang_ext {
        strip_matched_markdown_suffix(&relative)
    } else {
        relative
    };
    let parent = relative.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = relative
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let mut output_name = format!("{}{}{}", rule.prefix, name, rule.suffix);
    let expected_extension = format!(".{}", rule.extension);
    if !output_name.ends_with(&expected_extension) {
        output_name.push('.');
        output_name.push_str(&rule.extension);
    }
    if parent.as_os_str().is_empty() {
        PathBuf::from(output_name)
    } else {
        parent.join(output_name)
    }
}

fn markdown_relative_without_md(markdown_relative_path: &Path) -> PathBuf {
    let parent = markdown_relative_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let name = markdown_relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let without_md = name.strip_suffix(".md").unwrap_or(name);
    if parent.as_os_str().is_empty() {
        PathBuf::from(without_md)
    } else {
        parent.join(without_md)
    }
}

fn strip_matched_markdown_suffix(markdown_relative_path: &Path) -> PathBuf {
    let parent = markdown_relative_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let name = markdown_relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let matched_suffix = matched_markdown_suffix(&format!("{name}.md"));
    let stripped = matched_suffix
        .as_deref()
        .and_then(|suffix| name.strip_suffix(&format!(".{suffix}")))
        .unwrap_or(name);
    if parent.as_os_str().is_empty() {
        PathBuf::from(stripped)
    } else {
        parent.join(stripped)
    }
}

impl DescriptorRegistry {
    fn descriptors(&self) -> Vec<Descriptor> {
        let mut descriptors = self.by_id.values().cloned().collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        descriptors
    }

    fn descriptor_for_key(&self, key: &str) -> Option<&Descriptor> {
        let canonical = self.alias_to_id.get(key).map_or(key, String::as_str);
        self.by_id.get(canonical)
    }

    fn resolve_fence_label(&self, label: &str) -> FenceLabelResolution {
        let label = label.trim();
        if label.is_empty() {
            return FenceLabelResolution::Unknown;
        }
        if let Some(descriptor) = self.descriptor_for_key(label) {
            return FenceLabelResolution::Resolved(Lang::Other(descriptor.id.clone()));
        }

        let mut matched_ids = self
            .suffix_to_id
            .iter()
            .filter_map(|(suffix, id)| (suffix == label).then_some(id.clone()))
            .collect::<Vec<_>>();
        matched_ids.sort();
        matched_ids.dedup();
        match matched_ids.as_slice() {
            [] => FenceLabelResolution::Unknown,
            [id] => FenceLabelResolution::Resolved(Lang::Other(id.clone())),
            _ => FenceLabelResolution::Ambiguous(matched_ids),
        }
    }

    fn descriptor_for_fence_label(&self, label: &str) -> Option<&Descriptor> {
        match self.resolve_fence_label(label) {
            FenceLabelResolution::Resolved(Lang::Other(id)) => self.by_id.get(&id),
            _ => None,
        }
    }

    fn descriptor_for_markdown_name(&self, name: &str) -> Option<&Descriptor> {
        match self.resolve_markdown_name(name) {
            MarkdownPathLangResolution::Resolved {
                lang: Lang::Other(id),
                ..
            } => self.by_id.get(&id),
            _ => None,
        }
    }

    fn matched_markdown_suffix(&self, name: &str) -> Option<String> {
        match self.resolve_markdown_name(name) {
            MarkdownPathLangResolution::Resolved { matched_suffix, .. } => Some(matched_suffix),
            _ => None,
        }
    }

    fn resolve_markdown_path_lang(&self, path: &Path) -> MarkdownPathLangResolution {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return MarkdownPathLangResolution::Unknown;
        };
        self.resolve_markdown_name(name)
    }

    fn resolve_markdown_name(&self, name: &str) -> MarkdownPathLangResolution {
        let Some(without_md) = name.strip_suffix(".md") else {
            return MarkdownPathLangResolution::Unknown;
        };

        let mut matches = self
            .path_suffix_to_id
            .iter()
            .filter_map(|(suffix, id)| {
                let candidate = format!(".{suffix}");
                without_md
                    .ends_with(&candidate)
                    .then_some((suffix.clone(), id.clone()))
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return MarkdownPathLangResolution::Unknown;
        }

        let longest_suffix_len = matches
            .iter()
            .map(|(suffix, _)| suffix.len())
            .max()
            .unwrap_or_default();
        matches.retain(|(suffix, _)| suffix.len() == longest_suffix_len);
        matches.sort();
        matches.dedup();

        let matched_suffix = matches
            .first()
            .map(|(suffix, _)| suffix.clone())
            .unwrap_or_default();
        let mut matched_ids = matches.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
        matched_ids.sort();
        matched_ids.dedup();

        match matched_ids.as_slice() {
            [] => MarkdownPathLangResolution::Unknown,
            [id] => MarkdownPathLangResolution::Resolved {
                lang: Lang::Other(id.clone()),
                matched_suffix,
            },
            _ => MarkdownPathLangResolution::Ambiguous(matched_ids),
        }
    }

    fn resolve_markdown_lang<'a>(
        &self,
        path: &Path,
        fence_labels: impl IntoIterator<Item = &'a str>,
    ) -> Option<Lang> {
        match self.resolve_markdown_path_lang(path) {
            MarkdownPathLangResolution::Resolved { lang, .. } => return Some(lang),
            MarkdownPathLangResolution::Ambiguous(_) => return None,
            MarkdownPathLangResolution::Unknown => {}
        }

        let mut resolved = None;
        for label in fence_labels {
            let Some(descriptor) = self.descriptor_for_fence_label(label) else {
                continue;
            };
            let candidate = Lang::Other(descriptor.id.clone());
            match &resolved {
                None => resolved = Some(candidate),
                Some(current) if *current == candidate => {}
                Some(_) => return None,
            }
        }

        resolved
    }
}

impl ToolRegistry {
    fn tool_manifest_for_command(&self, command: &str) -> Option<&ToolManifest> {
        for (prefix, id) in &self.prefix_to_id {
            if command_matches_prefix(command, prefix) {
                return self.by_id.get(id);
            }
        }
        None
    }
}

impl PackageManagerManifest {
    pub fn metadata_path(&self, root: &Path) -> Option<PathBuf> {
        self.metadata_files.iter().find_map(|pattern| {
            if pattern.contains('*') {
                return expand_simple_glob(root, pattern).into_iter().next();
            }
            let candidate = root.join(pattern);
            candidate.exists().then_some(candidate)
        })
    }

    pub fn resolved_lang(&self) -> Lang {
        canonical_lang_for_key(&self.lang)
    }

    pub fn command(&self, kind: &str) -> Option<&str> {
        match kind {
            "install" => self.commands.install.as_deref(),
            "build" => self.commands.build.as_deref(),
            "typecheck" => self.commands.typecheck.as_deref(),
            "lint" => self.commands.lint.as_deref(),
            "test" => self.commands.test.as_deref(),
            _ => None,
        }
    }

    fn detected_lockfile_count(&self, root: &Path) -> usize {
        self.lockfiles
            .iter()
            .filter(|lockfile| root.join(lockfile).exists())
            .count()
    }

    fn score_for_root(&self, root: &Path) -> usize {
        let metadata_score = usize::from(self.metadata_path(root).is_some()) * 10;
        let lockfile_score = self.detected_lockfile_count(root);
        metadata_score + lockfile_score
    }
}

pub(crate) fn load_package_manager_scripts(
    manager: &PackageManagerManifest,
    root: &Path,
) -> HashMap<String, String> {
    if manager.metadata_reader != "node-package-json" {
        return HashMap::new();
    }
    let Some(path) = manager.metadata_path(root) else {
        return HashMap::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return HashMap::new();
    };
    let Some(object) = value.get("scripts").and_then(serde_json::Value::as_object) else {
        return HashMap::new();
    };
    object
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_str()
                .filter(|command| !command.trim().is_empty())
                .map(|command| (name.to_string(), command.to_string()))
        })
        .collect()
}

pub(crate) fn quality_script_command_from_scripts(
    manager: &PackageManagerManifest,
    scripts: &HashMap<String, String>,
    kind: &str,
) -> Option<String> {
    for script_name in quality_slot_script_names(kind) {
        if !scripts.contains_key(*script_name) {
            continue;
        }
        if *script_name == kind {
            if let Some(command) = manager.command(kind) {
                return Some(command.to_string());
            }
        }
        return Some(render_package_manager_script_command(manager, script_name));
    }
    None
}

fn quality_slot_script_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "typecheck" => &["typecheck"],
        "lint" => &["lint"],
        "fix" => &["fix", "format"],
        "test" => &["test"],
        _ => &[],
    }
}

fn render_package_manager_script_command(
    manager: &PackageManagerManifest,
    script_name: &str,
) -> String {
    match manager.id.as_str() {
        "npm" => {
            if script_name == "test" {
                "npm test".to_string()
            } else {
                format!("npm run {script_name}")
            }
        }
        "pnpm" => {
            if script_name == "test" {
                "pnpm test".to_string()
            } else {
                format!("pnpm {script_name}")
            }
        }
        "yarn" => format!("yarn {script_name}"),
        "bun" => {
            if script_name == "test" {
                "bun test".to_string()
            } else {
                format!("bun run {script_name}")
            }
        }
        _ => script_name.to_string(),
    }
}

impl PackageManagerRegistry {
    fn package_manager_for_id(&self, key: &str) -> Option<&PackageManagerManifest> {
        let canonical = self.alias_to_id.get(key).map_or(key, String::as_str);
        self.by_id.get(canonical)
    }

    fn package_managers_for_root(&self, root: &Path) -> Vec<PackageManagerManifest> {
        let mut manifests_by_metadata: HashMap<PathBuf, Vec<PackageManagerManifest>> =
            HashMap::new();
        for manifest in self.by_id.values() {
            let Some(metadata_path) = manifest.metadata_path(root) else {
                continue;
            };
            manifests_by_metadata
                .entry(metadata_path)
                .or_default()
                .push(manifest.clone());
        }

        let mut manifests: Vec<PackageManagerManifest> = manifests_by_metadata
            .into_iter()
            .map(|(metadata_path, manifests)| {
                self.select_active_package_manager(root, &metadata_path, manifests)
            })
            .collect();
        manifests.sort_by(|left, right| {
            right
                .score_for_root(root)
                .cmp(&left.score_for_root(root))
                .then_with(|| left.id.cmp(&right.id))
        });
        manifests
    }

    fn select_active_package_manager(
        &self,
        root: &Path,
        metadata_path: &Path,
        mut manifests: Vec<PackageManagerManifest>,
    ) -> PackageManagerManifest {
        manifests.sort_by(|left, right| {
            right
                .detected_lockfile_count(root)
                .cmp(&left.detected_lockfile_count(root))
                .then_with(|| left.id.cmp(&right.id))
        });

        let explicit_id = self.explicit_package_manager_id(metadata_path, &manifests);
        let highest_lockfile_count = manifests
            .first()
            .map(|manifest| manifest.detected_lockfile_count(root))
            .unwrap_or(0);

        if highest_lockfile_count > 0 {
            if let Some(index) =
                self.select_candidate_index(&manifests, explicit_id.as_deref(), |manifest| {
                    manifest.detected_lockfile_count(root) == highest_lockfile_count
                })
            {
                return manifests.remove(index);
            }

            if let Some(index) = self.select_conservative_candidate_index(&manifests, |manifest| {
                manifest.detected_lockfile_count(root) == highest_lockfile_count
            }) {
                return manifests.remove(index);
            }

            let index = manifests
                .iter()
                .position(|manifest| {
                    manifest.detected_lockfile_count(root) == highest_lockfile_count
                })
                .expect("lockfile candidate must exist");
            return manifests.remove(index);
        }

        if let Some(index) =
            self.select_candidate_index(&manifests, explicit_id.as_deref(), |_| true)
        {
            return manifests.remove(index);
        }

        if let Some(index) = self.select_conservative_candidate_index(&manifests, |_| true) {
            return manifests.remove(index);
        }

        manifests.remove(0)
    }

    fn explicit_package_manager_id(
        &self,
        metadata_path: &Path,
        manifests: &[PackageManagerManifest],
    ) -> Option<String> {
        if manifests.is_empty()
            || !manifests
                .iter()
                .all(|manifest| manifest.metadata_reader == "node-package-json")
        {
            return None;
        }

        let text = fs::read_to_string(metadata_path).ok()?;
        let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
        let explicit = value
            .get("packageManager")
            .and_then(serde_json::Value::as_str)?
            .trim();
        let explicit = explicit.split('@').next()?.trim();
        if explicit.is_empty() {
            return None;
        }

        let canonical = self.package_manager_for_id(explicit)?.id.clone();
        manifests
            .iter()
            .any(|manifest| manifest.id == canonical)
            .then_some(canonical)
    }

    fn select_candidate_index<F>(
        &self,
        manifests: &[PackageManagerManifest],
        candidate_id: Option<&str>,
        filter: F,
    ) -> Option<usize>
    where
        F: Fn(&PackageManagerManifest) -> bool,
    {
        let candidate_id = candidate_id?;
        manifests
            .iter()
            .position(|manifest| filter(manifest) && manifest.id == candidate_id)
    }

    fn select_conservative_candidate_index<F>(
        &self,
        manifests: &[PackageManagerManifest],
        filter: F,
    ) -> Option<usize>
    where
        F: Fn(&PackageManagerManifest) -> bool,
    {
        if manifests
            .iter()
            .all(|manifest| manifest.metadata_reader == "node-package-json")
        {
            return manifests
                .iter()
                .position(|manifest| filter(manifest) && manifest.id == "npm");
        }
        None
    }
}

fn canonical_lang_for_key(key: &str) -> Lang {
    registry()
        .descriptor_for_key(key)
        .map(|descriptor| Lang::Other(descriptor.id.clone()))
        .unwrap_or_else(|| Lang::Other(key.to_string()))
}

pub(crate) fn langs_match(left: &Lang, right: &Lang) -> bool {
    canonical_lang_for_key(left.key()) == canonical_lang_for_key(right.key())
}

pub(crate) fn descriptor_matches_lang(descriptor: &Descriptor, lang: &Lang) -> bool {
    descriptor.id == lang.key()
        || descriptor.aliases.iter().any(|alias| alias == lang.key())
        || matches!(canonical_lang_for_key(lang.key()), Lang::Other(ref id) if descriptor.id == *id)
}

pub(crate) fn descriptor_for_key(key: &str) -> Option<Descriptor> {
    registry().descriptor_for_key(key).cloned()
}

pub fn descriptor_origin_for_key(key: &str) -> Option<DescriptorOrigin> {
    let registry = registry();
    let canonical = registry.alias_to_id.get(key).map_or(key, String::as_str);
    registry.origins.get(canonical).cloned()
}

pub(crate) fn language_descriptors() -> Vec<Descriptor> {
    registry().descriptors()
}

pub fn resolved_language_descriptors(root: Option<&Path>) -> Vec<ResolvedLanguageDescriptor> {
    with_workspace_descriptor_root(root, || {
        let registry = registry();
        let mut descriptors = registry
            .descriptors()
            .into_iter()
            .filter_map(|descriptor| {
                let origin = registry.origins.get(&descriptor.id)?.clone();
                Some(ResolvedLanguageDescriptor {
                    id: descriptor.id,
                    aliases: descriptor.aliases,
                    match_suffixes: descriptor.match_suffixes,
                    primary_ext: descriptor.language.primary_ext,
                    vscode_id: descriptor.language.vscode_id,
                    origin,
                })
            })
            .collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        descriptors
    })
}

pub fn lang_for_markdown_path(path: &Path) -> Option<Lang> {
    resolve_markdown_lang(path, std::iter::empty::<&str>())
}

pub(crate) fn resolve_markdown_path_lang(path: &Path) -> MarkdownPathLangResolution {
    registry().resolve_markdown_path_lang(path)
}

pub fn resolve_markdown_lang<'a>(
    path: &Path,
    fence_labels: impl IntoIterator<Item = &'a str>,
) -> Option<Lang> {
    registry().resolve_markdown_lang(path, fence_labels)
}

pub(crate) fn resolve_fence_label(label: &str) -> FenceLabelResolution {
    registry().resolve_fence_label(label)
}

pub fn resolve_markdown_lang_at<'a>(
    root: Option<&Path>,
    path: &Path,
    fence_labels: impl IntoIterator<Item = &'a str>,
) -> Option<Lang> {
    with_workspace_descriptor_root(root, || resolve_markdown_lang(path, fence_labels))
}

pub fn markdown_suffixes_for_lang(lang: &Lang) -> Vec<String> {
    let Some(descriptor) = descriptor_for_key(lang.key()) else {
        return vec![lang.key().to_string()];
    };
    let mut suffixes = descriptor.match_suffixes;
    if !suffixes.contains(&descriptor.language.primary_ext) {
        suffixes.push(descriptor.language.primary_ext);
    }
    suffixes
}

pub fn markdown_suffix_for_lang(lang: &Lang) -> Option<String> {
    markdown_suffixes_for_lang(lang)
        .into_iter()
        .next()
        .map(|suffix| format!(".{suffix}.md"))
}

pub fn fence_labels_for_lang(lang: &Lang) -> Vec<String> {
    let Some(descriptor) = descriptor_for_key(lang.key()) else {
        return vec![lang.key().to_string()];
    };
    let mut labels = descriptor.match_suffixes;
    for alias in descriptor.aliases {
        if !labels.contains(&alias) {
            labels.push(alias);
        }
    }
    if !labels.contains(&descriptor.language.primary_ext) {
        labels.push(descriptor.language.primary_ext);
    }
    labels
}

pub fn all_fence_label_completions() -> Vec<(String, String)> {
    let mut labels = Vec::new();
    for descriptor in language_descriptors() {
        for label in fence_labels_for_lang(&Lang::Other(descriptor.id.clone())) {
            labels.push((label, descriptor.id.clone()));
        }
    }
    labels.sort();
    labels.dedup();
    labels
}

pub(crate) fn descriptor_for_markdown_name(name: &str) -> Option<Descriptor> {
    registry().descriptor_for_markdown_name(name).cloned()
}

pub fn is_root_module_markdown_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    descriptor_for_markdown_name(name)
        .is_some_and(|descriptor| descriptor.is_root_module_markdown_name(name))
}

pub fn is_overview_markdown_name(name: &str) -> bool {
    name == OVERVIEW_MARKDOWN_NAME
}

pub fn is_overview_markdown_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_overview_markdown_name)
}

pub fn overview_markdown_path(markdown_root: &Path) -> PathBuf {
    markdown_root.join(OVERVIEW_MARKDOWN_NAME)
}

pub fn package_overview_markdown_relative_path(
    config: &Config,
    kind: PackageOverviewKind,
) -> PathBuf {
    overview_markdown_path(kind.markdown_root(config))
}

pub fn package_overview_markdown_path(
    package_root: &Path,
    config: &Config,
    kind: PackageOverviewKind,
) -> PathBuf {
    package_root.join(package_overview_markdown_relative_path(config, kind))
}

pub fn package_source_overview_markdown_path(package_root: &Path, config: &Config) -> PathBuf {
    package_overview_markdown_path(package_root, config, PackageOverviewKind::Source)
}

pub fn package_test_overview_markdown_path(package_root: &Path, config: &Config) -> PathBuf {
    package_overview_markdown_path(package_root, config, PackageOverviewKind::Test)
}

pub fn is_package_overview_markdown_path(
    path: &Path,
    package_root: Option<&Path>,
    config: &Config,
    kind: PackageOverviewKind,
) -> bool {
    if !is_overview_markdown_path(path) {
        return false;
    }

    match package_root {
        Some(package_root) => path == package_overview_markdown_path(package_root, config, kind),
        None => path.ends_with(package_overview_markdown_relative_path(config, kind)),
    }
}

pub fn package_overview_kind_for_path(
    path: &Path,
    package_root: Option<&Path>,
    config: &Config,
) -> Option<PackageOverviewKind> {
    [PackageOverviewKind::Source, PackageOverviewKind::Test]
        .into_iter()
        .find(|kind| is_package_overview_markdown_path(path, package_root, config, *kind))
}

pub(crate) fn matched_markdown_suffix(name: &str) -> Option<String> {
    registry().matched_markdown_suffix(name)
}

pub(crate) fn resolve_output_rule_for_markdown(
    lang: &Lang,
    markdown_relative_path: &Path,
    kind: OutputKind,
) -> Result<Option<ResolvedOutputRule>, String> {
    let Some(descriptor) = descriptor_for_key(lang.key()) else {
        return Ok(None);
    };
    descriptor
        .resolve_output_rule(markdown_relative_path, kind)
        .map(Some)
}

pub(crate) fn tool_behavior_for_command(command: &str) -> Option<ToolBehavior> {
    tool_registry()
        .tool_manifest_for_command(command)
        .map(|tool| tool.behavior.clone())
}

pub fn package_manager_for_id(id: &str) -> Option<PackageManagerManifest> {
    package_manager_registry()
        .package_manager_for_id(id)
        .cloned()
}

pub fn package_manager_origin_for_id(id: &str) -> Option<DescriptorOrigin> {
    let registry = package_manager_registry();
    let canonical = registry.alias_to_id.get(id).map_or(id, String::as_str);
    registry.origins.get(canonical).cloned()
}

pub fn tool_origin_for_command(command: &str) -> Option<DescriptorOrigin> {
    let registry = tool_registry();
    let tool = registry.tool_manifest_for_command(command)?;
    registry.origins.get(&tool.id).cloned()
}

pub fn package_managers_for_root(root: &Path) -> Vec<PackageManagerManifest> {
    with_workspace_descriptor_root(Some(root), || {
        package_manager_registry().package_managers_for_root(root)
    })
}

pub fn detect_package_manager(root: &Path) -> Option<PackageManagerManifest> {
    package_managers_for_root(root).into_iter().next()
}

pub fn default_quality_commands_for_lang(lang: &Lang) -> InitQualityCommands {
    let descriptor = descriptor_for_key(lang.key());
    InitQualityCommands {
        lang: lang.clone(),
        type_check: descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.default_typecheck_command())
            .map(str::to_string),
        lint: descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.default_lint_command())
            .map(str::to_string),
        fix: descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.default_fix_command())
            .map(str::to_string),
        test: descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.default_test_command())
            .map(str::to_string),
    }
}

pub fn set_workspace_descriptor_root(root: Option<&Path>) {
    DESCRIPTOR_ROOT.with(|storage| {
        *storage.borrow_mut() = root.map(Path::to_path_buf);
    });
    reset_registry_caches();
}

pub fn with_workspace_descriptor_root<T>(root: Option<&Path>, action: impl FnOnce() -> T) -> T {
    let previous_root = DESCRIPTOR_ROOT.with(|storage| storage.borrow().clone());
    set_workspace_descriptor_root(root);
    let result = action();
    set_workspace_descriptor_root(previous_root.as_deref());
    result
}

fn reset_registry_caches() {
    REGISTRY_CACHE.with(|storage| *storage.borrow_mut() = None);
    TOOL_REGISTRY_CACHE.with(|storage| *storage.borrow_mut() = None);
    PACKAGE_MANAGER_REGISTRY_CACHE.with(|storage| *storage.borrow_mut() = None);
}

fn registry() -> DescriptorRegistry {
    let root = active_descriptor_root();
    REGISTRY_CACHE.with(|storage| {
        if let Some((cached_root, registry)) = storage.borrow().as_ref() {
            if *cached_root == root {
                return registry.clone();
            }
        }
        let registry = load_registry(root.as_deref());
        *storage.borrow_mut() = Some((root.clone(), registry.clone()));
        registry
    })
}

fn tool_registry() -> ToolRegistry {
    let root = active_descriptor_root();
    TOOL_REGISTRY_CACHE.with(|storage| {
        if let Some((cached_root, registry)) = storage.borrow().as_ref() {
            if *cached_root == root {
                return registry.clone();
            }
        }
        let registry = load_tool_registry(root.as_deref());
        *storage.borrow_mut() = Some((root.clone(), registry.clone()));
        registry
    })
}

fn package_manager_registry() -> PackageManagerRegistry {
    let root = active_descriptor_root();
    PACKAGE_MANAGER_REGISTRY_CACHE.with(|storage| {
        if let Some((cached_root, registry)) = storage.borrow().as_ref() {
            if *cached_root == root {
                return registry.clone();
            }
        }
        let registry = load_package_manager_registry(root.as_deref());
        *storage.borrow_mut() = Some((root.clone(), registry.clone()));
        registry
    })
}

fn active_descriptor_root() -> Option<PathBuf> {
    let configured = DESCRIPTOR_ROOT.with(|storage| storage.borrow().clone());
    if configured.is_some() {
        return configured;
    }
    std::env::current_dir().ok()
}

thread_local! {
    static DESCRIPTOR_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static REGISTRY_CACHE: RefCell<Option<(Option<PathBuf>, DescriptorRegistry)>> = const { RefCell::new(None) };
    static TOOL_REGISTRY_CACHE: RefCell<Option<(Option<PathBuf>, ToolRegistry)>> = const { RefCell::new(None) };
    static PACKAGE_MANAGER_REGISTRY_CACHE: RefCell<Option<(Option<PathBuf>, PackageManagerRegistry)>> = const { RefCell::new(None) };
    static DESCRIPTOR_DIAGNOSTICS: RefCell<Vec<Diagnostic>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn clear_descriptor_diagnostics() {
    DESCRIPTOR_DIAGNOSTICS.with(|storage| storage.borrow_mut().clear());
}

pub(crate) fn drain_descriptor_diagnostics() -> Vec<Diagnostic> {
    DESCRIPTOR_DIAGNOSTICS.with(|storage| storage.borrow_mut().drain(..).collect())
}

fn push_descriptor_diagnostic(path: &Path, message: impl Into<String>) {
    DESCRIPTOR_DIAGNOSTICS.with(|storage| {
        storage
            .borrow_mut()
            .push(Diagnostic::error(Some(path.to_path_buf()), message.into()));
    });
}

fn push_descriptor_warning(path: &Path, message: impl Into<String>) {
    DESCRIPTOR_DIAGNOSTICS.with(|storage| {
        storage.borrow_mut().push(Diagnostic::warning(
            Some(path.to_path_buf()),
            message.into(),
        ));
    });
}

impl DescriptorSource {
    fn origin(&self, file: Option<PathBuf>) -> DescriptorOrigin {
        DescriptorOrigin {
            source_id: self.id.clone(),
            source_kind: self.kind,
            root: self.root.clone(),
            file,
            priority: self.priority,
        }
    }
}

fn descriptor_sources(root: Option<&Path>) -> Vec<DescriptorSource> {
    let Some(root) = root else {
        return vec![DescriptorSource {
            id: "user-global-store".to_string(),
            kind: DescriptorSourceKind::Global,
            root: None,
            priority: 3,
        }];
    };

    let lock = read_descriptor_source_lock(root);
    let mut sources = Vec::new();
    sources.push(DescriptorSource {
        id: "package-local".to_string(),
        kind: DescriptorSourceKind::PackageLocal,
        root: Some(root.join(".mds/descriptors")),
        priority: 0,
    });

    for (id, shared_root) in [
        ("workspace-shared", root.join(".mds/shared/descriptors")),
        (
            "workspace-descriptors",
            root.join(".mds/workspace/descriptors"),
        ),
    ] {
        if shared_root.exists() {
            sources.push(DescriptorSource {
                id: id.to_string(),
                kind: DescriptorSourceKind::Workspace,
                root: Some(shared_root),
                priority: 1,
            });
        }
    }

    let config_path = root.join(".mds/descriptor-sources.toml");
    if config_path.exists() {
        match fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str::<DescriptorSourcesFile>(&content) {
                Ok(config) => {
                    for (index, source) in config.sources.into_iter().enumerate() {
                        if let Some(resolved) = resolve_configured_descriptor_source(
                            root,
                            &config_path,
                            source,
                            index,
                            &lock,
                        ) {
                            sources.push(resolved);
                        }
                    }
                }
                Err(error) => push_descriptor_diagnostic(
                    &config_path,
                    format!("failed to parse descriptor source config: {error}"),
                ),
            },
            Err(error) => push_descriptor_diagnostic(
                &config_path,
                format!("failed to read descriptor source config: {error}"),
            ),
        }
    } else if !lock.is_empty() {
        let lock_path = root.join(".mds/descriptor-sources.lock");
        push_descriptor_warning(
            &lock_path,
            "descriptor source lock exists without descriptor source config",
        );
    }

    sources.push(DescriptorSource {
        id: "user-global-store".to_string(),
        kind: DescriptorSourceKind::Global,
        root: None,
        priority: 3,
    });
    sources
}

fn read_descriptor_source_lock(root: &Path) -> HashMap<String, PathBuf> {
    let lock_path = root.join(".mds/descriptor-sources.lock");
    if !lock_path.exists() {
        return HashMap::new();
    }
    let content = match fs::read_to_string(&lock_path) {
        Ok(content) => content,
        Err(error) => {
            push_descriptor_diagnostic(
                &lock_path,
                format!("failed to read descriptor source lock: {error}"),
            );
            return HashMap::new();
        }
    };
    let parsed = match toml::from_str::<DescriptorSourceLockFile>(&content) {
        Ok(parsed) => parsed,
        Err(error) => {
            push_descriptor_diagnostic(
                &lock_path,
                format!("failed to parse descriptor source lock: {error}"),
            );
            return HashMap::new();
        }
    };

    let mut lock = HashMap::new();
    for (index, source) in parsed.sources.into_iter().enumerate() {
        let id = source.id.unwrap_or_else(|| format!("source-{index}"));
        let Some(path) = source.resolved_path.or(source.path) else {
            push_descriptor_diagnostic(
                &lock_path,
                format!("descriptor source lock entry `{id}` is missing resolved_path"),
            );
            continue;
        };
        lock.insert(id, normalize_descriptor_source_path(root, &path));
    }
    lock
}

fn resolve_configured_descriptor_source(
    root: &Path,
    config_path: &Path,
    source: DescriptorSourceConfig,
    index: usize,
    lock: &HashMap<String, PathBuf>,
) -> Option<DescriptorSource> {
    let id = source.id.unwrap_or_else(|| format!("source-{index}"));
    let source_kind =
        parse_descriptor_source_kind(source.source_type.as_deref(), source.url.as_deref());
    let configured_path = source
        .path
        .as_deref()
        .map(|path| normalize_descriptor_source_path(root, path));
    let locked_path = lock.get(&id).cloned();
    if let (Some(configured), Some(locked)) = (&configured_path, &locked_path) {
        if configured != locked {
            push_descriptor_warning(
                config_path,
                format!(
                    "descriptor source `{id}` lock resolved_path `{}` differs from config path `{}`; using lock",
                    locked.display(),
                    configured.display()
                ),
            );
        }
    }

    let resolved_path = locked_path.or(configured_path);
    match source_kind {
        DescriptorSourceKind::Local | DescriptorSourceKind::Workspace => {
            let Some(path) = resolved_path else {
                push_descriptor_diagnostic(
                    config_path,
                    format!("descriptor source `{id}` is missing path"),
                );
                return None;
            };
            if !path.exists() {
                push_descriptor_diagnostic(
                    config_path,
                    format!(
                        "descriptor source `{id}` resolved path does not exist: {}",
                        path.display()
                    ),
                );
                return None;
            }
            Some(DescriptorSource {
                id,
                kind: source_kind,
                root: Some(path),
                priority: if source_kind == DescriptorSourceKind::Workspace {
                    1
                } else {
                    2
                },
            })
        }
        DescriptorSourceKind::Git => {
            if let Some(path) = resolved_path {
                if path.exists() {
                    return Some(DescriptorSource {
                        id,
                        kind: DescriptorSourceKind::Git,
                        root: Some(path),
                        priority: 2,
                    });
                }
            }
            push_descriptor_diagnostic(
                config_path,
                format!(
                    "descriptor source `{id}` requires network resolution but no locked cache path is available"
                ),
            );
            None
        }
        DescriptorSourceKind::Global => Some(DescriptorSource {
            id,
            kind: DescriptorSourceKind::Global,
            root: resolved_path,
            priority: 3,
        }),
        DescriptorSourceKind::PackageLocal | DescriptorSourceKind::Unknown => {
            push_descriptor_diagnostic(
                config_path,
                format!("descriptor source `{id}` has unsupported type"),
            );
            None
        }
    }
}

fn normalize_descriptor_source_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn parse_descriptor_source_kind(
    source_type: Option<&str>,
    url: Option<&str>,
) -> DescriptorSourceKind {
    match source_type.map(str::trim) {
        None if url.is_some() => DescriptorSourceKind::Git,
        None => DescriptorSourceKind::Local,
        Some("local" | "path") => DescriptorSourceKind::Local,
        Some("workspace") => DescriptorSourceKind::Workspace,
        Some("git") => DescriptorSourceKind::Git,
        Some("global" | "user") => DescriptorSourceKind::Global,
        Some("") if url.is_some() => DescriptorSourceKind::Git,
        Some(_) => DescriptorSourceKind::Unknown,
    }
}

fn push_origin_diagnostic(origin: &DescriptorOrigin, message: impl Into<String>) {
    let path = origin.file.as_ref().or(origin.root.as_ref()).cloned();
    DESCRIPTOR_DIAGNOSTICS.with(|storage| {
        storage
            .borrow_mut()
            .push(Diagnostic::error(path, message.into()));
    });
}

fn render_optional_origin(origin: Option<&DescriptorOrigin>) -> String {
    origin
        .map(render_origin)
        .unwrap_or_else(|| "unknown origin".to_string())
}

fn render_origin(origin: &DescriptorOrigin) -> String {
    let path = origin
        .file
        .as_ref()
        .or(origin.root.as_ref())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<unresolved>".to_string());
    format!("{} ({:?})", path, origin.source_kind)
}

pub fn descriptor_registry_report(root: Option<&Path>) -> DescriptorRegistryReport {
    clear_descriptor_diagnostics();
    let mut entries = Vec::new();
    with_workspace_descriptor_root(root, || {
        let languages = registry();
        entries.extend(registry_entries(
            DescriptorKind::Language,
            &languages.origins,
        ));
        let tools = tool_registry();
        entries.extend(registry_entries(DescriptorKind::Tool, &tools.origins));
        let package_managers = package_manager_registry();
        entries.extend(registry_entries(
            DescriptorKind::PackageManager,
            &package_managers.origins,
        ));
    });
    entries.sort_by(|left, right| {
        kind_rank(left.kind)
            .cmp(&kind_rank(right.kind))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut diagnostics = drain_descriptor_diagnostics();
    diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.message.cmp(&right.message))
    });
    diagnostics.dedup_by(|left, right| left.path == right.path && left.message == right.message);
    DescriptorRegistryReport {
        entries,
        diagnostics,
    }
}

fn registry_entries(
    kind: DescriptorKind,
    origins: &HashMap<String, DescriptorOrigin>,
) -> Vec<DescriptorRegistryEntry> {
    origins
        .iter()
        .map(|(id, origin)| DescriptorRegistryEntry {
            kind,
            id: id.clone(),
            origin: origin.clone(),
        })
        .collect()
}

fn kind_rank(kind: DescriptorKind) -> usize {
    match kind {
        DescriptorKind::Language => 0,
        DescriptorKind::Tool => 1,
        DescriptorKind::PackageManager => 2,
    }
}

fn load_registry(root: Option<&Path>) -> DescriptorRegistry {
    let sources = descriptor_sources(root);
    let descriptors = select_loaded_descriptors(
        load_language_descriptor_candidates(&sources),
        "language descriptor",
    );
    let mut by_id = HashMap::new();
    let mut origins = HashMap::new();
    let mut alias_to_id = HashMap::new();
    let mut alias_origins = HashMap::new();
    let mut suffix_to_id = Vec::new();
    let mut path_suffix_to_id = Vec::new();

    for loaded in descriptors {
        let id = loaded.value.id.clone();
        register_alias(
            &mut alias_to_id,
            &mut alias_origins,
            &id,
            &id,
            &loaded.origin,
            "language descriptor alias",
        );
        for alias in &loaded.value.aliases {
            register_alias(
                &mut alias_to_id,
                &mut alias_origins,
                alias,
                &id,
                &loaded.origin,
                "language descriptor alias",
            );
        }
        for suffix in loaded.value.all_match_suffixes() {
            register_runtime_index_value(&mut suffix_to_id, suffix, &id, &loaded.origin);
        }
        for suffix in loaded.value.markdown_path_suffixes() {
            register_runtime_index_value(&mut path_suffix_to_id, suffix, &id, &loaded.origin);
        }
        origins.insert(id.clone(), loaded.origin);
        by_id.insert(id, loaded.value);
    }

    suffix_to_id.sort_by(|left, right| {
        right
            .0
            .len()
            .cmp(&left.0.len())
            .then_with(|| left.0.cmp(&right.0))
    });
    path_suffix_to_id.sort_by(|left, right| {
        right
            .0
            .len()
            .cmp(&left.0.len())
            .then_with(|| left.0.cmp(&right.0))
    });

    DescriptorRegistry {
        by_id,
        origins,
        alias_to_id,
        suffix_to_id,
        path_suffix_to_id,
    }
}

fn load_tool_registry(root: Option<&Path>) -> ToolRegistry {
    let sources = descriptor_sources(root);
    let tools =
        select_loaded_descriptors(load_tool_descriptor_candidates(&sources), "tool descriptor");
    let mut by_id = HashMap::new();
    let mut origins = HashMap::new();
    let mut prefix_to_id = Vec::new();
    let mut prefix_origins = HashMap::new();

    for loaded in tools {
        let id = loaded.value.id.clone();
        for prefix in &loaded.value.match_prefixes {
            register_index_value(
                &mut prefix_to_id,
                &mut prefix_origins,
                prefix,
                &id,
                &loaded.origin,
                "tool descriptor command prefix",
            );
        }
        origins.insert(id.clone(), loaded.origin);
        by_id.insert(id, loaded.value);
    }

    prefix_to_id.sort_by(|left, right| {
        right
            .0
            .split_whitespace()
            .count()
            .cmp(&left.0.split_whitespace().count())
            .then_with(|| right.0.len().cmp(&left.0.len()))
            .then_with(|| left.0.cmp(&right.0))
    });

    ToolRegistry {
        by_id,
        origins,
        prefix_to_id,
    }
}

fn load_package_manager_registry(root: Option<&Path>) -> PackageManagerRegistry {
    let sources = descriptor_sources(root);
    let managers = select_loaded_descriptors(
        load_package_manager_descriptor_candidates(&sources),
        "package manager descriptor",
    );
    let mut by_id = HashMap::new();
    let mut origins = HashMap::new();
    let mut alias_to_id = HashMap::new();
    let mut alias_origins = HashMap::new();

    for loaded in managers {
        let id = loaded.value.id.clone();
        register_alias(
            &mut alias_to_id,
            &mut alias_origins,
            &id,
            &id,
            &loaded.origin,
            "package manager descriptor alias",
        );
        for alias in &loaded.value.aliases {
            register_alias(
                &mut alias_to_id,
                &mut alias_origins,
                alias,
                &id,
                &loaded.origin,
                "package manager descriptor alias",
            );
        }
        origins.insert(id.clone(), loaded.origin);
        by_id.insert(id, loaded.value);
    }

    PackageManagerRegistry {
        by_id,
        origins,
        alias_to_id,
    }
}

fn register_alias(
    alias_to_id: &mut HashMap<String, String>,
    alias_origins: &mut HashMap<String, DescriptorOrigin>,
    alias: &str,
    id: &str,
    origin: &DescriptorOrigin,
    label: &str,
) {
    if alias.trim().is_empty() {
        push_origin_diagnostic(origin, format!("{label} for `{id}` is empty"));
        return;
    }
    match alias_to_id.get(alias) {
        Some(existing) if existing == id => {}
        Some(existing) => {
            let existing_origin = alias_origins.get(alias);
            push_origin_diagnostic(
                origin,
                format!(
                    "{label} `{alias}` collision: `{existing}` from {} and `{id}` from {}",
                    render_optional_origin(existing_origin),
                    render_origin(origin)
                ),
            );
        }
        None => {
            alias_to_id.insert(alias.to_string(), id.to_string());
            alias_origins.insert(alias.to_string(), origin.clone());
        }
    }
}

fn register_index_value(
    values: &mut Vec<(String, String)>,
    value_origins: &mut HashMap<String, (String, DescriptorOrigin)>,
    value: &str,
    id: &str,
    origin: &DescriptorOrigin,
    label: &str,
) {
    if value.trim().is_empty() {
        push_origin_diagnostic(origin, format!("{label} for `{id}` is empty"));
        return;
    }
    match value_origins.get(value) {
        Some((existing, _)) if existing == id => {}
        Some((existing, existing_origin)) => {
            push_origin_diagnostic(
                origin,
                format!(
                    "{label} `{value}` collision: `{existing}` from {} and `{id}` from {}",
                    render_origin(existing_origin),
                    render_origin(origin)
                ),
            );
            values.push((value.to_string(), id.to_string()));
        }
        None => {
            value_origins.insert(value.to_string(), (id.to_string(), origin.clone()));
            values.push((value.to_string(), id.to_string()));
        }
    }
}

fn register_runtime_index_value(
    values: &mut Vec<(String, String)>,
    value: &str,
    id: &str,
    origin: &DescriptorOrigin,
) {
    if value.trim().is_empty() {
        push_origin_diagnostic(
            origin,
            format!("language descriptor suffix for `{id}` is empty"),
        );
        return;
    }
    values.push((value.to_string(), id.to_string()));
}

fn select_loaded_descriptors<T>(
    mut candidates: Vec<LoadedDescriptor<T>>,
    label: &str,
) -> Vec<LoadedDescriptor<T>>
where
    T: DescriptorId,
{
    candidates.sort_by(|left, right| {
        left.origin
            .priority
            .cmp(&right.origin.priority)
            .then_with(|| left.origin.file.cmp(&right.origin.file))
            .then_with(|| left.value.descriptor_id().cmp(right.value.descriptor_id()))
    });

    let mut selected = Vec::new();
    let mut selected_by_id: HashMap<String, DescriptorOrigin> = HashMap::new();
    for candidate in candidates {
        let id = candidate.value.descriptor_id().to_string();
        match selected_by_id.get(&id) {
            Some(existing_origin) if existing_origin.priority == candidate.origin.priority => {
                push_origin_diagnostic(
                    &candidate.origin,
                    format!(
                        "{label} id `{id}` collision: {} and {} have the same resolution priority",
                        render_origin(existing_origin),
                        render_origin(&candidate.origin)
                    ),
                );
            }
            Some(_) => {}
            None => {
                selected_by_id.insert(id, candidate.origin.clone());
                selected.push(candidate);
            }
        }
    }
    selected
}

trait DescriptorId {
    fn descriptor_id(&self) -> &str;
}

impl DescriptorId for Descriptor {
    fn descriptor_id(&self) -> &str {
        &self.id
    }
}

impl DescriptorId for ToolManifest {
    fn descriptor_id(&self) -> &str {
        &self.id
    }
}

impl DescriptorId for PackageManagerManifest {
    fn descriptor_id(&self) -> &str {
        &self.id
    }
}

fn load_language_descriptor_candidates(
    sources: &[DescriptorSource],
) -> Vec<LoadedDescriptor<Descriptor>> {
    let mut descriptors = Vec::new();
    for source in sources {
        let Some(root) = &source.root else {
            continue;
        };
        let descriptors_root = root.join("languages");
        for loaded in load_workspace_descriptors(&descriptors_root, source) {
            descriptors.push(loaded);
        }
    }
    descriptors
}

fn load_tool_descriptor_candidates(
    sources: &[DescriptorSource],
) -> Vec<LoadedDescriptor<ToolManifest>> {
    let mut tools = Vec::new();
    for source in sources {
        let Some(root) = &source.root else {
            continue;
        };
        for tools_root in [root.join("linters"), root.join("tools")] {
            for loaded in load_workspace_tools(&tools_root, source) {
                tools.push(loaded);
            }
        }
    }
    tools
}

fn load_package_manager_descriptor_candidates(
    sources: &[DescriptorSource],
) -> Vec<LoadedDescriptor<PackageManagerManifest>> {
    let mut managers = Vec::new();
    for source in sources {
        let Some(root) = &source.root else {
            continue;
        };
        let managers_root = root.join("package-managers");
        for loaded in load_workspace_package_managers(&managers_root, source) {
            managers.push(loaded);
        }
    }
    managers
}

fn load_workspace_descriptors(
    root: &Path,
    source: &DescriptorSource,
) -> Vec<LoadedDescriptor<Descriptor>> {
    let mut descriptors = Vec::new();
    for path in collect_descriptor_files(root) {
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                push_descriptor_diagnostic(
                    &path,
                    format!("failed to read language descriptor: {error}"),
                );
                continue;
            }
        };
        let descriptor = match toml::from_str::<Descriptor>(&content) {
            Ok(descriptor) => descriptor,
            Err(error) => {
                push_descriptor_diagnostic(
                    &path,
                    format!("failed to parse language descriptor: {error}"),
                );
                continue;
            }
        };
        descriptors.push(LoadedDescriptor {
            value: descriptor,
            origin: source.origin(Some(path)),
        });
    }
    descriptors
}

fn load_workspace_tools(
    root: &Path,
    source: &DescriptorSource,
) -> Vec<LoadedDescriptor<ToolManifest>> {
    let mut tools = Vec::new();
    for path in collect_descriptor_files(root) {
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                push_descriptor_diagnostic(
                    &path,
                    format!("failed to read tool descriptor: {error}"),
                );
                continue;
            }
        };
        let tool = match toml::from_str::<ToolManifest>(&content) {
            Ok(tool) => tool,
            Err(error) => {
                push_descriptor_diagnostic(
                    &path,
                    format!("failed to parse tool descriptor: {error}"),
                );
                continue;
            }
        };
        tools.push(LoadedDescriptor {
            value: tool,
            origin: source.origin(Some(path)),
        });
    }
    tools
}

fn load_workspace_package_managers(
    root: &Path,
    source: &DescriptorSource,
) -> Vec<LoadedDescriptor<PackageManagerManifest>> {
    let mut managers = Vec::new();
    for path in collect_descriptor_files(root) {
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                push_descriptor_diagnostic(
                    &path,
                    format!("failed to read package manager manifest: {error}"),
                );
                continue;
            }
        };
        let manager = match toml::from_str::<PackageManagerManifest>(&content) {
            Ok(manager) => manager,
            Err(error) => {
                push_descriptor_diagnostic(
                    &path,
                    format!("failed to parse package manager manifest: {error}"),
                );
                continue;
            }
        };
        managers.push(LoadedDescriptor {
            value: manager,
            origin: source.origin(Some(path)),
        });
    }
    managers
}

fn collect_descriptor_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_descriptor_files_into(root, &mut files);
    files.sort();
    files
}

fn collect_descriptor_files_into(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_descriptor_files_into(&path, files);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("toml") {
            files.push(path);
        }
    }
}

fn command_matches_prefix(command: &str, prefix: &str) -> bool {
    let command_tokens: Vec<&str> = command.split_whitespace().collect();
    let prefix_tokens: Vec<&str> = prefix.split_whitespace().collect();
    if prefix_tokens.is_empty() || prefix_tokens.len() > command_tokens.len() {
        return false;
    }

    prefix_tokens.iter().enumerate().all(|(index, expected)| {
        let actual = command_tokens[index];
        if index == 0 {
            command_token_name(actual) == command_token_name(expected)
        } else {
            actual == *expected
        }
    })
}

fn command_token_name(token: &str) -> &str {
    Path::new(token)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(token)
}

fn expand_simple_glob(root: &Path, pattern: &str) -> Vec<PathBuf> {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut matches = Vec::new();
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(prefix) && name.ends_with(suffix) {
            matches.push(path);
        }
    }
    matches.sort();
    matches
}
