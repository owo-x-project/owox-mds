use mds_core::model::NewDocKind;
use mds_core::AgentKitCategory;
use mds_core::AiTarget;
use mds_core::BuildMode;
use mds_core::CliRequest;
use mds_core::Command;
use mds_core::DescriptorCommand;
use mds_core::DescriptorKind;
use mds_core::DescriptorSchemaFormat;
use mds_core::DoctorFormat;
use mds_core::InitDescriptorOptions;
use mds_core::InitOptions;
use mds_core::LabelPreset;
use mds_core::NewOptions;
use std::io::{BufRead, Write};
use std::path::PathBuf;
pub fn parse_args(cwd: PathBuf) -> Result<CliRequest, String> {
    parse_args_from(cwd, std::env::args().skip(1))
}

pub fn parse_args_from<I>(cwd: PathBuf, args: I) -> Result<CliRequest, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(command_name) = args.next() else {
        return Err("missing command".to_string());
    };

    let mut package = None;
    let mut verbose = false;
    let mut dry_run = false;
    let mut fix = false;
    let mut check = false;
    let mut format = None;
    let mut package_subcommand = None;
    let mut init_options = InitOptions::default();
    let mut init_targets: Option<Vec<AiTarget>> = None;
    let mut init_categories: Option<Vec<AgentKitCategory>> = None;
    let mut init_descriptor = false;
    let mut init_descriptor_kind: Option<DescriptorKind> = None;
    let mut init_descriptor_name: Option<String> = None;
    let mut init_descriptor_output: Option<String> = None;
    let mut descriptor_subcommand: Option<String> = None;
    let mut descriptor_path: Option<String> = None;
    let mut descriptor_target: Option<String> = None;
    let mut descriptor_kind: Option<DescriptorKind> = None;
    let mut descriptor_schema_format: Option<DescriptorSchemaFormat> = None;
    let mut new_path: Option<String> = None;
    let mut new_kind: Option<NewDocKind> = None;
    let mut new_force = false;
    let mut update_version: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--package" => {
                let Some(path) = args.next() else {
                    return Err("--package requires a path".to_string());
                };
                package = Some(PathBuf::from(path));
            }
            "--verbose" => verbose = true,
            "--dry-run" => dry_run = true,
            "--fix" => fix = true,
            "--check" => check = true,
            "--format" => {
                let Some(value) = args.next() else {
                    return Err("--format requires a value".to_string());
                };
                if command_name == "descriptor" {
                    descriptor_schema_format = Some(
                        DescriptorSchemaFormat::parse(&value)
                            .ok_or_else(|| "--format must be json-schema".to_string())?,
                    );
                } else {
                    format = Some(match value.as_str() {
                        "text" => DoctorFormat::Text,
                        "json" => DoctorFormat::Json,
                        _ => return Err("--format must be text or json".to_string()),
                    });
                }
            }
            "--kind" if command_name == "descriptor" => {
                let Some(value) = args.next() else {
                    return Err("--kind requires a value".to_string());
                };
                descriptor_kind = Some(parse_descriptor_kind(&value)?);
            }
            "--ai" if command_name == "init" => init_options.ai_only = true,
            "--yes" if command_name == "init" => init_options.yes = true,
            "--force" if command_name == "init" => init_options.force = true,
            "--output" if command_name == "init" && init_descriptor => {
                let Some(value) = args.next() else {
                    return Err("--output requires a path".to_string());
                };
                init_descriptor_output = Some(value);
            }
            "--target" if command_name == "init" => {
                let Some(value) = args.next() else {
                    return Err("--target requires a value".to_string());
                };
                init_targets = Some(parse_targets(&value)?);
            }
            "--categories" if command_name == "init" => {
                let Some(value) = args.next() else {
                    return Err("--categories requires a value".to_string());
                };
                init_categories = Some(parse_categories(&value)?);
            }
            "--install-project-deps" if command_name == "init" => {
                init_options.install_project_deps = true;
            }
            "--install-toolchains" if command_name == "init" => {
                init_options.install_toolchains = true;
            }
            "--install-ai-cli" if command_name == "init" => {
                init_options.install_ai_cli = true;
            }
            "--labels" if command_name == "init" => {
                let Some(value) = args.next() else {
                    return Err("--labels requires a value (en or ja)".to_string());
                };
                init_options.label_preset = LabelPreset::parse(&value)
                    .ok_or_else(|| format!("unknown label preset `{value}`; expected en or ja"))?;
            }
            flag @ ("--ts-tools" | "--py-tools" | "--rs-tools") if command_name == "init" => {
                return Err(format!(
                    "{flag} was removed; init quality now uses semantic slot commands via the wizard summary/advanced flow or explicit [quality.<lang>] config"
                ));
            }
            "sync" if command_name == "package" && package_subcommand.is_none() => {
                package_subcommand = Some(arg);
            }
            "descriptor" if command_name == "init" && !init_descriptor => {
                init_descriptor = true;
            }
            _ if command_name == "init" && init_descriptor && !arg.starts_with('-') => {
                if init_descriptor_kind.is_none() {
                    init_descriptor_kind = Some(parse_descriptor_kind(&arg)?);
                } else if init_descriptor_name.is_none() {
                    init_descriptor_name = Some(arg);
                } else {
                    return Err(
                        "init descriptor only accepts <kind> <name>, --package, --yes, --force, and --output"
                            .to_string(),
                    );
                }
            }
            _ if command_name == "descriptor" && !arg.starts_with('-') => {
                match descriptor_subcommand.as_deref() {
                    None => descriptor_subcommand = Some(arg),
                    Some("check") if descriptor_path.is_none() => descriptor_path = Some(arg),
                    Some("explain") if descriptor_target.is_none() => descriptor_target = Some(arg),
                    Some("schema") | Some("sources") => {
                        return Err(format!(
                            "descriptor {} does not accept positional argument `{arg}`",
                            descriptor_subcommand.as_deref().unwrap_or_default()
                        ));
                    }
                    Some(_) => {
                        return Err(format!(
                            "descriptor {} accepts too many positional arguments",
                            descriptor_subcommand.as_deref().unwrap_or_default()
                        ));
                    }
                }
            }
            "--force" if command_name == "new" => {
                new_force = true;
            }
            "--version" if command_name == "update" => {
                let Some(value) = args.next() else {
                    return Err("--version requires a value".to_string());
                };
                update_version = Some(value);
            }
            _ if command_name == "new" && !arg.starts_with('-') => {
                if new_path.is_none() {
                    new_path = Some(arg);
                } else if new_kind.is_none() {
                    new_kind = Some(parse_new_kind(&arg)?);
                } else {
                    return Err(
                        "new only accepts <path>, optional <kind>, --package, --force, and --verbose"
                            .to_string(),
                    );
                }
            }
            _ => return Err(format!("unknown option `{arg}`")),
        }
    }
    if let Some(targets) = &init_targets {
        init_options.targets = targets.clone();
    }
    if let Some(categories) = &init_categories {
        init_options.categories = categories.clone();
    }

    let command = match command_name.as_str() {
        "check" => {
            return Err("`mds check` was removed; use `mds lint` for structure validation, `mds typecheck` for type checks, and `mds test` for tests".to_string())
        }
        "build" => {
            if fix || check || format.is_some() {
                return Err("build only accepts --package, --verbose, and --dry-run".to_string());
            }
            Command::Build {
                mode: if dry_run {
                    BuildMode::DryRun
                } else {
                    BuildMode::Write
                },
            }
        }
        "lint" => {
            if format.is_some() {
                return Err(
                    "lint only accepts --package, --verbose, --fix, and --check".to_string(),
                );
            }
            if dry_run {
                return Err("--dry-run is only valid for build".to_string());
            }
            if check && !fix {
                return Err("--check is only valid with lint --fix or package sync".to_string());
            }
            Command::Lint { fix, check }
        }
        "typecheck" => {
            if dry_run || fix || check || format.is_some() {
                return Err("typecheck only accepts --package and --verbose".to_string());
            }
            Command::Typecheck
        }
        "test" => {
            if dry_run || fix || check || format.is_some() {
                return Err("test only accepts --package and --verbose".to_string());
            }
            Command::Test
        }
        "doctor" => {
            if dry_run || fix || check {
                return Err("doctor only accepts --package, --verbose, and --format".to_string());
            }
            Command::Doctor {
                format: format.unwrap_or(DoctorFormat::Text),
            }
        }
        "package" => {
            if dry_run || fix || format.is_some() {
                return Err(
                    "package sync only accepts --package, --verbose, and --check".to_string(),
                );
            }
            match package_subcommand.as_deref() {
                Some("sync") => Command::PackageSync { check },
                _ => return Err("package requires subcommand sync".to_string()),
            }
        }
        "descriptor" => {
            if dry_run || fix || check || format.is_some() {
                return Err(
                    "descriptor only accepts descriptor-specific options, --package, and --verbose"
                        .to_string(),
                );
            }
            let subcommand = descriptor_subcommand
                .as_deref()
                .ok_or_else(|| "descriptor requires subcommand check, explain, schema, or sources".to_string())?;
            match subcommand {
                "check" => {
                    if descriptor_kind.is_some() || descriptor_schema_format.is_some() || descriptor_target.is_some()
                    {
                        return Err(
                            "descriptor check only accepts optional <path>, --package, and --verbose"
                                .to_string(),
                        );
                    }
                    Command::Descriptor {
                        command: DescriptorCommand::Check {
                            path: descriptor_path,
                        },
                    }
                }
                "explain" => {
                    if descriptor_kind.is_some() || descriptor_schema_format.is_some() || descriptor_path.is_some()
                    {
                        return Err(
                            "descriptor explain only accepts <target>, --package, and --verbose"
                                .to_string(),
                        );
                    }
                    let target = descriptor_target
                        .ok_or_else(|| "descriptor explain requires <target>".to_string())?;
                    Command::Descriptor {
                        command: DescriptorCommand::Explain { target },
                    }
                }
                "schema" => {
                    if descriptor_path.is_some() || descriptor_target.is_some() {
                        return Err(
                            "descriptor schema only accepts --kind and --format json-schema"
                                .to_string(),
                        );
                    }
                    let kind = descriptor_kind
                        .ok_or_else(|| "descriptor schema requires --kind".to_string())?;
                    Command::Descriptor {
                        command: DescriptorCommand::Schema {
                            kind,
                            format: descriptor_schema_format
                                .unwrap_or(DescriptorSchemaFormat::JsonSchema),
                        },
                    }
                }
                "sources" => {
                    if descriptor_kind.is_some()
                        || descriptor_schema_format.is_some()
                        || descriptor_path.is_some()
                        || descriptor_target.is_some()
                    {
                        return Err(
                            "descriptor sources only accepts --package and --verbose".to_string()
                        );
                    }
                    Command::Descriptor {
                        command: DescriptorCommand::Sources,
                    }
                }
                _ => {
                    return Err(
                        "descriptor requires subcommand check, explain, schema, or sources"
                            .to_string(),
                    )
                }
            }
        }
        "init" => {
            if dry_run || fix || check || format.is_some() {
                return Err("init only accepts init-specific options and --package".to_string());
            }
            if init_descriptor {
                if init_targets.is_some()
                    || init_categories.is_some()
                    || init_options.ai_only
                    || init_options.install_project_deps
                    || init_options.install_toolchains
                    || init_options.install_ai_cli
                    || init_options.label_preset != LabelPreset::English
                {
                    return Err(
                        "init descriptor only accepts <kind> <name>, --package, --yes, --force, and --output"
                            .to_string(),
                    );
                }
                let kind = init_descriptor_kind.ok_or_else(|| {
                    "init descriptor requires <kind>; expected language, tool, or package-manager"
                        .to_string()
                })?;
                let name = init_descriptor_name
                    .ok_or_else(|| "init descriptor requires <name>".to_string())?;
                return Ok(CliRequest {
                    cwd,
                    package,
                    verbose,
                    command: Command::InitDescriptor {
                        options: InitDescriptorOptions {
                            kind,
                            name,
                            output: init_descriptor_output,
                            yes: init_options.yes,
                            force: init_options.force,
                            aliases: Vec::new(),
                            language: None,
                            tool: None,
                            package_manager: None,
                        },
                    },
                });
            }
            Command::Init {
                options: init_options,
            }
        }
        "new" => {
            if dry_run || fix || check || format.is_some() {
                return Err(
                    "new only accepts <path>, optional <kind>, --package, --force, and --verbose"
                        .to_string(),
                );
            }
            let path = new_path.ok_or_else(|| {
                "new requires a relative Markdown path (e.g. `mds new greet.ts.md impl`)"
                    .to_string()
            })?;
            Command::New {
                options: NewOptions {
                    path,
                    kind: new_kind,
                    force: new_force,
                },
            }
        }
        "update" => {
            if package.is_some() || verbose || dry_run || fix || check || format.is_some() {
                return Err("update only accepts --version".to_string());
            }
            Command::Update {
                version: update_version,
            }
        }
        _ => return Err(format!("unknown command `{command_name}`")),
    };

    Ok(CliRequest {
        cwd,
        package,
        verbose,
        command,
    })
}

