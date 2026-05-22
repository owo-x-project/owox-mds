use crate::descriptor::{self, DescriptorOrigin};
use crate::diagnostics::{Diagnostic, RunState};
use crate::model::{
    DescriptorCommand, DescriptorKind, DescriptorSchemaFormat, InitDescriptorOptions,
    InitLanguageDescriptorOptions, InitPackageManagerDescriptorOptions, InitToolDescriptorOptions,
};
use regex::Regex;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use toml::Value;

pub(crate) fn run_descriptor_command(
    cwd: &Path,
    package_root: &Path,
    command: &DescriptorCommand,
    state: &mut RunState,
) -> Result<(), String> {
    match command {
        DescriptorCommand::Check { path } => {
            let target = path
                .as_deref()
                .map(|path| resolve_cli_path(cwd, path))
                .unwrap_or_else(|| package_root.to_path_buf());
            run_descriptor_check(package_root, &target, state);
        }
        DescriptorCommand::Explain { target } => {
            run_descriptor_explain(package_root, target, state);
        }
        DescriptorCommand::Schema { kind, format } => {
            render_descriptor_schema(*kind, *format, state)?;
        }
        DescriptorCommand::Sources => {
            render_descriptor_sources(package_root, state);
        }
    }
    Ok(())
}

pub(crate) fn run_init_descriptor(
    package_root: &Path,
    options: &InitDescriptorOptions,
    state: &mut RunState,
) -> Result<(), String> {
    if !options.yes {
        state.diagnostics.push(Diagnostic::error(
            None,
            "init descriptor requires --yes until the descriptor wizard is available",
        ));
        return Ok(());
    }

    let id = descriptor_id_from_name(&options.name);
    if id.is_empty() {
        state.diagnostics.push(Diagnostic::error(
            None,
            "descriptor name must contain an ASCII identifier",
        ));
        return Ok(());
    }

    let path = descriptor_output_path(package_root, options, &id)?;
    if path.exists() && !options.force {
        state.diagnostics.push(Diagnostic::error(
            Some(path),
            "descriptor file already exists; pass --force to overwrite",
        ));
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create descriptor directory: {error}"))?;
    }
    fs::write(&path, render_minimal_descriptor(options.kind, &id, options))
        .map_err(|error| format!("failed to write descriptor: {error}"))?;
    state.stdout.push_str(&format!(
        "created descriptor: {}\n",
        display_path(package_root, &path)
    ));
    Ok(())
}

fn run_descriptor_check(package_root: &Path, target: &Path, state: &mut RunState) {
    let source_status = descriptor_source_status(package_root, state);
    let registry_report = descriptor::descriptor_registry_report(Some(package_root));
    state
        .diagnostics
        .extend(registry_report.diagnostics.clone());
    let mut records = if target == package_root {
        let mut records = load_descriptor_records_from_registry(&registry_report, state);
        records.extend(load_descriptor_records(
            &descriptor_check_target(target),
            state,
        ));
        dedupe_descriptor_records(records)
    } else {
        let descriptor_target = descriptor_check_target(target);
        load_descriptor_records(&descriptor_target, state)
    };
    records.sort_by(|left, right| left.path.cmp(&right.path));
    validate_descriptor_collisions(&records, state);

    state.stdout.push_str("Descriptor check:\n");
    state
        .stdout
        .push_str(&format!("- target: {}\n", target.display()));
    state
        .stdout
        .push_str(&format!("- descriptors: {}\n", records.len()));
    state.stdout.push_str(&format!(
        "- resolved registry entries: {}\n",
        registry_report.entries.len()
    ));
    state.stdout.push_str(&format!(
        "- source config: {}\n",
        source_status.config_status()
    ));
    state
        .stdout
        .push_str(&format!("- source lock: {}\n", source_status.lock_status()));
    if !state.has_errors() {
        state.stdout.push_str("descriptor check ok\n");
    }
}

fn descriptor_check_target(target: &Path) -> PathBuf {
    if target.is_dir() {
        let descriptors = target.join(".mds/descriptors");
        if descriptors.is_dir() {
            return descriptors;
        }
    }
    target.to_path_buf()
}

