use mds_core::descriptor;
use mds_core::package;
use mds_core::{
    detect_init_quality_toolchains, execute, CliRequest, Command, Config, InitOptions,
    InitQualitySource, Lang, RunState,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const TS_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/languages/ts.toml");
const MINIMAL_TS_PACKAGE_JSON: &str = include_str!("../../../examples/minimal-ts/package.json");
const NPM_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/package-managers/npm.toml");
const PNPM_DESCRIPTOR: &str = r#"id = "pnpm"
display_name = "Node.js (pnpm)"
lang = "ts"
metadata_files = ["package.json"]
lockfiles = ["pnpm-lock.yaml"]
metadata_reader = "node-package-json"

[commands]
install = "pnpm install"
build = "pnpm build"
typecheck = "pnpm typecheck"
lint = "pnpm lint"
test = "pnpm test"
"#;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mds-core-init-quality-{name}-{}-{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        seed_node_runtime_contract(&path);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn seed_node_runtime_contract(root: &Path) {
    std::fs::create_dir_all(root.join(".mds/descriptors/languages")).unwrap();
    std::fs::create_dir_all(root.join(".mds/descriptors/package-managers")).unwrap();
    std::fs::write(
        root.join(".mds/descriptors/languages/ts.toml"),
        TS_DESCRIPTOR,
    )
    .unwrap();
    std::fs::write(
        root.join(".mds/descriptors/package-managers/npm.toml"),
        NPM_DESCRIPTOR,
    )
    .unwrap();
    std::fs::write(
        root.join(".mds/descriptors/package-managers/pnpm.toml"),
        PNPM_DESCRIPTOR,
    )
    .unwrap();
}

fn write_node_package(dir: &TestDir, package_manager: Option<&str>) {
    let package_manager_field = package_manager
        .map(|value| format!(",\n  \"packageManager\": \"{value}\""))
        .unwrap_or_default();
    dir.write(
        "package.json",
        &format!(
            "{{\n  \"name\": \"quality-test\",\n  \"version\": \"0.1.0\"{package_manager_field}\n}}\n"
        ),
    );
}

fn rewrite_package_manager_manifest_lang(root: &Path, manager_id: &str, from: &str, to: &str) {
    let path = root.join(format!(
        ".mds/descriptors/package-managers/{manager_id}.toml"
    ));
    let current = std::fs::read_to_string(&path).unwrap();
    let updated = current.replace(&format!("lang = \"{from}\""), &format!("lang = \"{to}\""));
    assert_ne!(
        current,
        updated,
        "failed to rewrite {manager_id} lang in {}",
        path.display()
    );
    std::fs::write(path, updated).unwrap();
}

fn minimal_language_descriptor(id: &str, alias: &str, suffix: &str, lint: &str) -> String {
    format!(
        r#"id = "{id}"
aliases = ["{alias}"]
match_suffixes = ["{suffix}"]

[language]
primary_ext = "{suffix}"

[files.source]
extension = "{suffix}"

[files.test]
strip_lang_ext = true
suffix = ".test"
extension = "{suffix}"

[quality_defaults]
lint = "{lint}"
"#
    )
}

fn minimal_package_manager_descriptor(id: &str, lang: &str, metadata: &str) -> String {
    format!(
        r#"id = "{id}"
display_name = "{id}"
lang = "{lang}"
metadata_files = ["{metadata}"]
metadata_reader = "plain-text"
"#
    )
}

fn minimal_tool_descriptor(id: &str, prefix: &str) -> String {
    format!(
        r#"id = "{id}"
match_prefixes = ["{prefix}"]

[behavior]
input = "file"
output = "diagnostics"
"#
    )
}

