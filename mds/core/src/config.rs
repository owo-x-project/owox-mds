use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::model::CheckDiagnosticPolicy;
use crate::model::Config;
use crate::model::DoctorVersionFloor;
use crate::model::Lang;
use crate::model::LinkPolicy;
use crate::model::OutputKind;
use crate::model::OutputOverride;
use crate::model::CANONICAL_SOURCE_MD_ROOT;
use crate::model::CANONICAL_TEST_MD_ROOT;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
pub fn merge_config_file(config: &mut Config, path: &Path, state: &mut RunState) -> Option<()> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to read config: {error}"),
            ));
            return None;
        }
    };

    merge_config_text(config, path, &text, state)
}

pub fn merge_config_text(
    config: &mut Config,
    path: &Path,
    text: &str,
    state: &mut RunState,
) -> Option<()> {
    let value = match text.parse::<toml::Value>() {
        Ok(value) => value,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to parse mds.config.toml: {error}"),
            ));
            return None;
        }
    };
    let Some(root) = value.as_table() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            "mds.config.toml must contain TOML tables",
        ));
        return None;
    };

    for key in root.keys() {
        if !is_supported_top_level_table(key) {
            state.diagnostics.push(Diagnostic::warning(
                Some(path.to_path_buf()),
                format!("ignoring unsupported config table `{key}`"),
            ));
        }
    }

    if let Some(package) = root.get("package").and_then(toml::Value::as_table) {
        for (key, value) in package {
            match key.as_str() {
                "enabled" => config.enabled = bool_value(value, path, key, state),
                "allow_raw_source" => config.allow_raw_source = bool_value(value, path, key, state),
                "copy_source_assets" | "copy-source-assets" => {
                    config.copy_source_assets = bool_value(value, path, key, state)
                }
                "mds_version" | "mds-version" => {
                    config.mds_version = Some(string_value(value, path, key, state));
                }
                _ => warn_unsupported(path, state, "package config", key),
            }
        }
    }

    if let Some(authoring) = root.get("authoring").and_then(toml::Value::as_table) {
        for (key, value) in authoring {
            match key.as_str() {
                "link_policy" | "link-policy" => {
                    if let Some(policy) = link_policy_value(value, path, key, state) {
                        config.link_policy = policy;
                    }
                }
                _ => warn_unsupported(path, state, "authoring config", key),
            }
        }
    }

    for table_name in ["check", "checks"] {
        if let Some(check) = root.get(table_name).and_then(toml::Value::as_table) {
            for (key, value) in check {
                match key.as_str() {
                    "code_blocks_required" | "code_block_required" => {
                        config.check.code_blocks_required = bool_value(value, path, key, state)
                    }
                    "code_fence_integrity" | "code_fences" => {
                        config.check.code_fence_integrity = bool_value(value, path, key, state)
                    }
                    "duplicate_h2_sections" | "duplicate_sections" => {
                        config.check.duplicate_h2_sections = bool_value(value, path, key, state)
                    }
                    "markdown_links" | "links" => {
                        config.check.markdown_links = bool_value(value, path, key, state)
                    }
                    "import_with_implementation" | "imports_with_implementation" => {
                        config.check.import_with_implementation =
                            bool_value(value, path, key, state)
                    }
                    "top_level_fence_required" | "multiple_top_level_implementations" => {
                        config.check.top_level_fence_required = bool_value(value, path, key, state)
                    }
                    "doc_comments_outside_code" | "doc_comment_outside_code" => {
                        config.check.doc_comments_outside_code = bool_value(value, path, key, state)
                    }
                    "documented_sections" | "documentation_sections" => {
                        config.check.documented_sections = bool_value(value, path, key, state)
                    }
                    "documented_exports" | "export_documentation" => {
                        config.check.documented_exports = bool_value(value, path, key, state)
                    }
                    "legacy_tables" => {
                        if let Some(policy) = check_diagnostic_policy_value(value, path, key, state)
                        {
                            config.check.legacy_tables = policy;
                        }
                    }
                    "unresolved_module_symbols" => {
                        if let Some(policy) = check_diagnostic_policy_value(value, path, key, state)
                        {
                            config.check.unresolved_module_symbols = policy;
                        }
                    }
                    "implementation_section_only" => {
                        config.check.implementation_section_only =
                            bool_value(value, path, key, state)
                    }
                    "split_source_and_test" => {
                        config.check.split_source_and_test = bool_value(value, path, key, state)
                    }
                    _ => warn_unsupported(path, state, "check config", key),
                }
            }
        }
    }

    if let Some(roots) = root.get("roots").and_then(toml::Value::as_table) {
        for (key, value) in roots {
            match key.as_str() {
                "source_md" => {
                    if let Some(root) =
                        canonical_root_value(value, path, key, CANONICAL_SOURCE_MD_ROOT, state)
                    {
                        config.roots.source_md = root;
                    }
                }
                "test_md" => {
                    if let Some(root) =
                        canonical_root_value(value, path, key, CANONICAL_TEST_MD_ROOT, state)
                    {
                        config.roots.test_md = root;
                    }
                }
                "source_out" => {
                    config.roots.source_out = PathBuf::from(string_value(value, path, key, state))
                }
                "test_out" => {
                    config.roots.test_out = PathBuf::from(string_value(value, path, key, state))
                }
                "exclude" | "excludes" => {
                    config.excludes = string_array_value(value, path, key, state)
                }
                _ => warn_unsupported(path, state, "roots config", key),
            }
        }
    }

    if let Some(output) = root.get("output").and_then(toml::Value::as_table) {
        for (key, value) in output {
            match key.as_str() {
                "source" => config.output.source = Some(string_value(value, path, key, state)),
                "test" => config.output.test = Some(string_value(value, path, key, state)),
                "override" => {
                    config.output.overrides = output_override_array_value(value, path, state)
                }
                _ => warn_unsupported(path, state, "output config", key),
            }
        }
    }

    if let Some(adapters) = root.get("adapters").and_then(toml::Value::as_table) {
        for (adapter, value) in adapters {
            let Some(lang) = lang_from_key(adapter) else {
                warn_unsupported(path, state, "adapter config", adapter);
                continue;
            };
            let Some(table) = value.as_table() else {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("adapter config `{adapter}` must be a table"),
                ));
                continue;
            };
            for (key, value) in table {
                if key == "enabled" {
                    config
                        .adapters
                        .insert(lang.clone(), bool_value(value, path, key, state));
                } else {
                    warn_unsupported(path, state, &format!("adapter config `{adapter}`"), key);
                }
            }
        }
    }

    if let Some(quality) = root.get("quality").and_then(toml::Value::as_table) {
        for (adapter, value) in quality {
            let Some(lang) = lang_from_key(adapter) else {
                warn_unsupported(path, state, "quality config", adapter);
                continue;
            };
            let Some(table) = value.as_table() else {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("quality config `{adapter}` must be a table"),
                ));
                continue;
            };
            let entry = config
                .quality
                .entry(lang)
                .or_insert_with(crate::model::QualityConfig::default);
            for (key, value) in table {
                match key.as_str() {
                    "type_check" | "type_checker" => {
                        let (command, disabled) = optional_command_setting(value, path, key, state);
                        entry.type_check = command;
                        entry.type_check_disabled = disabled;
                    }
                    "lint" | "linter" => {
                        let (command, disabled) = optional_command_setting(value, path, key, state);
                        entry.lint = command;
                        entry.lint_disabled = disabled;
                    }
                    "fix" | "fixer" => {
                        let (command, disabled) = optional_command_setting(value, path, key, state);
                        entry.fix = command;
                        entry.fix_disabled = disabled;
                    }
                    "test" | "test_runner" => {
                        let (command, disabled) = optional_command_setting(value, path, key, state);
                        entry.test = command;
                        entry.test_disabled = disabled;
                    }
                    "required" => entry.required = string_array_value(value, path, key, state),
                    "optional" => entry.optional = string_array_value(value, path, key, state),
                    _ => warn_unsupported(path, state, &format!("quality config `{adapter}`"), key),
                }
            }
        }
    }

    if let Some(doctor) = root.get("doctor").and_then(toml::Value::as_table) {
        for (key, value) in doctor {
            match key.as_str() {
                "required" | "optional" => reject_legacy_doctor_tool_policy(path, key, state),
                "version_floor" | "version-floor" => {
                    config
                        .doctor
                        .version_floor
                        .extend(doctor_version_floor_value(value, path, key, state));
                }
                _ => warn_unsupported(path, state, "doctor config", key),
            }
        }
    }

    for table_name in ["package_sync", "package-sync"] {
        if let Some(package_sync) = root.get(table_name).and_then(toml::Value::as_table) {
            for (key, value) in package_sync {
                match key.as_str() {
                    "hook_enabled" | "hook-enabled" => {
                        config.package_sync_hook_enabled = bool_value(value, path, key, state);
                        if config.package_sync_hook_enabled && config.package_sync_hook.is_none() {
                            config.package_sync_hook = Some("mds package sync".to_string());
                        }
                    }
                    "hook" | "post_hook" | "post-command" | "post_command" | "hook_command"
                    | "hook-command" => {
                        config.package_sync_hook = Some(string_value(value, path, key, state));
                    }
                    _ => warn_unsupported(path, state, "package sync config", key),
                }
            }
        }
    }

    for table_name in ["labels", "label_overrides", "label-overrides"] {
        if let Some(labels) = root.get(table_name).and_then(toml::Value::as_table) {
            for (key, value) in labels {
                let canonical = key.to_ascii_lowercase();
                if is_supported_label(&canonical) {
                    config
                        .label_overrides
                        .insert(canonical, string_value(value, path, key, state));
                } else {
                    state.diagnostics.push(Diagnostic::error(
                        Some(path.to_path_buf()),
                        format!("unsupported label override `{key}`"),
                    ));
                }
            }
        }
    }

    Some(())
}

