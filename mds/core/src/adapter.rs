use std::path::Path;
use std::process::Command as ProcessCommand;
use std::process::Stdio;

pub(crate) struct ToolInvocation<'a> {
    pub command: &'a str,
    pub file_arg: Option<&'a Path>,
    pub cwd: &'a Path,
    pub stdin: Option<&'a str>,
    pub inline_arg: Option<&'a str>,
}

pub(crate) struct ToolRunOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) enum ToolRunStatus {
    EmptyCommand,
    MissingTool(String),
    Completed(ToolRunOutput),
}

pub(crate) fn run_toolchain_command(
    invocation: ToolInvocation<'_>,
) -> Result<ToolRunStatus, String> {
    let Some((program, args)) = split_command(invocation.command) else {
        return Ok(ToolRunStatus::EmptyCommand);
    };
    if !tool_available(program) {
        return Ok(ToolRunStatus::MissingTool(program.to_string()));
    }
    let mut process = ProcessCommand::new(program);
    process
        .args(args)
        .current_dir(invocation.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    match invocation.stdin {
        Some(_) => {
            process.stdin(Stdio::piped());
        }
        None => {
            process.stdin(Stdio::null());
        }
    }

    if let Some(file) = invocation.file_arg {
        process.arg(file);
    }
    if let Some(inline_arg) = invocation.inline_arg {
        process.arg(inline_arg);
    }

    let mut child = process
        .spawn()
        .map_err(|error| format!("failed to run toolchain: {error}"))?;
    if let Some(stdin) = invocation.stdin {
        if let Some(mut handle) = child.stdin.take() {
            use std::io::Write;
            if let Err(error) = handle.write_all(stdin.as_bytes()) {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(format!("failed to write toolchain stdin: {error}"));
                }
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for toolchain: {error}"))?;
    Ok(ToolRunStatus::Completed(ToolRunOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }))
}

pub(crate) fn tool_output_detail(output: &ToolRunOutput) -> String {
    let raw = if output.stderr.trim().is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    raw.trim().to_string()
}

pub(crate) fn path_variants(path: &Path, cwd: &Path) -> Vec<String> {
    let mut variants = vec![path.display().to_string()];
    if let Ok(relative) = path.strip_prefix(cwd) {
        variants.push(relative.display().to_string());
    }
    variants.sort();
    variants.dedup();
    variants
}

pub(crate) fn split_command(command: &str) -> Option<(&str, Vec<&str>)> {
    let mut parts = command.split_whitespace();
    let program = parts.next()?;
    Some((program, parts.collect()))
}

pub(crate) fn tool_available(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).exists();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(program).exists())
}