fn init_setup_plan_stdout(dir: &TestDir) -> String {
    let result = execute(CliRequest {
        cwd: dir.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                install_project_deps: true,
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    result.stdout
}

#[test]
fn descriptor_source_local_pack_resolves_language_and_package_manager_with_origin() {
    let temp = TestDir::new("descriptor-source-local-pack");
    std::fs::remove_dir_all(temp.path().join(".mds/descriptors")).unwrap();
    temp.write(
        "packs/base/languages/schema.toml",
        &minimal_language_descriptor("schema-lang", "schema", "schema", "schema lint"),
    );
    temp.write(
        "packs/base/package-managers/schema-pm.toml",
        &minimal_package_manager_descriptor("schema-pm", "schema-lang", "schema.pkg"),
    );
    temp.write("schema.pkg", "name = \"schema\"\n");
    temp.write(
        ".mds/descriptor-sources.toml",
        "[[sources]]\nid = \"base\"\ntype = \"local\"\npath = \"packs/base\"\n",
    );

    let report = descriptor::descriptor_registry_report(Some(temp.path()));
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report
        .entries
        .iter()
        .any(|entry| { entry.id == "schema-lang" && entry.origin.source_id == "base" }));
    let origin = descriptor::with_workspace_descriptor_root(Some(temp.path()), || {
        descriptor::descriptor_origin_for_key("schema")
    })
    .expect("schema origin should resolve through alias");
    assert_eq!(origin.source_id, "base");
    let quality = descriptor::with_workspace_descriptor_root(Some(temp.path()), || {
        descriptor::default_quality_commands_for_lang(&Lang::Other("schema-lang".to_string()))
    });
    assert_eq!(quality.lint.as_deref(), Some("schema lint"));
    assert_eq!(
        descriptor::detect_package_manager(temp.path())
            .expect("schema-pm should resolve from descriptor source")
            .id,
        "schema-pm"
    );
    let pm_origin = descriptor::with_workspace_descriptor_root(Some(temp.path()), || {
        descriptor::package_manager_origin_for_id("schema-pm")
    })
    .expect("schema-pm origin should resolve");
    assert_eq!(pm_origin.source_id, "base");
}

#[test]
fn descriptor_package_local_overrides_source_pack_descriptor_id() {
    let temp = TestDir::new("descriptor-package-local-override");
    std::fs::remove_dir_all(temp.path().join(".mds/descriptors")).unwrap();
    temp.write(
        "packs/base/languages/schema.toml",
        &minimal_language_descriptor("schema-lang", "schema-source", "schema", "source lint"),
    );
    temp.write(
        ".mds/descriptors/languages/schema.toml",
        &minimal_language_descriptor("schema-lang", "schema-local", "schema", "local lint"),
    );
    temp.write(
        ".mds/descriptor-sources.toml",
        "[[sources]]\nid = \"base\"\ntype = \"local\"\npath = \"packs/base\"\n",
    );

    let report = descriptor::descriptor_registry_report(Some(temp.path()));
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    let schema_entry = report
        .entries
        .iter()
        .find(|entry| entry.id == "schema-lang")
        .expect("schema-lang origin should be reported");
    assert_eq!(schema_entry.origin.source_id, "package-local");
    let quality = descriptor::with_workspace_descriptor_root(Some(temp.path()), || {
        descriptor::default_quality_commands_for_lang(&Lang::Other("schema-lang".to_string()))
    });
    assert_eq!(quality.lint.as_deref(), Some("local lint"));
}

#[test]
fn descriptor_source_reports_malformed_config_and_lock() {
    let config = TestDir::new("descriptor-source-malformed-config");
    std::fs::remove_dir_all(config.path().join(".mds/descriptors")).unwrap();
    config.write(".mds/descriptor-sources.toml", "[[sources]\nid = \"bad\"\n");

    let config_report = descriptor::descriptor_registry_report(Some(config.path()));
    assert!(config_report.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("failed to parse descriptor source config")));

    let lock = TestDir::new("descriptor-source-malformed-lock");
    std::fs::remove_dir_all(lock.path().join(".mds/descriptors")).unwrap();
    lock.write(".mds/descriptor-sources.lock", "[[sources]\nid = \"bad\"\n");

    let lock_report = descriptor::descriptor_registry_report(Some(lock.path()));
    assert!(lock_report.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("failed to parse descriptor source lock")));
}

