use mds_cli::args::{parse_args_from, resolve_new_options, usage_text};
use mds_core::model::NewDocKind;
use mds_core::AgentKitCategory;
use mds_core::AiTarget;
use mds_core::BuildMode;
use mds_core::Command;
use mds_core::DescriptorCommand;
use mds_core::DescriptorKind;
use mds_core::DescriptorSchemaFormat;
use mds_core::DoctorFormat;
use std::io::Cursor;
use std::path::PathBuf;
#[test]
fn parses_build_dry_run() {
    let request = parse_args_from(
        PathBuf::from("/repo"),
        ["build", "--dry-run", "--package", "pkg", "--verbose"].map(String::from),
    )
    .unwrap();
    assert_eq!(request.cwd, PathBuf::from("/repo"));
    assert_eq!(request.package, Some(PathBuf::from("pkg")));
    assert!(request.verbose);
    assert!(matches!(
        request.command,
        Command::Build {
            mode: BuildMode::DryRun
        }
    ));
}

#[test]
fn rejects_removed_check_command() {
    let error = parse_args_from(PathBuf::from("/repo"), ["check"].map(String::from)).unwrap_err();
    assert!(error.contains("mds check"));
    assert!(error.contains("mds lint"));
}

#[test]
fn parses_post_mvp_commands() {
    let lint = parse_args_from(
        PathBuf::from("/repo"),
        ["lint", "--fix", "--check", "--package", "pkg"].map(String::from),
    )
    .unwrap();
    assert!(matches!(
        lint.command,
        Command::Lint {
            fix: true,
            check: true
        }
    ));

    let typecheck = parse_args_from(
        PathBuf::from("/repo"),
        ["typecheck", "--package", "pkg"].map(String::from),
    )
    .unwrap();
    assert!(matches!(typecheck.command, Command::Typecheck));

    let doctor = parse_args_from(
        PathBuf::from("/repo"),
        ["doctor", "--format", "json"].map(String::from),
    )
    .unwrap();
    assert!(matches!(
        doctor.command,
        Command::Doctor {
            format: DoctorFormat::Json
        }
    ));

    let doctor_text = parse_args_from(
        PathBuf::from("/repo"),
        ["doctor", "--format", "text"].map(String::from),
    )
    .unwrap();
    assert!(matches!(
        doctor_text.command,
        Command::Doctor {
            format: DoctorFormat::Text
        }
    ));

    let sync = parse_args_from(
        PathBuf::from("/repo"),
        ["package", "sync", "--check"].map(String::from),
    )
    .unwrap();
    assert!(matches!(sync.command, Command::PackageSync { check: true }));
}

#[test]
fn parses_descriptor_commands() {
    let check = parse_args_from(
        PathBuf::from("/repo"),
        [
            "descriptor",
            "check",
            ".mds/descriptors",
            "--package",
            "pkg",
        ]
        .map(String::from),
    )
    .unwrap();
    assert_eq!(check.package, Some(PathBuf::from("pkg")));
    assert!(matches!(
        check.command,
        Command::Descriptor {
            command: DescriptorCommand::Check { path: Some(ref path) }
        } if path == ".mds/descriptors"
    ));

    let explain = parse_args_from(
        PathBuf::from("/repo"),
        ["descriptor", "explain", "eslint --stdin"].map(String::from),
    )
    .unwrap();
    assert!(matches!(
        explain.command,
        Command::Descriptor {
            command: DescriptorCommand::Explain { ref target }
        } if target == "eslint --stdin"
    ));

    let schema = parse_args_from(
        PathBuf::from("/repo"),
        [
            "descriptor",
            "schema",
            "--kind",
            "package-manager",
            "--format",
            "json-schema",
        ]
        .map(String::from),
    )
    .unwrap();
    assert!(matches!(
        schema.command,
        Command::Descriptor {
            command: DescriptorCommand::Schema {
                kind: DescriptorKind::PackageManager,
                format: DescriptorSchemaFormat::JsonSchema
            }
        }
    ));

    let sources = parse_args_from(
        PathBuf::from("/repo"),
        ["descriptor", "sources", "--verbose"].map(String::from),
    )
    .unwrap();
    assert!(sources.verbose);
    assert!(matches!(
        sources.command,
        Command::Descriptor {
            command: DescriptorCommand::Sources
        }
    ));
}

