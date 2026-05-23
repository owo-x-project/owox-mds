use crate::config::merge_config_file;
use crate::descriptor;
use crate::descriptor::{is_package_overview_markdown_path, PackageOverviewKind};
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::fs_utils::collect_files;
use crate::fs_utils::is_excluded;
use crate::markdown::source_markdown_root;
use crate::markdown::validate_markdown_links;
use crate::model::Package;
use crate::model::PackageMetadata;
use crate::package_sync::validate_source_overview_required_sections;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

pub fn discover_package_roots(cwd: &Path, package: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    if let Some(package) = package {
        let root = if package.is_absolute() {
            package.to_path_buf()
        } else {
            cwd.join(package)
        };
        return Ok(vec![root]);
    }

    let mut roots = collect_files(cwd, true)?
        .into_iter()
        .filter(|path| path.file_name() == Some(OsStr::new("mds.config.toml")))
        .filter_map(|path| path.parent().map(Path::to_path_buf))
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    Ok(roots)
}

pub fn discover_packages(
    cwd: &Path,
    package: Option<&Path>,
    state: &mut RunState,
) -> Result<Vec<Package>, String> {
    let root_config = cwd.join("mds.config.toml");
    let mut base_config = crate::model::Config::default();
    if root_config.exists() {
        merge_config_file(&mut base_config, &root_config, state);
    }

    let mut packages = Vec::new();
    for root in discover_package_roots(cwd, package)? {
        if let Some(package) = load_package(&root, &base_config, state) {
            packages.push(package);
        }
    }
    packages.sort_by(|left, right| left.root.cmp(&right.root));
    Ok(packages)
}

pub fn load_package(
    root: &Path,
    base_config: &crate::model::Config,
    state: &mut RunState,
) -> Option<Package> {
    let config_path = root.join("mds.config.toml");
    if !config_path.exists() {
        state.diagnostics.push(Diagnostic::error(
            Some(root.to_path_buf()),
            "mds.config.toml is required for an mds package",
        ));
        return None;
    }

    let mut config = base_config.clone();
    merge_config_file(&mut config, &config_path, state)?;
    if !config.enabled {
        return None;
    }

    descriptor::clear_descriptor_diagnostics();
    let package_manager = match descriptor::with_workspace_descriptor_root(Some(root), || {
        let manager = descriptor::detect_package_manager(root);
        let _ = descriptor::language_descriptors();
        manager
    }) {
        Some(manager) => manager,
        None => {
            state
                .diagnostics
                .extend(descriptor::drain_descriptor_diagnostics());
            state.diagnostics.push(Diagnostic::error(
                Some(root.to_path_buf()),
                "enabled package requires a recognized package manager metadata file",
            ));
            return None;
        }
    };
    state
        .diagnostics
        .extend(descriptor::drain_descriptor_diagnostics());

    Some(Package {
        root: root.to_path_buf(),
        config,
        package_manager_id: package_manager.id,
    })
}

pub fn validate_package_md(package: &Package, state: &mut RunState) {
    let Some((path, old, new)) = crate::package_sync::planned_package_overview(package, state)
    else {
        return;
    };
    if old != new {
        state.diagnostics.push(Diagnostic::error(
            Some(path),
            "dependency snapshot is not synchronized with package metadata; run `mds package sync`",
        ));
    }
}

pub fn read_package_metadata(package: &Package, state: &mut RunState) -> Option<PackageMetadata> {
    let Some(manager) = descriptor::with_workspace_descriptor_root(Some(&package.root), || {
        descriptor::package_manager_for_id(&package.package_manager_id)
    }) else {
        state.diagnostics.push(Diagnostic::error(
            Some(package.root.clone()),
            format!("unknown package manager `{}`", package.package_manager_id),
        ));
        return None;
    };
    read_package_metadata_for_manager(&package.root, &manager, state)
}