#[test]
fn descriptor_source_reports_alias_id_suffix_and_prefix_collisions() {
    let temp = TestDir::new("descriptor-source-collisions");
    std::fs::remove_dir_all(temp.path().join(".mds/descriptors")).unwrap();
    temp.write(
        "packs/one/languages/duplicate.toml",
        &minimal_language_descriptor("duplicate", "dup-one", "dupone", "one lint"),
    );
    temp.write(
        "packs/two/languages/duplicate.toml",
        &minimal_language_descriptor("duplicate", "dup-two", "duptwo", "two lint"),
    );
    temp.write(
        "packs/one/languages/first.toml",
        &minimal_language_descriptor("first", "shared", "shared", "first lint"),
    );
    temp.write(
        "packs/two/languages/second.toml",
        &minimal_language_descriptor("second", "shared", "shared", "second lint"),
    );
    temp.write(
        "packs/one/tools/eslint.toml",
        &minimal_tool_descriptor("eslint-a", "eslint"),
    );
    temp.write(
        "packs/two/tools/eslint.toml",
        &minimal_tool_descriptor("eslint-b", "eslint"),
    );
    temp.write(
        ".mds/descriptor-sources.toml",
        concat!(
            "[[sources]]\n",
            "id = \"one\"\n",
            "type = \"local\"\n",
            "path = \"packs/one\"\n\n",
            "[[sources]]\n",
            "id = \"two\"\n",
            "type = \"local\"\n",
            "path = \"packs/two\"\n",
        ),
    );

    let report = descriptor::descriptor_registry_report(Some(temp.path()));
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("language descriptor id `duplicate` collision")));
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("language descriptor alias `shared` collision")));
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("tool descriptor command prefix `eslint` collision")));
    let tool_origin = descriptor::with_workspace_descriptor_root(Some(temp.path()), || {
        descriptor::tool_origin_for_command("eslint .")
    })
    .expect("tool origin should resolve despite collision diagnostics");
    assert_eq!(tool_origin.source_id, "one");
}

fn init_toolchain_setup_plan_stdout(dir: &TestDir) -> String {
    let result = execute(CliRequest {
        cwd: dir.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                install_toolchains: true,
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    result.stdout
}

#[test]
fn load_package_reports_malformed_workspace_descriptors() {
    let dir = TestDir::new("malformed-descriptors");
    write_node_package(&dir, Some("npm@10.0.0"));
    dir.write("mds.config.toml", "[package]\nenabled = true\n");
    dir.write(
        ".mds/descriptors/languages/ts.toml",
        "id = \"ts\"\n[language\nprimary_ext = \"ts\"\n",
    );
    dir.write(
        ".mds/descriptors/package-managers/npm.toml",
        "id = \"npm\"\ndisplay_name = [\n",
    );
    dir.write(
        ".mds/descriptors/package-managers/pnpm.toml",
        "id = \"pnpm\"\ndisplay_name = [\n",
    );

    let mut state = RunState::default();
    let package = package::load_package(dir.path(), &Config::default(), &mut state);

    assert!(
        package.is_none(),
        "malformed npm manifest should prevent package detection"
    );
    assert!(
        state.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("failed to parse package manager manifest")),
        "package manager parse error should be reported: {:?}",
        state.diagnostics
    );
    assert!(
        state.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("failed to parse language descriptor")),
        "language descriptor parse error should be reported: {:?}",
        state.diagnostics
    );
}

#[test]
fn detect_init_quality_toolchains_uses_target_root_descriptor_and_package_manager_context() {
    let ambient = TestDir::new("ambient");
    let target = TestDir::new("target");
    target.write(
        ".mds/descriptors/package-managers/schema-pm.toml",
        r#"id = "schema-pm"
display_name = "Schema PM"
lang = "schema-lang"
metadata_files = ["schema.pkg"]
metadata_reader = "plain-text"
"#,
    );
    target.write(
        ".mds/descriptors/languages/schema-lang.toml",
        r#"id = "schema-lang"
match_suffixes = ["schema"]

[language]
primary_ext = "schema"

[files.source]
extension = "schema"

[files.test]
strip_lang_ext = true
suffix = ".test"
extension = "schema"

[quality_defaults]
fix = "schemafmt --write"
"#,
    );
    target.write("schema.pkg", "name = \"schema\"\n");

    let summaries = descriptor::with_workspace_descriptor_root(Some(ambient.path()), || {
        detect_init_quality_toolchains(target.path())
    });

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].lang, Lang::Other("schema-lang".to_string()));
    assert_eq!(summaries[0].display_name, "Schema PM");
    assert_eq!(summaries[0].metadata, "schema.pkg");
    assert_eq!(summaries[0].fix.source, InitQualitySource::Schema);
    assert_eq!(
        summaries[0].fix.command.as_deref(),
        Some("schemafmt --write")
    );
}

