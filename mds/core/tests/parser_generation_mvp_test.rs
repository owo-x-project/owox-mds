use mds_core::execute;
use mds_core::AgentKitCategory;
use mds_core::AiTarget;
use mds_core::BuildMode;
use mds_core::CliRequest;
use mds_core::Command;
use mds_core::GenerationPlan;
use mds_core::InitOptions;
use mds_core::InitQualityCommands;
use mds_core::InitTargetCategories;
use mds_core::LabelPreset;
use mds_core::Lang;
use mds_core::LinkPolicy;
use mds_core::OutputKind;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::{collections::HashMap, fs};

const TS_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/languages/ts.toml");
const NPM_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/package-managers/npm.toml");
const PY_DESCRIPTOR: &str = r##"id = "py"
aliases = ["python"]
match_suffixes = ["py"]

[language]
primary_ext = "py"
root_module_markdown_names = ["__init__.py.md"]

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "py"

[files.test]
strip_lang_ext = true
prefix = "test_"
suffix = ""
extension = "py"

[imports]
style = "python"

[syntax]
top_level_keywords = ["def ", "async def ", "class "]
comment_prefixes = ["#"]
doc_string_delimiters = ["\"\"\"", "'''"]

[[syntax.imports]]
starts_with = "import "

[[syntax.imports]]
starts_with = "from "

[scaffold]
fence_lang = "matched-suffix"
source_body = '''
# Implement your feature here.
'''
"##;
const RS_DESCRIPTOR: &str = r#"id = "rs"
aliases = ["rust"]
match_suffixes = ["rs"]

[language]
primary_ext = "rs"
root_module_markdown_names = ["lib.rs.md", "mod.rs.md"]

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "rs"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "rs"

[[special_files]]
match = "build.rs.md"
kind = "source"
output = "build.rs"
root = "package"

[imports]
style = "rust"

[quality_defaults]
lint = "cargo clippy"
fix = "cargo fmt"
test = "cargo test --lib"

[syntax]
top_level_keywords = ["pub fn ", "fn ", "pub struct ", "struct ", "pub enum ", "enum ", "impl ", "pub mod ", "mod "]
code_block_merge_prefixes = ["pub mod ", "mod ", "pub use ", "use "]
comment_prefixes = ["//", "/*", "*"]
doc_comment_prefixes = ["///", "//!"]

[[syntax.imports]]
starts_with = "use "

[scaffold]
fence_lang = "matched-suffix"
source_body = '''
// Implement your feature here.
'''
"#;
const GO_DESCRIPTOR: &str = r#"id = "go"
aliases = ["golang"]
match_suffixes = ["go"]

[language]
primary_ext = "go"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "go"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "go"

[imports]
style = "go"

[syntax]
top_level_keywords = ["func ", "type ", "var ", "const "]
comment_prefixes = ["//", "/*", "*"]

[[syntax.imports]]
starts_with = "import "

[[syntax.imports]]
starts_with = "import ("

[scaffold]
fence_lang = "matched-suffix"
source_body = '''
// Implement your feature here.
'''
"#;
const VUE_DESCRIPTOR: &str = r#"id = "vue"
match_suffixes = ["vue"]

[language]
primary_ext = "vue"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "vue"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = ".test"
extension = "ts"

[imports]
style = "typescript"

[syntax]
top_level_keywords = ["<template>", "<script", "export default ", "defineProps(", "defineEmits("]
comment_prefixes = ["<!--", "//", "/*", "*"]

[[syntax.imports]]
starts_with = "import "

[scaffold]
fence_lang = "matched-suffix"
source_body = '''
<template>
  <div><!-- Implement your feature here. --></div>
</template>

<script setup lang="ts">
// Implement your feature here.
</script>
'''
"#;
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
const CARGO_DESCRIPTOR: &str = r#"id = "cargo"
display_name = "Rust (Cargo)"
lang = "rs"
metadata_files = ["Cargo.toml"]
lockfiles = ["Cargo.lock"]
metadata_reader = "cargo-toml"

[commands]
install = "cargo fetch"
build = "cargo build"
typecheck = "cargo check"
lint = "cargo clippy"
test = "cargo test"
"#;
const POETRY_DESCRIPTOR: &str = r#"id = "poetry"
display_name = "Python (Poetry)"
lang = "py"
metadata_files = ["pyproject.toml"]
lockfiles = ["poetry.lock"]
metadata_reader = "pyproject-toml"

[commands]
install = "poetry install"
build = "poetry build"
typecheck = "poetry run mypy ."
lint = "poetry run ruff check"
test = "poetry run pytest"
"#;
#[path = "../../test_support/fixture_copy.rs"]
mod fixture_copy;

use fixture_copy::copy_example_package_to;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn builds_three_language_fixture() {
    let temp = TestDir::new();
    write_fixture(temp.path());

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
    assert!(check.stdout.contains("lint ok"));

    let dry_run = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::DryRun,
        },
    });
    assert_eq!(dry_run.exit_code, 0, "{}", dry_run.stderr);
    assert!(dry_run.stdout.contains("Build plan:"));
    assert!(dry_run.stdout.contains(".mds/manifest.toml"));
    assert!(dry_run.stdout.contains("--- /dev/null"));
    assert!(!temp.path().join("pkg/src/foo/bar.ts").exists());

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(temp.path().join("pkg/src/foo/bar.ts").exists());
    assert!(temp.path().join("pkg/src/pkg/foo.py").exists());
    assert!(temp.path().join("pkg/src/foo/bar.rs").exists());
    assert!(temp.path().join("pkg/.mds/manifest.toml").exists());
}

#[test]
fn build_discovers_fence_only_source_doc_from_runtime_descriptor() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let package_root = temp.path().join("pkg");
    fs::create_dir_all(package_root.join(".mds/descriptors/languages")).unwrap();
    fs::write(
        package_root.join(".mds/descriptors/languages/dart.toml"),
        r#"id = "dart"
match_suffixes = ["dart"]

[language]
primary_ext = "dart"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "dart"

[files.types]
strip_lang_ext = true
prefix = ""
suffix = ".types"
extension = "dart"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "dart"

[syntax]
top_level_keywords = ["class ", "void ", "final "]
comment_prefixes = ["//"]

[scaffold]
fence_lang = "matched-suffix"
source_body = '''
// Implement your feature here.
'''
"#,
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/source/foo/custom.md"),
        "# Custom\n\n## Purpose\n\nCustom language.\n\n## Contract\n\n- Compile custom language source.\n\n## Source\n\n```dart\nclass Custom {}\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(temp.path().join("pkg/src/foo/custom.dart").exists());
    assert!(!temp.path().join("pkg/src/foo/custom.md").exists());
}

#[test]
fn workspace_index_discovers_fence_only_source_doc_from_runtime_descriptor() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    write_minimal_authoring_package(&package_root, "");
    fs::create_dir_all(package_root.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package_root.join(".mds/descriptors/languages")).unwrap();
    fs::write(
        package_root.join(".mds/descriptors/languages/dart.toml"),
        r#"id = "dart"
match_suffixes = ["dart"]

[language]
primary_ext = "dart"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "dart"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "dart"
"#,
    )
    .unwrap();
    let path = package_root.join(".mds/source/foo/custom.md");
    fs::write(
        &path,
        "# Custom\n\n## Purpose\n\nCustom language.\n\n## Contract\n\n- Compile custom language source.\n\n## Source\n\n```dart\nclass Custom {}\n```\n",
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut parse_state = mds_core::RunState::default();
    let docs = mds_core::markdown::load_workspace_index_docs(&package, &mut parse_state).unwrap();

    assert!(
        parse_state.diagnostics.is_empty(),
        "{:?}",
        parse_state.diagnostics
    );
    let doc = docs
        .iter()
        .find(|doc| doc.path == path)
        .expect("source doc should be indexed");
    assert_eq!(doc.lang, Lang::Other("dart".to_string()));
    assert_eq!(doc.profile, mds_core::model::DocProfile::Impl);
    assert!(doc.source_code.contains("class Custom {}"));
}

#[test]
fn resolve_markdown_lang_uses_runtime_descriptor_from_fence_label() {
    let temp = TestDir::new();
    fs::create_dir_all(temp.path().join(".mds/descriptors/languages")).unwrap();
    fs::write(
        temp.path().join(".mds/descriptors/languages/dart.toml"),
        r#"id = "dart"
match_suffixes = ["dart"]

[language]
primary_ext = "dart"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "dart"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "dart"
"#,
    )
    .unwrap();

    let lang = mds_core::descriptor::resolve_markdown_lang_at(
        Some(temp.path()),
        Path::new("notes/custom.md"),
        ["dart"].into_iter(),
    );

    assert_eq!(lang, Some(Lang::Other("dart".to_string())));
}

#[test]
fn build_disambiguates_test_doc_language_from_fence_label() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    write_minimal_authoring_package(&package, "");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package.join(".mds/test/foo")).unwrap();
    fs::write(
        package.join(".mds/source/foo/greet.ts.md"),
        "# greet ts\n\n## Purpose\n\nTS source.\n\n## Contract\n\n- TS impl.\n\n## Source\n\n```ts\nexport const greet = () => 'ts';\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/greet.rs.md"),
        "# greet rs\n\n## Purpose\n\nRS source.\n\n## Contract\n\n- RS impl.\n\n## Source\n\n```rs\npub fn greet() -> &'static str { \"rs\" }\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/test/foo/greet.md"),
        "# greet test\n\n## Purpose\n\nVerify TS greet.\n\n## Covers\n\n- foo.greet\n\n## Cases\n\n- Resolves TS implementation.\n\n## Test\n\n```ts\nimport { greet } from '../../src/foo/greet';\nexpect(greet()).toBe('ts');\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(package.join("tests/foo/greet.test.ts").exists());
    assert!(!package.join("tests/foo/greet_test.rs").exists());
}

#[test]
fn build_uses_descriptor_test_file_rule_for_python() {
    let temp = TestDir::new();
    let package = temp.path().join("python-test-rule");
    write_minimal_authoring_package(&package, "");
    fs::create_dir_all(package.join(".mds/source/app")).unwrap();
    fs::create_dir_all(package.join(".mds/test/app")).unwrap();
    fs::write(
        package.join(".mds/source/app/greet.py.md"),
        "# greet\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Provide greeting.\n\n## Source\n\n```py\ndef greet(name: str) -> str:\n    return f'Hello, {name}'\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/test/app/greet.md"),
        "# greet test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- app.greet\n\n## Cases\n\n- Returns greeting.\n\n## Test\n\n```py\nfrom app.greet import greet\n\ndef test_greet() -> None:\n    assert greet('Ada') == 'Hello, Ada'\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(package.join("tests/app/test_greet.py").exists());
    assert!(!package.join("tests/app/greet.test.py").exists());
}

#[test]
fn rejects_unknown_fence_only_source_doc_from_index_and_generation_plan() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    write_minimal_authoring_package(&package_root, "");
    let path = package_root.join(".mds/source/foo/custom.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "# Custom\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Reject unknown fence-only source docs.\n\n## Source\n\n```unknown\ncontent\n```\n",
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut index_state = mds_core::RunState::default();
    let indexed_docs =
        mds_core::markdown::load_workspace_index_docs(&package, &mut index_state).unwrap();

    assert!(indexed_docs.iter().all(|doc| doc.path != path));
    assert!(
        index_state.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("source md Source fence labels do not resolve to a known language")
        }),
        "{:?}",
        index_state.diagnostics
    );

    let mut impl_state = mds_core::RunState::default();
    let docs = mds_core::markdown::load_implementation_docs(&package, &mut impl_state).unwrap();
    assert!(docs.iter().all(|doc| doc.path != path));
    assert!(
        impl_state.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("source md Source fence labels do not resolve to a known language")
        }),
        "{:?}",
        impl_state.diagnostics
    );

    let mut plan_state = mds_core::RunState::default();
    let plan = mds_core::plan_generation_with_source_map(&package, &docs, &mut plan_state);

    assert!(plan
        .generated
        .iter()
        .all(|file| file.path != package_root.join("src/foo/custom.md")));
}

#[test]
fn rejects_ambiguous_fence_only_source_doc_from_index_and_generation_plan() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    write_minimal_authoring_package(&package_root, "");
    fs::create_dir_all(package_root.join(".mds/descriptors/languages")).unwrap();
    fs::write(
        package_root.join(".mds/descriptors/languages/alpha.toml"),
        r#"id = "alpha"
match_suffixes = ["custom"]

[language]
primary_ext = "alpha"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "alpha"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "alpha"
"#,
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/descriptors/languages/beta.toml"),
        r#"id = "beta"
match_suffixes = ["custom"]

[language]
primary_ext = "beta"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "beta"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "beta"
"#,
    )
    .unwrap();

    let path = package_root.join(".mds/source/foo/ambiguous.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "# Ambiguous\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Reject ambiguous fence-only source docs.\n\n## Source\n\n```custom\ncontent\n```\n",
    )
    .unwrap();

    mds_core::descriptor::with_workspace_descriptor_root(Some(package_root.as_path()), || {
        let mut load_state = mds_core::RunState::default();
        let package = mds_core::package::load_package(
            &package_root,
            &mds_core::Config::default(),
            &mut load_state,
        )
        .unwrap();
        assert!(
            load_state.diagnostics.is_empty(),
            "{:?}",
            load_state.diagnostics
        );

        let mut index_state = mds_core::RunState::default();
        let indexed_docs =
            mds_core::markdown::load_workspace_index_docs(&package, &mut index_state).unwrap();

        assert!(indexed_docs.iter().all(|doc| doc.path != path));
        assert!(
            index_state.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("source md Source fence labels resolve ambiguously")
            }),
            "{:?}",
            index_state.diagnostics
        );

        let mut impl_state = mds_core::RunState::default();
        let docs = mds_core::markdown::load_implementation_docs(&package, &mut impl_state).unwrap();
        assert!(docs.iter().all(|doc| doc.path != path));
        assert!(
            impl_state.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("source md Source fence labels resolve ambiguously")
            }),
            "{:?}",
            impl_state.diagnostics
        );

        let mut plan_state = mds_core::RunState::default();
        let plan = mds_core::plan_generation_with_source_map(&package, &docs, &mut plan_state);

        assert!(plan
            .generated
            .iter()
            .all(|file| file.path != package_root.join("src/foo/ambiguous.md")));
    });
}

#[test]
fn rejects_ambiguous_suffix_source_doc_from_index_and_generation_plan() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    write_minimal_authoring_package(&package_root, "");
    fs::create_dir_all(package_root.join(".mds/descriptors/languages")).unwrap();
    fs::write(
        package_root.join(".mds/descriptors/languages/alpha.toml"),
        r#"id = "alpha"
match_suffixes = ["custom"]

[language]
primary_ext = "alpha"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "alpha"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "alpha"
"#,
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/descriptors/languages/beta.toml"),
        r#"id = "beta"
match_suffixes = ["custom"]

[language]
primary_ext = "beta"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "beta"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = "_test"
extension = "beta"
"#,
    )
    .unwrap();

    let path = package_root.join(".mds/source/foo/ambiguous.custom.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "# Ambiguous\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Reject ambiguous path suffix source docs.\n\n## Source\n\n```alpha\ncontent\n```\n",
    )
    .unwrap();

    mds_core::descriptor::with_workspace_descriptor_root(Some(package_root.as_path()), || {
        let mut load_state = mds_core::RunState::default();
        let package = mds_core::package::load_package(
            &package_root,
            &mds_core::Config::default(),
            &mut load_state,
        )
        .unwrap();
        assert!(
            load_state.diagnostics.is_empty(),
            "{:?}",
            load_state.diagnostics
        );

        let mut index_state = mds_core::RunState::default();
        let indexed_docs =
            mds_core::markdown::load_workspace_index_docs(&package, &mut index_state).unwrap();

        assert!(indexed_docs.iter().all(|doc| doc.path != path));
        assert!(
            index_state.diagnostics.iter().any(|diagnostic| {
                diagnostic.path.as_deref() == Some(path.as_path())
                    && diagnostic
                        .message
                        .contains("source md path suffix resolves ambiguously across languages")
            }),
            "{:?}",
            index_state.diagnostics
        );

        let mut impl_state = mds_core::RunState::default();
        let docs = mds_core::markdown::load_implementation_docs(&package, &mut impl_state).unwrap();
        assert!(docs.iter().all(|doc| doc.path != path));
        assert!(
            impl_state.diagnostics.iter().any(|diagnostic| {
                diagnostic.path.as_deref() == Some(path.as_path())
                    && diagnostic
                        .message
                        .contains("source md path suffix resolves ambiguously across languages")
            }),
            "{:?}",
            impl_state.diagnostics
        );

        let mut plan_state = mds_core::RunState::default();
        let plan = mds_core::plan_generation_with_source_map(&package, &docs, &mut plan_state);

        assert!(plan
            .generated
            .iter()
            .all(|file| { file.path != package_root.join("src/foo/ambiguous.custom.md") }));
        assert!(
            plan_state.diagnostics.iter().any(|diagnostic| {
                diagnostic.path.as_deref() == Some(path.as_path())
                    && diagnostic
                        .message
                        .contains("source md path suffix resolves ambiguously across languages")
            }),
            "{:?}",
            plan_state.diagnostics
        );
    });
}

