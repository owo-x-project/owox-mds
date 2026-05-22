use mds_core::Diagnostic;
use mds_core::Severity;
use tower_lsp::lsp_types;
pub fn to_lsp_diagnostic(diag: &Diagnostic) -> lsp_types::Diagnostic {
    let severity = match diag.severity {
        Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
        Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
    };

    let line = diag.line.unwrap_or(1).saturating_sub(1) as u32;
    let character = diag.column.unwrap_or(1).saturating_sub(1) as u32;

    lsp_types::Diagnostic {
        range: lsp_types::Range {
            start: lsp_types::Position { line, character },
            end: lsp_types::Position { line, character },
        },
        severity: Some(severity),
        source: Some("mds".to_string()),
        message: diag.message.clone(),
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn line_range(line_1based: usize) -> lsp_types::Range {
    let line = line_1based.saturating_sub(1) as u32;
    lsp_types::Range {
        start: lsp_types::Position { line, character: 0 },
        end: lsp_types::Position { line, character: 0 },
    }
}

pub fn line_at(text: &str, line: u32) -> Option<&str> {
    text.lines().nth(line as usize)
}

pub fn word_at_position(text: &str, position: lsp_types::Position) -> Option<String> {
    let line_text = line_at(text, position.line)?;
    let col = position.character as usize;
    if col > line_text.len() {
        return None;
    }

    let bytes = line_text.as_bytes();
    let mut start = col;
    let mut end = col;

    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }
    while end < bytes.len() && is_word_char(bytes[end]) {
        end += 1;
    }

    if start == end {
        None
    } else {
        Some(line_text[start..end].to_string())
    }
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'/' || b == b'.'
}

pub fn table_cell_at_position(text: &str, position: lsp_types::Position) -> Option<String> {
    let line_text = line_at(text, position.line)?;
    if !line_text.trim_start().starts_with('|') {
        return None;
    }

    let col = position.character as usize;
    if col >= line_text.len() {
        return None;
    }

    let bytes = line_text.as_bytes();

    // Find cell boundaries
    let mut start = col;
    while start > 0 && bytes[start - 1] != b'|' {
        start -= 1;
    }
    let mut end = col;
    while end < bytes.len() && bytes[end] != b'|' {
        end += 1;
    }

    Some(line_text[start..end].trim().to_string())
}
