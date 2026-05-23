use mds_core::model::CheckDiagnosticPolicy;
use mds_core::{Config, Package};
use mds_core::{DocKind, Lang};
use mds_lsp::capabilities::diagnostics;
use mds_lsp::state::{OpenFile, PackageState, WorkspaceIndex, WorkspaceState};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use tower_lsp::lsp_types::DiagnosticSeverity;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/mds-test/{name}"))
}

fn sample_markdown(text: &str) -> String {
    text.replace("{h2}", "##")
        .replace("{h3}", "###")
        .replace("{h5}", "#####")
        .replace("{fence}", "```")
}

fn impl_doc(
    root: &std::path::Path,
    path: PathBuf,
    doc_kind: mds_core::DocKind,
) -> mds_core::ImplDoc {
    let package = Package {
        root: root.to_path_buf(),
        config: Config::default(),
        package_manager_id: "npm".to_string(),
    };
    let lang = Lang::from_path(&path).unwrap_or_else(|| Lang::Other("md".to_string()));
    let mut state = mds_core::RunState::default();
    mds_core::markdown::parse_impl_doc(&package, doc_kind, lang, &path, &mut state)
        .expect("fixture impl doc should parse")
}

fn write_custom_descriptor(root: &std::path::Path) {
    let descriptor_root = root.join(".mds/descriptors/languages");
    fs::create_dir_all(&descriptor_root).unwrap();
    fs::write(
        descriptor_root.join("foo.toml"),
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
            "extension = \"foo\"\n",
        ),
    )
    .unwrap();
}

const TS_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/languages/ts.toml");
const NPM_PACKAGE_MANAGER: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/package-managers/npm.toml");

fn write_npm_runtime_descriptors(root: &std::path::Path) {
    let language_root = root.join(".mds/descriptors/languages");
    let package_manager_root = root.join(".mds/descriptors/package-managers");
    fs::create_dir_all(&language_root).unwrap();
    fs::create_dir_all(&package_manager_root).unwrap();
    fs::write(language_root.join("ts.toml"), TS_DESCRIPTOR).unwrap();
    fs::write(package_manager_root.join("npm.toml"), NPM_PACKAGE_MANAGER).unwrap();
}

#[test]
fn test_valid_impl_md_no_diagnostics_for_minimal() {
    let text = sample_markdown(
        r#"{h2} Purpose

A minimal test module.

{h2} Contract

Public contract.

{h2} Source
{fence}ts
export function main(): void {}
{fence}
"#,
    );

    let path = fixture_path("valid.ts.md");
    let config = Config::default();
    let diags = diagnostics::validate_impl_md_text(&path, &text, &config);
    assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
}

#[test]
fn test_missing_sections() {
    let text = sample_markdown(
        r#"{h2} Purpose

A module with implementation code but no contract.

{h2} Source

{fence}typescript
export function main(): void {}
{fence}
"#,
    );

    let path = fixture_path("missing.ts.md");
    let config = Config::default();
    let diags = diagnostics::validate_impl_md_text(&path, &text, &config);

    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("Contract")),
        "should report missing Contract: {messages:?}"
    );
}

#[test]
fn test_heading_depth_violation() {
    // H5+ is no longer an error in the new format
    let text = sample_markdown(
        r#"{h2} Purpose

A module.

{h5} Deep heading is allowed now

{h2} Source

{fence}typescript
function main() {}
{fence}
"#,
    );

    let path = fixture_path("deep-heading.ts.md");
    let config = Config::default();
    let diags = diagnostics::validate_impl_md_text(&path, &text, &config);

    // No heading-depth errors anymore
    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert!(
        !messages.iter().any(|m| m.contains("H3-H4")),
        "should not report deep heading: {messages:?}"
    );
}

#[test]
fn test_config_validation_invalid_toml() {
    let text = "this is not valid toml [[[";
    let path = fixture_path("mds.config.toml");
    let diags = diagnostics::validate_config_text(&path, text);
    assert!(!diags.is_empty(), "should report TOML parse error");
}

#[test]
fn test_code_block_language_mismatch_warning() {
    let text = sample_markdown(
        r#"{h2} Purpose

A module.

{h2} Contract

Contract.

{h2} Source

{fence}python
x = 1
{fence}

{h2} Source

{fence}python
def main(): pass
{fence}

{h2} Cases

Cases.

{h2} Test

{fence}python
def test_it(): assert True
{fence}
"#,
    );

    // File is .ts.md but code blocks use python
    let path = fixture_path("mismatch.ts.md");
    let config = Config::default();
    let diags = diagnostics::validate_impl_md_text(&path, &text, &config);

    let warnings: Vec<&str> = diags
        .iter()
        .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING))
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|m| m.contains("python") && m.contains("ts")),
        "should warn about language mismatch: {warnings:?}"
    );
}