#[test]
fn parse_root_module_source_section_without_code_as_spec_profile() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    write_minimal_authoring_package(&package_root, "");
    let path = package_root.join(".mds/source/index.ts.md");
    fs::write(
        &path,
        "# Package Root\n\n## Purpose\n\nFixture root module.\n\n## Contract\n\n- Own runtime behavior.\n\n## API\n\nDescribe root exports.\n\n## Source\n\nRuntime behavior intentionally missing code fence.\n",
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut parse_state = mds_core::RunState::default();
    let doc = mds_core::markdown::parse_impl_doc(
        &package,
        mds_core::DocKind::Source,
        Lang::Other("ts".to_string()),
        &path,
        &mut parse_state,
    )
    .unwrap();

    assert_eq!(doc.profile, mds_core::model::DocProfile::Spec);
    assert!(
        parse_state.diagnostics.is_empty(),
        "{:?}",
        parse_state.diagnostics
    );
}

#[test]
fn empty_source_fence_is_impl_and_requires_code_block() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    write_minimal_authoring_package(&package_root, "");
    let path = package_root.join(".mds/source/foo/empty.ts.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "# Empty\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Empty Source fence must fail.\n\n## Source\n\n```ts\n```\n",
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut parse_state = mds_core::RunState::default();
    let doc = mds_core::markdown::parse_impl_doc(
        &package,
        mds_core::DocKind::Source,
        Lang::Other("ts".to_string()),
        &path,
        &mut parse_state,
    )
    .unwrap();

    assert_eq!(doc.profile, mds_core::model::DocProfile::Impl);
    assert!(
        parse_state.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("implementation md requires at least one code block")
        }),
        "{:?}",
        parse_state.diagnostics
    );
}

#[test]
fn merges_source_out_root_from_workspace_and_package_config() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("mds.config.toml"),
        "[roots]\nsource_out = \"generated\"\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(temp.path().join("pkg/generated/foo/bar.ts").exists());
}

#[test]
fn rejects_non_canonical_authoring_markdown_roots() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[roots]\nsource_md = \"src-md\"\ntest_md = \"docs/test\"\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 1);
    assert!(build
        .stderr
        .contains("config `source_md` must be `.mds/source`"));
    assert!(build
        .stderr
        .contains("config `test_md` must be `.mds/test`"));
}

#[test]
fn load_package_parses_output_patterns_and_overrides() {
    let temp = TestDir::new();
    let package_root = temp.path().join("output-parse-fixture");
    fs::create_dir_all(&package_root).unwrap();
    write_npm_runtime_descriptors(&package_root);
    fs::write(
        package_root.join("package.json"),
        "{\"name\":\"output-parse-fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package_root.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[output]\nsource = \"{source_out}/{module}.{ext}\"\ntest = \"{test_out}/{module}.test.{ext}\"\n\n[[output.override]]\nmatch = \"build\"\nkind = \"source\"\npath = \"build.rs\"\n",
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );
    assert_eq!(
        package.config.output.source.as_deref(),
        Some("{source_out}/{module}.{ext}")
    );
    assert_eq!(
        package.config.output.test.as_deref(),
        Some("{test_out}/{module}.test.{ext}")
    );
    assert_eq!(package.config.output.overrides.len(), 1);
    let override_rule = &package.config.output.overrides[0];
    assert_eq!(override_rule.match_pattern, "build");
    assert_eq!(override_rule.kind, OutputKind::Source);
    assert_eq!(override_rule.path, "build.rs");
}

#[test]
fn check_config_defaults_include_phase_08_policies() {
    let config = mds_core::Config::default();

    assert_eq!(
        config.check.legacy_tables,
        mds_core::model::CheckDiagnosticPolicy::Warn
    );
    assert_eq!(
        config.check.unresolved_module_symbols,
        mds_core::model::CheckDiagnosticPolicy::Warn
    );
    assert!(config.check.implementation_section_only);
    assert!(config.check.split_source_and_test);
}

#[test]
fn merge_config_file_accepts_phase_08_check_policies() {
    for (value, expected) in [
        ("warn", mds_core::model::CheckDiagnosticPolicy::Warn),
        ("error", mds_core::model::CheckDiagnosticPolicy::Error),
        ("allow", mds_core::model::CheckDiagnosticPolicy::Allow),
    ] {
        let temp = TestDir::new();
        let config_path = temp.path().join("mds.config.toml");
        fs::write(
            &config_path,
            format!(
                "[check]\nlegacy_tables = \"{value}\"\nunresolved_module_symbols = \"{value}\"\nimplementation_section_only = false\nsplit_source_and_test = false\n"
            ),
        )
        .unwrap();

        let mut config = mds_core::Config::default();
        let mut state = mds_core::RunState::default();
        assert!(
            mds_core::config::merge_config_file(&mut config, &config_path, &mut state).is_some()
        );
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        assert_eq!(config.check.legacy_tables, expected);
        assert_eq!(config.check.unresolved_module_symbols, expected);
        assert!(!config.check.implementation_section_only);
        assert!(!config.check.split_source_and_test);
    }
}

#[test]
fn merge_config_file_rejects_invalid_phase_08_check_policy_values() {
    let temp = TestDir::new();
    let config_path = temp.path().join("mds.config.toml");
    fs::write(
        &config_path,
        "[check]\nlegacy_tables = \"maybe\"\nunresolved_module_symbols = \"maybe\"\n",
    )
    .unwrap();

    let mut config = mds_core::Config::default();
    let mut state = mds_core::RunState::default();
    assert!(mds_core::config::merge_config_file(&mut config, &config_path, &mut state).is_some());
    assert!(state.has_errors());
    assert_eq!(
        config.check.legacy_tables,
        mds_core::model::CheckDiagnosticPolicy::Warn
    );
    assert_eq!(
        config.check.unresolved_module_symbols,
        mds_core::model::CheckDiagnosticPolicy::Warn
    );

    let rendered = state
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.render())
        .collect::<String>();
    assert!(rendered.contains("config `legacy_tables` must be `warn`, `error`, or `allow`"));
    assert!(
        rendered.contains("config `unresolved_module_symbols` must be `warn`, `error`, or `allow`")
    );
}

#[test]
fn merge_config_file_parses_doctor_version_floor_policy() {
    let temp = TestDir::new();
    let config_path = temp.path().join("mds.config.toml");
    fs::write(
        &config_path,
        "[doctor.version_floor]\nnode = \"24.0\"\npython3 = \"3.13.1\"\n\"/tmp/custom-tool\" = \"1\"\n",
    )
    .unwrap();

    let mut config = mds_core::Config::default();
    let mut state = mds_core::RunState::default();
    assert!(mds_core::config::merge_config_file(&mut config, &config_path, &mut state).is_some());
    assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
    assert_eq!(
        config.doctor.version_floor.get("node"),
        Some(&mds_core::DoctorVersionFloor {
            major: 24,
            minor: 0,
            patch: 0,
        })
    );
    assert_eq!(
        config.doctor.version_floor.get("python3"),
        Some(&mds_core::DoctorVersionFloor {
            major: 3,
            minor: 13,
            patch: 1,
        })
    );
    assert_eq!(
        config.doctor.version_floor.get("/tmp/custom-tool"),
        Some(&mds_core::DoctorVersionFloor {
            major: 1,
            minor: 0,
            patch: 0,
        })
    );
}

#[test]
fn merge_config_file_merges_doctor_version_floor_across_root_and_package_configs() {
    let temp = TestDir::new();
    let root_config_path = temp.path().join("mds.config.toml");
    let package_dir = temp.path().join("pkg");
    let package_config_path = package_dir.join("mds.config.toml");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        &root_config_path,
        "[doctor.version_floor]\nnode = \"20\"\npython3 = \"3.11\"\n",
    )
    .unwrap();
    fs::write(
        &package_config_path,
        "[doctor.version_floor]\nnode = \"24.1\"\nuv = \"0.5.0\"\n",
    )
    .unwrap();

    let mut config = mds_core::Config::default();
    let mut state = mds_core::RunState::default();
    assert!(
        mds_core::config::merge_config_file(&mut config, &root_config_path, &mut state).is_some()
    );
    assert!(
        mds_core::config::merge_config_file(&mut config, &package_config_path, &mut state)
            .is_some()
    );
    assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
    assert_eq!(
        config.doctor.version_floor.get("node"),
        Some(&mds_core::DoctorVersionFloor {
            major: 24,
            minor: 1,
            patch: 0,
        })
    );
    assert_eq!(
        config.doctor.version_floor.get("python3"),
        Some(&mds_core::DoctorVersionFloor {
            major: 3,
            minor: 11,
            patch: 0,
        })
    );
    assert_eq!(
        config.doctor.version_floor.get("uv"),
        Some(&mds_core::DoctorVersionFloor {
            major: 0,
            minor: 5,
            patch: 0,
        })
    );
}

#[test]
fn merge_config_file_rejects_legacy_doctor_required_and_optional() {
    let temp = TestDir::new();
    let config_path = temp.path().join("mds.config.toml");
    fs::write(
        &config_path,
        "[doctor]\nrequired = [\"node\"]\noptional = [\"prettier\"]\n",
    )
    .unwrap();

    let mut config = mds_core::Config::default();
    let lang = Lang::Other("ts".to_string());
    config.quality.insert(
        lang.clone(),
        mds_core::QualityConfig {
            required: vec!["eslint".to_string()],
            optional: vec!["biome".to_string()],
            ..mds_core::QualityConfig::default()
        },
    );

    let mut state = mds_core::RunState::default();
    assert!(mds_core::config::merge_config_file(&mut config, &config_path, &mut state).is_some());
    assert!(state.has_errors());

    let quality = config.quality.get(&lang).unwrap();
    assert_eq!(quality.required, vec!["eslint".to_string()]);
    assert_eq!(quality.optional, vec!["biome".to_string()]);

    let rendered = state
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.render())
        .collect::<String>();
    assert!(rendered.contains(
        "config `doctor.required` is not supported in v1; use `quality.<lang>.required` instead"
    ));
    assert!(rendered.contains(
        "config `doctor.optional` is not supported in v1; use `quality.<lang>.optional` instead"
    ));
}

#[test]
fn build_ignores_code_blocks_outside_source_section() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/spec-only.ts.md"),
        "# Spec only\n\n## Purpose\n\nDocument planned behavior without generated source.\n\n## Contract\n\n- Keep the feature in spec state.\n\n## Cases\n\n```ts\nexport const planned = true;\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(!temp.path().join("pkg/src/foo/spec-only.ts").exists());
}

#[test]
fn build_includes_noncanonical_code_blocks_when_implementation_section_only_disabled() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    write_minimal_authoring_package(&package, "[check]\nimplementation_section_only = false\n");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::write(
        package.join(".mds/source/foo/spec-only.ts.md"),
        "# Spec only\n\n## Purpose\n\nDocument planned behavior without a canonical implementation section.\n\n## Contract\n\n- Allow non-canonical code fences when configured.\n\n## Cases\n\n```ts\nexport const planned = true;\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    let generated = fs::read_to_string(package.join("src/foo/spec-only.ts")).unwrap();
    assert!(generated.contains("export const planned = true;"));
}

#[test]
fn build_discovers_unsuffixed_source_doc_from_noncanonical_generated_fence() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    write_minimal_authoring_package(&package, "[check]\nimplementation_section_only = false\n");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::write(
        package.join(".mds/source/foo/spec-only.md"),
        "# Spec only\n\n## Purpose\n\nDocument planned behavior without a canonical implementation section.\n\n## Contract\n\n- Allow non-canonical code fences to drive language discovery.\n\n## Cases\n\n```ts\nexport const planned = true;\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    let generated = fs::read_to_string(package.join("src/foo/spec-only.ts")).unwrap();
    assert!(generated.contains("export const planned = true;"));
}

#[test]
fn parse_impl_doc_captures_code_fence_spans_by_section() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    fs::create_dir_all(package_root.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package_root);
    fs::write(
        package_root.join("package.json"),
        "{\"name\":\"span-fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package_root.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[check]\ncode_blocks_required = false\n",
    )
    .unwrap();

    let span_doc_path = package_root.join(".mds/source/span.ts.md");
    fs::write(
        &span_doc_path,
        r#"# Span fixture

## Purpose

Fixture.

## Contract

- Preserve code fence spans.

## Source

```ts
export const one = 1;
```

```ts
export const two = 2;
```

"#,
    )
    .unwrap();

    let test_doc_path = package_root.join(".mds/test/foo/span.md");
    fs::create_dir_all(test_doc_path.parent().unwrap()).unwrap();
    fs::write(
        &test_doc_path,
        r#"# Span test

## Purpose

Fixture.

## Covers

- [[foo.span-source]]

## Cases

- Preserve test fence spans.

## Test

```ts
expect(one).toBe(1);
```

```ts
expect(two).toBe(2);
```
"#,
    )
    .unwrap();

    let table_doc_path = package_root.join(".mds/source/table.ts.md");
    fs::write(
        &table_doc_path,
        r#"# Table source

## Purpose

Fixture.

## Contract

- Preserve table fallback.

## Source

| Statement |
| --- |
| `export const table_value = 1;` |
"#,
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut span_state = mds_core::RunState::default();
    let span_doc = mds_core::markdown::parse_impl_doc(
        &package,
        mds_core::DocKind::Source,
        Lang::Other("ts".to_string()),
        &span_doc_path,
        &mut span_state,
    )
    .unwrap();
    assert!(
        span_state.diagnostics.is_empty(),
        "{:?}",
        span_state.diagnostics
    );
    assert_eq!(
        span_doc
            .source_blocks
            .iter()
            .map(|block| (
                block.fence_index,
                block.content_start_line,
                block.content_end_line,
                block.content.as_str(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, 14, 14, "export const one = 1;"),
            (1, 18, 18, "export const two = 2;"),
        ]
    );
    assert_eq!(
        span_doc
            .test_blocks
            .iter()
            .map(|block| (
                block.fence_index,
                block.content_start_line,
                block.content_end_line,
                block.content.as_str(),
            ))
            .collect::<Vec<_>>(),
        Vec::<(usize, usize, usize, &str)>::new()
    );
    assert_eq!(
        span_doc.source_code,
        "export const one = 1;\n\nexport const two = 2;\n"
    );
    assert_eq!(span_doc.test_code, "");

    let mut test_state = mds_core::RunState::default();
    let test_doc = mds_core::markdown::parse_impl_doc(
        &package,
        mds_core::DocKind::Test,
        Lang::Other("ts".to_string()),
        &test_doc_path,
        &mut test_state,
    )
    .unwrap();
    assert!(
        test_state.diagnostics.is_empty(),
        "{:?}",
        test_state.diagnostics
    );
    assert!(test_doc.source_blocks.is_empty());
    assert_eq!(
        test_doc
            .test_blocks
            .iter()
            .map(|block| (
                block.fence_index,
                block.content_start_line,
                block.content_end_line,
                block.content.as_str(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, 18, 18, "expect(one).toBe(1);"),
            (1, 22, 22, "expect(two).toBe(2);")
        ]
    );
    assert_eq!(test_doc.source_code, "");
    assert_eq!(
        test_doc.test_code,
        "expect(one).toBe(1);\n\nexpect(two).toBe(2);\n"
    );

    let mut table_state = mds_core::RunState::default();
    let table_doc = mds_core::markdown::parse_impl_doc(
        &package,
        mds_core::DocKind::Source,
        Lang::Other("ts".to_string()),
        &table_doc_path,
        &mut table_state,
    )
    .unwrap();
    assert!(
        table_state.diagnostics.is_empty(),
        "{:?}",
        table_state.diagnostics
    );
    assert!(table_doc.source_blocks.is_empty());
    assert_eq!(table_doc.source_code, "export const table_value = 1;\n");
    assert!(table_doc.test_blocks.is_empty());
    assert_eq!(table_doc.test_code, "");
}

#[test]
fn plan_generation_with_source_map_maps_source_and_test_outputs_from_fences_only() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    fs::create_dir_all(package_root.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package_root.join(".mds/test/foo")).unwrap();
    write_npm_runtime_descriptors(&package_root);
    fs::write(
        package_root.join("package.json"),
        "{\"name\":\"source-map-fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package_root.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[check]\ncode_blocks_required = false\n",
    )
    .unwrap();

    let mapped_doc_path = package_root.join(".mds/source/foo/source-map.ts.md");
    fs::write(
        &mapped_doc_path,
        r#"# Source map

## Purpose

Fixture.

## Contract

- Preserve source map spans.

## Source

```ts
export const one = 1;
```

```ts
export function two(): number {
  return one + 1;
}
```
"#,
    )
    .unwrap();

    let mapped_test_doc_path = package_root.join(".mds/test/foo/source-map.md");
    fs::write(
        &mapped_test_doc_path,
        r#"# Source map test

## Purpose

Fixture.

## Covers

- [[foo.source-map]]

## Cases

- Preserve source map spans for test output.

## Test

```ts
expect(two()).toBe(2);
```
"#,
    )
    .unwrap();

    fs::write(
        package_root.join(".mds/source/foo/table-only.ts.md"),
        r#"# Table only

## Purpose

Fixture.

## Contract

- Keep table-derived output unmapped.

## Source

| Statement |
| --- |
| `export const tableOnly = true;` |
"#,
    )
    .unwrap();

    let plan = load_generation_plan(&package_root);

    let source_output_path = package_root.join("src/foo/source-map.ts");
    let source_first = plan
        .source_map
        .find_markdown(&mapped_doc_path, 14)
        .expect("missing first source span");
    assert_eq!(source_first.markdown_path, mapped_doc_path);
    assert_eq!(source_first.markdown_start_line, 14);
    assert_eq!(source_first.markdown_end_line, 14);
    assert_eq!(source_first.generated_path, source_output_path);
    assert_eq!(source_first.generated_start_line, 3);
    assert_eq!(source_first.generated_end_line, 3);
    assert_eq!(source_first.output_kind, OutputKind::Source);
    assert_eq!(source_first.extension_key, "ts");
    assert_eq!(source_first.fence_index, 0);

    let source_second = plan
        .source_map
        .find_generated(&source_output_path, 6)
        .expect("missing second source span");
    assert_eq!(source_second.markdown_path, mapped_doc_path);
    assert_eq!(source_second.markdown_start_line, 18);
    assert_eq!(source_second.markdown_end_line, 20);
    assert_eq!(source_second.generated_start_line, 5);
    assert_eq!(source_second.generated_end_line, 7);
    assert_eq!(source_second.output_kind, OutputKind::Source);
    assert_eq!(source_second.extension_key, "ts");
    assert_eq!(source_second.fence_index, 1);

    let test_output_path = package_root.join("tests/foo/source-map.test.ts");
    let test_span = plan
        .source_map
        .find_generated(&test_output_path, 3)
        .expect("missing test span");
    assert_eq!(test_span.markdown_path, mapped_test_doc_path);
    assert_eq!(test_span.markdown_start_line, 18);
    assert_eq!(test_span.markdown_end_line, 18);
    assert_eq!(test_span.generated_path, test_output_path);
    assert_eq!(test_span.generated_start_line, 3);
    assert_eq!(test_span.generated_end_line, 3);
    assert_eq!(test_span.output_kind, OutputKind::Test);
    assert_eq!(test_span.extension_key, "ts");
    assert_eq!(test_span.fence_index, 0);

    let table_output_path = package_root.join("src/foo/table-only.ts");
    assert!(plan
        .generated
        .iter()
        .any(|file| file.path == table_output_path));
    assert!(plan
        .source_map
        .find_generated(&table_output_path, 3)
        .is_none());
}

#[test]
fn plan_generation_with_source_map_uses_descriptor_special_file_path_rules() {
    let temp = TestDir::new();
    let package_root = temp.path().join("rust-build-script");
    fs::create_dir_all(package_root.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package_root);
    fs::write(
        package_root.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[output]\nsource = \"generated/{module}.{ext}\"\n",
    )
    .unwrap();
    fs::write(
        package_root.join("Cargo.toml"),
        "[package]\nname = \"rust-build-script\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let build_doc_path = package_root.join(".mds/source/build.rs.md");
    fs::write(
        &build_doc_path,
        r#"# build

## Purpose

Fixture.

## Contract

- Compile.

## Source

```rs
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
```
"#,
    )
    .unwrap();

    let plan = load_generation_plan(&package_root);

    let build_output_path = package_root.join("build.rs");
    assert!(plan
        .generated
        .iter()
        .any(|file| file.path == build_output_path));
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("src/build.rs")));
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("generated/build.rs")));

    let build_span = plan
        .source_map
        .find_generated(&build_output_path, 4)
        .expect("missing build.rs span");
    assert_eq!(build_span.markdown_path, build_doc_path);
    assert_eq!(build_span.markdown_start_line, 14);
    assert_eq!(build_span.markdown_end_line, 16);
    assert_eq!(build_span.generated_path, build_output_path);
    assert_eq!(build_span.generated_start_line, 3);
    assert_eq!(build_span.generated_end_line, 5);
    assert_eq!(build_span.output_kind, OutputKind::Source);
    assert_eq!(build_span.extension_key, "rs");
    assert_eq!(build_span.fence_index, 0);
}

