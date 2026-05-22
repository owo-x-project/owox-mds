use crate::config::merge_config_file;
use crate::descriptor;
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::fs_utils::is_mds_managed_file;
use crate::model::{Config, NewDocKind, NewOptions};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

struct ValidatedNewRequest {
    kind: NewDocKind,
    path: String,
    descriptor: Option<descriptor::Descriptor>,
}

pub fn validate_new_options(options: &NewOptions) -> Result<(), String> {
    validate_new_request(options).map(|_| ())
}

fn validate_new_request(options: &NewOptions) -> Result<ValidatedNewRequest, String> {
    let kind = options.kind.ok_or_else(|| {
        "new requires doc kind; expected impl, test, overview, or root-module".to_string()
    })?;
    let path = normalize_new_path(&options.path)?;
    let descriptor = descriptor_for_new_doc(kind, &path)?;

    Ok(ValidatedNewRequest {
        kind,
        path,
        descriptor,
    })
}

pub(crate) fn run_new(
    cwd: &Path,
    package: Option<&Path>,
    options: &NewOptions,
    _verbose: bool,
    state: &mut RunState,
) -> Result<(), String> {
    let root = package.map_or_else(|| cwd.to_path_buf(), |path| cwd.join(path));
    let validated = validate_new_request(options)?;
    let target = markdown_target_path(&root, &validated.path, validated.kind);
    if target.exists() && !is_mds_managed_file(&target) && !options.force {
        state.diagnostics.push(Diagnostic::error(
            Some(target.clone()),
            "file already exists and is not mds-managed; use --force to overwrite",
        ));
        return Ok(());
    }

    let labels = read_label_overrides(cwd, &root, state);

    let feature_name = extract_feature_name(&validated.path);
    let mut content = match validated.kind {
        NewDocKind::Impl => generate_impl_template(
            &validated.path,
            &feature_name,
            validated
                .descriptor
                .as_ref()
                .expect("impl requires descriptor"),
            &labels,
        ),
        NewDocKind::Test => generate_test_template(
            &validated.path,
            &feature_name,
            validated
                .descriptor
                .as_ref()
                .expect("test requires descriptor"),
            &labels,
        ),
        NewDocKind::Overview => generate_source_overview_template(&labels),
        NewDocKind::RootModule => generate_module_root_template(&feature_name, &labels),
    };
    if matches!(validated.kind, NewDocKind::Overview) {
        content = crate::package_sync::hydrate_generated_source_overview(&root, &target, &content);
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    fs::write(&target, &content)
        .map_err(|e| format!("failed to write {}: {e}", target.display()))?;

    state.generated.push(target.clone());
    state
        .stdout
        .push_str(&format!("created {}\n", target.display()));
    Ok(())
}

fn normalize_new_path(path: &str) -> Result<String, String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err("new path must be relative to the canonical authoring root".to_string());
    }

    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::ParentDir => {
                return Err("new path must stay within the package root".to_string())
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("new path must be relative to the canonical authoring root".to_string())
            }
        }
    }

    if parts.is_empty() {
        return Err("new requires a relative Markdown path".to_string());
    }

    let normalized = parts.join("/");
    if normalized.starts_with(".mds/") {
        return Err(
            "new path must be relative to `.mds/source` or `.mds/test`, without the root prefix"
                .to_string(),
        );
    }
    if !normalized.ends_with(".md") {
        return Err("new path must end in `.md`".to_string());
    }
    Ok(normalized)
}

fn descriptor_for_new_doc(
    kind: NewDocKind,
    path: &str,
) -> Result<Option<descriptor::Descriptor>, String> {
    match kind {
        NewDocKind::Overview => {
            if !descriptor::is_overview_markdown_name(path) {
                return Err(format!(
                    "overview kind requires path `{}`",
                    descriptor::OVERVIEW_MARKDOWN_NAME,
                ));
            }
            Ok(None)
        }
        NewDocKind::Impl | NewDocKind::Test | NewDocKind::RootModule => {
            let descriptor = descriptor::descriptor_for_markdown_name(path).ok_or_else(|| {
                format!(
                    "cannot detect language from `{path}`; expected a name ending in .{{lang}}.md (e.g. greet.ts.md, utils.go.md)"
                )
            })?;
            if matches!(kind, NewDocKind::RootModule) {
                let file_name = path.rsplit('/').next().unwrap_or(path);
                if !descriptor.is_root_module_markdown_name(file_name) {
                    return Err(format!(
                        "root-module kind requires a recognized root module Markdown name; got `{path}`"
                    ));
                }
            }
            Ok(Some(descriptor))
        }
    }
}

fn markdown_target_path(root: &Path, path: &str, kind: NewDocKind) -> PathBuf {
    match kind {
        NewDocKind::Impl | NewDocKind::Overview | NewDocKind::RootModule => {
            root.join(".mds/source").join(path)
        }
        NewDocKind::Test => root.join(".mds/test").join(path),
    }
}

fn read_label_overrides(cwd: &Path, root: &Path, state: &mut RunState) -> HashMap<String, String> {
    let mut config = Config::default();
    let workspace_config_path = cwd.join("mds.config.toml");
    if workspace_config_path.exists() {
        let _ = merge_config_file(&mut config, &workspace_config_path, state);
    }

    let package_config_path = root.join("mds.config.toml");
    if package_config_path != workspace_config_path && package_config_path.exists() {
        let _ = merge_config_file(&mut config, &package_config_path, state);
    }

    config.label_overrides
}