#[test]
fn test_code_block_language_mismatch_warning_with_long_fence() {
    let text = r#"{h2} Purpose

A module.

{h2} Contract

Contract.

{h2} Source

{fence4}python
def main(): pass
{fence4}
"#
    .replace("{h2}", "##")
    .replace("{fence4}", "````");

    let path = fixture_path("mismatch.ts.md");
    let config = Config::default();
    let diags = diagnostics::validate_impl_md_text(&path, &text, &config);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("python") && d.message.contains("ts")),
        "long fence label should be detected: {diags:?}"
    );
}

#[test]
fn test_import_reference_required_for_internal_imports() {
    let text = sample_markdown(
        r#"{h2} Purpose

A module.

{h2} Imports

| From | Target | Symbols | Via | Summary | Reference |
| --- | --- | --- | --- | --- | --- |
| internal | utils/helper | helper | - | Helper function | - |
"#,
    );
    let path = fixture_path("imports.ts.md");
    let config = Config::default();
    let diags = diagnostics::validate_impl_md_text(&path, &text, &config);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("requires a Markdown Reference")),
        "internal imports should require Reference links: {diags:?}"
    );
}

#[test]
fn test_empty_document_diagnostics() {
    let text = "";
    let path = fixture_path("empty.ts.md");
    let config = Config::default();
    let diags = diagnostics::validate_impl_md_text(&path, text, &config);
    assert!(
        !diags.is_empty(),
        "empty doc should report missing documentation: {diags:?}"
    );
    assert!(
        diags.iter().any(|d| d.message.contains("Purpose")),
        "should mention Purpose requirement: {diags:?}"
    );
}

#[test]
fn test_valid_config_with_canonical_roots_has_no_toml_errors() {
    let config_text =
        "[package]\nenabled = true\n\n[roots]\nsource_md = \".mds/source\"\ntest_md = \".mds/test\"\n";

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("mds.config.toml");
    std::fs::write(&config_path, config_text).unwrap();

    let diags = diagnostics::validate_config_text(&config_path, config_text);
    let toml_errors: Vec<&str> = diags
        .iter()
        .filter(|d| d.message.contains("TOML"))
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        toml_errors.is_empty(),
        "valid TOML should not have parse errors: {toml_errors:?}"
    );
}

#[test]
fn test_validate_config_text_uses_buffer_text_for_semantic_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("mds.config.toml");
    std::fs::write(&config_path, "[package]\nenabled = true\n").unwrap();

    let diags =
        diagnostics::validate_config_text(&config_path, "[labels]\nunsupported = \"oops\"\n");

    assert!(
        diags.iter().any(|diagnostic| diagnostic
            .message
            .contains("unsupported label override `unsupported`")),
        "expected semantic diagnostics from buffer text: {diags:?}"
    );
}

#[test]
fn test_legacy_roots_markdown_is_reported_as_unsupported() {
    let config_text = "[package]\nenabled = true\n\n[roots]\nmarkdown = \"src-md\"\n";

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("mds.config.toml");
    std::fs::write(&config_path, config_text).unwrap();

    let diags = diagnostics::validate_config_text(&config_path, config_text);
    assert!(
        diags.iter().any(|d| d
            .message
            .contains("ignoring unsupported roots config `markdown`")),
        "legacy roots.markdown should be reported as unsupported: {diags:?}"
    );
}

#[test]
fn test_legacy_table_policy_controls_severity() {
    let text = sample_markdown(
        r#"{h2} Purpose

Legacy metadata fixture.

{h2} Imports

| From | Target | Symbols | Via | Summary | Reference |
| --- | --- | --- | --- | --- | --- |
| builtin | node:path | join | - | Path join utility | - |
"#,
    );
    let path = fixture_path("legacy.ts.md");

    for (policy, expected) in [
        (
            CheckDiagnosticPolicy::Warn,
            Some(DiagnosticSeverity::WARNING),
        ),
        (
            CheckDiagnosticPolicy::Error,
            Some(DiagnosticSeverity::ERROR),
        ),
        (CheckDiagnosticPolicy::Allow, None),
    ] {
        let mut config = Config::default();
        config.check.legacy_tables = policy;
        let diags = diagnostics::validate_impl_md_text(&path, &text, &config);
        let legacy = diags
            .iter()
            .find(|diagnostic| diagnostic.message.contains("legacy table metadata"));

        match expected {
            Some(severity) => assert_eq!(
                legacy.and_then(|diagnostic| diagnostic.severity),
                Some(severity)
            ),
            None => assert!(
                legacy.is_none(),
                "allow should suppress legacy table diagnostics: {diags:?}"
            ),
        }
    }
}