fn dedupe_descriptor_records(mut records: Vec<DescriptorRecord>) -> Vec<DescriptorRecord> {
    records.sort_by(|left, right| left.path.cmp(&right.path));
    records.dedup_by(|left, right| left.path == right.path);
    records
}

fn run_descriptor_explain(package_root: &Path, target: &str, state: &mut RunState) {
    let records = load_descriptor_records_from_registry(
        &descriptor::descriptor_registry_report(Some(package_root)),
        state,
    );
    let explanation = explain_descriptor_target(&records, target);
    let resolved_origin = resolve_runtime_origin(package_root, target);
    state.stdout.push_str("Descriptor explanation:\n");
    state.stdout.push_str(&format!("- target: {target}\n"));
    match (explanation, resolved_origin) {
        (Some(explanation), origin) => {
            state
                .stdout
                .push_str(&format!("- kind: {}\n", explanation.kind.key()));
            state
                .stdout
                .push_str(&format!("- descriptor id: {}\n", explanation.id));
            let origin = origin
                .map(|origin| render_runtime_origin(package_root, &origin))
                .unwrap_or_else(|| {
                    format!(
                        "package-local:{}",
                        display_path(package_root, &explanation.path)
                    )
                });
            state.stdout.push_str(&format!("- origin: {origin}\n"));
            state
                .stdout
                .push_str(&format!("- matched by: {}\n", explanation.matched_by));
        }
        (None, _) => {
            let source_status = descriptor_source_status(package_root, state);
            state.stdout.push_str("- kind: unknown\n");
            state.stdout.push_str("- descriptor id: unresolved\n");
            state.stdout.push_str("- origin: unresolved\n");
            state.stdout.push_str(&format!(
                "- searched sources: package-local:{}, repo-config:{}, lock:{}\n",
                package_root.join(".mds/descriptors").display(),
                source_status.config_status(),
                source_status.lock_status()
            ));
            state.stdout.push_str(
                "- next action: run `mds init descriptor <kind> <name> --yes` or add a descriptor source\n",
            );
            state.diagnostics.push(Diagnostic::error(
                None,
                format!("descriptor target `{target}` could not be resolved"),
            ));
        }
    }
}

fn load_descriptor_records_from_registry(
    report: &descriptor::DescriptorRegistryReport,
    state: &mut RunState,
) -> Vec<DescriptorRecord> {
    let mut paths = report
        .entries
        .iter()
        .filter_map(|entry| entry.origin.file.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| load_descriptor_record(&path, state))
        .collect()
}

fn resolve_runtime_origin(package_root: &Path, target: &str) -> Option<DescriptorOrigin> {
    descriptor::with_workspace_descriptor_root(Some(package_root), || {
        if let Some(origin) = descriptor::descriptor_origin_for_key(target) {
            return Some(origin);
        }
        if let Some(origin) = descriptor::package_manager_origin_for_id(target) {
            return Some(origin);
        }
        if let Some(origin) = descriptor::tool_origin_for_command(target) {
            return Some(origin);
        }
        let target_path = Path::new(target);
        if target.contains('/') || target.contains('\\') || target.ends_with(".md") {
            if let Some(lang) =
                descriptor::resolve_markdown_lang(target_path, std::iter::empty::<&str>())
            {
                return descriptor::descriptor_origin_for_key(lang.key());
            }
        }
        None
    })
}

fn render_runtime_origin(package_root: &Path, origin: &DescriptorOrigin) -> String {
    let kind = match origin.source_kind {
        descriptor::DescriptorSourceKind::PackageLocal => "package-local",
        descriptor::DescriptorSourceKind::Workspace => "workspace",
        descriptor::DescriptorSourceKind::Local => "source-pack",
        descriptor::DescriptorSourceKind::Git => "git-source",
        descriptor::DescriptorSourceKind::Global => "global",
        descriptor::DescriptorSourceKind::Unknown => "unknown",
    };
    match origin.file.as_deref() {
        Some(file) => format!("{kind}:{}", display_path(package_root, file)),
        None => kind.to_string(),
    }
}