#[test]
fn plan_generation_with_source_map_keeps_output_patterns_for_non_special_files() {
    let temp = TestDir::new();
    let package_root = temp.path().join("rust-output-pattern");
    write_minimal_authoring_package(
        &package_root,
        "[output]\nsource = \"generated/{module}.{ext}\"\n",
    );
    fs::write(
        package_root.join("Cargo.toml"),
        "[package]\nname = \"rust-output-pattern\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/source/lib.rs.md"),
        "# lib\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Compile.\n\n## Source\n\n```rs\npub fn greet() -> &'static str {\n    \"hi\"\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/source/build.rs.md"),
        "# build\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Compile.\n\n## Source\n\n```rs\nfn main() {\n    println!(\"cargo:rerun-if-changed=build.rs\");\n}\n```\n",
    )
    .unwrap();

    let plan = load_generation_plan(&package_root);

    assert!(plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("generated/lib.rs")));
    assert!(plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("build.rs")));
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("generated/build.rs")));
}

#[test]
fn plan_generation_with_source_map_keeps_output_override_compatibility() {
    let temp = TestDir::new();
    let package_root = temp.path().join("rust-build-script-override");
    fs::create_dir_all(package_root.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package_root);
    fs::write(
        package_root.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[[output.override]]\nmatch = \"build\"\nkind = \"source\"\npath = \"scripts/build.rs\"\n",
    )
    .unwrap();
    fs::write(
        package_root.join("Cargo.toml"),
        "[package]\nname = \"rust-build-script-override\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let build_doc_path = package_root.join(".mds/source/build.rs.md");
    fs::write(
        &build_doc_path,
        r#"# build

## Purpose

Fixture.

## Contract

- Compile.

## Source

```rs
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
```
"#,
    )
    .unwrap();

    let plan = load_generation_plan(&package_root);

    assert!(plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("scripts/build.rs")));
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("build.rs")));
}

#[test]
fn plan_generation_with_source_map_treats_output_patterns_as_package_relative() {
    let temp = TestDir::new();
    let package_root = temp.path().join("output-pattern-root-relative");
    write_minimal_authoring_package(
        &package_root,
        "[output]\nsource = \"{source_out}/{module}.{ext}\"\ntest = \"{test_out}/{module}.test.{ext}\"\n",
    );
    fs::create_dir_all(package_root.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package_root.join(".mds/test/foo")).unwrap();
    fs::write(
        package_root.join(".mds/source/foo/greet.ts.md"),
        "# greet\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Compile.\n\n## Source\n\n```ts\nexport const greet = () => 'hi';\n```\n",
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/test/foo/greet.md"),
        "# greet test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- foo.greet\n\n## Cases\n\n- Works.\n\n## Test\n\n```ts\nimport { greet } from '../../src/foo/greet';\nexpect(greet()).toBe('hi');\n```\n",
    )
    .unwrap();

    let plan = load_generation_plan(&package_root);

    assert!(plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("src/foo/greet.ts")));
    assert!(plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("tests/foo/greet.test.ts")));
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("src/src/foo/greet.ts")));
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("tests/tests/foo/greet.test.ts")));
}

#[test]
fn plan_generation_with_source_map_reports_unknown_output_placeholder() {
    let temp = TestDir::new();
    let package_root = temp.path().join("invalid-output-pattern");
    fs::create_dir_all(package_root.join(".mds/source/foo")).unwrap();
    write_npm_runtime_descriptors(&package_root);
    fs::write(
        package_root.join("package.json"),
        "{\"name\":\"invalid-output-pattern\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package_root.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[output]\nsource = \"{source_out}/{module}.{unknown}\"\n",
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/source/foo/bad.ts.md"),
        "# bad\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Fail planning on unknown placeholder.\n\n## Source\n\n```ts\nexport const bad = true;\n```\n",
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut docs_state = mds_core::RunState::default();
    let docs = mds_core::markdown::load_implementation_docs(&package, &mut docs_state).unwrap();
    assert!(
        docs_state.diagnostics.is_empty(),
        "{:?}",
        docs_state.diagnostics
    );

    let mut plan_state = mds_core::RunState::default();
    let plan = mds_core::plan_generation_with_source_map(&package, &docs, &mut plan_state);
    assert!(plan_state.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unknown output path placeholder `{unknown}`")
    }));
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("src/foo/bad.ts")));
}

#[test]
fn package_metadata_dependencies_do_not_require_markdown_mirror() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"scripts\":{\"lint\":\"node -e \\\"process.exit(0)\\\"\",\"typecheck\":\"node -e \\\"process.exit(0)\\\"\",\"test\":\"node -e \\\"process.exit(0)\\\"\"},\"dependencies\":{\"left-pad\":\"1.3.0\"}}\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn package_sync_skips_markdown_package_metadata() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.2.0\",\"dependencies\":{\"left-pad\":\"1.3.0\"},\"devDependencies\":{\"vitest\":\"2.0.0\"}}\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: true },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("dependency snapshot is not synchronized"));

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);
    assert!(sync.stdout.contains("package sync ok"));
    let overview = fs::read_to_string(temp.path().join("pkg/.mds/source/overview.md")).unwrap();
    assert!(overview.contains("left-pad"));
    assert!(overview.contains("vitest"));
}

#[test]
fn reports_unsupported_config_key_as_warning() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nunknown = true\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
    assert!(check.stderr.contains("warning:"));
}

#[test]
fn allows_import_lines_inside_source_code_blocks() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/mixed.ts.md"),
        "# Mixed\n\n## Purpose\n\nMixed imports.\n\n## Contract\n\n- Allow import lines inside Source fences in the language-agnostic core path.\n\n## Source\n\n```ts\nimport { util } from './util';\nexport const mixed = util;\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn build_keeps_typescript_source_code_fence_without_rendering_imports_table() {
    let temp = TestDir::new();
    let package = temp.path().join("ts-import-fixture");
    fs::create_dir_all(package.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"ts-import-fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| ts-import-fixture | 0.1.0 |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/greet.ts.md"),
        "# greet\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Render a greeting through the formatter.\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| external | ./format-name | formatName | - | formatter | - |\n\n## Source\n\n```ts\nexport function greet(name: string) {\n  return formatName(name);\n}\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);

    let generated = fs::read_to_string(package.join("src/greet.ts")).unwrap();
    assert!(!generated.contains("import { formatName } from './format-name';"));
    assert!(generated.contains("return formatName(name);"));
}

#[test]
fn build_keeps_python_source_code_fence_without_rendering_imports_table() {
    let temp = TestDir::new();
    let package = temp.path().join("py-import-fixture");
    fs::create_dir_all(package.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join("pyproject.toml"),
        "[project]\nname = 'py-import-fixture'\nversion = '0.1.0'\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| py-import-fixture | 0.1.0 |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/format_name.py.md"),
        "# format_name\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Return the supplied name.\n\n##### format-name\n\nShared formatter.\n\n## Source\n\n```py\ndef format_name(name: str) -> str:\n    return name\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/greet.py.md"),
        "# greet\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Render a greeting through the formatter.\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| internal | [format_name](./format_name.py.md#format-name) | format_name | - | formatter | [format_name](./format_name.py.md#format-name) |\n\n## Source\n\n```py\ndef greet(name: str) -> str:\n    return format_name(name)\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);

    let generated = fs::read_to_string(package.join("src/greet.py")).unwrap();
    assert!(!generated.contains("from format_name import format_name"));
    assert!(generated.contains("return format_name(name)"));
}

#[test]
fn build_keeps_source_exports_for_all_language_descriptors_without_rendering_imports() {
    let temp = TestDir::new();
    let package = temp.path().join("all-language-imports");
    let descriptors = language_descriptor_examples();

    fs::create_dir_all(package.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("package.json"),
        "{\"name\":\"all-language-imports\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nAll language import fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| all-language-imports | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture.\n",
    )
    .unwrap();

    for descriptor in &descriptors {
        let dir = package.join(".mds/source").join(&descriptor.id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("feature.{}.md", descriptor.suffix)),
            all_language_import_doc(descriptor),
        )
        .unwrap();
    }

    let sync = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let result = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let generated_sources = generated_texts_under(&package.join("src"));
    for descriptor in &descriptors {
        let marker = export_marker(&descriptor.id);
        let generated = generated_sources
            .iter()
            .find(|content| content.contains(&marker))
            .unwrap_or_else(|| panic!("missing generated source for {}", descriptor.id));
        if let Some(import) = expected_import_statement(descriptor) {
            assert!(
                !generated.contains(&import),
                "{} generated source unexpectedly contained import `{}`:\n{}",
                descriptor.id,
                import,
                generated
            );
        }
        assert!(generated.contains(&marker));
        assert!(!generated.contains("| Name | Visibility | Summary |"));
    }
}

#[test]
fn build_keeps_test_code_fence_without_rendering_imports_table() {
    let temp = TestDir::new();
    let package = temp.path().join("test-import-fixture");
    fs::create_dir_all(package.join(".mds/source")).unwrap();
    fs::create_dir_all(package.join(".mds/test")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"test-import-fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| test-import-fixture | 0.1.0 |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/greet.ts.md"),
        "# greet\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Return a greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/test/greet.test.ts.md"),
        "# greet test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- greet\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| external | vitest | describe, expect, it | - | test helpers | - |\n| internal | ../src/greet | greet | - | function under test | [greet source](../source/greet.ts.md) |\n\n## Cases\n\n- Renders a greeting for a valid name.\n\n## Test\n\n```ts\ndescribe('greet', () => {\n  it('renders a greeting', () => {\n    expect(greet('Ada')).toBe('Hello, Ada');\n  });\n});\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);

    let generated = fs::read_to_string(package.join("tests/greet.test.test.ts")).unwrap();
    assert!(!generated.contains("import { describe, expect, it } from 'vitest';"));
    assert!(!generated.contains("import { greet } from '../src/greet';"));
    assert!(generated.contains("describe('greet'"));
}

#[test]
fn allows_multiple_top_level_implementations_in_one_code_block() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/multiple.ts.md"),
        "# Multiple\n\n## Purpose\n\nMultiple declarations.\n\n## Contract\n\n- Allow multiple declarations when configured.\n\n## Source\n\n```ts\nexport const first = 1;\nexport const second = 2;\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn allows_multiple_go_top_level_implementations_in_one_code_block() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/multiple.go.md"),
        "# Multiple\n\n## Purpose\n\nMultiple declarations.\n\n## Contract\n\n- Allow multiple top-level declarations in one fence in the language-agnostic core path.\n\n## Source\n\n```go\nfunc first() {}\nfunc second() {}\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn allows_doc_comments_inside_code_blocks() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/doc-comment.rs.md"),
        "# Doc comment\n\n## Purpose\n\nBroken doc comment.\n\n## Contract\n\n- Allow doc comments inside Source fences in the language-agnostic core path.\n\n## Source\n\n```rs\n/// Move me outside the fence.\npub fn broken() {}\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn allows_docstrings_inside_code_blocks_without_check_override() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/doc-comment.py.md"),
        "# Doc comment\n\n## Purpose\n\nBroken doc comment.\n\n## Contract\n\n- Allow docstrings in the standard core validation path.\n\n## Source\n\n```py\ndef broken() -> str:\n    \"\"\"Allowed in the language-agnostic core path.\"\"\"\n    return \"ok\"\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn workspace_descriptor_doc_comment_prefixes_do_not_trigger_core_lint() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::create_dir_all(temp.path().join(".mds/descriptors/languages")).unwrap();
    fs::write(
        temp.path().join(".mds/descriptors/languages/ts.toml"),
        r#"id = "ts"
aliases = ["typescript"]
match_suffixes = ["ts"]

[language]
primary_ext = "ts"

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "ts"

[files.types]
strip_lang_ext = true
prefix = ""
suffix = ".types"
extension = "ts"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = ".test"
extension = "ts"

[syntax]
top_level_keywords = ["export const ", "const "]
comment_prefixes = ["//", "/*", "*"]
doc_comment_prefixes = ["///"]

[[syntax.imports]]
starts_with = "import "
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("pkg/.mds/source/foo/doc-comment.ts.md"),
        "# Doc comment\n\n## Purpose\n\nBroken doc comment.\n\n## Contract\n\n- Ignore descriptor-level doc comment prefixes in the language-agnostic core path.\n\n## Source\n\n```ts\n/// Configured via descriptor TOML.\nexport const broken = 1;\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn rejects_unterminated_code_fence() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/unterminated.ts.md"),
        "# Unterminated\n\n## Purpose\n\nBroken fence.\n\n## Source\n\n```ts\nexport const broken = 1;\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("unterminated code fence"));
}

#[test]
fn check_config_can_disable_markdown_link_validation() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[check]\nmarkdown_links = false\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. [Missing](missing.md)");
    fs::write(doc, text).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn lint_fix_respects_markdown_links_flag_for_link_policy_normalization() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[check]\nmarkdown_links = false\n\n[authoring]\nlink_policy = \"markdown-only\"\n\n[quality.ts]\nlinter = false\nfixer = false\nrequired = []\noptional = []\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [[foo.util|Util]].");
    fs::write(&doc, text).unwrap();
    let original = fs::read_to_string(&doc).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(lint.exit_code, 0, "{}", lint.stderr);

    let fix_check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: true,
        },
    });
    assert_eq!(fix_check.exit_code, 0, "{}", fix_check.stderr);
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);
}

#[test]
fn rejects_new_fence_opener_before_previous_fence_closes() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/reopened.ts.md"),
        "# Reopened\n\n## Purpose\n\nBroken fence.\n\n## Source\n\n````ts\nexport const first = 1;\n````ts\nexport const second = 2;\n````\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("before a new fence opener"));
}

#[test]
fn rejects_duplicate_h2_sections() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/foo/duplicate-sections.ts.md"),
        "# Duplicate\n\n## Purpose\n\nFirst.\n\n## Source\n\n```ts\nexport const first = 1;\n```\n\n## Source\n\n```ts\nexport const second = 2;\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("duplicate H2 section"));
}