#[test]
fn test_split_source_and_test_reports_mixing_for_both_doc_kinds() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );

    let source_path = root.join(".mds/source/mixed.ts.md");
    let source_text = sample_markdown(
        r#"{h2} Purpose

Source doc.

{h2} Contract

Contract.

{h2} Test

{fence}typescript
test("it works", () => {});
{fence}
"#,
    );
    let source_diags = diagnostics::validate_impl_md_text_with_state(
        &source_path,
        &source_text,
        &config,
        Some(&state),
    );
    assert!(
        source_diags.iter().any(|diagnostic| diagnostic
            .message
            .contains("source md must not contain generated test code")),
        "source doc should report test mixing: {source_diags:?}"
    );

    let test_path = root.join(".mds/test/mixed.md");
    let test_text = sample_markdown(
        r#"{h2} Purpose

Test doc.

{h2} Covers

<!-- TODO -->

{h2} Cases

Case.

{h2} Source

{fence}typescript
export const value = 1;
{fence}
"#,
    );
    let test_diags = diagnostics::validate_impl_md_text_with_state(
        &test_path,
        &test_text,
        &config,
        Some(&state),
    );
    assert!(
        test_diags.iter().any(|diagnostic| diagnostic
            .message
            .contains("test md must not contain generated source code")),
        "test doc should report source mixing: {test_diags:?}"
    );
}

#[test]
fn test_source_overview_surfaces_fixed_heading_contract() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    std::fs::create_dir_all(root.join(".mds/source")).unwrap();
    std::fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"dependencies\":{},\"devDependencies\":{}}\n",
    )
    .unwrap();

    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let path = root.join(".mds/source/overview.md");
    let text = sample_markdown(
        r#"{h2} Purpose

Source overview.

{h2} Architecture

Fixture architecture.

{h3} Package Summary

| Name | Version |
| --- | --- |
| fixture | 0.1.0 |

{h3} Dev Dependencies

| Name | Version | Summary |
| --- | --- | --- |

"#,
    );

    let diags = diagnostics::validate_impl_md_text_with_state(&path, &text, &config, Some(&state));

    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("source overview is missing managed section `Dependencies`")
        }),
        "source overview should report missing fixed headings: {diags:?}"
    );
    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("source overview requires ## Rules")
        }),
        "source overview should report required sections from core policy: {diags:?}"
    );
}

#[test]
fn test_source_overview_surfaces_link_policy_and_target_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    std::fs::create_dir_all(root.join(".mds/source")).unwrap();
    std::fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"dependencies\":{},\"devDependencies\":{}}\n",
    )
    .unwrap();

    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let path = root.join(".mds/source/overview.md");
    let text = sample_markdown(
        r#"{h2} Purpose

Source overview.

See [Missing](nonexistent.md) and [[foo.missing|Missing]].

{h2} Architecture

Architecture.

{h3} Package Summary

| Name | Version |
| --- | --- |
| fixture | 0.1.0 |

{h3} Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h3} Dev Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h2} Rules

- Keep package notes here.
"#,
    );

    let diags = diagnostics::validate_impl_md_text_with_state(&path, &text, &config, Some(&state));

    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "link policy `wiki-only` requires wiki links: `[Missing](nonexistent.md)`",
            )
        }),
        "source overview should report markdown-vs-wiki policy violations: {diags:?}"
    );
    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("Markdown link target does not exist: `nonexistent.md`")
        }),
        "source overview should report missing markdown targets: {diags:?}"
    );
    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("wiki link target `[[foo.missing|Missing]]` does not resolve to a module")
        }),
        "source overview should report unresolved wiki targets: {diags:?}"
    );
}

