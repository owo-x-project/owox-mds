use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
const TS_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/languages/ts.toml");
const NPM_PACKAGE_MANAGER: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/package-managers/npm.toml");

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(prefix: &str) -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn write_executable(&self, relative: &str, content: &str) {
        self.write(relative, content);
        let path = self.path.join(relative);
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct CliRun {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_mds(cwd: &Path, args: &[&str]) -> CliRun {
    run_mds_with_env(cwd, args, Vec::new())
}

fn run_mds_with_env(cwd: &Path, args: &[&str], envs: Vec<(&str, String)>) -> CliRun {
    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_mds"))
        .current_dir(cwd)
        .args(args)
        .env("NO_COLOR", "1")
        .envs(envs)
        .output()
        .unwrap();
    cli_run(output)
}

fn cli_run(output: Output) -> CliRun {
    CliRun {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn write_node_package(root: &TestDir, name: &str) {
    root.write(
        "package.json",
        &format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"0.1.0\",\n  \"private\": true\n}}\n"
        ),
    );
    root.write("package-lock.json", "{}\n");
}

fn write_ts_descriptor(root: &TestDir) {
    root.write(".mds/descriptors/languages/ts.toml", TS_DESCRIPTOR);
}

fn write_npm_package_manager_descriptor(root: &TestDir) {
    root.write(
        ".mds/descriptors/package-managers/npm.toml",
        NPM_PACKAGE_MANAGER,
    );
}

fn write_eslint_tool_descriptor(root: &TestDir) {
    root.write(
        ".mds/descriptors/tools/eslint.toml",
        concat!(
            "id = \"eslint\"\n",
            "match_prefixes = [\"eslint\"]\n",
            "\n",
            "[behavior]\n",
            "input = \"stdin\"\n",
            "append_file_arg = true\n",
            "\n",
            "[[behavior.diagnostics]]\n",
            "pattern = '^(?P<path>.+?):(?P<line>\\d+):(?P<column>\\d+): (?P<message>.+)$'\n"
        ),
    );
}

fn write_stable_ts_config(root: &TestDir) {
    root.write(
        "mds.config.toml",
        concat!(
            "[package]\n",
            "enabled = true\n",
            "allow_raw_source = false\n",
            "\n",
            "[roots]\n",
            "source_md = \".mds/source\"\n",
            "test_md = \".mds/test\"\n",
            "source_out = \"src\"\n",
            "test_out = \"tests\"\n",
            "\n",
            "[output]\n",
            "source = \"{source_out}/{module}.{ext}\"\n",
            "test = \"{test_out}/{module}.test.{ext}\"\n",
            "\n",
            "[quality.ts]\n",
            "type_checker = false\n",
            "linter = false\n",
            "fixer = false\n",
            "test_runner = false\n",
            "required = []\n",
            "optional = []\n"
        ),
    );
}

fn write_extensionless_root_module_descriptor(root: &TestDir, package: &str) {
    root.write(
        &format!("{package}/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    );
    root.write(
        &format!("{package}/.mds/descriptors/languages/dts.toml"),
        concat!(
            "id = \"dts\"\n",
            "match_suffixes = [\"d.ts\"]\n\n",
            "[language]\n",
            "primary_ext = \"ts\"\n",
            "root_module_markdown_names = [\"index\"]\n\n",
            "[files.source]\n",
            "strip_lang_ext = true\n",
            "prefix = \"\"\n",
            "suffix = \"\"\n",
            "extension = \"ts\"\n\n",
            "[files.test]\n",
            "strip_lang_ext = true\n",
            "prefix = \"\"\n",
            "suffix = \".test\"\n",
            "extension = \"ts\"\n"
        ),
    );
}

fn prepend_path(dir: &Path) -> String {
    match std::env::var("PATH") {
        Ok(existing) if !existing.is_empty() => format!("{}:{existing}", dir.display()),
        _ => dir.display().to_string(),
    }
}

fn write_fake_update_curl(
    root: &TestDir,
    latest_json: Option<&str>,
    latest_exit: i32,
    install_exit: i32,
) {
    let install_script = format!("#!/bin/sh\nexit {install_exit}\n");
    write_fake_update_curl_with_install_script(root, latest_json, latest_exit, &install_script);
}

fn write_fake_update_curl_with_install_script(
    root: &TestDir,
    latest_json: Option<&str>,
    latest_exit: i32,
    install_script: &str,
) {
    let latest_branch = match latest_json {
        Some(json) => format!("    cat <<'EOF'\n{json}\nEOF"),
        None => format!("    exit {latest_exit}"),
    };
    root.write_executable(
        "bin/curl",
        &format!(
            r#"#!/bin/sh
url=""
for arg in "$@"; do
  url="$arg"
done

case "$url" in
  https://api.github.com/repos/owo-x-project/owox-mds/releases/latest)
{latest_branch}
    ;;
  https://raw.githubusercontent.com/owo-x-project/owox-mds/latest/install.sh)
    cat <<'EOF'
{install_script}
EOF
    ;;
  *)
    echo "unexpected url: $url" >&2
    exit 99
    ;;
esac
"#,
        ),
    );
}

#[test]
fn init_and_new_outputs_stay_consistent_without_overriding_generated_config() {
    let temp = TestDir::new("mds-cli-main-success");
    write_node_package(&temp, "cli-main-success");

    let init = run_mds(temp.path(), &["init", "--package", ".", "--yes"]);
    assert_eq!(
        init.code, 0,
        "stderr={} stdout={}",
        init.stderr, init.stdout
    );
    assert!(init.stdout.contains("Init plan:"));
    assert!(init.stdout.contains("init ok"));
    assert!(temp.path().join("mds.config.toml").exists());
    assert!(temp.path().join(".mds/source/overview.md").exists());
    let generated_config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();

    let new_doc = run_mds(
        temp.path(),
        &["new", "greet.ts.md", "impl", "--package", "."],
    );
    assert_eq!(
        new_doc.code, 0,
        "stderr={} stdout={}",
        new_doc.stderr, new_doc.stdout
    );
    assert!(new_doc.stdout.contains("created"));
    assert!(temp.path().join(".mds/source/greet.ts.md").exists());
    assert_eq!(
        fs::read_to_string(temp.path().join("mds.config.toml")).unwrap(),
        generated_config
    );

    let build = run_mds(temp.path(), &["build", "--package", "."]);
    assert_eq!(
        build.code, 0,
        "stderr={} stdout={}",
        build.stderr, build.stdout
    );
    assert!(build.stdout.contains("build ok"));
    assert!(temp.path().join("src/greet.ts").exists());
}

#[test]
fn major_command_paths_succeed_on_stable_package_fixture() {
    let temp = TestDir::new("mds-cli-main-stable");
    write_node_package(&temp, "cli-main-stable");

    let init = run_mds(temp.path(), &["init", "--package", ".", "--yes"]);
    assert_eq!(
        init.code, 0,
        "stderr={} stdout={}",
        init.stderr, init.stdout
    );
    write_stable_ts_config(&temp);

    let new_doc = run_mds(
        temp.path(),
        &["new", "greet.ts.md", "impl", "--package", "."],
    );
    assert_eq!(
        new_doc.code, 0,
        "stderr={} stdout={}",
        new_doc.stderr, new_doc.stdout
    );
    assert!(temp.path().join(".mds/source/greet.ts.md").exists());

    let build = run_mds(temp.path(), &["build", "--package", "."]);
    assert_eq!(
        build.code, 0,
        "stderr={} stdout={}",
        build.stderr, build.stdout
    );
    assert!(build.stdout.contains("build ok"));
    assert!(temp.path().join("src/greet.ts").exists());

    let lint = run_mds(temp.path(), &["lint", "--package", "."]);
    assert_eq!(
        lint.code, 0,
        "stderr={} stdout={}",
        lint.stderr, lint.stdout
    );
    assert!(lint.stdout.contains("lint ok"));

    let typecheck = run_mds(temp.path(), &["typecheck", "--package", "."]);
    assert_eq!(
        typecheck.code, 0,
        "stderr={} stdout={}",
        typecheck.stderr, typecheck.stdout
    );
    assert!(typecheck.stdout.contains("typecheck ok"));

    let test = run_mds(temp.path(), &["test", "--package", "."]);
    assert_eq!(
        test.code, 0,
        "stderr={} stdout={}",
        test.stderr, test.stdout
    );
    assert!(test.stdout.contains("test ok"));

    let doctor = run_mds(temp.path(), &["doctor", "--package", "."]);
    assert_eq!(
        doctor.code, 0,
        "stderr={} stdout={}",
        doctor.stderr, doctor.stdout
    );
    assert!(doctor.stdout.contains("Doctor summary:"));

    let sync = run_mds(
        temp.path(),
        &["package", "sync", "--package", ".", "--check"],
    );
    assert_eq!(
        sync.code, 0,
        "stderr={} stdout={}",
        sync.stderr, sync.stdout
    );
    assert!(sync.stdout.contains("package sync ok:"));
}

#[test]
fn descriptor_authoring_commands_cover_check_explain_schema_and_sources() {
    let temp = TestDir::new("mds-cli-main-descriptor");
    write_ts_descriptor(&temp);
    write_npm_package_manager_descriptor(&temp);
    write_eslint_tool_descriptor(&temp);
    temp.write(
        ".mds/descriptor-sources.toml",
        "[[sources]]\nid = \"local\"\npath = \".mds/descriptors\"\n",
    );
    temp.write(
        ".mds/descriptor-sources.lock",
        "[[sources]]\nid = \"local\"\nresolved_path = \".mds/descriptors\"\n",
    );

    let check = run_mds(temp.path(), &["descriptor", "check", "--package", "."]);
    assert_eq!(
        check.code, 0,
        "stderr={} stdout={}",
        check.stderr, check.stdout
    );
    assert!(check.stdout.contains("Descriptor check:"));
    assert!(check.stdout.contains("descriptor check ok"));
    assert!(check.stdout.contains("- descriptors: 3"));
    assert!(check.stdout.contains("- source config: present"));
    assert!(check.stdout.contains("- source lock: present"));

    let explain_lang = run_mds(
        temp.path(),
        &["descriptor", "explain", "greet.ts.md", "--package", "."],
    );
    assert_eq!(
        explain_lang.code, 0,
        "stderr={} stdout={}",
        explain_lang.stderr, explain_lang.stdout
    );
    assert!(explain_lang.stdout.contains("- kind: language"));
    assert!(explain_lang.stdout.contains("- descriptor id: ts"));
    assert!(explain_lang.stdout.contains("- origin: package-local:"));

    let explain_tool = run_mds(
        temp.path(),
        &["descriptor", "explain", "eslint --stdin", "--package", "."],
    );
    assert_eq!(
        explain_tool.code, 0,
        "stderr={} stdout={}",
        explain_tool.stderr, explain_tool.stdout
    );
    assert!(explain_tool.stdout.contains("- kind: tool"));
    assert!(explain_tool.stdout.contains("- descriptor id: eslint"));

    let schema = run_mds(
        temp.path(),
        &[
            "descriptor",
            "schema",
            "--kind",
            "language",
            "--format",
            "json-schema",
        ],
    );
    assert_eq!(
        schema.code, 0,
        "stderr={} stdout={}",
        schema.stderr, schema.stdout
    );
    assert!(schema
        .stdout
        .contains("\"title\": \"mds language descriptor\""));
    assert!(schema.stdout.contains("\"primary_ext\""));

    let sources = run_mds(temp.path(), &["descriptor", "sources", "--package", "."]);
    assert_eq!(
        sources.code, 0,
        "stderr={} stdout={}",
        sources.stderr, sources.stdout
    );
    assert!(sources.stdout.contains("Descriptor sources:"));
    assert!(sources.stdout.contains("- repo config: present"));
    assert!(sources.stdout.contains("- source lock: present"));
}

#[test]
fn init_descriptor_creates_minimal_language_descriptor_without_overwriting() {
    let temp = TestDir::new("mds-cli-main-init-descriptor");

    let init = run_mds(
        temp.path(),
        &["init", "descriptor", "language", "Gleam", "--yes"],
    );
    assert_eq!(
        init.code, 0,
        "stderr={} stdout={}",
        init.stderr, init.stdout
    );
    let descriptor = temp.path().join(".mds/descriptors/languages/gleam.toml");
    assert!(descriptor.exists(), "missing {}", descriptor.display());
    let content = fs::read_to_string(&descriptor).unwrap();
    assert!(content.contains("id = \"gleam\""));
    assert!(content.contains("[files.source]"));

    let check = run_mds(temp.path(), &["descriptor", "check", "--package", "."]);
    assert_eq!(
        check.code, 0,
        "stderr={} stdout={}",
        check.stderr, check.stdout
    );
    assert!(check.stdout.contains("descriptor check ok"));

    let overwrite = run_mds(
        temp.path(),
        &["init", "descriptor", "language", "Gleam", "--yes"],
    );
    assert_eq!(
        overwrite.code, 1,
        "stderr={} stdout={}",
        overwrite.stderr, overwrite.stdout
    );
    assert!(overwrite.stderr.contains("descriptor file already exists"));
}

#[test]
fn descriptor_check_reports_parse_required_field_and_collision_errors() {
    let temp = TestDir::new("mds-cli-main-descriptor-errors");
    temp.write(
        ".mds/descriptors/languages/a.toml",
        concat!(
            "id = \"dup\"\n",
            "match_suffixes = [\"dup\"]\n",
            "\n",
            "[language]\n",
            "primary_ext = \"dup\"\n",
            "\n",
            "[files.source]\n",
            "extension = \"dup\"\n",
            "\n",
            "[files.test]\n",
            "extension = \"dup\"\n"
        ),
    );
    temp.write(
        ".mds/descriptors/languages/b.toml",
        concat!(
            "id = \"other\"\n",
            "aliases = [\"dup\"]\n",
            "match_suffixes = [\"dup\"]\n",
            "\n",
            "[language]\n",
            "primary_ext = \"dup\"\n",
            "\n",
            "[files.source]\n",
            "extension = \"dup\"\n",
            "\n",
            "[files.test]\n",
            "extension = \"dup\"\n"
        ),
    );
    temp.write(".mds/descriptors/tools/bad.toml", "id = \"bad\"\n");
    temp.write(".mds/descriptors/package-managers/broken.toml", "id = \n");
    temp.write(".mds/descriptor-sources.lock", "[broken\n");

    let result = run_mds(temp.path(), &["descriptor", "check", "--package", "."]);
    assert_eq!(
        result.code, 1,
        "stderr={} stdout={}",
        result.stderr, result.stdout
    );
    assert!(result.stderr.contains("descriptor alias `dup` collides"));
    assert!(result
        .stderr
        .contains("language match suffix `dup` collides"));
    assert!(result
        .stderr
        .contains("descriptor requires non-empty string array `match_prefixes`"));
    assert!(result.stderr.contains("failed to parse descriptor TOML"));
    assert!(result
        .stderr
        .contains("failed to parse descriptor source lock"));
}

#[test]
fn usage_errors_return_exit_code_two_and_render_usage() {
    let temp = TestDir::new("mds-cli-main-usage");
    for format in ["text", "json"] {
        let result = run_mds(temp.path(), &["build", "--format", format]);

        assert_eq!(
            result.code, 2,
            "format={format} stderr={} stdout={}",
            result.stderr, result.stdout
        );
        assert!(
            result
                .stderr
                .contains("error: build only accepts --package, --verbose, and --dry-run"),
            "format={format} stderr={}",
            result.stderr
        );
        assert!(
            result
                .stderr
                .contains("mds build [--package <path>] [--dry-run]"),
            "format={format} stderr={}",
            result.stderr
        );
    }
}

#[test]
fn new_command_surface_constraints_return_usage_error_exit_code_two() {
    let temp = TestDir::new("mds-cli-main-new-usage");
    write_ts_descriptor(&temp);
    let cases: &[(&[&str], &str)] = &[
        (
            &["new", "../greet.ts.md", "impl"],
            "new path must stay within the package root",
        ),
        (
            &["new", "greet.ts.md", "root-module"],
            "root-module kind requires a recognized root module Markdown name",
        ),
    ];

    for (args, expected) in cases {
        let result = run_mds(temp.path(), args);
        assert_eq!(
            result.code, 2,
            "args={args:?} stderr={} stdout={}",
            result.stderr, result.stdout
        );
        assert!(
            result.stderr.contains(expected),
            "args={args:?} stderr={}",
            result.stderr
        );
        assert!(
            result.stderr.contains("mds new <path> <kind> [options]"),
            "args={args:?} stderr={}",
            result.stderr
        );
    }
}

#[test]
fn new_uses_package_local_descriptor_context_before_execute() {
    let temp = TestDir::new("mds-cli-main-new-package-root");
    write_extensionless_root_module_descriptor(&temp, "pkg");

    let result = run_mds(
        temp.path(),
        &["new", "index.d.ts.md", "root-module", "--package", "pkg"],
    );

    assert_eq!(
        result.code, 0,
        "stderr={} stdout={}",
        result.stderr, result.stdout
    );
    assert!(result.stdout.contains("created"));
    let created = temp.path().join("pkg/.mds/source/index.d.ts.md");
    assert!(created.exists(), "missing {}", created.display());
    let content = fs::read_to_string(created).unwrap();
    assert!(content.contains("## API"));
    assert!(!content.contains("## Source"));
}

#[test]
fn command_environment_and_internal_failures_map_to_distinct_exit_codes() {
    let command_failure = TestDir::new("mds-cli-main-command-failure");
    write_ts_descriptor(&command_failure);
    command_failure.write(".mds/source/existing.ts.md", "# unmanaged\n");
    let command_failure_result =
        run_mds(command_failure.path(), &["new", "existing.ts.md", "impl"]);
    assert_eq!(
        command_failure_result.code, 1,
        "stderr={} stdout={}",
        command_failure_result.stderr, command_failure_result.stdout
    );
    assert!(command_failure_result
        .stderr
        .contains("file already exists and is not mds-managed"));

    let environment_failure = TestDir::new("mds-cli-main-environment-failure");
    write_node_package(&environment_failure, "cli-main-environment-failure");
    write_npm_package_manager_descriptor(&environment_failure);
    environment_failure.write(
        "mds.config.toml",
        concat!(
            "[package]\n",
            "enabled = true\n",
            "\n",
            "[quality.ts]\n",
            "required = [\"/missing/mds-doctor-tool\"]\n",
            "optional = []\n"
        ),
    );
    let environment_failure_result =
        run_mds(environment_failure.path(), &["doctor", "--package", "."]);
    assert_eq!(
        environment_failure_result.code, 4,
        "stderr={} stdout={}",
        environment_failure_result.stderr, environment_failure_result.stdout
    );
    assert!(environment_failure_result
        .stderr
        .contains("DOCTOR001_TOOLCHAIN_MISSING"));

    let internal_failure = TestDir::new("mds-cli-main-internal-failure");
    write_ts_descriptor(&internal_failure);
    internal_failure.write(".mds/source/blocked", "not-a-directory\n");
    let internal_failure_result = run_mds(
        internal_failure.path(),
        &["new", "blocked/greet.ts.md", "impl"],
    );
    assert_eq!(
        internal_failure_result.code, 3,
        "stderr={} stdout={}",
        internal_failure_result.stderr, internal_failure_result.stdout
    );
    assert!(internal_failure_result
        .stderr
        .contains("internal error: failed to create"));
}

#[test]
fn update_same_version_exits_zero_without_package_context() {
    let temp = TestDir::new("mds-cli-main-update");
    let result = run_mds(
        temp.path(),
        &["update", "--version", env!("CARGO_PKG_VERSION")],
    );

    assert_eq!(
        result.code, 0,
        "stderr={} stdout={}",
        result.stderr, result.stdout
    );
    assert!(result.stdout.contains("already at version"));
}

#[test]
fn update_latest_path_fetches_tag_name_and_runs_install_script() {
    let temp = TestDir::new("mds-cli-main-update-latest");
    write_fake_update_curl(&temp, Some("{\"tag_name\":\"v9.8.7\"}"), 0, 0);

    let result = run_mds_with_env(
        temp.path(),
        &["update"],
        vec![("PATH", prepend_path(&temp.path().join("bin")))],
    );

    assert_eq!(
        result.code, 0,
        "stderr={} stdout={}",
        result.stderr, result.stdout
    );
    assert!(result.stderr.contains("Checking for latest version..."));
    assert!(result.stdout.contains("Updating mds from"));
    assert!(
        result.stdout.contains("to 9.8.7"),
        "stdout={}",
        result.stdout
    );
    assert!(
        !result.stdout.contains("to v9.8.7"),
        "stdout={}",
        result.stdout
    );
    assert!(result.stdout.contains("Successfully updated to mds 9.8.7"));
}

#[test]
fn update_latest_fetch_failure_includes_manual_recovery_hint() {
    let temp = TestDir::new("mds-cli-main-update-fetch-failure");
    write_fake_update_curl(&temp, None, 28, 0);

    let result = run_mds_with_env(
        temp.path(),
        &["update"],
        vec![("PATH", prepend_path(&temp.path().join("bin")))],
    );

    assert_eq!(
        result.code, 1,
        "stderr={} stdout={}",
        result.stderr, result.stdout
    );
    assert!(result.stderr.contains("Checking for latest version..."));
    assert!(
        result
            .stderr
            .contains("error: failed to fetch latest version from GitHub"),
        "stderr={}",
        result.stderr
    );
    assert!(
		result
			.stderr
			.contains("hint: Check https://github.com/owo-x-project/owox-mds/releases for a version, then run:"),
		"stderr={}",
		result.stderr
	);
    assert!(
		result
			.stderr
			.contains("curl -fsSL https://raw.githubusercontent.com/owo-x-project/owox-mds/latest/install.sh | sh -s -- --version <version>"),
		"stderr={}",
		result.stderr
	);
}

#[test]
fn update_install_script_non_zero_includes_manual_recovery_hint() {
    let temp = TestDir::new("mds-cli-main-update-install-failure");
    write_fake_update_curl(&temp, Some("{\"tag_name\":\"v9.8.7\"}"), 0, 17);

    let result = run_mds_with_env(
        temp.path(),
        &["update"],
        vec![("PATH", prepend_path(&temp.path().join("bin")))],
    );

    assert_eq!(
        result.code, 1,
        "stderr={} stdout={}",
        result.stderr, result.stdout
    );
    assert!(
        result.stderr.contains("Update failed with exit code: 17"),
        "stderr={}",
        result.stderr
    );
    assert!(
        result.stderr.contains("hint: Retry manually with:"),
        "stderr={}",
        result.stderr
    );
    assert!(
		result
			.stderr
			.contains("curl -fsSL https://raw.githubusercontent.com/owo-x-project/owox-mds/latest/install.sh | sh -s -- --version 9.8.7"),
		"stderr={}",
		result.stderr
	);
}

#[test]
fn update_explicit_version_treats_shell_metacharacters_as_literal_arguments() {
    let temp = TestDir::new("mds-cli-main-update-injection");
    let injected = temp.path().join("injected.txt");
    let install_args = temp.path().join("install-args.txt");
    write_fake_update_curl_with_install_script(
        &temp,
        None,
        0,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$MDS_TEST_SH_ARGS\"\nexit 0\n",
    );
    let version = format!("9.8.7; printf pwned > {}; #", injected.display());

    let result = run_mds_with_env(
        temp.path(),
        &["update", "--version", &version],
        vec![
            ("PATH", prepend_path(&temp.path().join("bin"))),
            ("MDS_TEST_SH_ARGS", install_args.display().to_string()),
        ],
    );

    assert_eq!(
        result.code, 0,
        "stderr={} stdout={}",
        result.stderr, result.stdout
    );
    assert!(!injected.exists(), "command injection marker created");
    assert_eq!(
        fs::read_to_string(&install_args).unwrap(),
        format!("--version\n{version}\n")
    );
    assert!(result.stdout.contains("Successfully updated to mds"));
}