fn parse_targets(value: &str) -> Result<Vec<AiTarget>, String> {
    if value == "all" {
        return Ok(AiTarget::all().to_vec());
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            AiTarget::parse(part).ok_or_else(|| {
                format!(
                    "unknown AI target `{part}`; expected all, claude-code, codex-cli, opencode, or github-copilot-cli"
                )
            })
        })
        .collect()
}

fn parse_categories(value: &str) -> Result<Vec<AgentKitCategory>, String> {
    if value == "all" {
        return Ok(AgentKitCategory::all().to_vec());
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            AgentKitCategory::parse(part).ok_or_else(|| {
                format!(
                    "unknown init category `{part}`; expected all, instructions, skills, or commands"
                )
            })
        })
        .collect()
}

fn parse_new_kind(value: &str) -> Result<NewDocKind, String> {
    NewDocKind::parse(value).ok_or_else(|| {
        format!(
            "unknown new kind `{value}`; expected {}",
            expected_new_kind_values()
        )
    })
}

fn parse_descriptor_kind(value: &str) -> Result<DescriptorKind, String> {
    DescriptorKind::parse(value).ok_or_else(|| {
        format!("unknown descriptor kind `{value}`; expected language, tool, or package-manager")
    })
}

fn expected_new_kind_values() -> &'static str {
    "impl, test, overview, or root-module"
}