#[test]
fn test_source_overview_surfaces_overview_rules_without_impl_false_positives() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    std::fs::create_dir_all(root.join(".mds/source")).unwrap();
    std::fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"dependencies\":{},\"devDependencies\":{}}\n",
    )
    .unwrap();

    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let path = root.join(".mds/source/overview.md");
    let text = sample_markdown(
        r#"{h2} Purpose

Source overview.

{h2} Architecture

Architecture.

{h3} Package Summary

| Name | Version |
| --- | --- |
| fixture | 0.1.0 |

{h3} Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h3} Dev Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h2} Imports

| From | Target | Reference |
| --- | --- | --- |
| internal | pkg.dep | - |

{h2} Exports

##### bad

Should not be here.

{h2} Source

{fence}typescript
export const bad = 1;
{fence}

{h2} Rules

- Keep package notes here.
"#,
    );

    let diags = diagnostics::validate_impl_md_text_with_state(&path, &text, &config, Some(&state));

    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("overview md must not contain ## Imports")
        }),
        "source overview should surface forbidden Imports sections: {diags:?}"
    );
    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("overview md must not contain ## Exports")
        }),
        "source overview should surface forbidden Exports sections: {diags:?}"
    );
    assert!(
        diags.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("source md requires ## Contract")
                && !diagnostic
                    .message
                    .contains("import `pkg.dep` requires a Markdown Reference link")
        }),
        "source overview should not reuse impl-only import or contract diagnostics: {diags:?}"
    );
}

#[test]
fn test_test_overview_remains_special_file_exception() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    std::fs::create_dir_all(root.join(".mds/test")).unwrap();
    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let path = root.join(".mds/test/overview.md");
    let text = sample_markdown(
        r#"{h2} Purpose

Supplementary test overview.

{h2} Architecture

Architecture.
"#,
    );

    let diags = diagnostics::validate_impl_md_text_with_state(&path, &text, &config, Some(&state));

    assert!(
        diags.is_empty(),
        "test overview should stay exempt from general and package overview diagnostics: {diags:?}"
    );
}

#[test]
fn test_nested_source_overview_is_not_special_file_exception() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    std::fs::create_dir_all(root.join(".mds/source/feature")).unwrap();
    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let path = root.join(".mds/source/feature/overview.md");
    let text = sample_markdown(
        r#"{h2} Contract

Feature overview.
"#,
    );

    let diags = diagnostics::validate_impl_md_text_with_state(&path, &text, &config, Some(&state));

    assert!(
        diags.iter().any(|diagnostic| diagnostic.message.contains("source overview requires ## Purpose")),
        "nested source overview should follow core overview section policy instead of being exempt: {diags:?}"
    );
}

#[test]
fn test_wiki_link_unresolved_uses_workspace_index_policies() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    std::fs::create_dir_all(root.join(".mds/source/pkg")).unwrap();
    std::fs::create_dir_all(root.join(".mds/test")).unwrap();
    let source_path = root.join(".mds/source/pkg/greet.ts.md");
    let test_path = root.join(".mds/test/greet.md");
    std::fs::write(
        &source_path,
        sample_markdown(
            r#"{h2} Purpose

`greet` returns a greeting.

{h2} Contract

- Return a greeting.

{h2} Source

{fence}typescript
export function greet(name: string): string {
  return `Hello, ${name}`;
}
{fence}
"#,
        ),
    )
    .unwrap();
    std::fs::write(&test_path, "# placeholder\n").unwrap();
    let mut docs = HashMap::new();
    docs.insert(
        source_path.clone(),
        impl_doc(&root, source_path.clone(), DocKind::Source),
    );
    docs.insert(
        test_path.clone(),
        impl_doc(&root, test_path.clone(), DocKind::Test),
    );
    let state = workspace_state(
        &root,
        Config::default(),
        docs,
        HashMap::new(),
        HashMap::new(),
    );

    let text = sample_markdown(
        r#"{h2} Purpose

Wiki link validation.

{h2} Covers

- [[pkg.greet]]
- [[pkg.missing]]
- [[pkg.greet#missing]]

{h2} Cases

Ensure unresolved links surface in the editor.
"#,
    );

    let diags = diagnostics::validate_impl_md_text_with_state(
        &test_path,
        &text,
        &Config::default(),
        Some(&state),
    );

    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("wiki link target `[[pkg.missing]]` does not resolve to a module")
                && diagnostic.severity == Some(DiagnosticSeverity::ERROR)
        }),
        "missing module should be an error: {diags:?}"
    );
    assert!(
        diags.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "wiki link target `[[pkg.greet#missing]]` does not resolve to a documented symbol",
            ) && diagnostic.severity == Some(DiagnosticSeverity::WARNING)
        }),
        "missing symbol should follow warn policy: {diags:?}"
    );
}