#[test]
fn package_check_uses_language_metadata_without_markdown_mirror() {
    let temp = TestDir::new();
    write_fixture(temp.path());

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn accepts_spec_state_source_doc_without_code_blocks() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/spec-only.ts.md"),
        "# Spec only\n\n## Purpose\n\nDescribe planned behavior.\n\n## Contract\n\n- Keep the planned boundary stable.\n\n## Exports\n\n| Name | Visibility | Summary |\n| --- | --- | --- |\n| SpecOnly | public | Planned source boundary. |\n\n##### SpecOnly\n\nPlanned source boundary referenced before implementation exists.\n\n## Cases\n\n- Planned behavior is documented before code is generated.\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn rejects_export_without_summary_or_h5_definition() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/undocumented.ts.md"),
        "# Undocumented\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Stable behavior.\n\n## Exports\n\n| Name | Visibility | Summary |\n| --- | --- | --- |\n| Undocumented | public | - |\n\n## Source\n\n```ts\nexport const undocumented = true;\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("export `Undocumented` requires a non-empty Summary"));
    assert!(check
        .stderr
        .contains("export `Undocumented` requires a matching H5 shared definition"));
}

#[test]
fn rejects_broken_manifest_before_building() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::create_dir_all(temp.path().join("pkg/.mds")).unwrap();
    fs::write(temp.path().join("pkg/.mds/manifest.toml"), "not manifest\n").unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 1);
    assert!(build.stderr.contains("manifest schema requires"));
    assert!(!temp.path().join("pkg/src/foo/bar.ts").exists());
}

#[test]
fn builds_source_and_test_outputs_from_fixed_authoring_roots() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package.join(".mds/test/foo")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/bar.ts.md"),
        "# Bar\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Generate source output.\n\n## Source\n\n```ts\nexport type Bar = string;\n```\n\n```ts\nexport const bar: Bar = 'ok';\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/test/foo/bar.md"),
        "# Bar test\n\n## Purpose\n\nVerify bar.\n\n## Covers\n\n- foo/bar\n\n## Cases\n\n- returns ok\n\n## Test\n\n```ts\nexpect(bar).toBe('ok');\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(package.join("src/foo/bar.ts").exists());
    assert!(!package.join("src/foo/bar.types.ts").exists());
    assert!(package.join("tests/foo/bar.test.ts").exists());
}

#[test]
fn builds_readable_tableless_source_and_split_test_docs() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    fs::create_dir_all(package.join(".mds/source/app/text")).unwrap();
    fs::create_dir_all(package.join(".mds/test/app")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(package.join(".mds/source/overview.md"), "# Overview\n\n## 目的\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n").unwrap();
    fs::write(
        package.join(".mds/source/app/text/normalize.ts.md"),
        "# app.text.normalize\n\n## 仕様\n\n- 空白名は Anonymous へ正規化する。\n\n## API\n\n`normalizeDisplayName` は表示名を返す。\n\n## 実装\n\n```ts\nexport function normalizeDisplayName(name?: string): string {\n  const trimmed = name?.trim();\n  return trimmed ? trimmed : 'Anonymous';\n}\n```\n",
    ).unwrap();
    fs::write(
        package.join(".mds/source/app/greet.ts.md"),
        "# app.greet\n\nユーザー名から挨拶文を生成する。\n\n## 仕様\n\n- 入力は `GreetOptions`。\n- `name` が未指定または空白のみの場合は `Anonymous` を使う。\n\n## API\n\n`greet` は外部公開API。\n\n## 実装\n\n名前の正規化は [[app.text.normalize#normalizeDisplayName]] に委譲する。\n\n```ts\nimport { normalizeDisplayName } from './text/normalize';\n\nexport type GreetOptions = {\n  name?: string;\n};\n\nexport function greet(options: GreetOptions): string {\n  const name = normalizeDisplayName(options.name);\n  return `Hello, ${name}!`;\n}\n```\n\n## 検証\n\nテストは [[app.greet.test]] に分離する。\n",
    ).unwrap();
    fs::write(
        package.join(".mds/test/app/greet.test.ts.md"),
        "# app.greet.test\n\n[[app.greet]] の挙動を検証する。\n\n## 対象\n\n- [[app.greet]]\n- [[app.greet#greet]]\n\n## ケース\n\n- 通常名\n- 空文字\n\n## 実装\n\n```ts\nimport { describe, expect, it } from 'vitest';\nimport { greet } from '../../src/app/greet';\n\ndescribe('greet', () => {\n  it('uses Anonymous when name is empty', () => {\n    expect(greet({ name: '' })).toBe('Hello, Anonymous!');\n  });\n});\n```\n",
    ).unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    let generated_source = fs::read_to_string(package.join("src/app/greet.ts")).unwrap();
    assert!(generated_source.contains("import { normalizeDisplayName } from './text/normalize';"));
    assert!(generated_source.contains("export function greet"));
    let generated_test = fs::read_to_string(package.join("tests/app/greet.test.test.ts")).unwrap();
    assert!(generated_test.contains("describe('greet'"));
}

#[test]
fn rejects_test_doc_without_covers() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package.join(".mds/test/foo")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/bar.ts.md"),
        "# Bar\n\n## Purpose\n\nFixture.\n\n## Source\n\n```ts\nexport const bar = 'ok';\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/test/foo/bar.md"),
        "# Bar test\n\n## Purpose\n\nVerify bar.\n\n## Test\n\n```ts\nexpect(bar).toBe('ok');\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("test md requires at least one Covers entry"));
}

#[test]
fn rejects_unresolved_module_wikilinks() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    write_minimal_authoring_package(&package, "");
    fs::create_dir_all(package.join(".mds/source/app")).unwrap();
    fs::write(
        package.join(".mds/source/app/greet.ts.md"),
        "# app.greet\n\n## Purpose\n\nSee [[app.missing]].\n\n## Contract\n\n- Return a greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("wiki link target `[[app.missing]]` does not resolve to a module"));
}

#[test]
fn unresolved_module_symbols_follow_policy() {
    for (value, expected_exit_code, expected_level) in [
        ("warn", 0, Some("warning:")),
        ("error", 1, Some("error:")),
        ("allow", 0, None),
    ] {
        let temp = TestDir::new();
        let package = temp.path().join("pkg");
        write_minimal_authoring_package(
            &package,
            &format!("[check]\nunresolved_module_symbols = \"{value}\"\n"),
        );
        fs::create_dir_all(package.join(".mds/source/app")).unwrap();
        fs::write(
            package.join(".mds/source/app/greet.ts.md"),
            "# app.greet\n\n## Purpose\n\n`greet` returns a greeting and references [[app.greet#missingSymbol]].\n\n## Contract\n\n- Return a greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
        )
        .unwrap();

        let check = execute(CliRequest {
            cwd: temp.path().to_path_buf(),
            package: None,
            verbose: false,
            command: Command::Lint {
                fix: false,
                check: false,
            },
        });
        assert_eq!(check.exit_code, expected_exit_code, "{}", check.stderr);
        match expected_level {
            Some(level) => {
                assert!(check.stderr.contains(level), "{}", check.stderr);
                assert!(check.stderr.contains(
                    "wiki link target `[[app.greet#missingSymbol]]` does not resolve to a documented symbol"
                ));
            }
            None => assert!(!check.stderr.contains("missingSymbol"), "{}", check.stderr),
        }
    }
}

#[test]
fn rejects_source_test_section_mixing_by_doc_kind() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    write_minimal_authoring_package(&package, "");
    fs::create_dir_all(package.join(".mds/source/app")).unwrap();
    fs::create_dir_all(package.join(".mds/test/app")).unwrap();
    fs::write(
        package.join(".mds/source/app/greet.ts.md"),
        "# app.greet\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Return a greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n\n## Test\n\n```ts\nexpect(greet('Ada')).toBe('Hello, Ada');\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/test/app/greet.test.ts.md"),
        "# app.greet.test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- [[app.greet]]\n\n## Cases\n\n- returns a greeting\n\n## Source\n\n```ts\nexport const invalid = true;\n```\n\n## Test\n\n```ts\nexpect(greet('Ada')).toBe('Hello, Ada');\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("source md must not contain generated test code in ## Test"));
    assert!(check
        .stderr
        .contains("test md must not contain generated source code in ## Source"));
}

#[test]
fn legacy_table_diagnostics_follow_policy() {
    for (value, expected_exit_code, expected_level) in [
        ("warn", 0, Some("warning:")),
        ("error", 1, Some("error:")),
        ("allow", 0, None),
    ] {
        let temp = TestDir::new();
        let package = temp.path().join("pkg");
        write_minimal_authoring_package(
            &package,
            &format!("[check]\nlegacy_tables = \"{value}\"\n"),
        );
        fs::create_dir_all(package.join(".mds/source/app")).unwrap();
        fs::write(
            package.join(".mds/source/app/greet.ts.md"),
            "# app.greet\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Return a greeting.\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| external | vitest | describe | - | helper | - |\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
        )
        .unwrap();

        let check = execute(CliRequest {
            cwd: temp.path().to_path_buf(),
            package: None,
            verbose: false,
            command: Command::Lint {
                fix: false,
                check: false,
            },
        });
        assert_eq!(check.exit_code, expected_exit_code, "{}", check.stderr);
        match expected_level {
            Some(level) => {
                assert!(check.stderr.contains(level), "{}", check.stderr);
                assert!(check
                    .stderr
                    .contains("legacy table metadata in ## Imports is deprecated"));
            }
            None => assert!(
                !check
                    .stderr
                    .contains("legacy table metadata in ## Imports is deprecated"),
                "{}",
                check.stderr
            ),
        }
    }
}

#[test]
fn test_doc_source_section_does_not_generate_test_output() {
    let temp = TestDir::new();
    let package_root = temp.path().join("pkg");
    write_minimal_authoring_package(&package_root, "[check]\nsplit_source_and_test = false\n");
    fs::create_dir_all(package_root.join(".mds/source/app")).unwrap();
    fs::create_dir_all(package_root.join(".mds/test/app")).unwrap();
    fs::write(
        package_root.join(".mds/source/app/greet.ts.md"),
        "# app.greet\n\n## Purpose\n\n`greet` returns a greeting.\n\n## Contract\n\n- Return a greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package_root.join(".mds/test/app/greet.test.ts.md"),
        "# app.greet.test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- [[app.greet]]\n\n## Cases\n\n- generated output stays empty\n\n## Source\n\n```ts\nexpect(greet('Ada')).toBe('Hello, Ada');\n```\n",
    )
    .unwrap();

    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        &package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut docs_state = mds_core::RunState::default();
    let docs = mds_core::markdown::load_implementation_docs(&package, &mut docs_state).unwrap();
    assert!(
        docs_state.diagnostics.is_empty(),
        "{:?}",
        docs_state.diagnostics
    );

    let mut plan_state = mds_core::RunState::default();
    let plan = mds_core::plan_generation_with_source_map(&package, &docs, &mut plan_state);
    assert!(
        plan_state.diagnostics.is_empty(),
        "{:?}",
        plan_state.diagnostics
    );
    assert!(!plan
        .generated
        .iter()
        .any(|file| file.path == package_root.join("tests/app/greet.test.test.ts")));
}

#[test]
fn lint_fix_check_reports_diff_without_writing_and_fix_writes_code_blocks_only() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let fixer = write_tool(
        temp.path(),
        "fixer",
        "#!/bin/sh\nprintf 'formatted_code()\n' > \"$1\"\n",
    );
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\nallow_raw_source = false\n\n[quality.ts]\nfixer = \"{}\"\nrequired = []\noptional = []\n\n[quality.py]\nfixer = false\nrequired = []\noptional = []\n\n[quality.rs]\nfixer = false\nrequired = []\noptional = []\n",
            fixer.display()
        ),
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let original = fs::read_to_string(&doc).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: true,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stdout.contains("formatted_code"));
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    let fixed = fs::read_to_string(&doc).unwrap();
    assert!(fixed.contains("formatted_code()"));
    assert!(fixed.contains("## Purpose"));
    assert!(fixed.contains("## Source"));
}

#[test]
fn lint_fix_rejects_invalid_rewrite_without_writing_doc() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let fixer = write_tool(temp.path(), "fixer", "#!/bin/sh\nprintf '```\n' > \"$1\"\n");
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\nallow_raw_source = false\n\n[quality.ts]\nfixer = \"{}\"\nrequired = []\noptional = []\n\n[quality.py]\nfixer = false\nrequired = []\noptional = []\n\n[quality.rs]\nfixer = false\nrequired = []\noptional = []\n",
            fixer.display()
        ),
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let original = fs::read_to_string(&doc).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: true,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(
        check.stderr.contains("unterminated code fence"),
        "{}",
        check.stderr
    );
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 1);
    assert!(
        fix.stderr.contains("unterminated code fence"),
        "{}",
        fix.stderr
    );
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);
}

#[test]
fn lint_and_test_use_configured_toolchain_commands() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let tool = write_tool(
        temp.path(),
        "ok-tool",
        "#!/bin/sh\ncat >/dev/null\nexit 0\n",
    );
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\nallow_raw_source = false\n\n[quality.ts]\nlinter = \"{}\"\ntest_runner = \"{}\"\nrequired = []\noptional = []\n\n[quality.py]\nlinter = false\ntest_runner = false\nrequired = []\noptional = []\n\n[quality.rs]\nlinter = false\ntest_runner = false\nrequired = []\noptional = []\n",
            tool.display(),
            tool.display()
        ),
    )
    .unwrap();

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(lint.exit_code, 0, "{}", lint.stderr);
    assert!(lint.stdout.contains("lint ok"));

    let test = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Test,
    });
    assert_eq!(test.exit_code, 0, "{}", test.stderr);
    assert!(test.stdout.contains("test ok"));
}

#[test]
fn lint_passes_only_code_fence_content_without_import_table_prefix() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    write_npm_runtime_descriptors(&package);
    let tool = write_tool(
        temp.path(),
        "capture-stdin",
        "#!/bin/sh\nmkdir -p \"$(dirname \"$1\")\"\ncat >> \"$1.captured\"\nexit 0\n",
    );
    fs::write(
        package.join("mds.config.toml"),
        format!(
            "[package]\nenabled = true\nallow_raw_source = false\n\n[quality.ts]\nlinter = \"{}\"\nrequired = []\noptional = []\n\n[quality.py]\nlinter = false\nrequired = []\noptional = []\n\n[quality.rs]\nlinter = false\nrequired = []\noptional = []\n",
            tool.display()
        ),
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        "{\"name\":\"lint-import-fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| lint-import-fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/util.ts.md"),
        "# Util\n\n## Purpose\n\nFixture helper.\n\n## Contract\n\n- Return a stable value.\n\n## Source\n\n```ts\nexport const util = 'ok';\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/bar.ts.md"),
        "# Bar\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Preserve fixture behavior.\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| internal | ./util | util | - | helper | [[foo.util|util]] |\n\n## Source\n\n```ts\nexport const bar = util;\n```\n",
    )
    .unwrap();

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(lint.exit_code, 0, "{}", lint.stderr);

    let captured = fs::read_to_string(package.join(".build/mds/tmp/source.ts.captured")).unwrap();
    assert!(!captured.contains("import { util } from './util';"));
    assert!(captured.contains("export const bar = util;"));
}

#[test]
fn lint_fix_remaps_second_code_fence_diagnostics_with_source_map() {
    let temp = TestDir::new();
    let package = copy_example_package_to(temp.path(), "broken-remap-ts", "pkg");
    let fixer = write_tool(
        temp.path(),
        "multifence-fixer",
        "#!/bin/sh\nif grep -q 'export const bar:' \"$1\"; then\n  printf '%s:1:1: lint failed\\n' \"$1\" >&2\n  exit 1\nfi\nexit 0\n",
    );
    replace_quality_ts_command(
        &package,
        "fixer",
        "npx --no-install prettier --write",
        &fixer.display().to_string(),
    );

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: true,
        },
    });

    let md_path = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    assert_eq!(lint.exit_code, 1);
    assert!(
        lint.stderr.contains(&format!("{}:18:1", md_path.display())),
        "{}",
        lint.stderr
    );
    assert!(!lint.stderr.contains(".build/mds/tmp/source.ts"));
}

#[test]
fn lint_preserves_mixed_remap_success_failure_and_noop_diagnostics() {
    let temp = TestDir::new();
    let package = copy_example_package_to(temp.path(), "broken-remap-ts", "pkg");
    let tool = write_tool(
        temp.path(),
        "mixed-remap-linter",
        "#!/bin/sh\nprintf '%s:2:1: remapped lint failed\\n' \"$1\" >&2\nprintf '%s:999:2: unremappable lint failed\\n' \"$1\" >&2\nprintf 'foreign.ts:3:4: passthrough lint failed\\n' >&2\nexit 1\n",
    );
    replace_quality_ts_command(
        &package,
        "linter",
        "npx --no-install prettier --check",
        &tool.display().to_string(),
    );

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });

    let md_path = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let generated_path = temp.path().join("pkg/.build/mds/tmp/source.ts");
    assert_eq!(lint.exit_code, 1, "{}", lint.stderr);
    assert!(
        lint.stderr.contains(&format!("{}:18:1", md_path.display())),
        "{}",
        lint.stderr
    );
    assert!(
        lint.stderr.contains(&format!(
            "{}: LINT001_TOOLCHAIN_FAILED: toolchain command failed: {}:999:2: unremappable lint failed",
            md_path.display(),
            generated_path.display()
        )),
        "{}",
        lint.stderr
    );
    assert!(
        lint.stderr
            .contains("foreign.ts:3:4: passthrough lint failed"),
        "{}",
        lint.stderr
    );
    assert!(
        !lint
            .stderr
            .contains(&format!("{}:999:2", md_path.display())),
        "{}",
        lint.stderr
    );
}