#[test]
fn detect_init_quality_toolchains_keeps_missing_schema_descriptor_non_fatal() {
    let ambient = TestDir::new("ambient-missing");
    let target = TestDir::new("target-missing");
    target.write(
        ".mds/descriptors/package-managers/missing-pm.toml",
        r#"id = "missing-pm"
display_name = "Missing PM"
lang = "missing-lang"
metadata_files = ["missing.pkg"]
metadata_reader = "plain-text"

[commands]
lint = "missingpm lint"
"#,
    );
    target.write("missing.pkg", "name = \"missing\"\n");

    let summaries = descriptor::with_workspace_descriptor_root(Some(ambient.path()), || {
        detect_init_quality_toolchains(target.path())
    });

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].lang, Lang::Other("missing-lang".to_string()));
    assert_eq!(summaries[0].lint.source, InitQualitySource::PackageManager);
    assert_eq!(summaries[0].lint.command.as_deref(), Some("missingpm lint"));
    assert_eq!(summaries[0].fix.source, InitQualitySource::None);
    assert_eq!(summaries[0].fix.command, None);
}

#[test]
fn init_rejects_active_package_manager_without_resolved_language_descriptor() {
    let temp = TestDir::new("missing-init-descriptor");
    temp.write(
        ".mds/descriptors/package-managers/schema-pm.toml",
        r#"id = "schema-pm"
display_name = "Schema PM"
lang = "schema-lang"
metadata_files = ["package.json"]
metadata_reader = "node-package-json"
"#,
    );
    temp.write(
        "package.json",
        "{\n  \"name\": \"missing-init-descriptor\",\n  \"version\": \"0.1.0\",\n  \"packageManager\": \"schema-pm@1.0.0\"\n}\n",
    );

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions::default(),
        },
    });

    assert_eq!(result.exit_code, 1, "{}", result.stdout);
    assert!(result.stderr.contains(
        "mds init requires package-local language descriptors for active package manager languages: schema-lang"
    ));
    assert!(!temp.path().join("mds.config.toml").exists());
    assert!(!temp.path().join(".mds/source/index.ts.md").exists());
}

#[test]
fn package_json_without_lockfile_defaults_to_single_npm_active_manager() {
    let temp = TestDir::new("npm-default");
    write_node_package(&temp, None);

    let managers = descriptor::package_managers_for_root(temp.path());
    assert_eq!(
        managers
            .iter()
            .map(|manager| manager.id.as_str())
            .collect::<Vec<_>>(),
        vec!["npm"]
    );
    assert_eq!(
        descriptor::detect_package_manager(temp.path())
            .expect("npm should be detected")
            .id,
        "npm"
    );

    let summaries = detect_init_quality_toolchains(temp.path());
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].display_name, "Node.js (npm)");
    assert_eq!(
        summaries[0].type_check.source,
        InitQualitySource::PackageManager
    );
    assert_eq!(
        summaries[0].type_check.command.as_deref(),
        Some("npm run typecheck")
    );

    let stdout = init_setup_plan_stdout(&temp);
    assert!(stdout.contains("- project-deps: npm install"));
}

#[test]
fn package_manager_field_selects_single_active_package_manager() {
    let temp = TestDir::new("explicit-pnpm");
    write_node_package(&temp, Some("pnpm@9.0.0"));

    let managers = descriptor::package_managers_for_root(temp.path());
    assert_eq!(
        managers
            .iter()
            .map(|manager| manager.id.as_str())
            .collect::<Vec<_>>(),
        vec!["pnpm"]
    );
    assert_eq!(
        descriptor::detect_package_manager(temp.path())
            .expect("pnpm should be detected")
            .id,
        "pnpm"
    );

    let summaries = detect_init_quality_toolchains(temp.path());
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].display_name, "Node.js (pnpm)");
    assert_eq!(summaries[0].lint.source, InitQualitySource::PackageManager);
    assert_eq!(summaries[0].lint.command.as_deref(), Some("pnpm lint"));

    let stdout = init_setup_plan_stdout(&temp);
    assert!(stdout.contains("- project-deps: pnpm install"));
}