pub(crate) fn is_supported_label(key: &str) -> bool {
    matches!(
        key,
        "purpose"
            | "contract"
            | "architecture"
            | "rules"
            | "source"
            | "covers"
            | "cases"
            | "test"
            | "expose"
            | "exposes"
            | "from"
            | "target"
            | "summary"
            | "kind"
            | "name"
            | "version"
    )
}

fn is_supported_top_level_table(section: &str) -> bool {
    matches!(
        section,
        "package"
            | "authoring"
            | "check"
            | "checks"
            | "roots"
            | "output"
            | "adapters"
            | "quality"
            | "doctor"
            | "package_sync"
            | "package-sync"
            | "labels"
            | "label_overrides"
            | "label-overrides"
    )
}

fn link_policy_value(
    value: &toml::Value,
    path: &Path,
    key: &str,
    state: &mut RunState,
) -> Option<LinkPolicy> {
    let Some(value) = value.as_str() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("config `{key}` must be a string"),
        ));
        return None;
    };
    match value {
        "wiki-only" => Some(LinkPolicy::WikiOnly),
        "markdown-only" => Some(LinkPolicy::MarkdownOnly),
        "mixed" => Some(LinkPolicy::Mixed),
        _ => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("unsupported link policy `{value}`"),
            ));
            None
        }
    }
}