#[test]
fn test_markdown_only_link_policy_matches_core_for_source_and_test_docs() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pkg");
    std::fs::create_dir_all(root.join(".mds/source/pkg")).unwrap();
    std::fs::create_dir_all(root.join(".mds/test/pkg")).unwrap();

    let util_path = root.join(".mds/source/pkg/util.ts.md");
    let source_path = root.join(".mds/source/pkg/greet.ts.md");
    let test_path = root.join(".mds/test/pkg/greet.test.ts.md");
    let source_text = sample_markdown(
        r#"{h2} Purpose

Source doc with a policy violation.

{h2} Contract

- Preserve markdown-only authoring.

See [[pkg.util]].

{h2} Source

{fence}typescript
export function greet(): string {
  return 'hi';
}
{fence}
"#,
    );
    let test_text = sample_markdown(
        r#"{h2} Purpose

Test doc with a policy violation.

{h2} Covers

- [Greet](pkg.greet)

{h2} Cases

- Surface editor diagnostics for wiki links.

See [[pkg.util]].

{h2} Test

{fence}typescript
expect(greet()).toBe('hi');
{fence}
"#,
    );

    std::fs::write(
        &util_path,
        sample_markdown(
            r#"{h2} Purpose

Utility doc.

{h2} Contract

- Provide a stable module target.

{h2} Source

{fence}typescript
export const util = 'ok';
{fence}
"#,
        ),
    )
    .unwrap();
    std::fs::write(&source_path, &source_text).unwrap();
    std::fs::write(&test_path, &test_text).unwrap();

    let mut docs = HashMap::new();
    docs.insert(
        util_path.clone(),
        impl_doc(&root, util_path.clone(), DocKind::Source),
    );
    docs.insert(
        source_path.clone(),
        impl_doc(&root, source_path.clone(), DocKind::Source),
    );
    docs.insert(
        test_path.clone(),
        impl_doc(&root, test_path.clone(), DocKind::Test),
    );

    let mut config = Config::default();
    config.link_policy = mds_core::LinkPolicy::MarkdownOnly;
    let state = workspace_state(&root, config.clone(), docs, HashMap::new(), HashMap::new());

    let source_diags = diagnostics::validate_impl_md_text_with_state(
        &source_path,
        &source_text,
        &config,
        Some(&state),
    );
    assert!(
        source_diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("link policy `markdown-only` requires Markdown links: `[[pkg.util]]`")
        }),
        "source doc should surface markdown-only violations: {source_diags:?}"
    );

    let test_diags = diagnostics::validate_impl_md_text_with_state(
        &test_path,
        &test_text,
        &config,
        Some(&state),
    );
    assert!(
        test_diags.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("link policy `markdown-only` requires Markdown links: `[[pkg.util]]`")
        }),
        "test doc should surface markdown-only violations: {test_diags:?}"
    );
}

#[test]
fn test_code_block_language_mismatch_warning_uses_workspace_descriptor_root() {
    let temp = tempdir().unwrap();
    let descriptor_root = temp.path().join(".mds/descriptors/languages");
    let source_root = temp.path().join(".mds/source");
    fs::create_dir_all(&descriptor_root).unwrap();
    fs::create_dir_all(&source_root).unwrap();
    fs::write(
        descriptor_root.join("foo.toml"),
        concat!(
            "id = \"foo\"\n",
            "aliases = [\"fooscript\"]\n",
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
            "extension = \"foo\"\n",
        ),
    )
    .unwrap();

    let path = source_root.join("mismatch.foo.md");
    let text = sample_markdown(
        r#"{h2} Purpose

A module.

{h2} Contract

Contract.

{h2} Source

{fence}bar
let value = 1;
{fence}
"#,
    );

    let diags = diagnostics::validate_impl_md_text(&path, &text, &Config::default());
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("bar") && d.message.contains("foo")),
        "workspace descriptor language mismatch should be detected: {diags:?}"
    );
}