fn render_descriptor_schema(
    kind: DescriptorKind,
    format: DescriptorSchemaFormat,
    state: &mut RunState,
) -> Result<(), String> {
    match format {
        DescriptorSchemaFormat::JsonSchema => {
            let schema = match kind {
                DescriptorKind::Language => language_schema(),
                DescriptorKind::Tool => tool_schema(),
                DescriptorKind::PackageManager => package_manager_schema(),
            };
            let rendered = serde_json::to_string_pretty(&schema)
                .map_err(|error| format!("failed to render descriptor schema: {error}"))?;
            state.stdout.push_str(&rendered);
            state.stdout.push('\n');
        }
    }
    Ok(())
}

fn render_descriptor_sources(package_root: &Path, state: &mut RunState) {
    let status = descriptor_source_status(package_root, state);
    state.stdout.push_str("Descriptor sources:\n");
    state
        .stdout
        .push_str(&format!("- repo config: {}\n", status.config_status()));
    state
        .stdout
        .push_str(&format!("- source lock: {}\n", status.lock_status()));
    state.stdout.push_str("- user store: not configured\n");
    state.stdout.push_str("- global store: not configured\n");
}

#[derive(Debug, Clone)]
struct DescriptorRecord {
    kind: DescriptorKind,
    id: String,
    path: PathBuf,
    aliases: Vec<String>,
    match_keys: Vec<String>,
}

#[derive(Debug)]
struct DescriptorExplanation {
    kind: DescriptorKind,
    id: String,
    path: PathBuf,
    matched_by: String,
}

#[derive(Debug)]
struct DescriptorSourceStatus {
    config: SourceFileStatus,
    lock: SourceFileStatus,
}

#[derive(Debug)]
enum SourceFileStatus {
    Missing(PathBuf),
    Present(PathBuf),
    Invalid(PathBuf),
}

impl DescriptorSourceStatus {
    fn config_status(&self) -> String {
        self.config.render()
    }

    fn lock_status(&self) -> String {
        self.lock.render()
    }
}

impl SourceFileStatus {
    fn render(&self) -> String {
        match self {
            Self::Missing(path) => format!("missing ({})", path.display()),
            Self::Present(path) => format!("present ({})", path.display()),
            Self::Invalid(path) => format!("invalid ({})", path.display()),
        }
    }
}

fn load_descriptor_records(target: &Path, state: &mut RunState) -> Vec<DescriptorRecord> {
    let files = if target.is_file() {
        vec![target.to_path_buf()]
    } else {
        collect_toml_files(target)
    };

    files
        .into_iter()
        .filter_map(|path| load_descriptor_record(&path, state))
        .collect()
}

fn load_descriptor_record(path: &Path, state: &mut RunState) -> Option<DescriptorRecord> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to read descriptor: {error}"),
            ));
            return None;
        }
    };
    let value = match content.parse::<Value>() {
        Ok(value) => value,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to parse descriptor TOML: {error}"),
            ));
            return None;
        }
    };
    let Some(kind) = infer_descriptor_kind(path, &value) else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            "descriptor kind could not be inferred; place it under languages, tools, linters, or package-managers",
        ));
        return None;
    };

    validate_required_fields(kind, path, &value, state);
    validate_diagnostic_capture_rules(path, &value, state);

    let id = string_field(&value, "id").unwrap_or_default();
    let aliases = string_array_field(&value, "aliases");
    let match_keys = match kind {
        DescriptorKind::Language => {
            let mut keys = string_array_field(&value, "match_suffixes");
            if let Some(primary_ext) = table_field(&value, "language")
                .and_then(|language| string_field(language, "primary_ext"))
            {
                if !keys.contains(&primary_ext) {
                    keys.push(primary_ext);
                }
            }
            keys
        }
        DescriptorKind::Tool => string_array_field(&value, "match_prefixes"),
        DescriptorKind::PackageManager => Vec::new(),
    };

    Some(DescriptorRecord {
        kind,
        id,
        path: path.to_path_buf(),
        aliases,
        match_keys,
    })
}