pub fn resolve_new_options<R, W>(
    mut options: NewOptions,
    interactive: bool,
    input: &mut R,
    output: &mut W,
) -> Result<NewOptions, String>
where
    R: BufRead,
    W: Write,
{
    if options.kind.is_some() {
        return Ok(options);
    }
    if !interactive {
        return Err(format!(
            "new requires <kind>; expected {}",
            expected_new_kind_values()
        ));
    }

    loop {
        writeln!(output, "Select a document kind for `{}`:", options.path)
            .map_err(|error| format!("failed to render new kind prompt: {error}"))?;
        for (index, kind) in NewDocKind::all().iter().enumerate() {
            writeln!(output, "  {}. {}", index + 1, kind.key())
                .map_err(|error| format!("failed to render new kind prompt: {error}"))?;
        }
        write!(output, "> ")
            .map_err(|error| format!("failed to render new kind prompt: {error}"))?;
        output
            .flush()
            .map_err(|error| format!("failed to flush new kind prompt: {error}"))?;

        let mut line = String::new();
        let read = input
            .read_line(&mut line)
            .map_err(|error| format!("failed to read new kind selection: {error}"))?;
        if read == 0 {
            return Err("new kind selection cancelled".to_string());
        }

        let choice = line.trim();
        let selected = match choice {
            "1" => Some(NewDocKind::Impl),
            "2" => Some(NewDocKind::Test),
            "3" => Some(NewDocKind::Overview),
            "4" => Some(NewDocKind::RootModule),
            _ => NewDocKind::parse(choice),
        };

        if let Some(kind) = selected {
            options.kind = Some(kind);
            return Ok(options);
        }

        writeln!(
            output,
            "Enter 1-4 or one of: {}",
            expected_new_kind_values()
        )
        .map_err(|error| format!("failed to render new kind prompt: {error}"))?;
    }
}