fn canonical_root_value(
    value: &toml::Value,
    path: &Path,
    key: &str,
    canonical: &str,
    state: &mut RunState,
) -> Option<PathBuf> {
    let Some(value) = value.as_str() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("config `{key}` must be a string"),
        ));
        return None;
    };
    if value == canonical {
        Some(PathBuf::from(value))
    } else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("config `{key}` must be `{canonical}`"),
        ));
        None
    }
}

fn output_override_array_value(
    value: &toml::Value,
    path: &Path,
    state: &mut RunState,
) -> Vec<OutputOverride> {
    let Some(values) = value.as_array() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            "config `output.override` must be an array of tables",
        ));
        return Vec::new();
    };

    values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let Some(table) = value.as_table() else {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    "config `output.override` must be an array of tables",
                ));
                return None;
            };

            let mut match_pattern = None;
            let mut kind = None;
            let mut override_path = None;

            for (key, value) in table {
                match key.as_str() {
                    "match" => {
                        match_pattern =
                            Some(string_value(value, path, "output.override.match", state))
                    }
                    "kind" => kind = output_kind_value(value, path, "output.override.kind", state),
                    "path" => {
                        override_path =
                            Some(string_value(value, path, "output.override.path", state))
                    }
                    _ => warn_unsupported(path, state, "output.override", key),
                }
            }

            match (match_pattern, kind, override_path) {
                (Some(match_pattern), Some(kind), Some(path_pattern))
                    if !match_pattern.is_empty() && !path_pattern.is_empty() =>
                {
                    Some(OutputOverride {
                        match_pattern,
                        kind,
                        path: path_pattern,
                    })
                }
                _ => {
                    state.diagnostics.push(Diagnostic::error(
                        Some(path.to_path_buf()),
                        format!(
                            "config `output.override[{index}]` requires `match`, `kind`, and `path`"
                        ),
                    ));
                    None
                }
            }
        })
        .collect()
}