#[test]
fn test_unknown_fence_only_source_doc_uses_core_reject_diagnostic() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source_root = root.join(".mds/source/foo");
    fs::create_dir_all(&source_root).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(root.join("mds.config.toml"), "[package]\nenabled = true\n").unwrap();

    let path = source_root.join("custom.md");
    let text = "# Custom\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Reject unknown fence-only source docs.\n\n## Source\n\n```unknown\ncontent\n```\n";
    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );

    let diags = diagnostics::validate_impl_md_text_with_state(&path, text, &config, Some(&state));

    assert_eq!(
        diags.len(),
        1,
        "expected only the core reject diagnostic: {diags:?}"
    );
    assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    assert!(
        diags[0]
            .message
            .contains("source md Source fence labels do not resolve to a known language"),
        "unknown fence-only source doc should surface the core reject diagnostic: {diags:?}"
    );
}

#[test]
fn test_ambiguous_fence_only_source_doc_uses_core_reject_diagnostic() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let descriptor_root = root.join(".mds/descriptors/languages");
    let source_root = root.join(".mds/source/foo");
    fs::create_dir_all(&descriptor_root).unwrap();
    fs::create_dir_all(&source_root).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(root.join("mds.config.toml"), "[package]\nenabled = true\n").unwrap();
    fs::write(
        descriptor_root.join("alpha.toml"),
        concat!(
            "id = \"alpha\"\n",
            "match_suffixes = [\"custom\"]\n\n",
            "[language]\n",
            "primary_ext = \"alpha\"\n\n",
            "[files.source]\n",
            "strip_lang_ext = false\n",
            "prefix = \"\"\n",
            "suffix = \"\"\n",
            "extension = \"alpha\"\n\n",
            "[files.test]\n",
            "strip_lang_ext = true\n",
            "prefix = \"\"\n",
            "suffix = \"_test\"\n",
            "extension = \"alpha\"\n",
        ),
    )
    .unwrap();
    fs::write(
        descriptor_root.join("beta.toml"),
        concat!(
            "id = \"beta\"\n",
            "match_suffixes = [\"custom\"]\n\n",
            "[language]\n",
            "primary_ext = \"beta\"\n\n",
            "[files.source]\n",
            "strip_lang_ext = false\n",
            "prefix = \"\"\n",
            "suffix = \"\"\n",
            "extension = \"beta\"\n\n",
            "[files.test]\n",
            "strip_lang_ext = true\n",
            "prefix = \"\"\n",
            "suffix = \"_test\"\n",
            "extension = \"beta\"\n",
        ),
    )
    .unwrap();

    let path = source_root.join("ambiguous.md");
    let text = "# Ambiguous\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Reject ambiguous fence-only source docs.\n\n## Source\n\n```custom\ncontent\n```\n";
    let config = Config::default();
    let state = workspace_state(
        &root,
        config.clone(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );

    let diags = diagnostics::validate_impl_md_text_with_state(&path, text, &config, Some(&state));

    assert_eq!(
        diags.len(),
        1,
        "expected only the core reject diagnostic: {diags:?}"
    );
    assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    assert!(
        diags[0]
            .message
            .contains("source md Source fence labels resolve ambiguously"),
        "ambiguous fence-only source doc should surface the core reject diagnostic: {diags:?}"
    );
}

#[test]
fn test_unsuffixed_test_doc_uses_cover_language_for_code_block_validation() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source_root = root.join(".mds/source/pkg");
    let test_root = root.join(".mds/test/pkg");
    fs::create_dir_all(&source_root).unwrap();
    fs::create_dir_all(&test_root).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(root.join("mds.config.toml"), "[package]\nenabled = true\n").unwrap();
    write_custom_descriptor(&root);

    let source_path = source_root.join("feature.foo.md");
    fs::write(
        &source_path,
        "# Feature\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Provide a covered source module.\n\n## Source\n\n```foo\nexport const feature = 1;\n```\n",
    )
    .unwrap();

    let mut docs = HashMap::new();
    docs.insert(
        source_path.clone(),
        impl_doc(&root, source_path.clone(), DocKind::Source),
    );

    let path = test_root.join("feature.md");
    let text = "# Feature test\n\n## Purpose\n\nFixture.\n\n## Covers\n\n- [feature](pkg.feature)\n\n## Cases\n\n- Warn on mismatched test fence labels.\n\n## Test\n\n```bar\nexpect(feature).toBe(1);\n```\n";
    let config = Config::default();
    let state = workspace_state(&root, config.clone(), docs, HashMap::new(), HashMap::new());

    let diags = diagnostics::validate_impl_md_text_with_state(&path, text, &config, Some(&state));

    assert!(
        diags.iter().any(|diagnostic| diagnostic
            .message
            .contains("code block language `bar` may not match file language `foo`")),
        "unsuffixed test doc should validate fences against covered source language: {diags:?}"
    );
}