fn validate_required_fields(
    kind: DescriptorKind,
    path: &Path,
    value: &Value,
    state: &mut RunState,
) {
    require_string(value, "id", path, state);
    match kind {
        DescriptorKind::Language => {
            let language = require_table(value, "language", path, state);
            if let Some(language) = language {
                require_string(language, "primary_ext", path, state);
            }
            let files = require_table(value, "files", path, state);
            if let Some(files) = files {
                for section in ["source", "test"] {
                    let file_rule = require_table(files, section, path, state);
                    if let Some(file_rule) = file_rule {
                        require_string(file_rule, "extension", path, state);
                    }
                }
            }
        }
        DescriptorKind::Tool => {
            require_non_empty_string_array(value, "match_prefixes", path, state);
        }
        DescriptorKind::PackageManager => {
            require_string(value, "display_name", path, state);
        }
    }
}

fn validate_descriptor_collisions(records: &[DescriptorRecord], state: &mut RunState) {
    let mut ids: HashMap<(DescriptorKind, String), &Path> = HashMap::new();
    let mut aliases: HashMap<(DescriptorKind, String), &Path> = HashMap::new();
    let mut match_keys: HashMap<(DescriptorKind, String), &Path> = HashMap::new();

    for record in records {
        if !record.id.is_empty() {
            record_unique(
                &mut ids,
                record.kind,
                &record.id,
                &record.path,
                "descriptor id",
                state,
            );
            record_unique(
                &mut aliases,
                record.kind,
                &record.id,
                &record.path,
                "descriptor alias",
                state,
            );
        }
        for alias in &record.aliases {
            record_unique(
                &mut aliases,
                record.kind,
                alias,
                &record.path,
                "descriptor alias",
                state,
            );
        }
        for key in &record.match_keys {
            let label = match record.kind {
                DescriptorKind::Language => "language match suffix",
                DescriptorKind::Tool => "tool match prefix",
                DescriptorKind::PackageManager => "package-manager match key",
            };
            record_unique(
                &mut match_keys,
                record.kind,
                key,
                &record.path,
                label,
                state,
            );
        }
    }
}

fn record_unique<'a>(
    seen: &mut HashMap<(DescriptorKind, String), &'a Path>,
    kind: DescriptorKind,
    key: &str,
    path: &'a Path,
    label: &str,
    state: &mut RunState,
) {
    if key.is_empty() {
        return;
    }
    let map_key = (kind, key.to_string());
    if let Some(existing) = seen.get(&map_key) {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("{label} `{key}` collides with {}", existing.display()),
        ));
        return;
    }
    seen.insert(map_key, path);
}

fn validate_diagnostic_capture_rules(path: &Path, value: &Value, state: &mut RunState) {
    validate_diagnostic_capture_rules_in_value(path, value, state);
}

fn validate_diagnostic_capture_rules_in_value(path: &Path, value: &Value, state: &mut RunState) {
    match value {
        Value::Table(table) => {
            for (key, child) in table {
                if key == "diagnostics" {
                    validate_diagnostics_array(path, child, state);
                }
                validate_diagnostic_capture_rules_in_value(path, child, state);
            }
        }
        Value::Array(items) => {
            for child in items {
                validate_diagnostic_capture_rules_in_value(path, child, state);
            }
        }
        _ => {}
    }
}

fn validate_diagnostics_array(path: &Path, value: &Value, state: &mut RunState) {
    let Some(items) = value.as_array() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            "diagnostics must be an array of capture rules",
        ));
        return;
    };
    for item in items {
        let Some(pattern) = string_field(item, "pattern") else {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "diagnostic capture rule requires pattern",
            ));
            continue;
        };
        let regex = match Regex::new(&pattern) {
            Ok(regex) => regex,
            Err(error) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("diagnostic capture pattern is invalid: {error}"),
                ));
                continue;
            }
        };
        let captures: HashSet<&str> = regex.capture_names().flatten().collect();
        for group_field in ["path_group", "line_group", "column_group", "message_group"] {
            let group = string_field(item, group_field).unwrap_or_else(|| {
                group_field
                    .strip_suffix("_group")
                    .unwrap_or(group_field)
                    .to_string()
            });
            if !captures.contains(group.as_str()) {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("diagnostic capture {group_field} `{group}` is not present in pattern"),
                ));
            }
        }
    }
}

