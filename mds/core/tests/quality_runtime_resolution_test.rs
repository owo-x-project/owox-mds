use mds_core::{execute, CliRequest, Command};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

const TS_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/languages/ts.toml");
const NPM_PACKAGE_MANAGER: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/package-managers/npm.toml");

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
static PATH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mds-core-quality-runtime-{name}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct PathGuard {
    old: Option<OsString>,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        if let Some(old) = self.old.take() {
            env::set_var("PATH", old);
        } else {
            env::remove_var("PATH");
        }
    }
}

fn path_guard(dir: &Path) -> PathGuard {
    let lock = PATH_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let old = env::var_os("PATH");
    let mut paths = vec![dir.to_path_buf()];
    if let Some(current) = &old {
        paths.extend(env::split_paths(current));
    }
    let joined = env::join_paths(paths).unwrap();
    env::set_var("PATH", joined);
    PathGuard { old, _lock: lock }
}

fn write_tool(root: &Path, name: &str, script: &str) -> PathBuf {
    let path = root.join("bin").join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, script).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

fn write_package(package: &Path, package_json: &str, extra_config: &str) {
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::write(package.join("package.json"), package_json).unwrap();
    fs::create_dir_all(package.join(".mds/descriptors/languages")).unwrap();
    fs::create_dir_all(package.join(".mds/descriptors/package-managers")).unwrap();
    fs::write(
        package.join(".mds/descriptors/languages/ts.toml"),
        TS_DESCRIPTOR,
    )
    .unwrap();
    fs::write(
        package.join(".mds/descriptors/package-managers/npm.toml"),
        NPM_PACKAGE_MANAGER,
    )
    .unwrap();
    fs::write(
        package.join("mds.config.toml"),
        format!("[package]\nenabled = true\nallow_raw_source = false\n\n{extra_config}"),
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
}

fn write_schema_source(package: &Path, name: &str) -> PathBuf {
    let path = package.join(format!(".mds/source/foo/{name}.schema.md"));
    fs::write(
        &path,
        "# Feature\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Preserve runtime behavior.\n\n## Source\n\n```schema\nvalue = 1\n```\n",
    )
    .unwrap();
    path
}

fn write_ts_source(package: &Path, name: &str) -> PathBuf {
    let path = package.join(format!(".mds/source/foo/{name}.ts.md"));
    fs::write(
        &path,
        "# Feature\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Preserve runtime behavior.\n\n## Source\n\n```ts\nexport const value = 1;\n```\n",
    )
    .unwrap();
    path
}

fn write_schema_manager(package: &Path, body: &str) {
    let path = package.join(".mds/descriptors/package-managers/schema-pm.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn write_schema_descriptor(package: &Path, body: &str) {
    let path = package.join(".mds/descriptors/languages/schema-lang.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn rewrite_package_manager_manifest_lang(root: &Path, manager_id: &str, from: &str, to: &str) {
    let path = root.join(format!(
        ".mds/descriptors/package-managers/{manager_id}.toml"
    ));
    let current = fs::read_to_string(&path).unwrap();
    let updated = current.replace(&format!("lang = \"{from}\""), &format!("lang = \"{to}\""));
    assert_ne!(
        current,
        updated,
        "failed to rewrite {manager_id} lang in {}",
        path.display()
    );
    fs::write(path, updated).unwrap();
}

fn write_tool_manifest(package: &Path, body: &str) {
    let path = package.join(".mds/descriptors/tools/shared-lint.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn run_lint(package: &Path) -> mds_core::CliResult {
    execute(CliRequest {
        cwd: package.to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    })
}

fn run_fix(package: &Path) -> mds_core::CliResult {
    execute(CliRequest {
        cwd: package.to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    })
}

#[test]
fn lint_writes_append_file_arg_input_without_markdown_padding() {
    let temp = TestDir::new("append-file-arg-input");
    let package = temp.path().join("pkg");
    let tool = write_tool(
        temp.path(),
        "assert-ts-file-input",
        "#!/bin/sh\n[ -f \"$1\" ] || { printf '%s:1:1: missing temp file\\n' \"$1\" >&2; exit 1; }\nfirst_line=$(sed -n '1p' \"$1\")\n[ -n \"$first_line\" ] || { printf '%s:1:1: leading blank line\\n' \"$1\" >&2; exit 1; }\ngrep -q 'export const value = 1;' \"$1\" || { printf '%s:1:1: missing source content\\n' \"$1\" >&2; exit 1; }\nexit 0\n",
    );
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
        &format!(
            "[quality.ts]\nlinter = \"{}\"\nrequired = []\noptional = []\n",
            tool.display()
        ),
    );
    write_ts_source(&package, "feature");

    let result = run_lint(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
}

#[test]
fn lint_uses_descriptor_default_when_slot_is_unset() {
    let temp = TestDir::new("descriptor-default");
    let package = temp.path().join("pkg");
    let tool = write_tool(
        temp.path(),
        "descriptor-default-lint",
        "#!/bin/sh\nprintf 'descriptor default\n' > \"$PWD/descriptor-default.txt\"\ncat \"$1\" >/dev/null\nexit 0\n",
    );
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"packageManager\":\"schema-pm@1.0.0\"}\n",
        "",
    );
    write_schema_manager(
        &package,
        "id = \"schema-pm\"\ndisplay_name = \"Schema PM\"\nlang = \"schema-lang\"\nmetadata_files = [\"package.json\"]\nmetadata_reader = \"node-package-json\"\n",
    );
    write_schema_descriptor(
        &package,
        &format!(
            concat!(
                "id = \"schema-lang\"\n",
                "match_suffixes = [\"schema\"]\n\n",
                "[language]\n",
                "primary_ext = \"schema\"\n",
                "root_module_markdown_names = [\"index.schema.md\"]\n\n",
                "[files.source]\n",
                "strip_lang_ext = false\n",
                "prefix = \"\"\n",
                "suffix = \"\"\n",
                "extension = \"schema\"\n\n",
                "[files.test]\n",
                "strip_lang_ext = true\n",
                "prefix = \"\"\n",
                "suffix = \".test\"\n",
                "extension = \"schema\"\n\n",
                "[quality_defaults]\n",
                "lint = \"{}\"\n\n",
                "[tooling.lint]\n",
                "input = \"tempfile\"\n",
                "append_file_arg = true\n",
            ),
            tool.display(),
        ),
    );
    write_schema_source(&package, "feature");

    let result = run_lint(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        package.join("descriptor-default.txt").exists(),
        "{}",
        result.stdout
    );
}

#[test]
fn lint_uses_package_manager_command_before_descriptor_default() {
    let temp = TestDir::new("manager-command");
    let package = temp.path().join("pkg");
    let manager_tool = write_tool(
        temp.path(),
        "manager-command-lint",
        "#!/bin/sh\nprintf 'manager command\n' > \"$PWD/manager-command.txt\"\ncat \"$1\" >/dev/null\nexit 0\n",
    );
    let default_tool = write_tool(
        temp.path(),
        "descriptor-fallback-lint",
        "#!/bin/sh\nprintf 'descriptor fallback\n' > \"$PWD/descriptor-fallback.txt\"\ncat \"$1\" >/dev/null\nexit 0\n",
    );
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"packageManager\":\"schema-pm@1.0.0\"}\n",
        "",
    );
    write_schema_manager(
        &package,
        &format!(
            concat!(
                "id = \"schema-pm\"\n",
                "display_name = \"Schema PM\"\n",
                "lang = \"schema-lang\"\n",
                "metadata_files = [\"package.json\"]\n",
                "metadata_reader = \"node-package-json\"\n\n",
                "[commands]\n",
                "lint = \"{}\"\n",
            ),
            manager_tool.display(),
        ),
    );
    write_schema_descriptor(
        &package,
        &format!(
            concat!(
                "id = \"schema-lang\"\n",
                "match_suffixes = [\"schema\"]\n\n",
                "[language]\n",
                "primary_ext = \"schema\"\n",
                "root_module_markdown_names = [\"index.schema.md\"]\n\n",
                "[files.source]\n",
                "strip_lang_ext = false\n",
                "prefix = \"\"\n",
                "suffix = \"\"\n",
                "extension = \"schema\"\n\n",
                "[files.test]\n",
                "strip_lang_ext = true\n",
                "prefix = \"\"\n",
                "suffix = \".test\"\n",
                "extension = \"schema\"\n\n",
                "[quality_defaults]\n",
                "lint = \"{}\"\n\n",
                "[tooling.lint]\n",
                "input = \"tempfile\"\n",
                "append_file_arg = true\n",
            ),
            default_tool.display(),
        ),
    );
    write_schema_source(&package, "feature");

    let result = run_lint(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        package.join("manager-command.txt").exists(),
        "{}",
        result.stdout
    );
    assert!(
        !package.join("descriptor-fallback.txt").exists(),
        "{}",
        result.stdout
    );
}

#[test]
fn lint_uses_package_manager_command_when_manifest_lang_is_descriptor_alias() {
    let temp = TestDir::new("manager-command-alias-lang");
    let package = temp.path().join("pkg");
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
        "",
    );
    rewrite_package_manager_manifest_lang(&package, "npm", "ts", "typescript");
    write_ts_source(&package, "feature");
    write_tool(
        temp.path(),
        "npm",
        "#!/bin/sh\nprintf 'npm command\n' > \"$PWD/npm-command.txt\"\nexit 0\n",
    );
    write_tool(
        temp.path(),
        "eslint",
        "#!/bin/sh\nprintf 'descriptor default\n' > \"$PWD/descriptor-default.txt\"\nexit 0\n",
    );

    let _guard = path_guard(&temp.path().join("bin"));
    let result = run_lint(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        package.join("npm-command.txt").exists(),
        "{}",
        result.stdout
    );
    assert!(
        !package.join("descriptor-default.txt").exists(),
        "{}",
        result.stdout
    );
}

#[test]
fn lint_does_not_use_descriptor_default_when_active_manager_language_mismatches_doc() {
    let temp = TestDir::new("manager-lang-mismatch");
    let package = temp.path().join("pkg");
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"packageManager\":\"schema-pm@1.0.0\"}\n",
        "",
    );
    write_schema_manager(
        &package,
        "id = \"schema-pm\"\ndisplay_name = \"Schema PM\"\nlang = \"schema-lang\"\nmetadata_files = [\"package.json\"]\nmetadata_reader = \"node-package-json\"\n",
    );
    write_ts_source(&package, "feature");
    write_tool(
        temp.path(),
        "eslint",
        "#!/bin/sh\nprintf 'descriptor default\n' > \"$PWD/manager-mismatch-eslint.txt\"\nexit 0\n",
    );

    let _guard = path_guard(&temp.path().join("bin"));
    let result = run_lint(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        !package.join("manager-mismatch-eslint.txt").exists(),
        "{}",
        result.stdout
    );
}