#[test]
fn test_unsaved_config_buffer_updates_impl_md_diagnostics() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source_root = root.join(".mds/source");
    fs::create_dir_all(&source_root).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(root.join("mds.config.toml"), "[package]\nenabled = true\n").unwrap();

    let path = source_root.join("missing.ts.md");
    let text = sample_markdown(
        r#"{h2} Purpose

A module with implementation code but no contract.

{h2} Source

{fence}typescript
export function main(): void {}
{fence}
"#,
    );

    let config_path = root.join("mds.config.toml");
    let mut state = workspace_state(
        &root,
        Config::default(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    state.workspace_folders = vec![root.clone()];
    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: "[package]\nenabled = true\n\n[check]\ndocumented_sections = false\n".to_string(),
            version: 1,
            lang: None,
        },
    );

    let effective_config = state
        .effective_config_for_path(&path)
        .expect("effective config");
    let diags = diagnostics::validate_impl_md_text_with_state(
        &path,
        &text,
        &effective_config,
        Some(&state),
    );
    assert!(
        diags
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("Contract")),
        "missing Contract should follow unsaved config buffer: {diags:?}"
    );
}

#[test]
fn test_unsaved_config_buffer_updates_link_policy_diagnostics() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source_pkg_root = root.join(".mds/source/pkg");
    let test_pkg_root = root.join(".mds/test/pkg");
    fs::create_dir_all(&source_pkg_root).unwrap();
    fs::create_dir_all(&test_pkg_root).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"dependencies\":{},\"devDependencies\":{}}\n",
    )
    .unwrap();
    fs::write(root.join("mds.config.toml"), "[package]\nenabled = true\n").unwrap();

    let source_path = source_pkg_root.join("greet.ts.md");
    let test_path = test_pkg_root.join("greet.test.ts.md");
    let overview_path = root.join(".mds/source/overview.md");
    let source_text = sample_markdown(
        r#"{h2} Purpose

Source doc.

See [Missing](nonexistent.md) and [[pkg.missing]].

{h2} Contract

- Preserve authoring link style.

{h2} Source

{fence}typescript
export function greet(): string {
  return 'hi';
}
{fence}
"#,
    );
    let test_text = sample_markdown(
        r#"{h2} Purpose

Test doc.

See [Missing](nonexistent.md) and [[pkg.missing]].

{h2} Covers

- [Greet](pkg.greet)

{h2} Cases

- Keep link diagnostics in sync.

{h2} Test

{fence}typescript
expect(greet()).toBe('hi');
{fence}
"#,
    );
    let overview_text = sample_markdown(
        r#"{h2} Purpose

Source overview.

See [Missing](nonexistent.md) and [[pkg.missing]].

{h2} Architecture

Architecture.

{h3} Package Summary

| Name | Version |
| --- | --- |
| fixture | 0.1.0 |

{h3} Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h3} Dev Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h2} Rules

- Keep package notes here.
"#,
    );

    let config_path = root.join("mds.config.toml");
    let mut state = workspace_state(
        &root,
        Config::default(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    state.workspace_folders = vec![root.clone()];
    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: concat!(
                "[package]\n",
                "enabled = true\n\n",
                "[authoring]\n",
                "link_policy = \"markdown-only\"\n",
            )
            .to_string(),
            version: 1,
            lang: None,
        },
    );

    let source_config = state
        .effective_config_for_path(&source_path)
        .expect("effective config");
    let source_diags = diagnostics::validate_impl_md_text_with_state(
        &source_path,
        &source_text,
        &source_config,
        Some(&state),
    );
    let test_config = state
        .effective_config_for_path(&test_path)
        .expect("effective config");
    let test_diags = diagnostics::validate_impl_md_text_with_state(
        &test_path,
        &test_text,
        &test_config,
        Some(&state),
    );
    let overview_config = state
        .effective_config_for_path(&overview_path)
        .expect("effective config");
    let overview_diags = diagnostics::validate_impl_md_text_with_state(
        &overview_path,
        &overview_text,
        &overview_config,
        Some(&state),
    );

    for (label, diags) in [
        ("source", source_diags),
        ("test", test_diags),
        ("source overview", overview_diags),
    ] {
        assert!(
            diags.iter().any(|diagnostic| {
                diagnostic.message.contains(
                    "link policy `markdown-only` requires Markdown links: `[[pkg.missing]]`",
                )
            }),
            "{label} should follow unsaved markdown-only link policy: {diags:?}"
        );
        assert!(
            diags.iter().all(|diagnostic| {
                !diagnostic.message.contains(
                    "link policy `wiki-only` requires wiki links: `[Missing](nonexistent.md)`",
                )
            }),
            "{label} should not keep stale wiki-only policy diagnostics: {diags:?}"
        );
    }
}