fn explain_descriptor_target(
    records: &[DescriptorRecord],
    target: &str,
) -> Option<DescriptorExplanation> {
    let target_path = Path::new(target);
    if target.contains('/') || target.contains('\\') || target.ends_with(".md") {
        let name = target_path.file_name().and_then(|name| name.to_str())?;
        let markdown_name = name.strip_suffix(".md").unwrap_or(name);
        for record in records
            .iter()
            .filter(|record| record.kind == DescriptorKind::Language)
        {
            if record.match_keys.iter().any(|suffix| {
                markdown_name == suffix || markdown_name.ends_with(&format!(".{suffix}"))
            }) {
                return Some(DescriptorExplanation {
                    kind: record.kind,
                    id: record.id.clone(),
                    path: record.path.clone(),
                    matched_by: format!("file path suffix `{name}`"),
                });
            }
        }
    }

    for record in records {
        if record.id == target || record.aliases.iter().any(|alias| alias == target) {
            return Some(DescriptorExplanation {
                kind: record.kind,
                id: record.id.clone(),
                path: record.path.clone(),
                matched_by: "id or alias".to_string(),
            });
        }
    }

    for record in records
        .iter()
        .filter(|record| record.kind == DescriptorKind::Tool)
    {
        if record
            .match_keys
            .iter()
            .any(|prefix| command_matches_prefix(target, prefix))
        {
            return Some(DescriptorExplanation {
                kind: record.kind,
                id: record.id.clone(),
                path: record.path.clone(),
                matched_by: "command prefix".to_string(),
            });
        }
    }

    None
}

fn descriptor_source_status(package_root: &Path, state: &mut RunState) -> DescriptorSourceStatus {
    let config_path = package_root.join(".mds/descriptor-sources.toml");
    let lock_path = package_root.join(".mds/descriptor-sources.lock");
    let config = parse_source_file(&config_path, "descriptor source config", state);
    let lock = parse_source_file(&lock_path, "descriptor source lock", state);
    if matches!(config, SourceFileStatus::Missing(_))
        && matches!(lock, SourceFileStatus::Present(_))
    {
        state.diagnostics.push(Diagnostic::warning(
            Some(lock_path),
            "descriptor source lock exists without descriptor-sources.toml",
        ));
    }
    DescriptorSourceStatus { config, lock }
}

fn parse_source_file(path: &Path, label: &str, state: &mut RunState) -> SourceFileStatus {
    if !path.exists() {
        return SourceFileStatus::Missing(path.to_path_buf());
    }
    match fs::read_to_string(path)
        .map_err(|error| format!("failed to read {label}: {error}"))
        .and_then(|content| {
            content
                .parse::<Value>()
                .map(|_| ())
                .map_err(|error| format!("failed to parse {label}: {error}"))
        }) {
        Ok(()) => SourceFileStatus::Present(path.to_path_buf()),
        Err(message) => {
            state
                .diagnostics
                .push(Diagnostic::error(Some(path.to_path_buf()), message));
            SourceFileStatus::Invalid(path.to_path_buf())
        }
    }
}

fn render_minimal_descriptor(
    kind: DescriptorKind,
    id: &str,
    options: &InitDescriptorOptions,
) -> String {
    match kind {
        DescriptorKind::Language => {
            render_language_descriptor(id, &options.aliases, options.language.as_ref())
        }
        DescriptorKind::Tool => render_tool_descriptor(id, &options.aliases, options.tool.as_ref()),
        DescriptorKind::PackageManager => render_package_manager_descriptor(
            id,
            &options.aliases,
            options.package_manager.as_ref(),
        ),
    }
}