#[test]
fn init_normalizes_alias_manager_lang_for_detection_and_generated_quality() {
    let temp = TestDir::new("alias-manager-lang");
    write_node_package(&temp, None);
    rewrite_package_manager_manifest_lang(temp.path(), "npm", "ts", "typescript");

    let summaries = detect_init_quality_toolchains(temp.path());
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].lang, Lang::Other("ts".to_string()));
    assert_eq!(
        summaries[0].type_check.source,
        InitQualitySource::PackageManager
    );
    assert_eq!(
        summaries[0].type_check.command.as_deref(),
        Some("npm run typecheck")
    );
    assert_eq!(summaries[0].fix.source, InitQualitySource::Schema);
    assert_eq!(
        summaries[0].fix.command.as_deref(),
        Some("prettier --write")
    );

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let config = std::fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains("[quality.ts]"));
    assert!(config.contains("type_checker = \"npm run typecheck\""));
    assert!(config.contains("linter = \"npm run lint\""));
    assert!(config.contains("fixer = \"prettier --write\""));
    assert!(config.contains("test_runner = \"npm test\""));
}

#[test]
fn empty_config_quality_command_is_treated_as_unset_in_init_detection() {
    let temp = TestDir::new("empty-config-quality-command");
    temp.write(
        "package.json",
        "{\n  \"name\": \"quality-test\",\n  \"version\": \"0.1.0\",\n  \"scripts\": {\n    \"format\": \"fake-format\"\n  }\n}\n",
    );
    temp.write(
        "mds.config.toml",
        "[package]\nenabled = true\nallow_raw_source = false\n\n[quality.ts]\nfixer = \"\"\n",
    );

    let summaries = detect_init_quality_toolchains(temp.path());
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].fix.source, InitQualitySource::ExistingScripts);
    assert_eq!(summaries[0].fix.command.as_deref(), Some("npm run format"));
}

#[test]
fn init_setup_plan_uses_effective_script_tools_instead_of_schema_defaults() {
    let temp = TestDir::new("script-tool-hints");
    temp.write(
        "package.json",
        concat!(
            "{\n",
            "  \"name\": \"script-tool-hints\",\n",
            "  \"version\": \"0.1.0\",\n",
            "  \"scripts\": {\n",
            "    \"lint\": \"biome lint .\",\n",
            "    \"format\": \"biome format --write .\",\n",
            "    \"test\": \"jest --runInBand\"\n",
            "  }\n",
            "}\n"
        ),
    );

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                install_toolchains: true,
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let stdout = result.stdout;

    assert!(stdout.contains("biome --version"));
    assert!(stdout.contains("jest --version"));
    assert!(!stdout.contains("eslint --version"));
    assert!(!stdout.contains("prettier --version"));
    assert!(!stdout.contains("vitest --version"));
}

#[test]
fn init_setup_plan_keeps_npx_wrapped_minimal_ts_tool_hints() {
    let temp = TestDir::new("minimal-ts-npx-tool-hints");
    temp.write("package.json", MINIMAL_TS_PACKAGE_JSON);

    let stdout = init_toolchain_setup_plan_stdout(&temp);

    assert!(stdout.contains("prettier --version"));
    assert!(stdout.contains("vitest --version"));
    assert!(!stdout.contains("eslint --version"));
}

#[test]
fn lockfile_priority_overrides_package_manager_field() {
    let temp = TestDir::new("lockfile-priority");
    write_node_package(&temp, Some("npm@10.0.0"));
    temp.write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    let managers = descriptor::package_managers_for_root(temp.path());
    assert_eq!(
        managers
            .iter()
            .map(|manager| manager.id.as_str())
            .collect::<Vec<_>>(),
        vec!["pnpm"]
    );
    assert_eq!(
        descriptor::detect_package_manager(temp.path())
            .expect("pnpm should win when its lockfile exists")
            .id,
        "pnpm"
    );

    let summaries = detect_init_quality_toolchains(temp.path());
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].display_name, "Node.js (pnpm)");
    assert_eq!(summaries[0].test.source, InitQualitySource::PackageManager);
    assert_eq!(summaries[0].test.command.as_deref(), Some("pnpm test"));

    let stdout = init_setup_plan_stdout(&temp);
    assert!(stdout.contains("- project-deps: pnpm install"));
}