#[test]
fn lint_reports_generic_failure_when_source_map_cannot_remap_line() {
    let temp = TestDir::new();
    let package = copy_example_package_to(temp.path(), "broken-remap-ts", "pkg");
    let tool = write_tool(
        temp.path(),
        "out-of-range-linter",
        "#!/bin/sh\nprintf '%s:999:1: lint failed\n' \"$1\" >&2\nexit 1\n",
    );
    replace_quality_ts_command(
        &package,
        "linter",
        "npx --no-install prettier --check",
        &tool.display().to_string(),
    );

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });

    let md_path = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    assert_eq!(lint.exit_code, 1, "{}", lint.stderr);
    assert!(!lint
        .stderr
        .contains(&format!("{}:999:1", md_path.display())));
    assert!(
        lint.stderr.contains("toolchain command failed"),
        "{}",
        lint.stderr
    );
    assert!(
        lint.stderr.contains(".build/mds/tmp/source.ts:999:1"),
        "{}",
        lint.stderr
    );
}

#[test]
fn lint_uses_descriptor_slot_behavior_before_tool_manifest_fallback() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let eslint = write_tool(
        temp.path(),
        "eslint",
        "#!/bin/sh\nprintf 'error: %s:2:1: lint failed\n' \"$1\" >&2\nexit 1\n",
    );
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\nallow_raw_source = false\n\n[quality.ts]\nlinter = \"{}\"\nrequired = []\noptional = []\n\n[quality.py]\nlinter = false\nrequired = []\noptional = []\n\n[quality.rs]\nlinter = false\nrequired = []\noptional = []\n",
            eslint.display()
        ),
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });

    let md_path = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    assert_eq!(lint.exit_code, 1);
    assert!(
        lint.stderr.contains(&format!("{}:18:1", md_path.display())),
        "{}",
        lint.stderr
    );
    assert!(
        !lint.stderr.contains("toolchain command failed"),
        "{}",
        lint.stderr
    );
}

#[test]
fn lint_reports_environment_missing_as_exit_code_four() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\n\n[quality.ts]\nlinter = \"/missing/mds-tool\"\nrequired = []\noptional = []\n\n[quality.py]\nlinter = false\nrequired = []\noptional = []\n\n[quality.rs]\nlinter = false\nrequired = []\noptional = []\n",
    )
    .unwrap();

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(lint.exit_code, 4);
    assert!(lint.stderr.contains("required toolchain"));
}

#[test]
fn doctor_outputs_json_and_uses_exit_code_four_for_missing_required_tools() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\n\n[adapters.py]\nenabled = false\n\n[adapters.rs]\nenabled = false\n\n[quality.ts]\nrequired = [\"/missing/mds-doctor-tool\"]\noptional = []\n",
    )
    .unwrap();

    let doctor = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Doctor {
            format: mds_core::DoctorFormat::Json,
        },
    });
    assert_eq!(doctor.exit_code, 4);
    assert!(doctor.stdout.starts_with("{\"checks\":"));
    assert!(doctor.stdout.contains("/missing/mds-doctor-tool"));
}

#[test]
fn doctor_uses_configured_version_floor_for_required_tools() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let node = write_tool(temp.path(), "node", "#!/bin/sh\nprintf 'v23.0.0\n'\n");
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\n\n[adapters.py]\nenabled = false\n\n[adapters.rs]\nenabled = false\n\n[quality.ts]\nrequired = [\"{}\"]\noptional = []\n\n[doctor.version_floor]\nnode = \"24.0\"\n",
            node.display()
        ),
    )
    .unwrap();

    let doctor = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Doctor {
            format: mds_core::DoctorFormat::Text,
        },
    });
    assert_eq!(doctor.exit_code, 4);
    assert!(doctor.stderr.contains("DOCTOR002_VERSION_TOO_OLD"));
}

#[test]
fn doctor_does_not_use_hardcoded_version_floor_when_config_omits_it() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let node = write_tool(temp.path(), "node", "#!/bin/sh\nprintf 'v23.0.0\n'\n");
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\n\n[adapters.py]\nenabled = false\n\n[adapters.rs]\nenabled = false\n\n[quality.ts]\nrequired = [\"{}\"]\noptional = []\n",
            node.display()
        ),
    )
    .unwrap();

    let doctor = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Doctor {
            format: mds_core::DoctorFormat::Text,
        },
    });
    assert_eq!(doctor.exit_code, 0, "{}", doctor.stderr);
    assert!(!doctor.stderr.contains("DOCTOR002_VERSION_TOO_OLD"));
    assert!(doctor.stdout.contains(node.to_string_lossy().as_ref()));
    assert!(doctor.stdout.contains("available"));
}

#[test]
fn doctor_warns_for_optional_tool_version_floor_without_failing() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let node = write_tool(temp.path(), "node", "#!/bin/sh\nprintf 'v23.0.0\n'\n");
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\n\n[adapters.py]\nenabled = false\n\n[adapters.rs]\nenabled = false\n\n[quality.ts]\nrequired = []\noptional = [\"{}\"]\n\n[doctor.version_floor]\nnode = \"24.0\"\n",
            node.display()
        ),
    )
    .unwrap();

    let doctor = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Doctor {
            format: mds_core::DoctorFormat::Text,
        },
    });
    assert_eq!(doctor.exit_code, 0, "{}", doctor.stderr);
    assert!(doctor.stderr.contains("DOCTOR002_VERSION_TOO_OLD"));
    assert!(doctor.stdout.contains("warning"));
}

#[test]
fn exclude_skips_markdown_discovery_and_generation_outputs() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[roots]\nexclude = [\".mds/source/foo/bar.rs.md\", \"src/foo/bar.rs\"]\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(!temp.path().join("pkg/src/foo/bar.rs").exists());
    assert!(temp.path().join("pkg/src/foo/bar.ts").exists());
}

#[test]
fn label_overrides_preserve_canonical_table_and_section_meaning() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[labels]\nfrom = \"Origin\"\ntarget = \"Module\"\nexpose = \"Symbols\"\nsummary = \"Notes\"\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace(
            "| From | Target | Expose | Summary |\n| --- | --- | --- | --- |\n| internal | foo/util | Util | helper |",
            "| Origin | Module | Symbols | Notes |\n| --- | --- | --- | --- |\n| internal | foo/util | Util | helper |",
        );
    fs::write(doc, text).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn rejects_types_label_override() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[labels]\ntypes = \"Type Definitions\"\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("unsupported label override `types`"));
}

#[test]
fn validates_local_markdown_links() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. [Missing](missing.md)");
    fs::write(doc, text).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("Markdown link target does not exist"));
}

#[test]
fn rejects_unknown_authoring_link_policy() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[authoring]\nlink_policy = \"invalid\"\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("unsupported link policy `invalid`"));
}

#[test]
fn markdown_only_link_policy_rejects_wikilinks() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[authoring]\nlink_policy = \"markdown-only\"\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [[foo.util]].");
    fs::write(doc, text).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("markdown-only"), "{}", check.stderr);
}

#[test]
fn mixed_link_policy_accepts_wiki_and_markdown_module_links() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[authoring]\nlink_policy = \"mixed\"\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc).unwrap().replace(
        "Fixture.",
        "Fixture. See [[foo.util]]. Also [Util](foo.util).",
    );
    fs::write(doc, text).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn lint_fix_preserves_links_for_mixed_link_policy() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[authoring]\nlink_policy = \"mixed\"\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc).unwrap().replace(
        "Fixture.",
        "Fixture. See [[foo.util]]. Also [Util](foo.util).",
    );
    fs::write(&doc, text).unwrap();
    let original = fs::read_to_string(&doc).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);
}

#[test]
fn lint_fix_normalizes_markdown_links_to_default_wiki_only_policy() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [Util](foo.util).");
    fs::write(&doc, text).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    let fixed = fs::read_to_string(&doc).unwrap();
    assert!(fixed.contains("[[foo.util|Util]]"));
    assert!(!fixed.contains("[Util](foo.util)"));
}

#[test]
fn lint_fix_normalizes_source_overview_links_without_breaking_managed_sections() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let overview = temp.path().join("pkg/.mds/source/overview.md");
    let text = fs::read_to_string(&overview).unwrap().replace(
        "Fixture package.",
        "Fixture package. See [Util](foo.util).\n",
    );
    fs::write(&overview, text).unwrap();
    let original = fs::read_to_string(&overview).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: true,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("lint --fix --check found markdown changes"));
    assert_eq!(fs::read_to_string(&overview).unwrap(), original);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    let fixed = fs::read_to_string(&overview).unwrap();
    assert!(fixed.contains("[[foo.util|Util]]"), "{}", fixed);
    assert!(!fixed.contains("[Util](foo.util)"));
    assert!(fixed.contains("### Package Summary"));
    assert!(fixed.contains("### Dependencies"));
    assert!(fixed.contains("### Dev Dependencies"));
}

#[test]
fn lint_fix_rejects_unresolved_wikilink_targets_created_by_markdown_normalization() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [Missing](foo.missing).\n");
    fs::write(&doc, text).unwrap();
    let original = fs::read_to_string(&doc).unwrap();

    let lint = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(lint.exit_code, 1);
    assert!(lint
        .stderr
        .contains("link target `[Missing](foo.missing)` does not resolve to a module"));

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: true,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("wiki link target `[[foo.missing|Missing]]` does not resolve to a module"));
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 1, "{}", fix.stderr);
    assert!(fix
        .stderr
        .contains("wiki link target `[[foo.missing|Missing]]` does not resolve to a module"));
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);
}

#[test]
fn lint_and_fix_ignore_inline_code_link_examples() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[quality.ts]\nlinter = false\nfixer = false\nrequired = []\noptional = []\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc).unwrap().replace(
        "Fixture.",
        "Fixture. Example `[Util](foo.util)` and `[[foo.util|Util]]`.",
    );
    fs::write(&doc, text).unwrap();
    let original = fs::read_to_string(&doc).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);
}

#[test]
fn lint_fix_normalizes_internal_relative_markdown_links_to_canonical_wikilinks() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [Util](./util.ts.md).")
        .replace(
            "Preserve fixture behavior.",
            "Preserve fixture behavior. Also [Py Foo](../pkg/foo.py.md).\n",
        );
    fs::write(&doc, text).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    let fixed = fs::read_to_string(&doc).unwrap();
    assert!(fixed.contains("[[foo.util|Util]]"), "{}", fixed);
    assert!(fixed.contains("[[pkg.foo|Py Foo]]"), "{}", fixed);
    assert!(!fixed.contains("[[./util.ts.md|Util]]"), "{}", fixed);
    assert!(!fixed.contains("[[../pkg/foo.py.md|Py Foo]]"), "{}", fixed);
}

#[test]
fn lint_fix_does_not_generate_invalid_wikilinks_for_non_mds_relative_markdown_targets() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(temp.path().join("pkg/README.md"), "# Fixture\n").unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [README](../../../README.md).")
        .replace("Preserve fixture behavior.", "Preserve fixture behavior.\n");
    fs::write(&doc, text).unwrap();

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 1, "{}", fix.stderr);
    assert!(fix.stderr.contains("wiki-only"), "{}", fix.stderr);
    let fixed = fs::read_to_string(&doc).unwrap();
    assert!(fixed.contains("[README](../../../README.md)"), "{}", fixed);
    assert!(
        !fixed.contains("[[../../../README.md|README]]"),
        "{}",
        fixed
    );
}

#[test]
fn lint_fix_check_reports_unfixable_link_policy_violation() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(temp.path().join("pkg/README.md"), "# Fixture\n").unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [README](../../../README.md).")
        .replace("Preserve fixture behavior.", "Preserve fixture behavior.\n");
    fs::write(&doc, text).unwrap();
    let original = fs::read_to_string(&doc).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: true,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check.stderr.contains("wiki-only"), "{}", check.stderr);
    assert_eq!(fs::read_to_string(&doc).unwrap(), original);
}

#[test]
fn lint_fix_normalizes_wikilinks_to_markdown_only_policy() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[authoring]\nlink_policy = \"markdown-only\"\n",
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("Fixture.", "Fixture. See [[foo.util|Util]].");
    fs::write(&doc, text).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 0, "{}", fix.stderr);
    let fixed = fs::read_to_string(&doc).unwrap();
    assert!(fixed.contains("[Util](foo.util)"));
    assert!(!fixed.contains("[[foo.util|Util]]"));
}

#[test]
fn markdown_only_link_policy_accepts_markdown_covers_links() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::create_dir_all(temp.path().join("pkg/.mds/test/foo")).unwrap();
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[authoring]\nlink_policy = \"markdown-only\"\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("pkg/.mds/test/foo/bar.test.ts.md"),
        "# Util test\n\n## Purpose\n\nVerify util.\n\n## Covers\n\n- [Util](../../source/foo/util.ts.md)\n\n## Cases\n\n- returns ok\n\n## Test\n\n```ts\nimport { expect, test } from 'vitest';\nimport { util } from '../../src/foo/util';\n\ntest('util', () => {\n  expect(util).toBe('ok');\n});\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn mixed_link_policy_accepts_raw_wiki_and_markdown_covers_entries() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::create_dir_all(temp.path().join("pkg/.mds/test/foo")).unwrap();
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[authoring]\nlink_policy = \"mixed\"\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("pkg/.mds/test/foo/bar.test.ts.md"),
        "# Util test\n\n## Purpose\n\nVerify util. See [[foo.util]].\n\n## Covers\n\n- foo.util\n- [[foo.util]]\n- [Util](../../source/foo/util.ts.md)\n\n## Cases\n\n- returns ok\n\n## Test\n\n```ts\nimport { expect, test } from 'vitest';\nimport { util } from '../../src/foo/util';\n\ntest('util', () => {\n  expect(util).toBe('ok');\n});\n```\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn table_parser_keeps_pipes_inside_code_spans() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace(
            "| internal | foo/util | Util | helper |",
            "| internal | foo/util | Util | helper `a | b` |",
        )
        .replace("## Contract", "[[foo.util|Util]]\n\n## Contract");
    fs::write(doc, text).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);
}

#[test]
fn metadata_parser_accepts_common_json_toml_dependency_shapes() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/package.json"),
        "{\n  \"name\": \"fixture\",\n  \"version\": \"0.1.0\",\n  \"scripts\": {\n    \"lint\": \"node -e \\\"process.exit(0)\\\"\",\n    \"typecheck\": \"node -e \\\"process.exit(0)\\\"\",\n    \"test\": \"node -e \\\"process.exit(0)\\\"\"\n  },\n  \"dependencies\": {\n    \"simple\": \"1.0.0\",\n    \"detailed\": { \"version\": \"2.0.0\" }\n  }\n}\n",
    )
    .unwrap();
    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);
    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 0, "{}", check.stderr);

    let rust_pkg = temp.path().join("rust-pkg");
    fs::create_dir_all(rust_pkg.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&rust_pkg);
    fs::write(
        rust_pkg.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        rust_pkg.join("Cargo.toml"),
        "[package]\nname = \"rust-fixture\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = { version = \"1.0\", features = [\"derive\"] }\n",
    )
    .unwrap();
    fs::write(
        rust_pkg.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nRust fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| rust-fixture | 0.1.0 |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n| serde | 1.0 |  |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Test fixture.\n",
    )
    .unwrap();
    let rust_check = execute(CliRequest {
        cwd: rust_pkg.clone(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(rust_check.exit_code, 0, "{}", rust_check.stderr);
}

#[test]
fn package_sync_requires_source_overview() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::remove_file(temp.path().join("pkg/.mds/source/overview.md")).unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 1);
    assert!(sync.stderr.contains("failed to read source overview"));
}

#[test]
fn build_ignores_nested_overview_for_package_level_validation() {
    let temp = TestDir::new();
    let package = temp.path().join("nested-overview-fixture");
    write_minimal_authoring_package(&package, "");
    fs::create_dir_all(package.join(".mds/source/guides")).unwrap();
    fs::write(
        package.join(".mds/source/guides/overview.md"),
        "# Guide\n\nNested overview note.\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: package,
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::DryRun,
        },
    });

    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(!build.stderr.contains("source overview requires ## Purpose"));
    assert!(!build
        .stderr
        .contains("source overview requires ## Architecture"));
    assert!(!build.stderr.contains("source overview requires ## Rules"));
}

#[test]
fn package_sync_rejects_legacy_marker_only_source_overview() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n<!-- mds:begin package-summary -->\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n<!-- mds:end package-summary -->\n\n<!-- mds:begin dependencies -->\n| Name | Version | Summary |\n| --- | --- | --- |\n<!-- mds:end dependencies -->\n\n<!-- mds:begin dev-dependencies -->\n| Name | Version | Summary |\n| --- | --- | --- |\n<!-- mds:end dev-dependencies -->\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });

    assert_eq!(sync.exit_code, 1);
    assert!(sync
        .stderr
        .contains("source overview is missing managed section `Package Summary`"));
}

#[test]
fn lint_rejects_source_overview_missing_fixed_heading() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });

    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("source overview is missing managed section `Dependencies`"));
}

#[test]
fn lint_rejects_source_overview_missing_required_section() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/.mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });

    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("source overview requires ## Architecture"));
}

#[test]
fn lint_rejects_source_overview_link_policy_and_target_violations() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let overview = temp.path().join("pkg/.mds/source/overview.md");
    let text = fs::read_to_string(&overview).unwrap().replace(
        "Fixture package.",
        "Fixture package. See [Missing](nonexistent.md) and [[foo.missing|Missing]].",
    );
    fs::write(&overview, text).unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });

    assert_eq!(check.exit_code, 1, "{}", check.stderr);
    assert!(check
        .stderr
        .contains("link policy `wiki-only` requires wiki links: `[Missing](nonexistent.md)`"));
    assert!(check
        .stderr
        .contains("Markdown link target does not exist: `nonexistent.md`"));
    assert!(check
        .stderr
        .contains("wiki link target `[[foo.missing|Missing]]` does not resolve to a module"));
}

