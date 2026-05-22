use crate::descriptor;
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::diff::unified_diff;
use crate::markdown::sections_with_labels;
use crate::model::{Config, Package};
use crate::package::read_package_metadata;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub fn validate_source_overview_required_sections(
    path: &Path,
    text: &str,
    label_overrides: &HashMap<String, String>,
    state: &mut RunState,
) {
    let sections = sections_with_labels(text, label_overrides);
    for required in ["Purpose", "Architecture", "Rules"] {
        if !sections.contains_key(required) {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("source overview requires ## {required}"),
            ));
        }
    }
}

pub(crate) fn hydrate_generated_source_overview(root: &Path, path: &Path, text: &str) -> String {
    let Some(package_manager) = descriptor::detect_package_manager(root) else {
        return text.to_string();
    };
    let package = Package {
        root: root.to_path_buf(),
        config: Config::default(),
        package_manager_id: package_manager.id,
    };
    let mut state = RunState::default();
    planned_package_overview_text(&package, path, text, &mut state)
        .unwrap_or_else(|| text.to_string())
}

pub fn validate_source_overview_text(
    package: &Package,
    path: &Path,
    text: &str,
    state: &mut RunState,
) {
    validate_source_overview_required_sections(path, text, &package.config.label_overrides, state);
    let Some(new) = planned_package_overview_text(package, path, text, state) else {
        return;
    };
    if text != new {
        push_stale_overview_diagnostic(path, state);
    }
}

pub(crate) fn sync_package_md(
    package: &Package,
    check: bool,
    state: &mut RunState,
) -> Result<(), String> {
    if package.config.package_sync_hook_enabled {
        let command = package
            .config
            .package_sync_hook
            .as_deref()
            .unwrap_or("mds package sync");
        state
            .stdout
            .push_str(&format!("package sync hook command: {command}\n"));
    }
    let Some((path, old, new)) = planned_package_overview(package, state) else {
        return Ok(());
    };
    if old == new {
        state
            .stdout
            .push_str(&format!("package sync ok: {}\n", package.root.display()));
        return Ok(());
    }
    state.stdout.push_str(&unified_diff(&path, &old, &new));
    if check {
        push_stale_overview_diagnostic(&path, state);
    } else {
        fs::write(&path, &new)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        state.generated.push(path.clone());
        state
            .stdout
            .push_str(&format!("package sync ok: {}\n", package.root.display()));
    }
    Ok(())
}

pub(crate) fn planned_package_overview(
    package: &Package,
    state: &mut RunState,
) -> Option<(PathBuf, String, String)> {
    let path = source_overview_path(package);
    let old = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.clone()),
                format!("failed to read source overview for package sync: {error}"),
            ));
            return None;
        }
    };
    let new = planned_package_overview_text(package, &path, &old, state)?;
    Some((path, old, new))
}

fn source_overview_path(package: &Package) -> PathBuf {
    descriptor::package_source_overview_markdown_path(&package.root, &package.config)
}

fn planned_package_overview_text(
    package: &Package,
    path: &Path,
    text: &str,
    state: &mut RunState,
) -> Option<String> {
    let metadata = read_package_metadata(package, state)?;
    replace_managed_region(
        text,
        "package-summary",
        &package_summary_table(&metadata.name, &metadata.version),
        path,
        state,
    )
    .and_then(|updated| {
        replace_managed_region(
            &updated,
            "dependencies",
            &dependency_table(&metadata.dependencies),
            path,
            state,
        )
    })
    .and_then(|updated| {
        replace_managed_region(
            &updated,
            "dev-dependencies",
            &dependency_table(&metadata.dev_dependencies),
            path,
            state,
        )
    })
}

fn push_stale_overview_diagnostic(path: &Path, state: &mut RunState) {
    state.diagnostics.push(Diagnostic::error(
        Some(path.to_path_buf()),
        "dependency snapshot is not synchronized with package metadata; run `mds package sync`",
    ));
}

fn replace_managed_region(
    text: &str,
    name: &str,
    replacement: &str,
    path: &Path,
    state: &mut RunState,
) -> Option<String> {
    replace_managed_section(text, name, replacement, path, state)
}

fn replace_managed_section(
    text: &str,
    name: &str,
    replacement: &str,
    path: &Path,
    state: &mut RunState,
) -> Option<String> {
    let heading = format!("### {}", managed_section_heading(name));
    let lines = text.lines().collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| line.trim() == heading) else {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!(
                "source overview is missing managed section `{}`",
                managed_section_heading(name)
            ),
        ));
        return None;
    };
    let mut end = start + 1;
    while end < lines.len() {
        let trimmed = lines[end].trim();
        if managed_section_boundary(trimmed) {
            break;
        }
        end += 1;
    }

    let mut output = String::new();
    output.push_str(&lines[..=start].join("\n"));
    output.push_str("\n\n");
    output.push_str(replacement.trim_end());
    output.push('\n');
    if end < lines.len() {
        output.push('\n');
        output.push_str(&lines[end..].join("\n"));
    }
    if text.ends_with('\n') && !output.ends_with('\n') {
        output.push('\n');
    }
    Some(output)
}

fn managed_section_boundary(line: &str) -> bool {
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    hashes > 0 && hashes <= 3 && line.as_bytes().get(hashes) == Some(&b' ')
}

fn managed_section_heading(name: &str) -> &'static str {
    match name {
        "package-summary" => "Package Summary",
        "dependencies" => "Dependencies",
        "dev-dependencies" => "Dev Dependencies",
        _ => "Managed Section",
    }
}

fn package_summary_table(name: &str, version: &str) -> String {
    format!("| Name | Version |\n| --- | --- |\n| {name} | {version} |\n")
}

fn dependency_table(dependencies: &std::collections::HashMap<String, String>) -> String {
    let sorted = dependencies
        .iter()
        .map(|(name, version)| (name.clone(), version.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut output = String::from("| Name | Version | Summary |\n| --- | --- | --- |\n");
    for (name, version) in sorted {
        output.push_str(&format!("| {name} | {version} |  |\n"));
    }
    output
}
