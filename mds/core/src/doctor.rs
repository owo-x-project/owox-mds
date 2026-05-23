use crate::adapter::tool_available;
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::model::DoctorConfig;
use crate::model::DoctorFormat;
use crate::model::DoctorVersionFloor;
use crate::model::Package;
use std::process::Command as ProcessCommand;
pub(crate) fn run_doctor(packages: &[Package], format: DoctorFormat, state: &mut RunState) {
    let mut checks = Vec::new();
    let current_version = env!("CARGO_PKG_VERSION");
    checks.push(DoctorCheck::ok("mds", current_version.to_string()));
    checks.push(DoctorCheck::ok("packages", packages.len().to_string()));
    for package in packages {
        // Check mds_version compatibility
        if let Some(ref expected_version) = package.config.mds_version {
            if expected_version != current_version {
                checks.push(DoctorCheck::warning(
                    "mds_version",
                    format!(
                        "project expects mds {expected_version}, but running {current_version}. Run `mds update --version {expected_version}` to match."
                    ),
                ));
            }
        }
        checks.push(DoctorCheck::ok(
            "package",
            package.root.display().to_string(),
        ));
        for lang in package.config.quality.keys() {
            if !package.config.adapters.get(lang).copied().unwrap_or(true) {
                continue;
            }
            let Some(config) = package.config.quality.get(lang) else {
                continue;
            };
            for command in &config.required {
                check_toolchain(
                    command,
                    true,
                    resolve_version_floor(&package.config.doctor, command),
                    package,
                    &mut checks,
                    state,
                );
            }
            for command in &config.optional {
                check_toolchain(
                    command,
                    false,
                    resolve_version_floor(&package.config.doctor, command),
                    package,
                    &mut checks,
                    state,
                );
            }
        }
    }
    match format {
        DoctorFormat::Text => render_text(&checks, state),
        DoctorFormat::Json => render_json(&checks, state),
    }
}

fn check_toolchain(
    command: &str,
    required: bool,
    version_floor: Option<&DoctorVersionFloor>,
    package: &Package,
    checks: &mut Vec<DoctorCheck>,
    state: &mut RunState,
) {
    if !tool_available(command) {
        if required {
            state.environment_missing = true;
            checks.push(DoctorCheck::error(command, "missing".to_string()));
            state.diagnostics.push(Diagnostic::error(
                Some(package.root.clone()),
                format!(
                    "DOCTOR001_TOOLCHAIN_MISSING: required toolchain `{command}` is not available"
                ),
            ));
        } else {
            checks.push(DoctorCheck::warning(command, "missing".to_string()));
            state.diagnostics.push(Diagnostic::warning(
                Some(package.root.clone()),
                format!("optional toolchain `{command}` is not available"),
            ));
        }
        return;
    }

    let Some(version_floor) = version_floor else {
        checks.push(DoctorCheck::ok(command, "available".to_string()));
        return;
    };

    match command_version(command) {
        Some(version) if version_at_least(&version, version_floor) => {
            checks.push(DoctorCheck::ok(command, render_version(&version)));
        }
        Some(version) => {
            let detail = format!(
                "{} is below required {}",
                render_version(&version),
                render_floor(version_floor)
            );
            let message = format!(
                "DOCTOR002_VERSION_TOO_OLD: `{command}` version {} is below required {}",
                render_version(&version),
                render_floor(version_floor)
            );
            if required {
                state.environment_missing = true;
                checks.push(DoctorCheck::error(command, detail));
                state
                    .diagnostics
                    .push(Diagnostic::error(Some(package.root.clone()), message));
            } else {
                checks.push(DoctorCheck::warning(command, detail));
                state
                    .diagnostics
                    .push(Diagnostic::warning(Some(package.root.clone()), message));
            }
        }
        None => checks.push(DoctorCheck::warning(
            command,
            "version unavailable".to_string(),
        )),
    }
}

fn resolve_version_floor<'a>(
    doctor: &'a DoctorConfig,
    command: &str,
) -> Option<&'a DoctorVersionFloor> {
    doctor.version_floor.get(command).or_else(|| {
        let name = command.rsplit(['/', '\\']).next().unwrap_or(command);
        if name == command {
            None
        } else {
            doctor.version_floor.get(name)
        }
    })
}

fn command_version(command: &str) -> Option<(u32, u32, u32)> {
    let output = ProcessCommand::new(command)
        .arg("--version")
        .output()
        .ok()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    parse_version(&text)
}

fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let start = text.find(|ch: char| ch.is_ascii_digit())?;
    let version = text[start..]
        .split(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .next()?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn version_at_least(version: &(u32, u32, u32), minimum: &DoctorVersionFloor) -> bool {
    *version >= (minimum.major, minimum.minor, minimum.patch)
}

fn render_version(version: &(u32, u32, u32)) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

fn render_floor(version: &DoctorVersionFloor) -> String {
    format!("{}.{}.{}", version.major, version.minor, version.patch)
}

#[derive(Debug)]
struct DoctorCheck {
    name: String,
    status: &'static str,
    detail: String,
}

impl DoctorCheck {
    fn ok(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            status: "ok",
            detail,
        }
    }

    fn warning(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            status: "warning",
            detail,
        }
    }

    fn error(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            status: "error",
            detail,
        }
    }
}

fn render_text(checks: &[DoctorCheck], state: &mut RunState) {
    state.stdout.push_str("Doctor summary:\n");
    for check in checks {
        state.stdout.push_str(&format!(
            "- {}: {} ({})\n",
            check.name, check.status, check.detail
        ));
    }
}

fn render_json(checks: &[DoctorCheck], state: &mut RunState) {
    state.stdout.push_str("{\"checks\":[");
    for (index, check) in checks.iter().enumerate() {
        if index > 0 {
            state.stdout.push(',');
        }
        state.stdout.push_str(&format!(
            "{{\"name\":\"{}\",\"status\":\"{}\",\"detail\":\"{}\"}}",
            escape_json(&check.name),
            check.status,
            escape_json(&check.detail)
        ));
    }
    state.stdout.push_str("]}\n");
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