fn render_language_descriptor(
    id: &str,
    aliases: &[String],
    options: Option<&InitLanguageDescriptorOptions>,
) -> String {
    let match_suffixes = options
        .map(|options| clean_list(&options.match_suffixes))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![id.to_string()]);
    let fence_lang = options
        .map(|options| options.fence_lang.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let primary_ext = options
        .map(|options| options.primary_ext.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let root_module_markdown_names = options
        .map(|options| clean_list(&options.root_module_markdown_names))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![format!("index.{id}.md")]);
    let source_strip_lang_ext = options
        .map(|options| options.source_strip_lang_ext)
        .unwrap_or(true);
    let source_prefix = options
        .map(|options| options.source_prefix.trim())
        .unwrap_or("");
    let source_suffix = options
        .map(|options| options.source_suffix.trim())
        .unwrap_or("");
    let source_extension = options
        .map(|options| options.source_extension.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let test_strip_lang_ext = options
        .map(|options| options.test_strip_lang_ext)
        .unwrap_or(true);
    let test_prefix = options
        .map(|options| options.test_prefix.trim())
        .unwrap_or("");
    let test_suffix = options
        .map(|options| options.test_suffix.trim())
        .unwrap_or(".test");
    let test_extension = options
        .map(|options| options.test_extension.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(id);

    let mut rendered = format!(
        concat!(
            "id = \"{id}\"\n",
            "aliases = {aliases}\n",
            "match_suffixes = {match_suffixes}\n",
            "\n",
            "[language]\n",
            "primary_ext = \"{primary_ext}\"\n",
            "root_module_markdown_names = {root_module_markdown_names}\n",
            "\n",
            "[files.source]\n",
            "strip_lang_ext = {source_strip_lang_ext}\n",
            "prefix = \"{source_prefix}\"\n",
            "suffix = \"{source_suffix}\"\n",
            "extension = \"{source_extension}\"\n",
            "\n",
            "[files.test]\n",
            "strip_lang_ext = {test_strip_lang_ext}\n",
            "prefix = \"{test_prefix}\"\n",
            "suffix = \"{test_suffix}\"\n",
            "extension = \"{test_extension}\"\n",
            "\n",
            "[scaffold]\n",
            "fence_lang = \"{fence_lang}\"\n",
            "source_body = '''\n",
            "Implement your feature here.\n",
            "'''\n"
        ),
        id = toml_escape(id),
        aliases = render_string_array(aliases),
        match_suffixes = render_string_array(&match_suffixes),
        primary_ext = toml_escape(primary_ext),
        root_module_markdown_names = render_string_array(&root_module_markdown_names),
        source_strip_lang_ext = source_strip_lang_ext,
        source_prefix = toml_escape(source_prefix),
        source_suffix = toml_escape(source_suffix),
        source_extension = toml_escape(source_extension),
        test_strip_lang_ext = test_strip_lang_ext,
        test_prefix = toml_escape(test_prefix),
        test_suffix = toml_escape(test_suffix),
        test_extension = toml_escape(test_extension),
        fence_lang = toml_escape(fence_lang),
    );
    if let Some(options) = options {
        let quality = [
            ("typecheck", options.quality_typecheck.as_deref()),
            ("lint", options.quality_lint.as_deref()),
            ("fix", options.quality_fix.as_deref()),
            ("test", options.quality_test.as_deref()),
        ]
        .into_iter()
        .filter_map(|(key, value)| non_empty(value).map(|value| (key, value)))
        .collect::<Vec<_>>();
        if !quality.is_empty() {
            rendered.push_str("\n[quality_defaults]\n");
            for (key, value) in quality {
                rendered.push_str(&format!("{key} = \"{}\"\n", toml_escape(value)));
            }
        }
    }
    rendered
}

fn render_tool_descriptor(
    id: &str,
    aliases: &[String],
    options: Option<&InitToolDescriptorOptions>,
) -> String {
    let match_prefixes = options
        .map(|options| clean_list(&options.match_prefixes))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![id.to_string()]);
    let input = options
        .map(|options| options.input.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("stdin");
    let output = options
        .map(|options| options.output.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("stdout");
    let append_file_arg = options
        .map(|options| options.append_file_arg)
        .unwrap_or(false);
    format!(
        concat!(
            "id = \"{id}\"\n",
            "aliases = {aliases}\n",
            "match_prefixes = {match_prefixes}\n",
            "\n",
            "[behavior]\n",
            "input = \"{input}\"\n",
            "output = \"{output}\"\n",
            "append_file_arg = {append_file_arg}\n"
        ),
        id = toml_escape(id),
        aliases = render_string_array(aliases),
        match_prefixes = render_string_array(&match_prefixes),
        input = toml_escape(input),
        output = toml_escape(output),
        append_file_arg = append_file_arg,
    )
}

fn render_package_manager_descriptor(
    id: &str,
    aliases: &[String],
    options: Option<&InitPackageManagerDescriptorOptions>,
) -> String {
    let display_name = options
        .map(|options| options.display_name.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let lang = options.map(|options| options.lang.trim()).unwrap_or("");
    let metadata_files = options
        .map(|options| clean_list(&options.metadata_files))
        .unwrap_or_default();
    let lockfiles = options
        .map(|options| clean_list(&options.lockfiles))
        .unwrap_or_default();
    let metadata_reader = options
        .map(|options| options.metadata_reader.trim())
        .unwrap_or("");
    let mut rendered = format!(
        concat!(
            "id = \"{id}\"\n",
            "aliases = {aliases}\n",
            "display_name = \"{display_name}\"\n",
            "lang = \"{lang}\"\n",
            "metadata_files = {metadata_files}\n",
            "lockfiles = {lockfiles}\n",
            "metadata_reader = \"{metadata_reader}\"\n",
            "\n",
            "[commands]\n"
        ),
        id = toml_escape(id),
        aliases = render_string_array(aliases),
        display_name = toml_escape(display_name),
        lang = toml_escape(lang),
        metadata_files = render_string_array(&metadata_files),
        lockfiles = render_string_array(&lockfiles),
        metadata_reader = toml_escape(metadata_reader),
    );
    if let Some(options) = options {
        for (key, value) in [
            ("install", options.install.as_deref()),
            ("build", options.build.as_deref()),
            ("typecheck", options.typecheck.as_deref()),
            ("lint", options.lint.as_deref()),
            ("test", options.test.as_deref()),
        ] {
            if let Some(value) = non_empty(value) {
                rendered.push_str(&format!("{key} = \"{}\"\n", toml_escape(value)));
            }
        }
    }
    rendered
}

fn clean_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn render_string_array(values: &[String]) -> String {
    let items = clean_list(values)
        .into_iter()
        .map(|value| format!("\"{}\"", toml_escape(&value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn descriptor_output_path(
    package_root: &Path,
    options: &InitDescriptorOptions,
    id: &str,
) -> Result<PathBuf, String> {
    let path = match options.output.as_deref() {
        Some(output) => resolve_cli_path(package_root, output),
        None => package_root
            .join(".mds/descriptors")
            .join(match options.kind {
                DescriptorKind::Language => "languages",
                DescriptorKind::Tool => "tools",
                DescriptorKind::PackageManager => "package-managers",
            })
            .join(format!("{id}.toml")),
    };
    let normalized_root = normalize_path(package_root);
    let normalized_path = normalize_path(&path);
    if !normalized_path.starts_with(&normalized_root) {
        return Err("descriptor output path must stay within the package root".to_string());
    }
    Ok(normalized_path)
}

fn descriptor_id_from_name(name: &str) -> String {
    let mut id = String::new();
    let mut previous_dash = false;
    for ch in name.trim().chars() {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '-' | '_' | ' ') {
            Some('-')
        } else {
            None
        };
        let Some(next) = next else {
            continue;
        };
        if next == '-' {
            if id.is_empty() || previous_dash {
                continue;
            }
            previous_dash = true;
            id.push(next);
        } else {
            previous_dash = false;
            id.push(next);
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    id
}

fn infer_descriptor_kind(path: &Path, value: &Value) -> Option<DescriptorKind> {
    let components: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
        .collect();
    if components.iter().any(|part| part == "languages") {
        return Some(DescriptorKind::Language);
    }
    if components
        .iter()
        .any(|part| part == "tools" || part == "linters")
    {
        return Some(DescriptorKind::Tool);
    }
    if components.iter().any(|part| part == "package-managers") {
        return Some(DescriptorKind::PackageManager);
    }
    if table_field(value, "language").is_some() && table_field(value, "files").is_some() {
        return Some(DescriptorKind::Language);
    }
    if value.get("match_prefixes").is_some() || table_field(value, "behavior").is_some() {
        return Some(DescriptorKind::Tool);
    }
    if value.get("display_name").is_some() || value.get("metadata_files").is_some() {
        return Some(DescriptorKind::PackageManager);
    }
    None
}

fn collect_toml_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_toml_files_into(root, &mut files);
    files.sort();
    files
}

fn collect_toml_files_into(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_toml_files_into(&path, files);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("toml") {
            files.push(path);
        }
    }
}

fn resolve_cli_path(base: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        normalize_path(path)
    } else {
        normalize_path(&base.join(path))
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn command_matches_prefix(command: &str, prefix: &str) -> bool {
    let command_tokens: Vec<&str> = command.split_whitespace().collect();
    let prefix_tokens: Vec<&str> = prefix.split_whitespace().collect();
    if prefix_tokens.is_empty() || prefix_tokens.len() > command_tokens.len() {
        return false;
    }
    prefix_tokens
        .iter()
        .zip(command_tokens.iter())
        .all(|(expected, actual)| actual == expected)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_string)
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn table_field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key).filter(|value| value.is_table())
}

fn require_table<'a>(
    value: &'a Value,
    key: &str,
    path: &Path,
    state: &mut RunState,
) -> Option<&'a Value> {
    match value.get(key) {
        Some(value) if value.is_table() => Some(value),
        _ => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("descriptor requires table `{key}`"),
            ));
            None
        }
    }
}

fn require_string(value: &Value, key: &str, path: &Path, state: &mut RunState) {
    match value.get(key).and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => {}
        _ => state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("descriptor requires string field `{key}`"),
        )),
    }
}

