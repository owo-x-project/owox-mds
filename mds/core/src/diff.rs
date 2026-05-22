use crate::diagnostics::RunState;
use crate::model::GeneratedFile;
use std::fs;
use std::path::Path;
pub(crate) fn render_dry_run(generated: &[GeneratedFile], state: &mut RunState) {
    state.stdout.push_str("Build plan:\n");
    for file in generated {
        state
            .stdout
            .push_str(&format!("- {}\n", file.path.display()));
    }
    state.stdout.push('\n');
    for file in generated {
        let old = fs::read_to_string(&file.path).unwrap_or_default();
        state
            .stdout
            .push_str(&unified_diff(&file.path, &old, &file.content));
    }
}

pub(crate) fn write_generated(
    generated: &[GeneratedFile],
    state: &mut RunState,
) -> Result<(), String> {
    for file in generated {
        if let Some(parent) = file.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create directory {}: {error}", parent.display())
            })?;
        }
        fs::write(&file.path, &file.content)
            .map_err(|error| format!("failed to write {}: {error}", file.path.display()))?;
        state.generated.push(file.path.clone());
    }
    state
        .stdout
        .push_str(&format!("build ok: {} files written\n", generated.len()));
    Ok(())
}

pub(crate) fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let old_label = if old.is_empty() {
        "/dev/null".to_string()
    } else {
        format!("a/{}", path.display())
    };
    let new_label = format!("b/{}", path.display());
    let old_lines = old.lines().count();
    let new_lines = new.lines().count();
    let mut out =
        format!("--- {old_label}\n+++ {new_label}\n@@ -1,{old_lines} +1,{new_lines} @@\n");
    for line in old.lines() {
        out.push('-');
        out.push_str(line);
        out.push('\n');
    }
    for line in new.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    out
}
