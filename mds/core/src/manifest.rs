use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::hash::sha256;
use crate::model::GeneratedFile;
use crate::model::GeneratedKind;
use crate::model::ImplDoc;
use crate::model::Package;
use std::fs;
use std::path::Component;
use std::path::Path;
pub(crate) fn validate_manifest(package: &Package, state: &mut RunState) {
    let path = package.root.join(".mds/manifest.toml");
    if !path.exists() {
        return;
    }
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            state.diagnostics.push(Diagnostic::error(
                Some(path),
                format!("failed to read manifest: {error}"),
            ));
            return;
        }
    };
    if !text.contains("[[sources]]") {
        state.diagnostics.push(Diagnostic::error(
            Some(path),
            "manifest schema requires [[sources]] entries",
        ));
    }
}

pub(crate) fn plan_manifest(
    package: &Package,
    docs: &[ImplDoc],
    generated: &[GeneratedFile],
) -> GeneratedFile {
    let mut content = String::new();
    for doc in docs {
        let source_hash = sha256(&doc.normalized_input);
        content.push_str("[[sources]]\n");
        content.push_str(&format!(
            "path = \"{}\"\n",
            toml_path(&doc.package_relative_path)
        ));
        content.push_str(&format!("adapter = \"{}\"\n", doc.lang.key()));
        content.push_str(&format!("hash = \"{source_hash}\"\n"));
        for file in generated.iter().filter(|file| {
            file.source_path.as_ref() == Some(&doc.package_relative_path)
                && matches!(file.kind, GeneratedKind::Output(_))
        }) {
            let kind = match file.kind {
                GeneratedKind::Output(kind) => kind.manifest_kind(),
                _ => continue,
            };
            let path = file.path.strip_prefix(&package.root).unwrap_or(&file.path);
            content.push_str("[[sources.outputs]]\n");
            content.push_str(&format!("kind = \"{kind}\"\n"));
            content.push_str(&format!("path = \"{}\"\n", toml_path(path)));
            content.push_str(&format!("hash = \"{}\"\n", sha256(&file.content)));
        }
        content.push('\n');
    }
    for asset in generated
        .iter()
        .filter(|file| matches!(file.kind, GeneratedKind::Asset))
    {
        let Some(source_path) = asset.source_path.as_ref() else {
            continue;
        };
        content.push_str("[[assets]]\n");
        content.push_str(&format!("path = \"{}\"\n", toml_path(source_path)));
        content.push_str(&format!("hash = \"{}\"\n", sha256(&asset.content)));
        let path = asset
            .path
            .strip_prefix(&package.root)
            .unwrap_or(&asset.path);
        content.push_str("[[assets.outputs]]\n");
        content.push_str(&format!("path = \"{}\"\n", toml_path(path)));
        content.push_str(&format!("hash = \"{}\"\n\n", sha256(&asset.content)));
    }
    GeneratedFile {
        path: package.root.join(".mds/manifest.toml"),
        content,
        kind: GeneratedKind::Manifest,
        source_path: None,
    }
}

fn toml_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