pub(crate) fn read_package_metadata_for_manager(
    root: &Path,
    manager: &descriptor::PackageManagerManifest,
    state: &mut RunState,
) -> Option<PackageMetadata> {
    let Some(path) = manager.metadata_path(root) else {
        state.diagnostics.push(Diagnostic::error(
            Some(root.to_path_buf()),
            format!("package manager `{}` metadata file is missing", manager.id),
        ));
        return None;
    };
    match manager.metadata_reader.as_str() {
        "node-package-json" | "vcpkg-json" => read_node_metadata(&path, state),
        "pyproject-toml" => read_python_metadata(&path, state),
        "cargo-toml" => read_toml_metadata(&path, &["package"], state),
        "pubspec-yaml" => read_pubspec_metadata(&path, state),
        "dotnet-xml" => read_dotnet_metadata(&path, state),
        "cmake-text" => read_cmake_metadata(&path, state),
        "meson-text" => read_meson_metadata(&path, state),
        "conan-text" => read_conan_metadata(&path, state),
        "bundler" => read_bundler_metadata(root, &path, state),
        "zig-zon" => read_zig_metadata(&path, state),
        _ => read_generic_metadata(root, &path),
    }
}

pub fn read_node_metadata(path: &Path, state: &mut RunState) -> Option<PackageMetadata> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to read package metadata: {error}"),
            ));
            return None;
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to parse package.json: {error}"),
            ));
            return None;
        }
    };
    let name = json_string_value(value.get("name"));
    let version = json_string_value(value.get("version"));
    match (name, version) {
        (Some(name), Some(version)) => Some(PackageMetadata {
            name,
            version,
            dependencies: json_dependency_object(value.get("dependencies")),
            dev_dependencies: json_dependency_object(value.get("devDependencies")),
        }),
        _ => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "package metadata requires name and version",
            ));
            None
        }
    }
}

fn json_string_value(value: Option<&serde_json::Value>) -> Option<String> {
    value?.as_str().map(ToOwned::to_owned)
}

fn json_dependency_object(value: Option<&serde_json::Value>) -> HashMap<String, String> {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return HashMap::new();
    };
    object
        .iter()
        .map(|(name, value)| (name.clone(), json_dependency_version(value)))
        .collect()
}

fn json_dependency_version(value: &serde_json::Value) -> String {
    if let Some(version) = value.as_str() {
        return version.to_string();
    }
    if let Some(version) = value.get("version").and_then(serde_json::Value::as_str) {
        return version.to_string();
    }
    value.to_string()
}

pub fn read_toml_metadata(
    path: &Path,
    table_path: &[&str],
    state: &mut RunState,
) -> Option<PackageMetadata> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to read package metadata: {error}"),
            ));
            return None;
        }
    };
    let value = match text.parse::<toml::Value>() {
        Ok(value) => value,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to parse TOML package metadata: {error}"),
            ));
            return None;
        }
    };
    let section_name = table_path.join(".");
    let fields = toml_table_path(&value, table_path);
    let name = fields.and_then(|fields| toml_string_value(fields.get("name")));
    let version = fields.and_then(|fields| toml_string_value(fields.get("version")));
    match (name, version) {
        (Some(name), Some(version)) => Some(PackageMetadata {
            name,
            version,
            dependencies: toml_dependency_section(&value, "dependencies"),
            dev_dependencies: toml_dependency_section(&value, "dev-dependencies"),
        }),
        _ => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("package metadata requires [{section_name}] name and version"),
            ));
            None
        }
    }
}

pub fn read_python_metadata(path: &Path, state: &mut RunState) -> Option<PackageMetadata> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to read package metadata: {error}"),
            ));
            return None;
        }
    };
    let value = match text.parse::<toml::Value>() {
        Ok(value) => value,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to parse pyproject.toml: {error}"),
            ));
            return None;
        }
    };
    let project = toml_table_path(&value, &["project"]);
    let name = project.and_then(|project| toml_string_value(project.get("name")));
    let version = project.and_then(|project| toml_string_value(project.get("version")));
    match (name, version) {
        (Some(name), Some(version)) => Some(PackageMetadata {
            name,
            version,
            dependencies: project
                .and_then(|project| project.get("dependencies"))
                .map(toml_dependency_array)
                .unwrap_or_default(),
            dev_dependencies: pyproject_optional_dependencies(&value, "dev"),
        }),
        _ => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "package metadata requires [project] name and version",
            ));
            None
        }
    }
}