fn output_kind_value(
    value: &toml::Value,
    path: &Path,
    key: &str,
    state: &mut RunState,
) -> Option<OutputKind> {
    let Some(value) = value.as_str() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("config `{key}` must be a string"),
        ));
        return None;
    };

    match value {
        "source" => Some(OutputKind::Source),
        "test" => Some(OutputKind::Test),
        _ => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("config `{key}` must be `source` or `test`"),
            ));
            None
        }
    }
}

fn check_diagnostic_policy_value(
    value: &toml::Value,
    path: &Path,
    key: &str,
    state: &mut RunState,
) -> Option<CheckDiagnosticPolicy> {
    let Some(value) = value.as_str() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("config `{key}` must be a string"),
        ));
        return None;
    };

    match value {
        "warn" => Some(CheckDiagnosticPolicy::Warn),
        "error" => Some(CheckDiagnosticPolicy::Error),
        "allow" => Some(CheckDiagnosticPolicy::Allow),
        _ => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("config `{key}` must be `warn`, `error`, or `allow`"),
            ));
            None
        }
    }
}

fn lang_from_key(key: &str) -> Option<Lang> {
    if !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(Lang::Other(key.to_string()))
    } else {
        None
    }
}

fn bool_value(value: &toml::Value, path: &Path, key: &str, state: &mut RunState) -> bool {
    match value.as_bool() {
        Some(value) => value,
        None => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("config `{key}` must be a boolean"),
            ));
            false
        }
    }
}

fn string_value(value: &toml::Value, path: &Path, key: &str, state: &mut RunState) -> String {
    match value.as_str() {
        Some(value) => value.to_string(),
        None => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("config `{key}` must be a string"),
            ));
            String::new()
        }
    }
}

fn optional_command_setting(
    value: &toml::Value,
    path: &Path,
    key: &str,
    state: &mut RunState,
) -> (Option<String>, bool) {
    if value.as_bool() == Some(false) {
        return (None, true);
    }
    match value.as_str() {
        Some(command) => {
            let command = command.trim();
            if command.is_empty() {
                (None, false)
            } else if command == "false" {
                (None, true)
            } else {
                (Some(command.to_string()), false)
            }
        }
        None => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("config `{key}` must be a string or false"),
            ));
            (None, false)
        }
    }
}

fn doctor_version_floor_value(
    value: &toml::Value,
    path: &Path,
    key: &str,
    state: &mut RunState,
) -> HashMap<String, DoctorVersionFloor> {
    let Some(table) = value.as_table() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("config `{key}` must be a table"),
        ));
        return HashMap::new();
    };

    let mut version_floor = HashMap::new();
    for (command, floor) in table {
        let Some(value) = floor.as_str() else {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("config `{key}.{command}` must be a string"),
            ));
            continue;
        };
        let Some(floor) = parse_doctor_version_floor(value) else {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("config `{key}.{command}` must be `x`, `x.y`, or `x.y.z`"),
            ));
            continue;
        };
        version_floor.insert(command.to_string(), floor);
    }

    version_floor
}

fn reject_legacy_doctor_tool_policy(path: &Path, key: &str, state: &mut RunState) {
    state.diagnostics.push(Diagnostic::error(
        Some(path.to_path_buf()),
        format!("config `doctor.{key}` is not supported in v1; use `quality.<lang>.{key}` instead"),
    ));
}

fn parse_doctor_version_floor(value: &str) -> Option<DoctorVersionFloor> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = match parts.next() {
        Some(part) => part.parse().ok()?,
        None => 0,
    };
    let patch = match parts.next() {
        Some(part) => part.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(DoctorVersionFloor {
        major,
        minor,
        patch,
    })
}

fn string_array_value(
    value: &toml::Value,
    path: &Path,
    key: &str,
    state: &mut RunState,
) -> Vec<String> {
    let Some(values) = value.as_array() else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("config `{key}` must be an array of strings"),
        ));
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|value| match value.as_str() {
            Some(value) => Some(value.to_string()),
            None => {
                state.diagnostics.push(Diagnostic::error(
                    Some(path.to_path_buf()),
                    format!("config `{key}` must contain only strings"),
                ));
                None
            }
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn warn_unsupported(path: &Path, state: &mut RunState, scope: &str, key: &str) {
    state.diagnostics.push(Diagnostic::warning(
        Some(path.to_path_buf()),
        format!("ignoring unsupported {scope} `{key}`"),
    ));
}
