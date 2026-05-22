use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use std::collections::HashMap;
use std::path::Path;
pub fn parse_table_with_labels(
    section: &str,
    required: &[&str],
    path: &Path,
    label_overrides: &HashMap<String, String>,
    state: &mut RunState,
) -> Option<Vec<HashMap<String, String>>> {
    let lines = section.lines().collect::<Vec<_>>();
    let mut saw_table = false;
    for idx in 0..lines.len().saturating_sub(1) {
        if !lines[idx].trim_start().starts_with('|') || !lines[idx + 1].contains("---") {
            continue;
        }
        saw_table = true;
        let headers = split_table_row(lines[idx]);
        let canonical = headers
            .iter()
            .map(|header| canonical_header(header, required, label_overrides))
            .collect::<Vec<_>>();
        let required_canonical = required
            .iter()
            .map(|header| header.to_ascii_lowercase())
            .collect::<Vec<_>>();
        if !required_canonical
            .iter()
            .all(|required| canonical.contains(required))
        {
            continue;
        }
        for header in &canonical {
            if !required_canonical.contains(header) && header != "reference" {
                state.diagnostics.push(Diagnostic::warning(
                    Some(path.to_path_buf()),
                    format!("ignoring unsupported table column `{header}`"),
                ));
            }
        }

        let mut rows = Vec::new();
        for row_line in lines.iter().skip(idx + 2) {
            if !row_line.trim_start().starts_with('|') {
                break;
            }
            let cells = split_table_row(row_line);
            let mut row = HashMap::new();
            for (index, header) in canonical.iter().enumerate() {
                if required_canonical.contains(header) {
                    row.insert(
                        header.clone(),
                        cells
                            .get(index)
                            .map(|cell| cell.trim())
                            .unwrap_or_default()
                            .to_string(),
                    );
                }
            }
            rows.push(row);
        }
        return Some(rows);
    }
    if saw_table {
        state.diagnostics.push(Diagnostic::error(
            Some(path.to_path_buf()),
            format!("table is missing required columns: {}", required.join(", ")),
        ));
    }
    None
}

fn canonical_header(
    header: &str,
    required: &[&str],
    label_overrides: &HashMap<String, String>,
) -> String {
    let header = header.trim().to_ascii_lowercase();
    for canonical in required {
        let canonical_lower = canonical.to_ascii_lowercase();
        if header == canonical_lower {
            return canonical_lower;
        }
        if label_overrides
            .get(&canonical_lower)
            .is_some_and(|override_label| override_label.trim().eq_ignore_ascii_case(&header))
        {
            return canonical_lower;
        }
    }
    header
}

pub fn split_table_row(line: &str) -> Vec<String> {
    let line = line.trim().trim_matches('|');
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    let mut in_code = false;
    for ch in line.chars() {
        if escaped {
            cell.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '`' => {
                in_code = !in_code;
                cell.push(ch);
            }
            '|' if !in_code => {
                cells.push(cell.trim().to_string());
                cell.clear();
            }
            _ => cell.push(ch),
        }
    }
    if escaped {
        cell.push('\\');
    }
    cells.push(cell.trim().to_string());
    cells
}