pub fn read_pubspec_metadata(path: &Path, state: &mut RunState) -> Option<PackageMetadata> {
    let text = fs::read_to_string(path).ok()?;
    let value = match serde_yaml::from_str::<serde_yaml::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!("failed to parse pubspec.yaml: {error}"),
            ));
            return None;
        }
    };
    let name = yaml_string(value.get("name"));
    let version = yaml_string(value.get("version")).unwrap_or_else(|| "0.1.0".to_string());
    Some(PackageMetadata {
        name: name.unwrap_or_else(|| fallback_package_name(path)),
        version,
        dependencies: yaml_dependency_object(value.get("dependencies")),
        dev_dependencies: yaml_dependency_object(value.get("dev_dependencies")),
    })
}

pub fn read_dotnet_metadata(path: &Path, state: &mut RunState) -> Option<PackageMetadata> {
    let text = fs::read_to_string(path).ok()?;
    let name = capture_first(
        &text,
        &[
            r"<AssemblyName>([^<]+)</AssemblyName>",
            r"<PackageId>([^<]+)</PackageId>",
        ],
    )
    .unwrap_or_else(|| fallback_package_name(path));
    let version = capture_first(&text, &[r"<Version>([^<]+)</Version>"])
        .unwrap_or_else(|| "0.1.0".to_string());
    Some(PackageMetadata {
        name,
        version,
        dependencies: capture_many_pairs(
            &text,
            r#"<PackageReference[^>]*Include=\"([^\"]+)\"[^>]*Version=\"([^\"]+)\"[^>]*/?>"#,
            state,
        ),
        dev_dependencies: HashMap::new(),
    })
}

pub fn read_cmake_metadata(path: &Path, _state: &mut RunState) -> Option<PackageMetadata> {
    let text = fs::read_to_string(path).ok()?;
    let name = capture_first(
        &text,
        &[r"project\(([^\s\)]+)", r"set\(PROJECT_NAME\s+([^\s\)]+)"],
    )
    .unwrap_or_else(|| fallback_package_name(path));
    let version =
        capture_first(&text, &[r"VERSION\s+([^\s\)]+)"]).unwrap_or_else(|| "0.1.0".to_string());
    Some(PackageMetadata {
        name,
        version,
        dependencies: capture_many_names(&text, r"find_package\(([^\s\)]+)"),
        dev_dependencies: HashMap::new(),
    })
}