#[test]
fn fix_uses_package_manager_script_when_slot_is_unset() {
    let temp = TestDir::new("script-fallback");
    let package = temp.path().join("pkg");
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"scripts\":{\"format\":\"fake-format\"}}\n",
        "",
    );
    write_ts_source(&package, "feature");
    write_tool(
        temp.path(),
        "npm",
        "#!/bin/sh\nif [ \"$1\" = \"run\" ] && [ \"$2\" = \"format\" ]; then\nprintf 'script fallback\n' > \"$PWD/npm-format.txt\"\nexit 0\nfi\nprintf 'unexpected npm args: %s %s %s\n' \"$1\" \"$2\" \"$3\" >&2\nexit 1\n",
    );

    let _guard = path_guard(&temp.path().join("bin"));
    let result = run_fix(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(package.join("npm-format.txt").exists(), "{}", result.stdout);
}

#[test]
fn explicit_false_disables_runtime_fallback() {
    let temp = TestDir::new("explicit-false");
    let package = temp.path().join("pkg");
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"scripts\":{\"format\":\"fake-format\"}}\n",
        "[quality.ts]\nfixer = false\nrequired = []\noptional = []\n",
    );
    write_ts_source(&package, "feature");
    write_tool(
        temp.path(),
        "npm",
        "#!/bin/sh\nprintf 'should not run\n' > \"$PWD/npm-format.txt\"\nexit 0\n",
    );

    let _guard = path_guard(&temp.path().join("bin"));
    let result = run_fix(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        !package.join("npm-format.txt").exists(),
        "{}",
        result.stdout
    );
}

