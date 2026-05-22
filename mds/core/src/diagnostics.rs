use std::path::PathBuf;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

impl Diagnostic {
    pub fn warning(path: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            path,
            line: None,
            column: None,
            message: message.into(),
        }
    }

    pub fn error(path: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            path,
            line: None,
            column: None,
            message: message.into(),
        }
    }

    pub fn at_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn at_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    pub fn render(&self) -> String {
        let level = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        match (&self.path, self.line, self.column) {
            (Some(path), Some(line), Some(column)) => {
                format!(
                    "{level}: {}:{line}:{column}: {}\n",
                    path.display(),
                    self.message
                )
            }
            (Some(path), Some(line), None) => {
                format!("{level}: {}:{line}: {}\n", path.display(), self.message)
            }
            (Some(path), None, _) => format!("{level}: {}: {}\n", path.display(), self.message),
            (None, _, _) => format!("{level}: {}\n", self.message),
        }
    }
}

#[derive(Debug, Default)]
pub struct RunState {
    pub stdout: String,
    pub diagnostics: Vec<Diagnostic>,
    pub generated: Vec<PathBuf>,
    pub environment_missing: bool,
}

impl RunState {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}