fn require_non_empty_string_array(value: &Value, key: &str, path: &Path, state: &mut RunState) {
    let valid = value
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.as_str().is_some_and(|s| !s.is_empty()))
        });
    if !valid {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("descriptor requires non-empty string array `{key}`"),
        ));
    }
}

fn language_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "mds language descriptor",
        "type": "object",
        "required": ["id", "language", "files"],
        "additionalProperties": true,
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "aliases": { "type": "array", "items": { "type": "string" } },
            "match_suffixes": { "type": "array", "items": { "type": "string" } },
            "language": {
                "type": "object",
                "required": ["primary_ext"],
                "additionalProperties": true,
                "properties": {
                    "primary_ext": { "type": "string", "minLength": 1 },
                    "root_module_markdown_names": { "type": "array", "items": { "type": "string" } }
                }
            },
            "files": {
                "type": "object",
                "required": ["source", "test"],
                "additionalProperties": true,
                "properties": {
                    "source": { "$ref": "#/$defs/fileRule" },
                    "test": { "$ref": "#/$defs/fileRule" }
                }
            }
        },
        "$defs": {
            "fileRule": {
                "type": "object",
                "required": ["extension"],
                "additionalProperties": true,
                "properties": {
                    "strip_lang_ext": { "type": "boolean" },
                    "prefix": { "type": "string" },
                    "suffix": { "type": "string" },
                    "extension": { "type": "string", "minLength": 1 }
                }
            }
        }
    })
}

