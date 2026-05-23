use mds_cli::args::parse_args;
use mds_cli::args::print_usage;
use mds_cli::args::resolve_new_options;
use mds_cli::wizard::{run_interactive_init, run_interactive_init_descriptor, WizardOutcome};
use mds_core::descriptor::with_workspace_descriptor_root;
use mds_core::execute;
use mds_core::validate_new_options;
use mds_core::CliRequest;
use mds_core::Command;
use std::io::IsTerminal;
use std::process::{Command as ProcessCommand, Stdio};
const VERSION: &str = match option_env!("MDS_RELEASE_VERSION") {
    Some(version) if !version.is_empty() => version,
    _ => env!("CARGO_PKG_VERSION"),
};

fn main() -> std::process::ExitCode {
    run()
}

fn run() -> std::process::ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("internal error: failed to read current directory: {error}");
            return exit_code(3);
        }
    };

    let args: Vec<String> = std::env::args().skip(1).collect();

    // Handle --help and --version before any other processing
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return exit_code(0);
    }
    if args.len() == 1 && args.iter().any(|a| a == "--version" || a == "-V") {
        println!("mds {VERSION}");
        return exit_code(0);
    }

    // Interactive wizard: `mds init` with no additional flags
    if args.len() == 1 && args[0] == "init"
        || args.len() == 3 && args[0] == "init" && args[1] == "--package"
        || args.len() == 2 && args[0] == "init" && args[1] == "descriptor"
        || args.len() == 4 && args[0] == "init" && args[1] == "descriptor" && args[2] == "--package"
    {
        let descriptor_mode = args.get(1).map(String::as_str) == Some("descriptor");
        let package = if args.len() == 3 {
            Some(std::path::PathBuf::from(&args[2]))
        } else if args.len() == 4 {
            Some(std::path::PathBuf::from(&args[3]))
        } else {
            None
        };

        let wizard_result = if descriptor_mode {
            run_interactive_init_descriptor(&cwd, package.as_deref())
        } else {
            run_interactive_init(&cwd, package.as_deref())
        };
        match wizard_result {
            Ok(options) => {
                let command = match options {
                    WizardOutcome::Init(options) => Command::Init { options },
                    WizardOutcome::InitDescriptor(options) => Command::InitDescriptor { options },
                };
                let request = CliRequest {
                    cwd,
                    package,
                    verbose: false,
                    command,
                };
                let result = execute(request);
                print_cli_output(&result.stdout, &result.stderr);
                return exit_code(result.exit_code);
            }
            Err(message) => {
                eprintln!("{message}");
                return exit_code(2);
            }
        }
    }

    let mut request = match parse_args(cwd) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!();
            if message.contains("missing command") {
                eprintln!("hint: Run `mds init` to set up a new project interactively.");
                eprintln!("      Run `mds lint --package <path>` to validate an existing project.");
            } else if message.contains("unknown command") {
                eprintln!("hint: Available commands: init, init descriptor, new, descriptor, build, typecheck, lint, test, doctor, package sync, update");
            } else if message.contains("unknown option") {
                eprintln!("hint: Use --verbose for detailed output. Run `mds` without arguments for full usage.");
            }
            print_usage();
            return exit_code(2);
        }
    };

    let new_descriptor_root = request
        .package
        .as_deref()
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                request.cwd.join(path)
            }
        })
        .unwrap_or_else(|| request.cwd.clone());

    if let Command::New { options } = &mut request.command {
        let stdin_is_terminal = std::io::stdin().is_terminal();
        let mut stdin = std::io::stdin().lock();
        let mut stderr = std::io::stderr();
        match with_workspace_descriptor_root(Some(&new_descriptor_root), || -> Result<_, String> {
            let resolved =
                resolve_new_options(options.clone(), stdin_is_terminal, &mut stdin, &mut stderr)?;
            validate_new_options(&resolved)?;
            Ok(resolved)
        }) {
            Ok(resolved) => *options = resolved,
            Err(message) => {
                eprintln!("error: {message}");
                eprintln!();
                print_usage();
                return exit_code(2);
            }
        }
    }

    // Handle update command directly (it replaces the current binary)
    if let Command::Update { ref version } = request.command {
        return run_self_update(version.as_deref());
    }

    let result = execute(request);
    print_cli_output(&result.stdout, &result.stderr);
    exit_code(result.exit_code)
}