#[test]
fn check_and_build_reject_stale_dependency_snapshot() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.2.0\",\"dependencies\":{\"left-pad\":\"1.3.0\"}}\n",
    )
    .unwrap();

    let check = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(check.exit_code, 1);
    assert!(check
        .stderr
        .contains("dependency snapshot is not synchronized"));

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 1);
    assert!(build
        .stderr
        .contains("dependency snapshot is not synchronized"));
}

#[test]
fn package_sync_hook_enabled_uses_default_sync_command() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[package_sync]\nhook_enabled = true\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: true },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);
    assert!(sync
        .stdout
        .contains("package sync hook command: mds package sync"));
}

#[test]
fn lint_fix_updates_successful_quality_blocks_only() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    let fixer = write_tool(
        temp.path(),
        "partial-fixer",
        "#!/bin/sh\nif grep -q DO_NOT_FIX \"$1\"; then exit 1; fi\nprintf 'fixed_code()\n' > \"$1\"\n",
    );
    fs::write(
        temp.path().join("pkg/mds.config.toml"),
        format!(
            "[package]\nenabled = true\n\n[quality.ts]\nfixer = \"{}\"\nrequired = []\noptional = []\n\n[quality.py]\nfixer = false\nrequired = []\noptional = []\n\n[quality.rs]\nfixer = false\nrequired = []\noptional = []\n",
            fixer.display()
        ),
    )
    .unwrap();
    let doc = temp.path().join("pkg/.mds/source/foo/bar.ts.md");
    // Replace the last code block content with DO_NOT_FIX to simulate partial failure
    let text = fs::read_to_string(&doc)
        .unwrap()
        .replace("expect(bar).toBe(\"ok\");", "DO_NOT_FIX");
    fs::write(&doc, text).unwrap();

    let fix = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: true,
            check: false,
        },
    });
    assert_eq!(fix.exit_code, 1);
    let fixed = fs::read_to_string(&doc).unwrap();
    // Successful blocks get fixed, failing block stays unchanged
    assert!(fixed.contains("fixed_code()"));
    assert!(fixed.contains("DO_NOT_FIX"));
}

#[test]
fn refuses_to_overwrite_unmanaged_file() {
    let temp = TestDir::new();
    write_fixture(temp.path());
    fs::create_dir_all(temp.path().join("pkg/src/foo")).unwrap();
    fs::write(temp.path().join("pkg/src/foo/bar.ts"), "manual\n").unwrap();

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 1);
    assert!(build.stderr.contains("refusing to overwrite"));
    assert_eq!(
        fs::read_to_string(temp.path().join("pkg/src/foo/bar.ts")).unwrap(),
        "manual\n"
    );
}

#[test]
fn init_ai_plan_does_not_write_without_yes() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: false,
                targets: vec![AiTarget::ClaudeCode],
                categories: vec![AgentKitCategory::Instructions],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("Init plan:"));
    assert!(result.stdout.contains("No changes written"));
    assert!(!temp.path().join("CLAUDE.md").exists());
}

#[test]
fn init_generates_selected_ai_agent_kit_and_project_skeleton() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: vec![AiTarget::ClaudeCode, AiTarget::Opencode],
                categories: vec![AgentKitCategory::Instructions, AgentKitCategory::Commands],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(temp.path().join("mds.config.toml").exists());
    assert!(!temp.path().join("index.md").exists());
    assert!(temp.path().join(".mds/source/overview.md").exists());
    assert!(temp.path().join(".mds/source/index.ts.md").exists());
    assert!(temp.path().join(".mds/test/overview.md").exists());
    assert!(temp.path().join(".claude/rules/mds.md").exists());
    assert!(!temp.path().join(".claude/commands/mds-check.md").exists());
    assert!(temp.path().join(".claude/commands/mds-build.md").exists());
    assert!(temp.path().join(".claude/commands/mds-lint.md").exists());
    assert!(temp.path().join(".opencode/agents/mds-build.md").exists());
    assert!(temp.path().join(".opencode/agents/mds-lint.md").exists());
    assert!(!temp.path().join(".claude/skills/mds/SKILL.md").exists());
    let rules = fs::read_to_string(temp.path().join(".claude/rules/mds.md")).unwrap();
    assert!(rules.contains("mds-managed: true"));
    assert!(rules.contains("mds lint"));
    assert!(rules.contains("Normal import/use/require statements belong in code blocks"));
    assert!(!rules.contains("### {{IMPORTS}} Section"));
    let config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains("type_checker = \"npm run typecheck\""));
    assert!(config.contains("linter = \"npm run lint\""));
    assert!(config.contains("fixer = \"prettier --write\""));
    assert!(config.contains("test_runner = \"npm test\""));
    let overview = fs::read_to_string(temp.path().join(".mds/source/overview.md")).unwrap();
    assert!(!overview.contains("## Exposes"));
    let root_module = fs::read_to_string(temp.path().join(".mds/source/index.ts.md")).unwrap();
    assert!(root_module.contains("## API"));
    assert!(!root_module.contains("## Exports"));
    assert!(!root_module.contains("## Imports"));
    assert!(!root_module.contains("## Source"));
    let reference_root =
        fs::read_to_string(temp.path().join(".mds/reference/root-module.md")).unwrap();
    assert!(reference_root.contains("## API"));
    assert!(reference_root.contains("## Source"));
    assert!(!reference_root.contains("## Exports"));
    assert!(!reference_root.contains("## Imports"));
    let reference_impl = fs::read_to_string(temp.path().join(".mds/reference/impl.md")).unwrap();
    assert!(reference_impl.contains("## API"));
    assert!(reference_impl.contains("import { formatName } from './format-name';"));
    assert!(!reference_impl.contains("## Exports"));
    assert!(!reference_impl.contains("## Imports"));
    assert!(!reference_impl.contains("## Types"));
    let reference_test = fs::read_to_string(temp.path().join(".mds/reference/test.md")).unwrap();
    assert!(reference_test.contains("## Covers"));
    assert!(reference_test.contains("## Test"));
    assert!(reference_test.contains("import { greet } from './greet';"));
    assert!(!reference_test.contains("## Imports"));
    assert!(!reference_test.contains("## Types"));
}

#[test]
fn init_writes_detected_package_manager_scripts_to_config() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("package.json"),
        concat!(
            "{\n",
            "  \"name\": \"init-fixture\",\n",
            "  \"version\": \"0.1.0\",\n",
            "  \"scripts\": {\n",
            "    \"test\": \"jest\"\n",
            "  }\n",
            "}\n"
        ),
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: Vec::new(),
                categories: Vec::new(),
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains("test_runner = \"npm test\""));
    assert!(!config.contains("test_runner = \"vitest run\""));
}

#[test]
fn init_writes_selected_quality_tool_config() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: Vec::new(),
                categories: Vec::new(),
                quality_commands: vec![InitQualityCommands {
                    lang: Lang::Other("ts".to_string()),
                    type_check: None,
                    lint: Some("biome lint".to_string()),
                    fix: Some("biome format --write".to_string()),
                    test: Some("jest".to_string()),
                }],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains("[quality.ts]"));
    assert!(config.contains("linter = \"biome lint\""));
    assert!(config.contains("fixer = \"biome format --write\""));
    assert!(config.contains("test_runner = \"jest\""));
    assert!(config.contains("required = [\"node\", \"biome\", \"jest\"]"));
    assert!(!config.contains("[quality.py]"));
    assert!(!config.contains("[quality.rs]"));
}

#[test]
fn init_writes_custom_quality_commands() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: Vec::new(),
                categories: Vec::new(),
                quality_commands: vec![InitQualityCommands {
                    lang: Lang::Other("ts".to_string()),
                    type_check: Some("npm run typecheck".to_string()),
                    lint: Some("npm run lint".to_string()),
                    fix: Some("npm run fix".to_string()),
                    test: Some("npm test".to_string()),
                }],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains("type_checker = \"npm run typecheck\""));
    assert!(config.contains("linter = \"npm run lint\""));
    assert!(config.contains("fixer = \"npm run fix\""));
    assert!(config.contains("test_runner = \"npm test\""));
}

#[test]
fn init_writes_link_policy_and_custom_labels() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: Vec::new(),
                categories: Vec::new(),
                link_policy: LinkPolicy::MarkdownOnly,
                label_overrides: HashMap::from([
                    ("purpose".to_string(), "Goal".to_string()),
                    ("contract".to_string(), "Guarantee".to_string()),
                    ("source".to_string(), "Implementation".to_string()),
                    ("covers".to_string(), "Verifies".to_string()),
                    ("cases".to_string(), "Scenarios".to_string()),
                    ("test".to_string(), "Checks".to_string()),
                ]),
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains("[authoring]"));
    assert!(config.contains("link_policy = \"markdown-only\""));
    assert!(config.contains("purpose = \"Goal\""));
    assert!(config.contains("contract = \"Guarantee\""));
    assert!(config.contains("source = \"Implementation\""));
    assert!(config.contains("covers = \"Verifies\""));
    assert!(config.contains("cases = \"Scenarios\""));
    assert!(config.contains("test = \"Checks\""));

    let overview = fs::read_to_string(temp.path().join(".mds/source/overview.md")).unwrap();
    assert!(overview.contains("## Goal"));

    let reference_impl = fs::read_to_string(temp.path().join(".mds/reference/impl.md")).unwrap();
    assert!(reference_impl.contains("## Goal"));
    assert!(reference_impl.contains("## Guarantee"));
    assert!(reference_impl.contains("## Implementation"));
    assert!(reference_impl.contains("## Scenarios"));

    let reference_test = fs::read_to_string(temp.path().join(".mds/reference/test.md")).unwrap();
    assert!(reference_test.contains("## Verifies"));
    assert!(reference_test.contains("## Scenarios"));
    assert!(reference_test.contains("## Checks"));
}

#[test]
fn init_generated_covers_label_round_trips_through_config_parser() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let init = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                label_overrides: HashMap::from([("covers".to_string(), "Verifies".to_string())]),
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(init.exit_code, 0, "{}", init.stderr);

    let config_path = temp.path().join("mds.config.toml");
    let mut config = mds_core::Config::default();
    let mut state = mds_core::RunState::default();
    assert!(mds_core::config::merge_config_file(&mut config, &config_path, &mut state).is_some());
    assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
    assert_eq!(
        config.label_overrides.get("covers"),
        Some(&"Verifies".to_string())
    );
}

#[test]
fn init_generated_source_overview_is_package_sync_clean_with_label_preset() {
    let temp = TestDir::new();
    let package = temp.path().join("init-overview-fixture");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("package.json"),
        "{\n  \"name\": \"actual-init-package\",\n  \"version\": \"0.2.0\",\n  \"dependencies\": {\n    \"left-pad\": \"1.3.0\"\n  },\n  \"devDependencies\": {\n    \"vitest\": \"1.6.0\"\n  }\n}\n",
    )
    .unwrap();

    let init = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                label_preset: LabelPreset::Japanese,
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(init.exit_code, 0, "{}", init.stderr);

    let overview = fs::read_to_string(package.join(".mds/source/overview.md")).unwrap();
    assert!(overview.contains("## 目的"));
    assert!(overview.contains("| Name | Version |"));
    assert!(overview.contains("| Name | Version | Summary |"));
    assert!(overview.contains("| actual-init-package | 0.2.0 |"));
    assert!(overview.contains("| left-pad | 1.3.0 |  |"));
    assert!(overview.contains("| vitest | 1.6.0 |  |"));
    assert!(!overview.contains("| 名前 | Version |"));
    assert!(!overview.contains("| Name | Version | 概要 |"));

    let sync = execute(CliRequest {
        cwd: package,
        package: None,
        verbose: false,
        command: Command::PackageSync { check: true },
    });
    assert_eq!(sync.exit_code, 0, "{}\n{}", sync.stdout, sync.stderr);
    assert!(sync.stdout.contains("package sync ok"), "{}", sync.stdout);
    assert!(!sync.stdout.contains("--- "), "{}", sync.stdout);
}

#[test]
fn init_generated_source_overview_uses_non_node_package_metadata() {
    let temp = TestDir::new();
    let package = temp.path().join("init-rust-overview-fixture");
    fs::create_dir_all(&package).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"actual-rust-package\"\n",
            "version = \"0.2.0\"\n",
            "edition = \"2021\"\n\n",
            "[dependencies]\n",
            "serde = \"1.0\"\n\n",
            "[dev-dependencies]\n",
            "insta = \"1.39\"\n",
        ),
    )
    .unwrap();

    let init = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(init.exit_code, 0, "{}", init.stderr);

    let overview = fs::read_to_string(package.join(".mds/source/overview.md")).unwrap();
    assert!(overview.contains("| actual-rust-package | 0.2.0 |"));
    assert!(overview.contains("| serde | 1.0 |  |"));
    assert!(overview.contains("| insta | 1.39 |  |"));

    let sync = execute(CliRequest {
        cwd: package,
        package: None,
        verbose: false,
        command: Command::PackageSync { check: true },
    });
    assert_eq!(sync.exit_code, 0, "{}\n{}", sync.stdout, sync.stderr);
    assert!(sync.stdout.contains("package sync ok"), "{}", sync.stdout);
    assert!(!sync.stdout.contains("--- "), "{}", sync.stdout);
}

#[test]
fn init_uses_descriptor_root_module_markdown_name_for_detected_language() {
    for (metadata_file, metadata, expected_root_module, unexpected_root_module) in [
        (
            "pyproject.toml",
            "[project]\nname = \"init-fixture\"\nversion = \"0.1.0\"\n",
            "__init__.py.md",
            "index.py.md",
        ),
        (
            "Cargo.toml",
            "[package]\nname = \"init-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            "lib.rs.md",
            "index.rs.md",
        ),
    ] {
        let temp = TestDir::new();
        write_npm_runtime_descriptors(temp.path());
        fs::write(temp.path().join(metadata_file), metadata).unwrap();

        let init = execute(CliRequest {
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
        assert_eq!(init.exit_code, 0, "{}", init.stderr);

        assert!(temp
            .path()
            .join(".mds/source")
            .join(expected_root_module)
            .exists());
        assert!(!temp
            .path()
            .join(".mds/source")
            .join(unexpected_root_module)
            .exists());

        let overview = fs::read_to_string(temp.path().join(".mds/source/overview.md")).unwrap();
        assert!(overview.contains(expected_root_module));
        assert!(!overview.contains(unexpected_root_module));
    }
}

#[test]
fn new_root_module_kind_accepts_extensionless_runtime_root_module_name() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    write_multi_part_suffix_descriptor(temp.path(), &["index"]);

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "index.d.ts.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::RootModule),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let content = fs::read_to_string(temp.path().join(".mds/source/index.d.ts.md")).unwrap();
    assert!(content.contains("## API"));
    assert!(!content.contains("## Source"));
}

#[test]
fn init_generates_ai_categories_per_target() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: vec![AiTarget::ClaudeCode, AiTarget::Opencode],
                categories: Vec::new(),
                target_categories: vec![
                    InitTargetCategories {
                        target: AiTarget::ClaudeCode,
                        categories: vec![AgentKitCategory::Instructions],
                    },
                    InitTargetCategories {
                        target: AiTarget::Opencode,
                        categories: vec![AgentKitCategory::Skills],
                    },
                ],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(temp.path().join(".claude/rules/mds.md").exists());
    assert!(!temp.path().join(".claude/skills/mds/SKILL.md").exists());
    assert!(temp.path().join(".opencode/skills/mds/SKILL.md").exists());
    assert!(temp
        .path()
        .join(".opencode/skills/mds-ts/SKILL.md")
        .exists());
    assert!(!temp
        .path()
        .join(".opencode/skills/mds-rs/SKILL.md")
        .exists());
    assert!(!temp.path().join(".claude/skills/mds-ts/SKILL.md").exists());
    assert!(!temp.path().join(".opencode/agents/mds-lint.md").exists());
    let ts_skill =
        fs::read_to_string(temp.path().join(".opencode/skills/mds-ts/SKILL.md")).unwrap();
    assert!(ts_skill.contains("Descriptor import style: `typescript`"));
    assert!(ts_skill.contains("Write dependencies in the code fence itself"));
    assert!(ts_skill.contains("import { ImportedSymbol } from './dep';"));
    assert!(!ts_skill.contains("## Imports"));
    assert!(!ts_skill.contains("## Exports"));
}

#[test]
fn init_uses_active_package_descriptor_for_quality_and_ai_skills() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("package.json"),
        "{\n  \"name\": \"schema-init\",\n  \"version\": \"0.1.0\",\n  \"packageManager\": \"schema-pm@1.0.0\"\n}\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join(".mds/descriptors/package-managers")).unwrap();
    fs::write(
        temp.path().join(".mds/descriptors/package-managers/schema-pm.toml"),
        "id = \"schema-pm\"\ndisplay_name = \"Schema PM\"\nlang = \"schema-lang\"\nmetadata_files = [\"package.json\"]\nmetadata_reader = \"node-package-json\"\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join(".mds/descriptors/languages")).unwrap();
    fs::write(
        temp.path()
            .join(".mds/descriptors/languages/schema-lang.toml"),
        concat!(
            "id = \"schema-lang\"\n",
            "aliases = [\"schema\"]\n",
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
            "lint = \"schemafmt check\"\n",
        ),
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: vec![AiTarget::Opencode],
                categories: vec![AgentKitCategory::Skills],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains("[quality.schema-lang]"));
    assert!(config.contains("linter = \"schemafmt check\""));
    assert!(!config.contains("[quality.ts]"));

    assert!(temp
        .path()
        .join(".opencode/skills/mds-schema-lang/SKILL.md")
        .exists());
    assert!(!temp
        .path()
        .join(".opencode/skills/mds-ts/SKILL.md")
        .exists());

    let skill = fs::read_to_string(
        temp.path()
            .join(".opencode/skills/mds-schema-lang/SKILL.md"),
    )
    .unwrap();
    assert!(skill.contains("descriptor `schema-lang`"));
    assert!(skill.contains("```schema"));
}