#[test]
fn parses_init_descriptor_command() {
    let request = parse_args_from(
        PathBuf::from("/repo"),
        [
            "init",
            "descriptor",
            "language",
            "Gleam",
            "--package",
            "pkg",
            "--yes",
            "--force",
            "--output",
            ".mds/descriptors/languages/gleam.toml",
        ]
        .map(String::from),
    )
    .unwrap();
    assert_eq!(request.package, Some(PathBuf::from("pkg")));
    match request.command {
        Command::InitDescriptor { options } => {
            assert_eq!(options.kind, DescriptorKind::Language);
            assert_eq!(options.name, "Gleam");
            assert!(options.yes);
            assert!(options.force);
            assert_eq!(
                options.output,
                Some(".mds/descriptors/languages/gleam.toml".to_string())
            );
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_command_specific_usage_errors() {
    let cases: &[(&[&str], &str)] = &[
        (
            &["build", "--format", "json"],
            "build only accepts --package, --verbose, and --dry-run",
        ),
        (
            &["lint", "--format", "json"],
            "lint only accepts --package, --verbose, --fix, and --check",
        ),
        (
            &["typecheck", "--format", "json"],
            "typecheck only accepts --package and --verbose",
        ),
        (
            &["test", "--format", "json"],
            "test only accepts --package and --verbose",
        ),
        (
            &["doctor", "--check"],
            "doctor only accepts --package, --verbose, and --format",
        ),
        (
            &["package", "sync", "--format", "json"],
            "package sync only accepts --package, --verbose, and --check",
        ),
        (
            &["descriptor", "schema"],
            "descriptor schema requires --kind",
        ),
        (
            &["descriptor", "explain"],
            "descriptor explain requires <target>",
        ),
        (
            &["init", "--format", "json"],
            "init only accepts init-specific options and --package",
        ),
        (
            &["init", "descriptor", "language"],
            "init descriptor requires <name>",
        ),
        (
            &["new", "greet.ts.md", "impl", "--format", "json"],
            "new only accepts <path>, optional <kind>, --package, --force, and --verbose",
        ),
        (
            &["update", "--package", "pkg"],
            "update only accepts --version",
        ),
    ];

    for (args, expected) in cases {
        let error = parse_args_from(
            PathBuf::from("/repo"),
            args.iter().copied().map(String::from),
        )
        .unwrap_err();
        assert_eq!(error, *expected, "args={args:?}");
    }
}

#[test]
fn rejects_non_doctor_format_option_even_when_text() {
    let cases: &[(&[&str], &str)] = &[
        (
            &["build", "--format", "text"],
            "build only accepts --package, --verbose, and --dry-run",
        ),
        (
            &["lint", "--format", "text"],
            "lint only accepts --package, --verbose, --fix, and --check",
        ),
        (
            &["typecheck", "--format", "text"],
            "typecheck only accepts --package and --verbose",
        ),
        (
            &["test", "--format", "text"],
            "test only accepts --package and --verbose",
        ),
        (
            &["package", "sync", "--format", "text"],
            "package sync only accepts --package, --verbose, and --check",
        ),
        (
            &["init", "--format", "text"],
            "init only accepts init-specific options and --package",
        ),
        (
            &["new", "greet.ts.md", "impl", "--format", "text"],
            "new only accepts <path>, optional <kind>, --package, --force, and --verbose",
        ),
        (
            &["update", "--format", "text"],
            "update only accepts --version",
        ),
    ];

    for (args, expected) in cases {
        let error = parse_args_from(
            PathBuf::from("/repo"),
            args.iter().copied().map(String::from),
        )
        .unwrap_err();
        assert_eq!(error, *expected, "args={args:?}");
    }
}

#[test]
fn parses_init_command() {
    let request = parse_args_from(
        PathBuf::from("/repo"),
        [
            "init",
            "--ai",
            "--target",
            "claude-code,opencode",
            "--categories",
            "instructions,commands",
            "--yes",
            "--force",
            "--install-project-deps",
            "--install-toolchains",
            "--install-ai-cli",
            "--labels",
            "ja",
        ]
        .map(String::from),
    )
    .unwrap();
    match request.command {
        Command::Init { options } => {
            assert!(options.ai_only);
            assert!(options.yes);
            assert!(options.force);
            assert!(options.install_project_deps);
            assert!(options.install_toolchains);
            assert!(options.install_ai_cli);
            assert_eq!(options.label_preset, mds_core::LabelPreset::Japanese);
            assert_eq!(
                options.targets,
                vec![AiTarget::ClaudeCode, AiTarget::Opencode]
            );
            assert_eq!(
                options.categories,
                vec![AgentKitCategory::Instructions, AgentKitCategory::Commands]
            );
            assert!(options.quality_commands.is_empty());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_removed_init_tool_flags() {
    let error = parse_args_from(
        PathBuf::from("/repo"),
        ["init", "--ts-tools", "biome,jest"].map(String::from),
    )
    .unwrap_err();
    assert!(error.contains("--ts-tools was removed"));
}

#[test]
fn parses_new_command_with_kind() {
    let request = parse_args_from(
        PathBuf::from("/repo"),
        [
            "new",
            "docs/greet.ts.md",
            "impl",
            "--package",
            "pkg",
            "--force",
        ]
        .map(String::from),
    )
    .unwrap();

    match request.command {
        Command::New { options } => {
            assert_eq!(options.path, "docs/greet.ts.md");
            assert_eq!(options.kind, Some(NewDocKind::Impl));
            assert!(options.force);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn allows_new_command_without_kind_for_interactive_fallback() {
    let request = parse_args_from(
        PathBuf::from("/repo"),
        ["new", "docs/greet.ts.md"].map(String::from),
    )
    .unwrap();

    match request.command {
        Command::New { options } => {
            assert_eq!(options.path, "docs/greet.ts.md");
            assert_eq!(options.kind, None);
            assert!(!options.force);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_unknown_new_kind() {
    let error = parse_args_from(
        PathBuf::from("/repo"),
        ["new", "docs/greet.ts.md", "unknown-kind"].map(String::from),
    )
    .unwrap_err();
    assert!(error.contains("unknown new kind"));
    assert!(error.contains("root-module"));
}

#[test]
fn resolves_new_kind_interactively() {
    let mut input = Cursor::new(b"4\n".to_vec());
    let mut output = Vec::new();

    let resolved = resolve_new_options(
        mds_core::NewOptions {
            path: "index.ts.md".to_string(),
            kind: None,
            force: false,
        },
        true,
        &mut input,
        &mut output,
    )
    .unwrap();

    assert_eq!(resolved.kind, Some(NewDocKind::RootModule));
    let rendered = String::from_utf8(output).unwrap();
    assert!(rendered.contains("Select a document kind"));
    assert!(rendered.contains("root-module"));
}

#[test]
fn usage_marks_new_kind_required_except_for_interactive_fallback() {
    let usage = usage_text();
    assert!(usage.contains("mds new <path> <kind> [options]"));
    assert!(usage.contains("mds init descriptor <kind> <name> --yes"));
    assert!(usage.contains("mds descriptor check [path] [options]"));
    assert!(usage.contains("mds descriptor schema --kind <kind>"));
    assert!(usage.contains("mds update [--version <version>]"));
    assert!(!usage.contains("--ts-tools"));
    assert!(!usage.contains("--py-tools"));
    assert!(!usage.contains("--rs-tools"));
    assert!(usage.contains("Interactive terminals may omit <kind> to choose from a prompt."));
    assert!(
        usage.contains("mds new greet.ts.md                            # Interactive terminal only: prompt for kind")
    );
    assert!(!usage.contains("mds new <path> [kind]"));
}