fn run_self_update(version: Option<&str>) -> std::process::ExitCode {
    let repo = "owo-x-project/owox-mds";
    let target_version = match version {
        Some(v) => v.to_string(),
        None => {
            eprintln!("Checking for latest version...");
            match fetch_latest_version(repo) {
                Some(v) => v,
                None => {
                    eprintln!("error: failed to fetch latest version from GitHub");
                    print_manual_update_hint(repo, None);
                    return exit_code(1);
                }
            }
        }
    };

    if target_version == VERSION {
        println!("mds is already at version {VERSION}");
        return exit_code(0);
    }

    println!("Updating mds from {VERSION} to {target_version}...");

    let install_script = format!("https://raw.githubusercontent.com/{repo}/latest/install.sh");

    match run_install_script(&install_script, &target_version) {
        Ok(()) => {
            println!("Successfully updated to mds {target_version}");
            exit_code(0)
        }
        Err(message) => {
            eprintln!("{message}");
            print_manual_update_hint(repo, Some(&target_version));
            exit_code(1)
        }
    }
}

fn run_install_script(install_script: &str, target_version: &str) -> Result<(), String> {
    let mut curl = ProcessCommand::new("curl")
        .args(["-fsSL", install_script])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("error: failed to start curl: {error}"))?;

    let curl_stdout = curl
        .stdout
        .take()
        .ok_or_else(|| "error: failed to capture install script output".to_string())?;

    let sh_status = match ProcessCommand::new("sh")
        .args(["-s", "--", "--version", target_version])
        .stdin(Stdio::from(curl_stdout))
        .status()
    {
        Ok(status) => status,
        Err(error) => {
            let _ = curl.wait();
            return Err(format!("error: failed to run update: {error}"));
        }
    };

    let curl_status = curl
        .wait()
        .map_err(|error| format!("error: failed to wait for curl: {error}"))?;

    if !curl_status.success() {
        return Err(format!(
            "Update failed with exit code: {}",
            curl_status.code().unwrap_or(1)
        ));
    }

    if !sh_status.success() {
        return Err(format!(
            "Update failed with exit code: {}",
            sh_status.code().unwrap_or(1)
        ));
    }

    Ok(())
}

fn print_manual_update_hint(repo: &str, version: Option<&str>) {
    match version {
        Some(version) => {
            eprintln!("hint: Retry manually with:");
            eprintln!(
                "  curl -fsSL https://raw.githubusercontent.com/{repo}/latest/install.sh | sh -s -- --version {version}"
            );
        }
        None => {
            eprintln!("hint: Check https://github.com/{repo}/releases for a version, then run:");
            eprintln!(
                "  curl -fsSL https://raw.githubusercontent.com/{repo}/latest/install.sh | sh -s -- --version <version>"
            );
        }
    }
}

fn print_cli_output(stdout: &str, stderr: &str) {
    let stdout_color = std::io::stdout().is_terminal() && use_color();
    let stderr_color = std::io::stderr().is_terminal() && use_color();
    if !stdout.is_empty() {
        print!("{}", colorize_output(stdout, stdout_color));
    }
    if !stderr.is_empty() {
        eprint!("{}", colorize_output(stderr, stderr_color));
    }
}

fn use_color() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

fn colorize_output(output: &str, enabled: bool) -> String {
    if !enabled {
        return output.to_string();
    }
    let mut rendered = String::new();
    for chunk in output.split_inclusive('\n') {
        rendered.push_str(&colorize_line(chunk));
    }
    if !output.ends_with('\n') && !output.is_empty() {
        let tail = output.lines().last().unwrap_or_default();
        if rendered.is_empty() {
            rendered.push_str(&colorize_line(tail));
        }
    }
    rendered
}

fn colorize_line(line: &str) -> String {
    let trimmed = line.trim_end_matches('\n');
    let suffix = if line.ends_with('\n') { "\n" } else { "" };
    let prefix = if trimmed.starts_with("error:") {
        "\x1b[31m"
    } else if trimmed.starts_with("warning:") {
        "\x1b[33m"
    } else if trimmed.starts_with("hint:") {
        "\x1b[36m"
    } else if trimmed.ends_with(" ok") || trimmed.contains(" ok:") {
        "\x1b[32m"
    } else {
        ""
    };
    if prefix.is_empty() {
        return line.to_string();
    }
    format!("{prefix}{trimmed}\x1b[0m{suffix}")
}

fn exit_code(code: i32) -> std::process::ExitCode {
    std::process::ExitCode::from(code.clamp(0, 255) as u8)
}

fn fetch_latest_version(repo: &str) -> Option<String> {
    let output = ProcessCommand::new("curl")
        .args([
            "-fsSL",
            &format!("https://api.github.com/repos/{repo}/releases/latest"),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let body: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let tag = body.get("tag_name")?.as_str()?;
    Some(tag.strip_prefix('v').unwrap_or(tag).to_string())
}