pub fn read_meson_metadata(path: &Path, _state: &mut RunState) -> Option<PackageMetadata> {
    let text = fs::read_to_string(path).ok()?;
    let name = capture_first(&text, &[r#"project\(['\"]([^'\"]+)['\"]"#])
        .unwrap_or_else(|| fallback_package_name(path));
    let version = capture_first(&text, &[r#"version\s*:\s*['\"]([^'\"]+)['\"]"#])
        .unwrap_or_else(|| "0.1.0".to_string());
    Some(PackageMetadata {
        name,
        version,
        dependencies: HashMap::new(),
        dev_dependencies: HashMap::new(),
    })
}

pub fn read_conan_metadata(path: &Path, _state: &mut RunState) -> Option<PackageMetadata> {
    let text = fs::read_to_string(path).ok()?;
    let mut metadata = read_generic_metadata(path.parent().unwrap_or(path), path)?;
    metadata.dependencies = text
        .lines()
        .filter_map(|line| line.split_once('/'))
        .map(|(name, version)| (name.trim().to_string(), version.trim().to_string()))
        .collect();
    Some(metadata)
}

pub fn read_bundler_metadata(
    root: &Path,
    path: &Path,
    _state: &mut RunState,
) -> Option<PackageMetadata> {
    if path.extension().and_then(|value| value.to_str()) == Some("gemspec") {
        let text = fs::read_to_string(path).ok()?;
        let name = capture_first(&text, &[r#"\.name\s*=\s*['\"]([^'\"]+)['\"]"#])
            .unwrap_or_else(|| fallback_package_name(path));
        let version = capture_first(&text, &[r#"\.version\s*=\s*['\"]([^'\"]+)['\"]"#])
            .unwrap_or_else(|| "0.1.0".to_string());
        return Some(PackageMetadata {
            name,
            version,
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
        });
    }

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(|entry| entry.ok()) {
            let candidate = entry.path();
            if candidate.extension().and_then(|value| value.to_str()) == Some("gemspec") {
                if let Some(metadata) = read_bundler_metadata(root, &candidate, _state) {
                    return Some(metadata);
                }
            }
        }
    }

    read_generic_metadata(root, path)
}

pub fn read_zig_metadata(path: &Path, _state: &mut RunState) -> Option<PackageMetadata> {
    let text = fs::read_to_string(path).ok()?;
    let name = capture_first(&text, &[r#"\.name\s*=\s*\"([^\"]+)\""#])
        .unwrap_or_else(|| fallback_package_name(path));
    let version = capture_first(&text, &[r#"\.version\s*=\s*\"([^\"]+)\""#])
        .unwrap_or_else(|| "0.1.0".to_string());
    Some(PackageMetadata {
        name,
        version,
        dependencies: HashMap::new(),
        dev_dependencies: HashMap::new(),
    })
}

fn read_generic_metadata(root: &Path, _path: &Path) -> Option<PackageMetadata> {
    Some(PackageMetadata {
        name: root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("mds-package")
            .to_string(),
        version: "0.1.0".to_string(),
        dependencies: HashMap::new(),
        dev_dependencies: HashMap::new(),
    })
}

fn toml_table_path<'a>(
    value: &'a toml::Value,
    table_path: &[&str],
) -> Option<&'a toml::map::Map<String, toml::Value>> {
    let mut current = value;
    for segment in table_path {
        current = current.get(*segment)?;
    }
    current.as_table()
}

fn toml_string_value(value: Option<&toml::Value>) -> Option<String> {
    value?.as_str().map(ToOwned::to_owned)
}

fn toml_dependency_section(value: &toml::Value, section_name: &str) -> HashMap<String, String> {
    let Some(table) = toml_table_path(value, &[section_name]) else {
        return HashMap::new();
    };
    table
        .iter()
        .map(|(name, value)| (name.clone(), toml_dependency_version(value)))
        .collect()
}

fn toml_dependency_version(value: &toml::Value) -> String {
    if let Some(version) = value.as_str() {
        return version.to_string();
    }
    if let Some(version) = value.get("version").and_then(toml::Value::as_str) {
        return version.to_string();
    }
    value.to_string()
}

fn pyproject_optional_dependencies(value: &toml::Value, group: &str) -> HashMap<String, String> {
    toml_table_path(value, &["project", "optional-dependencies"])
        .and_then(|section| section.get(group))
        .map(toml_dependency_array)
        .unwrap_or_default()
}

fn toml_dependency_array(value: &toml::Value) -> HashMap<String, String> {
    let Some(array) = value.as_array() else {
        return HashMap::new();
    };
    array
        .iter()
        .filter_map(toml::Value::as_str)
        .map(split_dependency_spec)
        .collect()
}

fn split_dependency_spec(value: &str) -> (String, String) {
    for marker in [">=", "<=", "==", "~=", "!=", ">", "<"] {
        if let Some((name, version)) = value.split_once(marker) {
            return (
                name.trim().to_string(),
                format!("{marker}{}", version.trim()),
            );
        }
    }
    (value.trim().to_string(), String::new())
}

fn yaml_string(value: Option<&serde_yaml::Value>) -> Option<String> {
    value?.as_str().map(ToOwned::to_owned)
}

fn yaml_dependency_object(value: Option<&serde_yaml::Value>) -> HashMap<String, String> {
    let Some(mapping) = value.and_then(serde_yaml::Value::as_mapping) else {
        return HashMap::new();
    };
    mapping
        .iter()
        .filter_map(|(key, value)| {
            Some((key.as_str()?.to_string(), yaml_dependency_version(value)))
        })
        .collect()
}

fn yaml_dependency_version(value: &serde_yaml::Value) -> String {
    value.as_str().map(ToOwned::to_owned).unwrap_or_else(|| {
        serde_yaml::to_string(value)
            .unwrap_or_default()
            .trim()
            .to_string()
    })
}

fn capture_first(text: &str, patterns: &[&str]) -> Option<String> {
    for pattern in patterns {
        let regex = Regex::new(pattern).ok()?;
        if let Some(captures) = regex.captures(text) {
            if let Some(value) = captures.get(1) {
                return Some(value.as_str().trim().to_string());
            }
        }
    }
    None
}

fn capture_many_names(text: &str, pattern: &str) -> HashMap<String, String> {
    let Ok(regex) = Regex::new(pattern) else {
        return HashMap::new();
    };
    regex
        .captures_iter(text)
        .filter_map(|captures| {
            captures
                .get(1)
                .map(|value| (value.as_str().trim().to_string(), String::new()))
        })
        .collect()
}

fn capture_many_pairs(text: &str, pattern: &str, _state: &mut RunState) -> HashMap<String, String> {
    let Ok(regex) = Regex::new(pattern) else {
        return HashMap::new();
    };
    regex
        .captures_iter(text)
        .filter_map(|captures| {
            Some((
                captures.get(1)?.as_str().trim().to_string(),
                captures.get(2)?.as_str().trim().to_string(),
            ))
        })
        .collect()
}

fn fallback_package_name(path: &Path) -> String {
    path.parent()
        .and_then(|value| value.file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("mds-package")
        .to_string()
}

pub fn rust_expose_modules(package: &Package, state: &mut RunState) -> Vec<Vec<String>> {
    let markdown_root = source_markdown_root(package);
    let Ok(files) = collect_files(&markdown_root, false) else {
        return Vec::new();
    };
    let modules = Vec::new();
    for path in files
        .into_iter()
        .filter(|path| !is_excluded(&package.root, path, &package.config.excludes))
        .filter(|path| is_source_overview_doc(package, path))
    {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        validate_markdown_links(&path, &text, state);
        validate_markdown_links(&path, &text, state);
    }
    modules
}

pub fn validate_index_docs(package: &Package, state: &mut RunState) {
    let markdown_root = source_markdown_root(package);
    if !markdown_root.exists() {
        return;
    }
    let Ok(files) = collect_files(&markdown_root, false) else {
        return;
    };
    for path in files
        .into_iter()
        .filter(|path| !is_excluded(&package.root, path, &package.config.excludes))
        .filter(|path| is_source_overview_doc(package, path))
    {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                state.diagnostics.push(Diagnostic::error(
                    Some(path),
                    format!("failed to read source overview: {error}"),
                ));
                continue;
            }
        };
        validate_source_overview_required_sections(
            &path,
            &text,
            &package.config.label_overrides,
            state,
        );
    }
}

fn is_source_overview_doc(package: &Package, path: &Path) -> bool {
    is_package_overview_markdown_path(
        path,
        Some(&package.root),
        &package.config,
        PackageOverviewKind::Source,
    )
}

pub fn validate_expose_rows(rows: &[HashMap<String, String>], path: &Path, state: &mut RunState) {
    let mut seen = HashSet::new();
    for row in rows {
        let kind = row.get("kind").map(String::as_str).unwrap_or_default();
        let name = row.get("name").map(String::as_str).unwrap_or_default();
        if !matches!(kind, "type" | "value" | "function" | "class" | "module") {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                format!(
                    "Expose.Kind must be one of type, value, function, class, module: `{kind}`"
                ),
            ));
        }
        if name.is_empty() {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "Expose.Name is required",
            ));
        }
        if !seen.insert((kind.to_string(), name.to_string())) {
            state.diagnostics.push(Diagnostic::error(
                Some(path.to_path_buf()),
                "duplicate Expose row with the same Kind and Name",
            ));
        }
    }
}