fn tool_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "mds tool descriptor",
        "type": "object",
        "required": ["id", "match_prefixes"],
        "additionalProperties": true,
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "match_prefixes": { "type": "array", "items": { "type": "string" }, "minItems": 1 },
            "behavior": { "$ref": "#/$defs/toolBehavior" }
        },
        "$defs": {
            "toolBehavior": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "input": { "type": "string" },
                    "output": { "type": "string" },
                    "append_file_arg": { "type": "boolean" },
                    "diagnostics": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/diagnosticCapture" }
                    }
                }
            },
            "diagnosticCapture": {
                "type": "object",
                "required": ["pattern"],
                "additionalProperties": true,
                "properties": {
                    "pattern": { "type": "string" },
                    "path_group": { "type": "string" },
                    "line_group": { "type": "string" },
                    "column_group": { "type": "string" },
                    "message_group": { "type": "string" },
                    "severity": { "type": "string" },
                    "line_offset": { "type": "integer" }
                }
            }
        }
    })
}

fn package_manager_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "mds package-manager descriptor",
        "type": "object",
        "required": ["id", "display_name"],
        "additionalProperties": true,
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "aliases": { "type": "array", "items": { "type": "string" } },
            "display_name": { "type": "string", "minLength": 1 },
            "lang": { "type": "string" },
            "metadata_files": { "type": "array", "items": { "type": "string" } },
            "lockfiles": { "type": "array", "items": { "type": "string" } },
            "metadata_reader": { "type": "string" },
            "commands": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "install": { "type": "string" },
                    "build": { "type": "string" },
                    "typecheck": { "type": "string" },
                    "lint": { "type": "string" },
                    "test": { "type": "string" }
                }
            }
        }
    })
}