#[test]
fn test_unsaved_config_buffer_disables_link_diagnostics() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source_pkg_root = root.join(".mds/source/pkg");
    let test_pkg_root = root.join(".mds/test/pkg");
    fs::create_dir_all(&source_pkg_root).unwrap();
    fs::create_dir_all(&test_pkg_root).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"version\":\"0.1.0\",\"dependencies\":{},\"devDependencies\":{}}\n",
    )
    .unwrap();
    fs::write(root.join("mds.config.toml"), "[package]\nenabled = true\n").unwrap();

    let source_path = source_pkg_root.join("greet.ts.md");
    let test_path = test_pkg_root.join("greet.test.ts.md");
    let overview_path = root.join(".mds/source/overview.md");
    let source_text = sample_markdown(
        r#"{h2} Purpose

Source doc.

See [Missing](nonexistent.md) and [[pkg.missing]].

{h2} Contract

- Preserve authoring link style.

{h2} Source

{fence}typescript
export function greet(): string {
  return 'hi';
}
{fence}
"#,
    );
    let test_text = sample_markdown(
        r#"{h2} Purpose

Test doc.

See [Missing](nonexistent.md) and [[pkg.missing]].

{h2} Covers

- [Greet](pkg.greet)

{h2} Cases

- Keep link diagnostics in sync.

{h2} Test

{fence}typescript
expect(greet()).toBe('hi');
{fence}
"#,
    );
    let overview_text = sample_markdown(
        r#"{h2} Purpose

Source overview.

See [Missing](nonexistent.md) and [[pkg.missing]].

{h2} Architecture

Architecture.

{h3} Package Summary

| Name | Version |
| --- | --- |
| fixture | 0.1.0 |

{h3} Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h3} Dev Dependencies

| Name | Version | Summary |
| --- | --- | --- |

{h2} Rules

- Keep package notes here.
"#,
    );

    let config_path = root.join("mds.config.toml");
    let mut state = workspace_state(
        &root,
        Config::default(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    state.workspace_folders = vec![root.clone()];
    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: concat!(
                "[package]\n",
                "enabled = true\n\n",
                "[check]\n",
                "markdown_links = false\n",
            )
            .to_string(),
            version: 1,
            lang: None,
        },
    );

    let source_config = state
        .effective_config_for_path(&source_path)
        .expect("effective config");
    let source_diags = diagnostics::validate_impl_md_text_with_state(
        &source_path,
        &source_text,
        &source_config,
        Some(&state),
    );
    let test_config = state
        .effective_config_for_path(&test_path)
        .expect("effective config");
    let test_diags = diagnostics::validate_impl_md_text_with_state(
        &test_path,
        &test_text,
        &test_config,
        Some(&state),
    );
    let overview_config = state
        .effective_config_for_path(&overview_path)
        .expect("effective config");
    let overview_diags = diagnostics::validate_impl_md_text_with_state(
        &overview_path,
        &overview_text,
        &overview_config,
        Some(&state),
    );

    for (label, diags) in [
        ("source", source_diags),
        ("test", test_diags),
        ("source overview", overview_diags),
    ] {
        assert!(
            diags.iter().all(|diagnostic| {
                !diagnostic.message.contains("link policy `")
                    && !diagnostic.message.contains("Markdown link target does not exist")
                    && !diagnostic.message.contains("wiki link target `[[pkg.missing]]`")
            }),
            "{label} should disable link diagnostics when markdown_links=false in unsaved config: {diags:?}"
        );
    }
}

fn workspace_state(
    root: &std::path::Path,
    config: Config,
    docs: HashMap<PathBuf, mds_core::ImplDoc>,
    module_index: HashMap<String, Vec<PathBuf>>,
    symbol_index: HashMap<(String, String), Vec<PathBuf>>,
) -> WorkspaceState {
    write_npm_runtime_descriptors(root);
    WorkspaceState {
        packages: vec![PackageState {
            package: Package {
                root: root.to_path_buf(),
                config,
                package_manager_id: "npm".to_string(),
            },
            index: WorkspaceIndex {
                docs,
                module_index,
                symbol_index,
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    }
}