fn label<'a>(labels: &'a HashMap<String, String>, canonical: &str, default: &'a str) -> &'a str {
    labels.get(canonical).map(|s| s.as_str()).unwrap_or(default)
}

fn extract_feature_name(name: &str) -> String {
    let mut base = name.trim_end_matches(".md");
    if let Some(suffix) = descriptor::matched_markdown_suffix(name) {
        let suffix = format!(".{suffix}");
        if base.ends_with(&suffix) {
            base = &base[..base.len() - suffix.len()];
        }
    }
    let base = if base == "index"
        || base.ends_with("/index")
        || base == "overview"
        || base.ends_with("/overview")
    {
        if let Some(pos) = base.rfind('/') {
            let dir = &base[..pos];
            if let Some(pos2) = dir.rfind('/') {
                &dir[pos2 + 1..]
            } else {
                dir
            }
        } else {
            "Index"
        }
    } else if let Some(pos) = base.rfind('/') {
        &base[pos + 1..]
    } else {
        base
    };
    to_title_case(base)
}

fn canonical_module_id(path: &str) -> String {
    descriptor::markdown_module_id(Path::new(path))
}

fn to_title_case(s: &str) -> String {
    s.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_source_overview_template(labels: &HashMap<String, String>) -> String {
    let l_purpose = label(labels, "purpose", "Purpose");
    format!(
        "<!-- Generated by mds new. -->\n\
         # Source Overview\n\
         \n\
         ## {l_purpose}\n\
         \n\
         Describe the package-level source hierarchy and authoring intent.\n\
         \n\
         ## Architecture\n\
         \n\
         Markdown files in this directory describe package and source hierarchy rules.\n\
         \n\
         ### Package Summary\n\
         \n\
         | Name | Version |\n\
         | --- | --- |\n\
         | package-name | 0.1.0 |\n\
         \n\
         ### Dependencies\n\
         \n\
         | Name | Version | Summary |\n\
         | --- | --- | --- |\n\
         \n\
         ### Dev Dependencies\n\
         \n\
         | Name | Version | Summary |\n\
         | --- | --- | --- |\n\
         \n\
         ## Rules\n\
         \n\
         - Keep package or directory root API notes in the root module doc.\n\
         - Add source code blocks there only when the root module owns runtime behavior.\n\
         - Keep one source md per feature.\n"
    )
}

fn generate_module_root_template(feature_name: &str, labels: &HashMap<String, String>) -> String {
    let l_purpose = label(labels, "purpose", "Purpose");
    let l_contract = label(labels, "contract", "Contract");
    let l_source = label(labels, "source", "Source");

    format!(
        "<!-- Generated by mds new. -->\n\
         # {feature_name}\n\
         \n\
         ## {l_purpose}\n\
         \n\
         Describe the package or directory root module surface.\n\
         \n\
         ## {l_contract}\n\
         \n\
         - Describe stable exports, re-exports, and entrypoint constraints.\n\
         \n\
         ## API\n\
         \n\
         Describe public exports, re-exports, and package entrypoint behavior in prose.\n\
         \n\
         Add a `{l_source}` section only when this root module owns runtime behavior, and keep import or export declarations in that code block instead of duplicate metadata tables.\n"
    )
}

fn generate_impl_template(
    name: &str,
    feature_name: &str,
    descriptor: &descriptor::Descriptor,
    labels: &HashMap<String, String>,
) -> String {
    let matched_suffix = descriptor::matched_markdown_suffix(name);
    let source_block = descriptor.render_source_block(matched_suffix.as_deref());

    let l_purpose = label(labels, "purpose", "Purpose");
    let l_contract = label(labels, "contract", "Contract");
    let l_source = label(labels, "source", "Source");
    let l_cases = label(labels, "cases", "Cases");

    format!(
        "<!-- Generated by mds new. -->\n\
         # {feature_name}\n\
         \n\
         ## {l_purpose}\n\
         \n\
         Describe the purpose of this feature.\n\
         \n\
         ## {l_contract}\n\
         \n\
         - Describe the stable behavior and constraints.\n\
         \n\
         ## API\n\
         \n\
         Describe the public surface in prose. When this feature depends on other modules, write normal import or use statements directly inside the generated source block.\n\
         \n\
         ## {l_source}\n\
         \n\
         {source_block}\n\
         \n\
         ## {l_cases}\n\
         \n\
         - Describe the expected behavior.\n"
    )
}

fn generate_test_template(
    path: &str,
    feature_name: &str,
    descriptor: &descriptor::Descriptor,
    labels: &HashMap<String, String>,
) -> String {
    let l_purpose = label(labels, "purpose", "Purpose");
    let l_covers = label(labels, "covers", "Covers");
    let l_cases = label(labels, "cases", "Cases");
    let l_test = label(labels, "test", "Test");
    let module_id = canonical_module_id(path);
    let matched_suffix = descriptor::matched_markdown_suffix(path);
    let test_block = descriptor.render_source_block(matched_suffix.as_deref());

    format!(
        "<!-- Generated by mds new. -->\n\
         # {feature_name} test\n\
         \n\
         ## {l_purpose}\n\
         \n\
         Describe the behavior being verified.\n\
         \n\
         ## {l_covers}\n\
         \n\
         - {module_id}\n\
         \n\
         ## {l_cases}\n\
         \n\
         - Describe the expected behavior.\n\
         \n\
         ## {l_test}\n\
         \n\
         {test_block}\n"
    )
}