pub fn usage_text() -> &'static str {
    concat!(
        "\n",
        "mds — Markdown-driven code generation toolchain\n",
        "\n",
        "Commands:\n",
        "  mds init                                  Interactive project setup (wizard mode)\n",
        "  mds init [options] --yes                  Non-interactive project setup\n",
        "  mds init descriptor <kind> <name> --yes   Create a minimal descriptor\n",
        "  mds new <path> <kind> [options]           Create new authoring Markdown\n",
        "  mds descriptor check [path] [options]     Validate descriptors and source state\n",
        "  mds descriptor explain <target>           Explain descriptor resolution\n",
        "  mds descriptor schema --kind <kind>       Print descriptor JSON Schema\n",
        "  mds descriptor sources [options]          Show descriptor source and lock state\n",
        "  mds build [--package <path>] [--dry-run]  Generate derived code\n",
        "  mds typecheck [--package <path>]          Run type checks on code blocks\n",
        "  mds lint [--package <path>] [--fix]       Run linters on code blocks\n",
        "  mds test [--package <path>]               Run tests from code blocks\n",
        "  mds doctor [--package <path>]             Diagnose environment\n",
        "  mds package sync [--package <path>]       Sync package overview snapshot\n",
        "  mds update [--version <version>]          Update the installed mds CLI\n",
        "\n",
        "Init options:\n",
        "  --ai                      AI agent kit only (skip project files)\n",
        "  --target <list>           AI targets: all, claude-code, codex-cli, opencode, github-copilot-cli\n",
        "  --categories <list>       Agent kit categories: all, instructions, skills, commands\n",
        "  --labels <preset>         Section label language: en (default), ja (Japanese)\n",
        "  --yes                     Execute without confirmation\n",
        "  --force                   Overwrite non-managed files\n",
        "  --install-project-deps    Run npm install / cargo fetch / uv sync\n",
        "  --install-toolchains      Check required toolchains\n",
        "  --install-ai-cli          Check AI CLI tools\n",
        "\n",
        "Descriptor kinds:\n",
        "  language                 Language descriptor\n",
        "  tool                     Tool descriptor\n",
        "  package-manager          Package manager descriptor\n",
        "\n",
        "Descriptor options:\n",
        "  --kind <kind>             Schema kind: language, tool, package-manager\n",
        "  --format json-schema      Schema output format\n",
        "  --output <path>           init descriptor output path\n",
        "  --force                   init descriptor overwrite existing descriptor\n",
        "\n",
        "New kinds:\n",
        "  impl                     Create an implementation doc under .mds/source\n",
        "  test                     Create a test doc under .mds/test\n",
        "  overview                 Create the source overview special file\n",
        "  root-module              Create a prose-first root module doc under .mds/source\n",
        "\n",
        "New kind fallback:\n",
        "  Interactive terminals may omit <kind> to choose from a prompt.\n",
        "  Non-interactive runs must pass one of: impl, test, overview, root-module.\n",
        "\n",
        "Global options:\n",
        "  --package <path>          Target package directory\n",
        "  --verbose                 Show detailed output\n",
        "  --help, -h                Show this help message\n",
        "  --version, -V             Show version\n",
        "\n",
        "Examples:\n",
        "  mds init                                      # Interactive wizard\n",
        "  mds init --package ./my-pkg --yes              # Quick setup with defaults\n",
        "  mds init descriptor language gleam --yes       # Create .mds/descriptors/languages/gleam.toml\n",
        "  mds descriptor check --package ./my-pkg        # Validate package descriptors\n",
        "  mds descriptor explain gleam --package ./my-pkg # Explain language/tool/package-manager origin\n",
        "  mds descriptor schema --kind language --format json-schema\n",
        "  mds descriptor sources --package ./my-pkg      # Show descriptor source config and lock\n",
        "  mds new greet.ts.md impl                       # New TypeScript implementation\n",
        "  mds new greet.ts.md test --package ./my-pkg    # New test Markdown\n",
        "  mds new overview.md overview                   # New source overview\n",
        "  mds new index.ts.md root-module                # New root module doc\n",
        "  mds new greet.ts.md                            # Interactive terminal only: prompt for kind\n",
        "  mds build --package ./my-pkg --dry-run         # Preview generation\n",
        "  mds build --package ./my-pkg                   # Generate code\n",
        "  mds update --version 0.2.1-alpha              # Update to a specific release\n",
        "\n",
        "Documentation: https://github.com/owox/mds\n"
    )
}

pub fn print_usage() {
    eprint!("{}", usage_text());
}
