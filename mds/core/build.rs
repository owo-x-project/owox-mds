use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    generate_template_registry(&out_dir);
}

fn generate_template_registry(out_dir: &Path) {
    let templates_dir = Path::new("src/init/templates");
    let dest_path = out_dir.join("template_registry.rs");

    println!("cargo:rerun-if-changed=src/init/templates");

    let mut code = String::new();
    code.push_str("/// Auto-generated template registry from manifest.toml files.\n");
    code.push_str("pub(crate) struct TemplateEntry {\n");
    code.push_str("    pub target: &'static str,\n");
    code.push_str("    pub output_path: &'static str,\n");
    code.push_str("    pub category: &'static str,\n");
    code.push_str("    pub content: &'static str,\n");
    code.push_str("}\n\n");
    code.push_str("pub(crate) const TEMPLATES: &[TemplateEntry] = &[\n");

    if let Ok(entries) = fs::read_dir(templates_dir) {
        let mut dirs: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
        dirs.sort_by_key(|entry| entry.file_name());

        for entry in dirs {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join("manifest.toml");
            if !manifest_path.exists() {
                continue;
            }

            let target_name = path.file_name().unwrap().to_str().unwrap().to_string();

            println!("cargo:rerun-if-changed={}", manifest_path.display());

            let manifest_content = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
                panic!("failed to read {}: {error}", manifest_path.display())
            });

            let manifest: toml::Value = manifest_content.parse().unwrap_or_else(|error| {
                panic!("failed to parse {}: {error}", manifest_path.display())
            });

            let files = manifest
                .get("file")
                .and_then(|value| value.as_array())
                .unwrap_or_else(|| panic!("no [[file]] entries in {}", manifest_path.display()));

            for file_entry in files {
                let template = file_entry
                    .get("template")
                    .and_then(|value| value.as_str())
                    .unwrap();
                let output_path = file_entry
                    .get("output_path")
                    .and_then(|value| value.as_str())
                    .unwrap();
                let category = file_entry
                    .get("category")
                    .and_then(|value| value.as_str())
                    .unwrap();

                let template_path = path.join(template);
                println!("cargo:rerun-if-changed={}", template_path.display());

                let content = fs::read_to_string(&template_path).unwrap_or_else(|error| {
                    panic!("failed to read {}: {error}", template_path.display())
                });

                code.push_str("    TemplateEntry {\n");
                code.push_str(&format!("        target: {:?},\n", target_name));
                code.push_str(&format!("        output_path: {:?},\n", output_path));
                code.push_str(&format!("        category: {:?},\n", category));
                code.push_str(&format!("        content: {:?},\n", content));
                code.push_str("    },\n");
            }
        }
    }

    code.push_str("];\n\n");
    code.push_str(
        "pub(crate) fn templates_for_target(target: &str) -> Vec<&'static TemplateEntry> {\n",
    );
    code.push_str("    TEMPLATES.iter().filter(|entry| entry.target == target).collect()\n");
    code.push_str("}\n");

    fs::write(&dest_path, code).unwrap();
}