#[test]
fn explicit_default_slot_behavior_stops_tool_manifest_fallback() {
    let temp = TestDir::new("explicit-default-slot-behavior");
    let package = temp.path().join("pkg");
    let tool = write_tool(
        temp.path(),
        "shared-fix-default-contract",
        "#!/bin/sh\nprintf 'export const value = 2;\n'\nexit 0\n",
    );
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"packageManager\":\"schema-pm@1.0.0\"}\n",
        &format!(
            "[quality.schema-lang]\nfixer = \"{}\"\nrequired = []\noptional = []\n",
            tool.display()
        ),
    );
    write_schema_manager(
        &package,
        "id = \"schema-pm\"\ndisplay_name = \"Schema PM\"\nlang = \"schema-lang\"\nmetadata_files = [\"package.json\"]\nmetadata_reader = \"node-package-json\"\n",
    );
    write_schema_descriptor(
        &package,
        concat!(
            "id = \"schema-lang\"\n",
            "match_suffixes = [\"schema\"]\n\n",
            "[language]\n",
            "primary_ext = \"schema\"\n",
            "root_module_markdown_names = [\"index.schema.md\"]\n\n",
            "[files.source]\n",
            "strip_lang_ext = false\n",
            "prefix = \"\"\n",
            "suffix = \"\"\n",
            "extension = \"schema\"\n\n",
            "[files.test]\n",
            "strip_lang_ext = true\n",
            "prefix = \"\"\n",
            "suffix = \".test\"\n",
            "extension = \"schema\"\n\n",
            "[tooling.fix]\n",
            "input = \"tempfile\"\n",
            "output = \"none\"\n",
        ),
    );
    write_tool_manifest(
        &package,
        concat!(
            "id = \"shared-fix-default-contract\"\n",
            "match_prefixes = [\"shared-fix-default-contract\"]\n\n",
            "[behavior]\n",
            "output = \"stdout\"\n",
        ),
    );
    let markdown_path = write_schema_source(&package, "feature");
    let before = fs::read_to_string(&markdown_path).unwrap();

    let result = run_fix(&package);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let after = fs::read_to_string(&markdown_path).unwrap();
    assert_eq!(after, before, "{}", result.stdout);
}