#[test]
fn init_generated_ai_kit_uses_new_command_path_kind_contract() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: AiTarget::all().to_vec(),
                categories: AgentKitCategory::all().to_vec(),
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let corpus = generated_texts_under(temp.path()).join("\n");
    for legacy in [
        "`mds new <name.lang.md>`",
        "`mds new overview.md`",
        "`mds new lib.rs.md` / `mds new sub/mod.rs.md` / `mds new index.ts.md`",
        "`mds new lib.rs.md`, `mds new sub/mod.rs.md`, or `mds new index.ts.md`",
        "mds new greet.ts.md                # New TypeScript feature",
        "mds new utils/helper.py.md         # New Python feature in subdirectory",
        "mds new parser.rs.md               # New Rust feature",
        "sub/overview.md overview",
        "mds new sub/overview.md            # New source overview for subdirectory",
        "mds new sub/mod.rs.md              # New Rust directory root module doc",
        "`mds new greet.ts.md`, `mds new sub/overview.md`",
        "`mds new greet.ts.md`, `mds new sub/overview.md`, `mds new index.ts.md`",
    ] {
        assert!(
            !corpus.contains(legacy),
            "legacy init template syntax remained in generated corpus: {legacy}"
        );
    }

    assert!(corpus.contains("mds new greet.ts.md impl"));
    assert!(corpus.contains("mds new greet.ts.md test"));
    assert!(corpus.contains("mds new overview.md overview"));
    assert!(corpus.contains("mds new index.ts.md root-module"));
}

#[test]
fn init_can_disable_language_quality_tools() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                targets: Vec::new(),
                categories: Vec::new(),
                quality_commands: vec![InitQualityCommands {
                    lang: Lang::Other("ts".to_string()),
                    type_check: None,
                    lint: None,
                    fix: None,
                    test: None,
                }],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let config = fs::read_to_string(temp.path().join("mds.config.toml")).unwrap();
    assert!(config.contains(
        "[quality.ts]\nlinter = false\nfixer = false\ntest_runner = false\nrequired = []"
    ));
    assert!(!config.contains("[quality.py]"));
    assert!(!config.contains("[quality.rs]"));
}

#[test]
fn init_setup_plan_uses_selected_quality_tools() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                install_toolchains: true,
                targets: Vec::new(),
                categories: Vec::new(),
                quality_commands: vec![InitQualityCommands {
                    lang: Lang::Other("ts".to_string()),
                    type_check: None,
                    lint: Some("biome lint".to_string()),
                    fix: Some("biome format --write".to_string()),
                    test: Some("jest".to_string()),
                }],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("Setup plan:"));
    assert!(result.stdout.contains("biome --version"));
    assert!(result.stdout.contains("jest --version"));
    assert!(!result.stdout.contains("black --version"));
    assert!(!result.stdout.contains("pytest --version"));
    assert!(!result.stdout.contains("cargo-nextest --version"));
    assert!(!temp.path().join("mds.config.toml").exists());
}

#[test]
fn init_refuses_nonmanaged_overwrite_without_force() {
    let temp = TestDir::new();
    fs::create_dir_all(temp.path().join(".claude/rules")).unwrap();
    fs::write(temp.path().join(".claude/rules/mds.md"), "manual\n").unwrap();
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                ai_only: true,
                yes: true,
                targets: vec![AiTarget::ClaudeCode],
                categories: vec![AgentKitCategory::Instructions],
                ..InitOptions::default()
            },
        },
    });
    assert_eq!(result.exit_code, 1);
    assert!(result.stderr.contains("refusing to overwrite non-managed"));
    assert_eq!(
        fs::read_to_string(temp.path().join(".claude/rules/mds.md")).unwrap(),
        "manual\n"
    );
}

#[test]
fn init_reports_setup_partial_failures() {
    let temp = TestDir::new();
    write_init_package_metadata(temp.path());
    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Init {
            options: InitOptions {
                yes: true,
                install_ai_cli: true,
                ..InitOptions::default()
            },
        },
    });
    assert_ne!(result.exit_code, 0);
    assert!(result.stdout.contains("Setup plan:"));
    assert!(result.stderr.contains("setup action"));
    assert!(temp.path().join("mds.config.toml").exists());
}

#[test]
fn new_creates_source_doc_under_fixed_authoring_root() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "greet.ts.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Impl),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let path = temp.path().join(".mds/source/greet.ts.md");
    assert!(path.exists());
    assert!(!temp.path().join("src-md/greet.ts.md").exists());
    let content = fs::read_to_string(path).unwrap();
    assert!(content.contains("## API"));
    assert!(content.contains("## Source"));
    assert!(content.contains("## Cases"));
    assert!(!content.contains("## Exports"));
    assert!(!content.contains("## Imports"));
    assert!(!content.contains("## Types"));
}

#[test]
fn new_creates_metadata_only_root_module_doc() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "index.ts.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::RootModule),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let content = fs::read_to_string(temp.path().join(".mds/source/index.ts.md")).unwrap();
    assert!(content.contains("## API"));
    assert!(!content.contains("## Source"));
    assert!(!content.contains("## Exports"));
    assert!(!content.contains("## Imports"));
    assert!(!content.contains("## Types"));
}

#[test]
fn new_creates_test_doc_under_fixed_test_root() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "greet.ts.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Test),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let path = temp.path().join(".mds/test/greet.ts.md");
    assert!(path.exists());
    let content = fs::read_to_string(path).unwrap();
    assert!(content.contains("## Covers"));
    assert!(content.contains("## Cases"));
    assert!(content.contains("## Test"));
    assert!(content.contains("```ts"));
    assert!(!content.contains("## Source"));
    assert!(!content.contains("## Imports"));
    assert!(!content.contains("## Types"));
}

#[test]
fn new_test_template_uses_canonical_module_id_and_passes_lint_and_build() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    write_minimal_authoring_package(
        &package,
        "[quality.ts]\nlinter = false\nfixer = false\ntest_runner = false\nrequired = []\noptional = []\n",
    );

    let new_impl = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "foo/bar.ts.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Impl),
                force: false,
            },
        },
    });
    assert_eq!(new_impl.exit_code, 0, "{}", new_impl.stderr);

    let new_test = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "foo/bar.ts.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Test),
                force: false,
            },
        },
    });
    assert_eq!(new_test.exit_code, 0, "{}", new_test.stderr);

    let content = fs::read_to_string(package.join(".mds/test/foo/bar.ts.md")).unwrap();
    assert!(content.contains("# Bar test"));
    assert!(content.contains("\n- foo.bar\n"));
    assert!(!content.contains("\n- Bar\n"));

    let lint = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Lint {
            fix: false,
            check: false,
        },
    });
    assert_eq!(lint.exit_code, 0, "{}", lint.stderr);
    assert!(lint.stdout.contains("lint ok"), "{}", lint.stdout);

    let build = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(package.join("src/foo/bar.ts").exists());
    assert!(package.join("tests/foo/bar.test.ts").exists());
}

#[test]
fn build_uses_multi_part_suffix_module_id_for_covers_and_outputs() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    let source_path = package.join(".mds/source/foo/index.d.ts.md");
    let test_path = package.join(".mds/test/foo/index.md");
    write_minimal_authoring_package(&package, "");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package.join(".mds/test/foo")).unwrap();
    write_multi_part_suffix_descriptor(&package, &["index.d.ts.md"]);
    fs::write(
        &source_path,
        "# Index\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Provide declarations.\n\n## Source\n\n```ts\nexport interface Thing { value: string }\n```\n",
    )
    .unwrap();
    fs::write(
        &test_path,
        "# Index test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- foo.index\n\n## Cases\n\n- Resolves declaration source doc.\n\n## Test\n\n```ts\nexpect(true).toBe(true)\n```\n",
    )
    .unwrap();

    let normalized_module_id =
        mds_core::descriptor::with_workspace_descriptor_root(Some(&package), || {
            mds_core::descriptor::markdown_module_id_for_lang(
                &Lang::Other("dts".to_string()),
                Path::new("foo/index.d.ts.md"),
            )
        });
    assert_eq!(normalized_module_id, "foo.index");

    let mut package_state = mds_core::RunState::default();
    let loaded_package =
        mds_core::package::load_package(&package, &mds_core::Config::default(), &mut package_state)
            .unwrap();
    assert!(
        package_state.diagnostics.is_empty(),
        "{:?}",
        package_state.diagnostics
    );

    let source_text = fs::read_to_string(&source_path).unwrap();
    let test_text = fs::read_to_string(&test_path).unwrap();
    let mut probe_state = mds_core::RunState::default();
    let source_doc = mds_core::markdown::parse_impl_doc_text(
        &loaded_package,
        mds_core::DocKind::Source,
        Lang::Other("dts".to_string()),
        &source_path,
        &source_text,
        &mut probe_state,
    )
    .expect("source doc");
    let discovered_test_lang = mds_core::markdown::discover_test_doc_lang_from_text(
        &loaded_package,
        &test_path,
        &test_text,
        &[source_doc],
        &mut probe_state,
    );
    assert_eq!(
        discovered_test_lang,
        Some(Lang::Other("dts".to_string())),
        "{:?}",
        probe_state.diagnostics
    );

    let build = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(package.join("src/foo/index.ts").exists());
    assert!(package.join("tests/foo/index.test.ts").exists());
}

#[test]
fn build_rejects_stale_single_segment_module_id_for_multi_part_suffix_doc() {
    let temp = TestDir::new();
    let package = temp.path().join("pkg");
    write_minimal_authoring_package(&package, "");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package.join(".mds/test/foo")).unwrap();
    write_multi_part_suffix_descriptor(&package, &["index.d.ts.md"]);
    fs::write(
        package.join(".mds/source/foo/index.d.ts.md"),
        "# Index\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Provide declarations.\n\n## Source\n\n```ts\nexport interface Thing { value: string }\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/test/foo/index.md"),
        "# Index test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- foo.index.d\n\n## Cases\n\n- Reject stale single-segment normalization.\n\n## Test\n\n```ts\nexpect(true).toBe(true)\n```\n",
    )
    .unwrap();

    let build = execute(CliRequest {
        cwd: package,
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 1, "{}", build.stdout);
    assert!(
        build
            .stderr
            .contains("test md Covers entry `foo.index.d` does not resolve to a source md"),
        "{}",
        build.stderr
    );
}

#[test]
fn new_uses_descriptor_scaffold_for_vue_source_docs() {
    let temp = TestDir::new();
    write_npm_runtime_descriptors(temp.path());
    fs::write(
        temp.path().join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "greet.vue.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Impl),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let content = fs::read_to_string(temp.path().join(".mds/source/greet.vue.md")).unwrap();
    assert!(content.contains("```vue"));
    assert!(content.contains("<template>"));
}

#[test]
fn new_creates_source_overview_with_managed_sections() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "overview.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Overview),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let content = fs::read_to_string(temp.path().join(".mds/source/overview.md")).unwrap();
    assert!(content.contains("### Package Summary"));
    assert!(content.contains("### Dependencies"));
    assert!(content.contains("### Dev Dependencies"));
}

#[test]
fn new_generated_source_overview_is_package_sync_clean_with_custom_labels() {
    let temp = TestDir::new();
    let package = temp.path().join("new-fixture");
    fs::create_dir_all(&package).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("package.json"),
        "{\n  \"name\": \"new-fixture\",\n  \"version\": \"0.2.0\",\n  \"dependencies\": {\n    \"left-pad\": \"1.3.0\"\n  },\n  \"devDependencies\": {\n    \"vitest\": \"1.6.0\"\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[labels]\npurpose = \"Goal\"\nname = \"Display Name\"\nsummary = \"Notes\"\n",
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: package.clone().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "overview.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Overview),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let overview = fs::read_to_string(package.join(".mds/source/overview.md")).unwrap();
    assert!(overview.contains("## Goal"));
    assert!(overview.contains("| Name | Version |"));
    assert!(overview.contains("| Name | Version | Summary |"));
    assert!(overview.contains("| new-fixture | 0.2.0 |"));
    assert!(overview.contains("| left-pad | 1.3.0 |  |"));
    assert!(overview.contains("| vitest | 1.6.0 |  |"));
    assert!(!overview.contains("| Display Name | Version |"));
    assert!(!overview.contains("| Name | Version | Notes |"));

    let sync = execute(CliRequest {
        cwd: package,
        package: None,
        verbose: false,
        command: Command::PackageSync { check: true },
    });
    assert_eq!(sync.exit_code, 0, "{}\n{}", sync.stdout, sync.stderr);
    assert!(sync.stdout.contains("package sync ok"), "{}", sync.stdout);
    assert!(!sync.stdout.contains("--- "), "{}", sync.stdout);
}

#[test]
fn new_uses_effective_label_overrides_for_nested_package() {
    let temp = TestDir::new();
    let package = temp.path().join("packages/app");
    fs::write(
        temp.path().join("mds.config.toml"),
        "[labels]\npurpose = \"Workspace Goal\"\ncontract = \"Workspace Contract\"\n",
    )
    .unwrap();
    write_minimal_authoring_package(
        &package,
        "[labels]\npurpose = \"Package Goal\"\nsource = \"Implementation\"\n",
    );
    fs::create_dir_all(package.join(".mds/descriptors/languages")).unwrap();
    fs::write(
        package.join(".mds/descriptors/languages/foo.toml"),
        concat!(
            "id = \"foo\"\n",
            "match_suffixes = [\"foo\"]\n\n",
            "[language]\n",
            "primary_ext = \"foo\"\n\n",
            "[files.source]\n",
            "strip_lang_ext = false\n",
            "prefix = \"\"\n",
            "suffix = \"\"\n",
            "extension = \"foo\"\n\n",
            "[files.test]\n",
            "strip_lang_ext = true\n",
            "prefix = \"\"\n",
            "suffix = \".test\"\n",
            "extension = \"foo\"\n\n",
            "[scaffold]\n",
            "fence_lang = \"matched-suffix\"\n",
        ),
    )
    .unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: Some(PathBuf::from("packages/app")),
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "greet.foo.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Impl),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);

    let content = fs::read_to_string(package.join(".mds/source/greet.foo.md")).unwrap();
    assert!(content.contains("## Package Goal"), "{content}");
    assert!(content.contains("## Workspace Contract"), "{content}");
    assert!(content.contains("## Implementation"), "{content}");
    assert!(content.contains("```foo"), "{content}");
    assert!(!content.contains("## Workspace Goal"), "{content}");
}

#[test]
fn new_refuses_to_overwrite_unmanaged_file_without_force() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join(".mds/source")).unwrap();
    fs::write(temp.path().join(".mds/source/greet.ts.md"), "manual\n").unwrap();

    let result = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::New {
            options: mds_core::NewOptions {
                path: "greet.ts.md".to_string(),
                kind: Some(mds_core::model::NewDocKind::Impl),
                force: false,
            },
        },
    });
    assert_eq!(result.exit_code, 1);
    assert!(result.stderr.contains("not mds-managed"));
    assert_eq!(
        fs::read_to_string(temp.path().join(".mds/source/greet.ts.md")).unwrap(),
        "manual\n"
    );
}

#[test]
fn build_copies_source_assets_by_default() {
    let temp = TestDir::new();
    let package = temp.path().join("asset-pkg");
    fs::create_dir_all(package.join(".mds/source/templates")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        "{\n  \"name\": \"asset-pkg\",\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nAsset fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| asset-pkg | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n## Rules\n\n- Fixture.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/greet.ts.md"),
        "# greet\n\n## Purpose\n\nGreeting.\n\n## Contract\n\n- Return greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/templates/snippet.md"),
        "template asset\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let result = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(
        fs::read_to_string(package.join("src/templates/snippet.md")).unwrap(),
        "template asset\n"
    );
}

#[test]
fn build_can_disable_source_asset_copy() {
    let temp = TestDir::new();
    let package = temp.path().join("asset-pkg-off");
    fs::create_dir_all(package.join(".mds/source/templates")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\ncopy_source_assets = false\n",
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        "{\n  \"name\": \"asset-pkg-off\",\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nAsset fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| asset-pkg-off | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n## Rules\n\n- Fixture.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/greet.ts.md"),
        "# greet\n\n## Purpose\n\nGreeting.\n\n## Contract\n\n- Return greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/templates/snippet.md"),
        "template asset\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let result = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(!package.join("src/templates/snippet.md").exists());
}

#[test]
fn build_overwrites_generated_source_assets_when_source_changes() {
    let temp = TestDir::new();
    let package = temp.path().join("asset-pkg-update");
    fs::create_dir_all(package.join(".mds/source/templates")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        "{\n  \"name\": \"asset-pkg-update\",\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nAsset fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| asset-pkg-update | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n## Rules\n\n- Fixture.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/greet.ts.md"),
        "# greet\n\n## Purpose\n\nGreeting.\n\n## Contract\n\n- Return greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/templates/snippet.md"),
        "template asset\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let first = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(first.exit_code, 0, "{}", first.stderr);

    fs::write(
        package.join(".mds/source/templates/snippet.md"),
        "updated template asset\n",
    )
    .unwrap();

    let second = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(second.exit_code, 0, "{}", second.stderr);
    assert_eq!(
        fs::read_to_string(package.join("src/templates/snippet.md")).unwrap(),
        "updated template asset\n"
    );
}