#[test]
fn lint_diagnostics_prefer_descriptor_slot_behavior_over_tool_manifest() {
    let temp = TestDir::new("capture-precedence");
    let package = temp.path().join("pkg");
    let tool = write_tool(
        temp.path(),
        "shared-lint",
        "#!/bin/sh\nprintf 'slot:1:descriptor-owned\n' >&2\nexit 1\n",
    );
    write_package(
        &package,
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"packageManager\":\"schema-pm@1.0.0\"}\n",
        &format!(
            "[quality.schema-lang]\nlinter = \"{}\"\nrequired = []\noptional = []\n",
            tool.display()
        ),
    );
    let markdown_path = write_schema_source(&package, "feature");
    write_schema_manager(
        &package,
        "id = \"schema-pm\"\ndisplay_name = \"Schema PM\"\nlang = \"schema-lang\"\nmetadata_files = [\"package.json\"]\nmetadata_reader = \"node-package-json\"\n",
    );
    write_schema_descriptor(
        &package,
        concat!(
            "id = \"schema-lang\"\n",
            "match_suffixes = [\"schema\"]\n\n",
            "[language]\n",
            "primary_ext = \"schema\"\n",
            "root_module_markdown_names = [\"index.schema.md\"]\n\n",
            "[files.source]\n",
            "strip_lang_ext = false\n",
            "prefix = \"\"\n",
            "suffix = \"\"\n",
            "extension = \"schema\"\n\n",
            "[files.test]\n",
            "strip_lang_ext = true\n",
            "prefix = \"\"\n",
            "suffix = \".test\"\n",
            "extension = \"schema\"\n\n",
            "[tooling.lint]\n",
            "input = \"tempfile\"\n",
            "append_file_arg = true\n\n",
            "[[tooling.lint.diagnostics]]\n",
            "pattern = '^slot:(?P<line>\\d+):(?P<message>.+)$'\n",
        ),
    );
    write_tool_manifest(
        &package,
        concat!(
            "id = \"shared-lint\"\n",
            "match_prefixes = [\"shared-lint\"]\n\n",
            "[behavior]\n",
            "input = \"tempfile\"\n",
            "append_file_arg = true\n\n",
            "[[behavior.diagnostics]]\n",
            "pattern = '^slot:(?P<line>\\d+):(?P<message>.+)$'\n",
            "severity = \"warning\"\n",
        ),
    );

    let result = run_lint(&package);
    assert_eq!(
        result.exit_code, 1,
        "stdout:\n{}\nstderr:\n{}",
        result.stdout, result.stderr
    );
    assert!(result.stderr.contains("error:"), "{}", result.stderr);
    assert!(!result.stderr.contains("warning:"), "{}", result.stderr);
    assert!(
        result
            .stderr
            .contains(markdown_path.to_string_lossy().as_ref()),
        "{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("descriptor-owned"),
        "{}",
        result.stderr
    );
}