#[test]
fn build_treats_template_markdown_as_asset_not_source_doc() {
    let temp = TestDir::new();
    let package = temp.path().join("template-asset-pkg");
    fs::create_dir_all(package.join(".mds/source/init/templates/demo")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join("package.json"),
        "{\n  \"name\": \"template-asset-pkg\",\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nAsset fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| template-asset-pkg | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n## Rules\n\n- Fixture.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/greet.ts.md"),
        "# greet\n\n## Purpose\n\nGreeting.\n\n## Contract\n\n- Return greeting.\n\n## Source\n\n```ts\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/init/templates/demo/command-build.prompt.md"),
        "# Build prompt\n\nThis is a template asset, not an implementation markdown.\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let build = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert_eq!(
        fs::read_to_string(package.join("src/init/templates/demo/command-build.prompt.md"))
            .unwrap(),
        "# Build prompt\n\nThis is a template asset, not an implementation markdown.\n"
    );
}

#[test]
fn build_uses_descriptor_special_file_for_build_rs() {
    let temp = TestDir::new();
    let package = temp.path().join("rust-build-script");
    fs::create_dir_all(package.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n\n[output]\nsource = \"generated/{module}.{ext}\"\n",
    )
    .unwrap();
    fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"rust-build-script\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nRust fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| rust-build-script | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n## Rules\n\n- Fixture.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/lib.rs.md"),
        "# lib\n\n## Purpose\n\nLibrary.\n\n## Contract\n\n- Compile.\n\n## Source\n\n```rs\npub fn greet() -> &'static str {\n    \"hello\"\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/build.rs.md"),
        "# build\n\n## Purpose\n\nBuild script.\n\n## Contract\n\n- Compile.\n\n## Source\n\n```rs\nfn main() {\n    println!(\"cargo:rerun-if-changed=build.rs\");\n}\n```\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let result = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::DryRun,
        },
    });
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("rust-build-script/build.rs"));
    assert!(!result.stdout.contains("rust-build-script/src/build.rs"));
    assert!(!result
        .stdout
        .contains("rust-build-script/generated/build.rs"));
}

#[test]
fn build_write_updates_rust_package_source_tree_directly() {
    let temp = TestDir::new();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"mds/core\"]\nresolver = \"2\"\n",
    )
    .unwrap();

    let package = temp.path().join("mds/core");
    fs::create_dir_all(package.join(".mds/source")).unwrap();
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"mds-core-fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nRust fixture.\n\n## Architecture\n\nFixture source.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| mds-core-fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n## Rules\n\n- Fixture.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/lib.rs.md"),
        "# lib\n\n## Purpose\n\nLibrary.\n\n## Contract\n\n- Compile.\n\n## Source\n\n```rs\npub fn greet() -> &'static str {\n    \"hello\"\n}\n```\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/build.rs.md"),
        "# build\n\n## Purpose\n\nBuild script.\n\n## Contract\n\n- Compile.\n\n## Source\n\n```rs\nfn main() {\n    println!(\"cargo:rerun-if-changed=build.rs\");\n}\n```\n",
    )
    .unwrap();

    let sync = execute(CliRequest {
        cwd: package.clone(),
        package: None,
        verbose: false,
        command: Command::PackageSync { check: false },
    });
    assert_eq!(sync.exit_code, 0, "{}", sync.stderr);

    let build = execute(CliRequest {
        cwd: temp.path().to_path_buf(),
        package: None,
        verbose: false,
        command: Command::Build {
            mode: BuildMode::Write,
        },
    });
    assert_eq!(build.exit_code, 0, "{}", build.stderr);
    assert!(package.join("src/lib.rs").exists());
    assert!(package.join("build.rs").exists());
    assert!(!package.join("src/build.rs").exists());
    assert!(!temp.path().join(".build/rust").exists());
    assert!(!build.stdout.contains("workspace mirror ok:"));

    let lib_rs = fs::read_to_string(package.join("src/lib.rs")).unwrap();
    assert!(lib_rs.contains("pub fn greet() -> &'static str"));

    let build_rs = fs::read_to_string(package.join("build.rs")).unwrap();
    assert!(build_rs.contains("cargo:rerun-if-changed=build.rs"));
}

#[test]
fn descriptor_catalog_example_is_removed_from_live_examples() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().unwrap().parent().unwrap();
    assert!(!repo_root.join("examples/descriptor-catalog").exists());
}

#[test]
fn descriptor_samples_example_is_removed_from_live_examples() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().unwrap().parent().unwrap();
    assert!(!repo_root.join("examples/descriptor-samples").exists());
}

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("mds-core-test-{}-{id}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).unwrap();
        }
        fs::create_dir_all(&path).unwrap();
        write_npm_runtime_descriptors(&path);
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

fn write_init_package_metadata(root: &Path) {
    fs::write(
        root.join("package.json"),
        "{\n  \"name\": \"init-fixture\",\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
}

fn write_npm_fixture_package_json(root: &Path, name: &str) {
    fs::write(
        root.join("package.json"),
        format!(
            concat!(
                "{{\n",
                "  \"name\": \"{name}\",\n",
                "  \"version\": \"0.1.0\",\n",
                "  \"scripts\": {{\n",
                "    \"lint\": \"node -e \\\"process.exit(0)\\\"\",\n",
                "    \"typecheck\": \"node -e \\\"process.exit(0)\\\"\",\n",
                "    \"test\": \"node -e \\\"process.exit(0)\\\"\"\n",
                "  }}\n",
                "}}\n"
            ),
            name = name,
        ),
    )
    .unwrap();
}

fn write_npm_runtime_descriptors(root: &Path) {
    fs::create_dir_all(root.join(".mds/descriptors/languages")).unwrap();
    fs::create_dir_all(root.join(".mds/descriptors/package-managers")).unwrap();
    write_descriptor_file(root, ".mds/descriptors/languages/ts.toml", TS_DESCRIPTOR);
    write_descriptor_file(root, ".mds/descriptors/languages/py.toml", PY_DESCRIPTOR);
    write_descriptor_file(root, ".mds/descriptors/languages/rs.toml", RS_DESCRIPTOR);
    write_descriptor_file(root, ".mds/descriptors/languages/go.toml", GO_DESCRIPTOR);
    write_descriptor_file(root, ".mds/descriptors/languages/vue.toml", VUE_DESCRIPTOR);
    write_descriptor_file(
        root,
        ".mds/descriptors/package-managers/npm.toml",
        NPM_DESCRIPTOR,
    );
    write_descriptor_file(
        root,
        ".mds/descriptors/package-managers/pnpm.toml",
        PNPM_DESCRIPTOR,
    );
    write_descriptor_file(
        root,
        ".mds/descriptors/package-managers/cargo.toml",
        CARGO_DESCRIPTOR,
    );
    write_descriptor_file(
        root,
        ".mds/descriptors/package-managers/poetry.toml",
        POETRY_DESCRIPTOR,
    );
}

fn write_descriptor_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn replace_quality_ts_command(
    package_root: &Path,
    field: &str,
    expected_value: &str,
    replacement_value: &str,
) {
    let config_path = package_root.join("mds.config.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let from = format!("{field} = \"{expected_value}\"");
    let to = format!("{field} = \"{replacement_value}\"");
    let replaced = config.replacen(&from, &to, 1);
    assert_ne!(
        replaced, config,
        "missing shared fixture quality override target: {}",
        from
    );
    fs::write(config_path, replaced).unwrap();
}

fn write_fixture(root: &Path) {
    let package = root.join("pkg");
    fs::create_dir_all(package.join(".mds/source/foo")).unwrap();
    fs::create_dir_all(package.join(".mds/source/pkg")).unwrap();
    write_npm_fixture_package_json(&package, "fixture");
    write_npm_runtime_descriptors(&package);
    fs::write(
        package.join("mds.config.toml"),
        "[package]\nenabled = true\nallow_raw_source = false\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/overview.md"),
        "# Overview\n\n## Purpose\n\nFixture package.\n\n## Architecture\n\nFixture architecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n## Exposes\n\n| Kind | Name | Target | Summary |\n| --- | --- | --- | --- |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Fixture rules.\n",
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/util.ts.md"),
        impl_doc(
            "ts",
            "Util",
            "export type Util = string;",
            "export const util = \"ok\";",
            "expect(util).toBe(\"ok\");",
            "",
        ),
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/bar.ts.md"),
        impl_doc(
            "ts",
            "Bar",
            "export type Bar = Util;",
            "export const bar: Bar = util;",
            "expect(bar).toBe(\"ok\");",
            "| internal | foo/util | Util | helper |",
        ),
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/pkg/foo.py.md"),
        impl_doc(
            "py",
            "Foo",
            "class Foo: ...",
            "VALUE = 1",
            "assert VALUE == 1",
            "",
        ),
    )
    .unwrap();
    fs::write(
        package.join(".mds/source/foo/bar.rs.md"),
        impl_doc(
            "rs",
            "bar",
            "pub type Bar = String;",
            "pub fn bar() -> Bar { String::from(\"ok\") }",
            "#[test]\nfn works() { assert_eq!(bar(), \"ok\"); }",
            "",
        ),
    )
    .unwrap();
}

fn write_minimal_authoring_package(package: &Path, extra_config: &str) {
    fs::create_dir_all(package.join(".mds/source")).unwrap();
    write_npm_fixture_package_json(package, "fixture");
    write_npm_runtime_descriptors(package);
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

fn write_multi_part_suffix_descriptor(root: &Path, root_module_names: &[&str]) {
    fs::create_dir_all(root.join(".mds/descriptors/languages")).unwrap();
    let root_module_names = root_module_names
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join(".mds/descriptors/languages/dts.toml"),
        format!(
            concat!(
                "id = \"dts\"\n",
                "match_suffixes = [\"d.ts\"]\n\n",
                "[language]\n",
                "primary_ext = \"ts\"\n",
                "root_module_markdown_names = [{}]\n\n",
                "[files.source]\n",
                "strip_lang_ext = true\n",
                "prefix = \"\"\n",
                "suffix = \"\"\n",
                "extension = \"ts\"\n\n",
                "[files.test]\n",
                "strip_lang_ext = true\n",
                "prefix = \"\"\n",
                "suffix = \".test\"\n",
                "extension = \"ts\"\n",
            ),
            root_module_names,
        ),
    )
    .unwrap();
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

fn load_generation_plan(package_root: &Path) -> GenerationPlan {
    let mut load_state = mds_core::RunState::default();
    let package = mds_core::package::load_package(
        package_root,
        &mds_core::Config::default(),
        &mut load_state,
    )
    .unwrap();
    assert!(
        load_state.diagnostics.is_empty(),
        "{:?}",
        load_state.diagnostics
    );

    let mut docs_state = mds_core::RunState::default();
    let docs = mds_core::markdown::load_implementation_docs(&package, &mut docs_state).unwrap();
    assert!(
        docs_state.diagnostics.is_empty(),
        "{:?}",
        docs_state.diagnostics
    );

    let mut plan_state = mds_core::RunState::default();
    let plan = mds_core::plan_generation_with_source_map(&package, &docs, &mut plan_state);
    assert!(
        plan_state.diagnostics.is_empty(),
        "{:?}",
        plan_state.diagnostics
    );
    plan
}

fn impl_doc(
    lang: &str,
    name: &str,
    types: &str,
    source: &str,
    test: &str,
    _uses_row: &str,
) -> String {
    format!(
        "# {name}\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Preserve fixture behavior.\n\n## Source\n\n```{lang}\n{types}\n```\n\n```{lang}\n{source}\n```\n\n```{lang}\n{test}\n```\n"
    )
}

fn catalog_ids(catalog: &toml::Value, key: &str) -> std::collections::BTreeSet<String> {
    catalog
        .get(key)
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("missing catalog key `{key}`"))
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

fn descriptor_ids(root: PathBuf) -> std::collections::BTreeSet<String> {
    descriptor_ids_many(&[root])
}

fn descriptor_ids_many(roots: &[PathBuf]) -> std::collections::BTreeSet<String> {
    let mut ids = std::collections::BTreeSet::new();
    for root in roots {
        collect_descriptor_ids(root, &mut ids);
    }
    ids
}

fn collect_descriptor_ids(root: &Path, ids: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_descriptor_ids(&path, ids);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap();
        let parsed: toml::Value = content.parse().unwrap();
        let id = parsed
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("descriptor missing id: {}", path.display()));
        ids.insert(id.to_string());
    }
}

fn sample_descriptor_ids(source_root: &Path) -> std::collections::BTreeSet<String> {
    let mut ids = std::collections::BTreeSet::new();
    for entry in fs::read_dir(source_root).unwrap() {
        let path = entry.unwrap().path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(id) = file_name
            .strip_prefix("sample.")
            .and_then(|value| value.strip_suffix(".md"))
        else {
            continue;
        };
        ids.insert(id.to_string());
    }
    ids
}

#[derive(Debug)]
struct LanguageDescriptorExample {
    id: String,
    suffix: String,
    import_style: String,
}

fn language_descriptor_examples() -> Vec<LanguageDescriptorExample> {
    [TS_DESCRIPTOR, PY_DESCRIPTOR, RS_DESCRIPTOR]
        .iter()
        .map(|content| language_descriptor_example(content))
        .collect()
}

fn language_descriptor_example(content: &str) -> LanguageDescriptorExample {
    let parsed: toml::Value = content.parse().unwrap();
    let id = parsed.get("id").and_then(toml::Value::as_str).unwrap();
    let primary_ext = parsed
        .get("language")
        .and_then(|value| value.get("primary_ext"))
        .and_then(toml::Value::as_str)
        .unwrap();
    let suffix = parsed
        .get("match_suffixes")
        .and_then(toml::Value::as_array)
        .and_then(|values| values.first())
        .and_then(toml::Value::as_str)
        .unwrap_or(primary_ext);
    let import_style = parsed
        .get("imports")
        .and_then(|value| value.get("style"))
        .and_then(toml::Value::as_str)
        .unwrap_or("none");
    LanguageDescriptorExample {
        id: id.to_string(),
        suffix: suffix.to_string(),
        import_style: import_style.to_string(),
    }
}

fn all_language_import_doc(descriptor: &LanguageDescriptorExample) -> String {
    let target = import_target(descriptor);
    let marker = export_marker(&descriptor.id);
    let source = source_marker_statement(&descriptor.import_style, &marker);
    format!(
        "# {id} import/export fixture\n\n\
         ## Purpose\n\n\
          Verify Imports and Exports metadata for `{id}`.\n\n\
          ## Contract\n\n\
          - Render descriptor-specific imports and source exports.\n\n\
          ## Exports\n\n\
         | Name | Visibility | Summary |\n\
         | --- | --- | --- |\n\
         | exported_{id} | public | Export marker generated from Source. |\n\n\
          ## Imports\n\n\
          | From | Target | Symbols | Via | Summary | Reference |\n\
          | --- | --- | --- | --- | --- | --- |\n\
          | internal | {target} | ImportedSymbol | {via} | Dependency import | #dep |\n\n\
          ## Source\n\n\
          ##### exported-{id}\n\n\
          Shared export anchor for `{id}`.\n\n\
          ```{suffix}\n\
          {source}\n\
         ```\n",
        id = descriptor.id,
        suffix = descriptor.suffix,
        source = source,
        target = target,
        via = import_via(descriptor),
    )
}

fn import_target(descriptor: &LanguageDescriptorExample) -> &'static str {
    match descriptor.import_style.as_str() {
        "typescript" => "./dep",
        "python" | "mojo" => "dep_module",
        "rust" => "crate::dep_module",
        "go" => "example.com/fixture/dep",
        "java" => "com.example.dep",
        "csharp" => "Example.Dep",
        "c-include" => "dep/module.h",
        "dart" => "package:fixture/dep.dart",
        "ruby" => "./dep_module",
        "scss" => "dep/module",
        "zig" => "dep/module.zig",
        _ => "dep/module",
    }
}

fn import_via(descriptor: &LanguageDescriptorExample) -> &'static str {
    match descriptor.import_style.as_str() {
        "scss" => "as dep",
        "zig" => "dep",
        _ => "-",
    }
}

fn expected_import_statement(descriptor: &LanguageDescriptorExample) -> Option<String> {
    let target = import_target(descriptor);
    match descriptor.import_style.as_str() {
        "typescript" => Some(format!("import {{ ImportedSymbol }} from '{target}';")),
        "python" | "mojo" => Some(format!("from {target} import ImportedSymbol")),
        "rust" => Some(format!("use {target}::{{ImportedSymbol}};")),
        "go" => Some(format!("import \"{target}\"")),
        "java" => Some(format!("import {target}.ImportedSymbol;")),
        "csharp" => Some(format!("using {target};")),
        "c-include" => Some(format!("#include \"{target}\"")),
        "dart" => Some(format!("import '{target}' show ImportedSymbol;")),
        "ruby" => Some(format!("require_relative '{target}'")),
        "scss" => Some(format!("@use '{target}' as dep;")),
        "zig" => Some(format!("const dep = @import(\"{target}\");")),
        _ => None,
    }
}

fn export_marker(id: &str) -> String {
    format!(
        "MDS_EXPORT_MARKER__{}__",
        id.replace('-', "_").to_ascii_uppercase()
    )
}

fn source_marker_statement(import_style: &str, marker: &str) -> String {
    match import_style {
        "python" | "mojo" | "ruby" => format!("# {marker}"),
        "sql" => format!("-- {marker}"),
        "html" => format!("<!-- {marker} -->"),
        _ => format!("// {marker}"),
    }
}

fn generated_texts_under(root: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    collect_generated_texts(root, &mut texts);
    texts
}

fn collect_generated_texts(root: &Path, texts: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_generated_texts(&path, texts);
            continue;
        }
        if let Ok(content) = fs::read_to_string(&path) {
            texts.push(content);
        }
    }
}
