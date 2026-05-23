use mds_core::descriptor::{
    markdown_module_id_for_lang, markdown_module_path_for_lang, with_workspace_descriptor_root,
};
use mds_core::markdown::parse_impl_doc_text;
use mds_core::Package;
use mds_core::{Config, DocKind, Lang, RunState};
use mds_lsp::capabilities::authoring::{guided_authoring_path_kind, GuidedAuthoringPathKind};
use mds_lsp::capabilities::code_action::{provide_code_actions, provide_code_actions_with_state};
use mds_lsp::capabilities::completion::provide_completions;
use mds_lsp::capabilities::diagnostics::validate_impl_md_text;
use mds_lsp::capabilities::hover::provide_hover;
use mds_lsp::capabilities::navigation::{find_references, goto_definition};
use mds_lsp::capabilities::symbols::{document_symbols, workspace_symbols};
use mds_lsp::convert::line_at;
use mds_lsp::convert::table_cell_at_position;
use mds_lsp::convert::word_at_position;
use mds_lsp::state::OpenFile;
use mds_lsp::state::PackageState;
use mds_lsp::state::WorkspaceIndex;
use mds_lsp::state::WorkspaceState;
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;
use tower_lsp::lsp_types::*;

fn action_texts(actions: &CodeActionResponse) -> Vec<String> {
    actions
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => action
                .edit
                .as_ref()
                .and_then(|edit| edit.changes.as_ref())
                .and_then(|changes| changes.values().next())
                .map(|edits| {
                    edits
                        .iter()
                        .map(|edit| edit.new_text.clone())
                        .collect::<String>()
                }),
            CodeActionOrCommand::Command(_) => None,
        })
        .collect()
}

fn write_multi_suffix_descriptor(root: &std::path::Path) {
    let descriptor_root = root.join(".mds/descriptors/languages");
    fs::create_dir_all(&descriptor_root).unwrap();
    fs::write(
        descriptor_root.join("foo.toml"),
        concat!(
            "id = \"foo\"\n",
            "aliases = [\"fooscript\"]\n",
            "match_suffixes = [\"foo\", \"fooscript\"]\n\n",
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

fn write_multi_part_suffix_descriptor(root: &std::path::Path) {
    let descriptor_root = root.join(".mds/descriptors/languages");
    fs::create_dir_all(&descriptor_root).unwrap();
    fs::write(
        descriptor_root.join("dts.toml"),
        concat!(
            "id = \"dts\"\n",
            "match_suffixes = [\"d.ts\"]\n\n",
            "[language]\n",
            "primary_ext = \"ts\"\n",
            "root_module_markdown_names = [\"index.d.ts.md\"]\n\n",
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

fn package_state_for_root(root: &std::path::Path) -> WorkspaceState {
    write_npm_runtime_descriptors(root);
    WorkspaceState {
        workspace_folders: vec![root.to_path_buf()],
        packages: vec![PackageState {
            package: Package {
                root: root.to_path_buf(),
                config: Config::default(),
                package_manager_id: "npm".to_string(),
            },
            index: WorkspaceIndex::default(),
        }],
        ..WorkspaceState::default()
    }
}

fn parse_doc_for_test(
    package: &Package,
    path: &std::path::Path,
    text: &str,
    doc_kind: DocKind,
    lang: Lang,
) -> mds_core::ImplDoc {
    let mut run_state = RunState::default();
    parse_impl_doc_text(package, doc_kind, lang, path, text, &mut run_state).expect("test doc")
}

fn completion_labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

fn completion_sort_texts(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .map(|item| item.sort_text.as_deref().unwrap_or_default())
        .collect()
}

#[test]
fn test_document_symbols_extracts_headings() {
    let text = r#"## Purpose

A module.

## Contract

Contract.

## Source

Source content.

### Uses

Uses content.

##### SharedName

Shared definition.
"#;

    let symbols = document_symbols(text);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Purpose"),
        "should contain Purpose: {names:?}"
    );
    assert!(
        names.contains(&"Contract"),
        "should contain Contract: {names:?}"
    );
    assert!(
        names.contains(&"Source"),
        "should contain Source: {names:?}"
    );
    assert!(names.contains(&"Uses"), "should contain Uses: {names:?}");
    assert!(
        names.contains(&"SharedName"),
        "should contain H5 shared definition: {names:?}"
    );
}

#[test]
fn test_section_completion_on_heading_prefix() {
    let text = "## ";
    let position = Position {
        line: 0,
        character: 3,
    };
    let config = mds_core::Config::default();
    let path = std::path::Path::new("/workspace/.mds/source/app/greet.ts.md");
    let items = provide_completions(text, position, Some(path), &config, None);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"Purpose"),
        "should offer Purpose: {labels:?}"
    );
    assert!(
        labels.contains(&"Contract"),
        "should offer Contract: {labels:?}"
    );
    assert!(
        labels.contains(&"Source"),
        "should offer Source: {labels:?}"
    );
}

#[test]
fn test_section_completion_uses_effective_config_label_overrides() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let mut state = package_state_for_root(&root);
    let config_path = root.join("mds.config.toml");
    let path = root.join(".mds/source/app/greet.ts.md");

    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: concat!(
                "[package]\n",
                "enabled = true\n\n",
                "[labels]\n",
                "purpose = \"目的\"\n",
                "contract = \"仕様\"\n",
                "source = \"実装\"\n",
                "cases = \"ケース\"\n",
                "architecture = \"構成\"\n",
                "rules = \"規約\"\n",
            )
            .to_string(),
            version: 1,
            lang: None,
        },
    );

    let items = provide_completions(
        "## ",
        Position {
            line: 0,
            character: 3,
        },
        Some(&path),
        &Config::default(),
        Some(&state),
    );
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();

    assert!(
        labels.contains(&"目的"),
        "missing custom purpose label: {labels:?}"
    );
    assert!(
        labels.contains(&"仕様"),
        "missing custom contract label: {labels:?}"
    );
    assert!(
        labels.contains(&"実装"),
        "missing custom source label: {labels:?}"
    );
    assert!(
        !labels.contains(&"Purpose"),
        "stale default labels should not win: {labels:?}"
    );

    let overview_items = provide_completions(
        "## ",
        Position {
            line: 0,
            character: 3,
        },
        Some(&root.join(".mds/test/overview.md")),
        &Config::default(),
        Some(&state),
    );
    let overview_labels: Vec<&str> = overview_items
        .iter()
        .map(|item| item.label.as_str())
        .collect();

    assert_eq!(overview_labels, vec!["目的", "構成", "規約"]);
    assert!(
        !overview_labels.contains(&"Architecture"),
        "stale overview label should not win: {overview_labels:?}"
    );
    assert!(
        !overview_labels.contains(&"Rules"),
        "stale overview label should not win: {overview_labels:?}"
    );
}

#[test]
fn test_section_completion_uses_authoring_kind_aware_candidates_and_sort_order() {
    let config = mds_core::Config::default();
    let position = Position {
        line: 0,
        character: 3,
    };

    let source_overview = provide_completions(
        "## ",
        position,
        Some(std::path::Path::new("/workspace/.mds/source/overview.md")),
        &config,
        None,
    );
    assert_eq!(
        completion_labels(&source_overview),
        vec!["Purpose", "Architecture", "Rules"]
    );
    assert_eq!(
        completion_sort_texts(&source_overview),
        vec!["00_purpose", "01_architecture", "02_rules"]
    );

    let nested_overview = provide_completions(
        "## ",
        position,
        Some(std::path::Path::new(
            "/workspace/.mds/source/pkg/overview.md",
        )),
        &config,
        None,
    );
    assert_eq!(
        completion_labels(&nested_overview),
        vec!["Purpose", "Architecture", "Rules"]
    );

    let test_overview = provide_completions(
        "## ",
        position,
        Some(std::path::Path::new("/workspace/.mds/test/overview.md")),
        &config,
        None,
    );
    assert_eq!(
        completion_labels(&test_overview),
        vec!["Purpose", "Architecture", "Rules"]
    );

    let root_module_temp = tempdir().unwrap();
    let root_module_root = root_module_temp.path().join("pkg");
    let root_module_state = package_state_for_root(&root_module_root);
    let root_module_path = root_module_root.join(".mds/source/index.ts.md");
    let root_module = provide_completions(
        "## ",
        position,
        Some(&root_module_path),
        &config,
        Some(&root_module_state),
    );
    assert_eq!(
        completion_labels(&root_module),
        vec!["Purpose", "Contract", "API", "Source"]
    );
    assert_eq!(
        completion_sort_texts(&root_module),
        vec!["00_purpose", "01_contract", "02_api", "03_source"]
    );

    let source = provide_completions(
        "## ",
        position,
        Some(std::path::Path::new(
            "/workspace/.mds/source/app/greet.ts.md",
        )),
        &config,
        None,
    );
    assert_eq!(
        completion_labels(&source),
        vec!["Purpose", "Contract", "API", "Source", "Cases"]
    );

    let test = provide_completions(
        "## ",
        position,
        Some(std::path::Path::new(
            "/workspace/.mds/test/app/greet.test.ts.md",
        )),
        &config,
        None,
    );
    assert_eq!(
        completion_labels(&test),
        vec!["Purpose", "Covers", "Cases", "Test"]
    );
}

#[test]
fn test_source_overview_managed_heading_completion_is_exact_path_only() {
    let config = mds_core::Config::default();
    let position = Position {
        line: 0,
        character: 4,
    };

    let source_overview = provide_completions(
        "### ",
        position,
        Some(std::path::Path::new("/workspace/.mds/source/overview.md")),
        &config,
        None,
    );
    assert_eq!(
        completion_labels(&source_overview),
        vec!["Package Summary", "Dependencies", "Dev Dependencies"]
    );
    assert_eq!(
        completion_sort_texts(&source_overview),
        vec![
            "00_package_summary",
            "01_dependencies",
            "02_dev_dependencies"
        ]
    );

    for path in [
        "/workspace/.mds/source/pkg/overview.md",
        "/workspace/.mds/test/overview.md",
        "/workspace/.mds/source/index.ts.md",
        "/workspace/.mds/source/app/greet.ts.md",
        "/workspace/.mds/test/app/greet.test.ts.md",
    ] {
        let items = provide_completions(
            "### ",
            position,
            Some(std::path::Path::new(path)),
            &config,
            None,
        );
        let labels = completion_labels(&items);
        assert!(
            !labels.contains(&"Package Summary"),
            "managed source overview headings should not appear for {path}: {labels:?}"
        );
        assert!(
            !labels.contains(&"Dependencies"),
            "managed source overview headings should not appear for {path}: {labels:?}"
        );
        assert!(
            !labels.contains(&"Dev Dependencies"),
            "managed source overview headings should not appear for {path}: {labels:?}"
        );
    }
}

#[test]
fn test_guided_authoring_path_kind_uses_package_descriptor_root_for_custom_root_module_name() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let state = package_state_for_root(&root);
    let path = root.join(".mds/source/pkg/index.d.ts.md");

    write_multi_part_suffix_descriptor(&root);

    assert_eq!(
        guided_authoring_path_kind(Some(&path), &Config::default(), Some(&state)),
        GuidedAuthoringPathKind::RootModule
    );
}

#[test]
fn test_package_for_path_prefers_longest_matching_root() {
    let temp = tempdir().unwrap();
    let workspace_root = temp.path().join("workspace");
    let parent_root = workspace_root.join("packages");
    let child_root = parent_root.join("app");
    let nested_doc = child_root.join(".mds/source/feature/greet.ts.md");

    let state = WorkspaceState {
        workspace_folders: vec![workspace_root],
        packages: vec![
            PackageState {
                package: Package {
                    root: parent_root,
                    config: Config::default(),
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            },
            PackageState {
                package: Package {
                    root: child_root.clone(),
                    config: Config {
                        label_overrides: HashMap::from([(
                            "purpose".to_string(),
                            "Child Goal".to_string(),
                        )]),
                        ..Config::default()
                    },
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            },
        ],
        ..WorkspaceState::default()
    };

    let package = state.package_for_path(&nested_doc).expect("nested package");
    assert_eq!(package.package.root, child_root);

    let effective_config = state
        .effective_config_for_path(&nested_doc)
        .expect("effective config");
    assert_eq!(
        effective_config.label_overrides.get("purpose"),
        Some(&"Child Goal".to_string())
    );
}

#[test]
fn test_h5_completion_on_heading_prefix() {
    let text = "##### ";
    let position = Position {
        line: 0,
        character: 6,
    };
    let config = mds_core::Config::default();
    let items = provide_completions(text, position, None, &config, None);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"mds: Shared Definition Section"),
        "should offer H5 shared definition snippet: {labels:?}"
    );
}

#[test]
fn test_types_heading_has_no_special_hover_affordance() {
    let hover = provide_hover(
        "## Types",
        Position {
            line: 0,
            character: 3,
        },
        std::path::Path::new("/test/example.ts.md"),
        &WorkspaceState::default(),
    );
    assert!(hover.is_none());
}

#[test]
fn test_impl_diagnostics_do_not_emit_legacy_types_message() {
    let text = r#"## Purpose

A module.

## Contract

Stable contract.

## Types

```ts
export type Greeting = string;
```

## Source

```ts
export const greet = (): Greeting => 'hi';
```
"#;

    let diagnostics = validate_impl_md_text(
        std::path::Path::new("/test/example.ts.md"),
        text,
        &Config::default(),
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("legacy section")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn test_code_block_language_completion() {
    let text = "```";
    let position = Position {
        line: 0,
        character: 3,
    };
    let config = mds_core::Config::default();
    let path = std::path::Path::new("/test/example.ts.md");
    let items = provide_completions(text, position, Some(path), &config, None);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"ts"), "should offer ts: {labels:?}");
}

#[test]
fn test_code_block_language_completion_with_long_fence() {
    let text = "````";
    let position = Position {
        line: 0,
        character: 4,
    };
    let config = mds_core::Config::default();
    let path = std::path::Path::new("/test/example.rs.md");
    let items = provide_completions(text, position, Some(path), &config, None);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"rs"), "should offer rs: {labels:?}");
}

#[test]
fn test_wiki_link_definition_resolves_module_symbol() {
    let root = std::env::temp_dir().join(format!("mds-lsp-wiki-{}", std::process::id()));
    let source = root.join(".mds/source/app/greet.ts.md");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "# app.greet\n\n## 実装\n\n### greet\n").unwrap();

    let mut module_index = HashMap::new();
    module_index.insert("app.greet".to_string(), vec![source.clone()]);
    let mut symbol_index = HashMap::new();
    symbol_index.insert(
        ("app.greet".to_string(), "greet".to_string()),
        vec![source.clone()],
    );
    let state = WorkspaceState {
        packages: vec![PackageState {
            package: Package {
                root: root.clone(),
                config: Config::default(),
                package_manager_id: "npm".to_string(),
            },
            index: WorkspaceIndex {
                module_index,
                symbol_index,
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    };

    let text = "See [[app.greet#greet]]";
    let response = goto_definition(
        text,
        Position {
            line: 0,
            character: 12,
        },
        &root.join(".mds/source/app/caller.ts.md"),
        &state,
    );
    assert!(matches!(response, Some(GotoDefinitionResponse::Scalar(_))));
}

#[test]
fn test_wiki_link_definition_resolves_multi_part_suffix_symbol_from_path_key() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source = root.join(".mds/source/pkg/index.d.ts.md");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    write_multi_part_suffix_descriptor(&root);
    let source_text =
        "# pkg.index\n\n## Purpose\n\nFixture.\n\n## Exports\n\n##### Thing\n\nShared export.\n";
    fs::write(&source, source_text).unwrap();
    let package = Package {
        root: root.clone(),
        config: Config::default(),
        package_manager_id: "npm".to_string(),
    };

    let module_path = with_workspace_descriptor_root(Some(&root), || {
        markdown_module_path_for_lang(
            &Lang::Other("dts".to_string()),
            std::path::Path::new("pkg/index.d.ts.md"),
        )
    });
    let module_id = with_workspace_descriptor_root(Some(&root), || {
        markdown_module_id_for_lang(
            &Lang::Other("dts".to_string()),
            std::path::Path::new("pkg/index.d.ts.md"),
        )
    });
    let docs = HashMap::from([(
        source.clone(),
        parse_doc_for_test(
            &package,
            &source,
            source_text,
            DocKind::Source,
            Lang::Other("dts".to_string()),
        ),
    )]);

    let state = WorkspaceState {
        packages: vec![PackageState {
            package,
            index: WorkspaceIndex {
                docs,
                module_index: HashMap::from([
                    (module_path, vec![source.clone()]),
                    (module_id.clone(), vec![source.clone()]),
                ]),
                symbol_index: HashMap::from([(
                    (module_id, "Thing".to_string()),
                    vec![source.clone()],
                )]),
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    };

    let response = goto_definition(
        "See [[pkg/index#Thing]]",
        Position {
            line: 0,
            character: 10,
        },
        &root.join(".mds/source/pkg/caller.ts.md"),
        &state,
    );

    let Some(GotoDefinitionResponse::Scalar(location)) = response else {
        panic!("expected scalar definition response, got: {response:?}");
    };
    assert_eq!(location.uri.to_file_path().unwrap(), source);
}

#[test]
fn test_markdown_link_definition_resolves_body_link_under_cursor() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source = root.join(".mds/source/app/greet.ts.md");
    let caller = root.join(".mds/source/app/caller.ts.md");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "# app.greet\n\n## Exports\n\n##### greet\n").unwrap();

    let state = package_state_for_root(&root);
    let text = "See [greet](greet.ts.md#greet).";

    for character in [6, 20] {
        let response = goto_definition(text, Position { line: 0, character }, &caller, &state);

        let Some(GotoDefinitionResponse::Scalar(location)) = response else {
            panic!("expected scalar definition response, got: {response:?}");
        };
        assert_eq!(location.uri.to_file_path().unwrap(), source);
        assert_eq!(location.range.start.line, 4);
    }
}

#[test]
fn test_wiki_link_completion_offers_modules_and_symbols() {
    let mut module_index = HashMap::new();
    module_index.insert("app.greet".to_string(), Vec::new());
    let mut symbol_index = HashMap::new();
    symbol_index.insert(("app.greet".to_string(), "greet".to_string()), Vec::new());
    let state = WorkspaceState {
        packages: vec![PackageState {
            package: Package {
                root: std::env::temp_dir(),
                config: Config::default(),
                package_manager_id: "npm".to_string(),
            },
            index: WorkspaceIndex {
                module_index,
                symbol_index,
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    };

    let config = Config::default();
    let modules = provide_completions(
        "[[app",
        Position {
            line: 0,
            character: 5,
        },
        None,
        &config,
        Some(&state),
    );
    assert!(modules.iter().any(|item| item.label == "app.greet"));
    let symbols = provide_completions(
        "[[app.greet#g",
        Position {
            line: 0,
            character: 13,
        },
        None,
        &config,
        Some(&state),
    );
    assert!(symbols.iter().any(|item| item.label == "greet"));
}

#[test]
fn test_find_references_prefers_structured_refs_over_textual_noise() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source = root.join(".mds/source/app/greet.ts.md");
    let caller = root.join(".mds/source/app/caller.ts.md");
    let noise = root.join(".mds/source/app/noise.ts.md");
    write_npm_runtime_descriptors(&root);
    fs::create_dir_all(source.parent().unwrap()).unwrap();

    let source_text = concat!(
        "# app.greet\n\n",
        "## Purpose\n\n",
        "A module.\n\n",
        "## Exports\n\n",
        "##### greet\n\n",
        "Shared entrypoint.\n\n",
        "## Source\n\n",
        "```ts\n",
        "export function greet(): string { return 'hi'; }\n",
        "```\n",
    );
    let caller_text = concat!(
        "# app.caller\n\n",
        "## Purpose\n\n",
        "Uses [[app.greet#greet]].\n\n",
        "## Source\n\n",
        "```ts\n",
        "export const caller = greet;\n",
        "```\n",
    );
    let noise_text = concat!(
        "# app.noise\n\n",
        "## Purpose\n\n",
        "This prose mentions greet but does not reference the module.\n\n",
        "## Source\n\n",
        "```ts\n",
        "export const note = 'greet';\n",
        "```\n",
    );

    fs::write(&source, source_text).unwrap();
    fs::write(&caller, caller_text).unwrap();
    fs::write(&noise, noise_text).unwrap();

    let package = Package {
        root: root.clone(),
        config: Config::default(),
        package_manager_id: "npm".to_string(),
    };
    let docs = HashMap::from([
        (
            source.clone(),
            parse_doc_for_test(
                &package,
                &source,
                source_text,
                DocKind::Source,
                Lang::Other("ts".to_string()),
            ),
        ),
        (
            caller.clone(),
            parse_doc_for_test(
                &package,
                &caller,
                caller_text,
                DocKind::Source,
                Lang::Other("ts".to_string()),
            ),
        ),
        (
            noise.clone(),
            parse_doc_for_test(
                &package,
                &noise,
                noise_text,
                DocKind::Source,
                Lang::Other("ts".to_string()),
            ),
        ),
    ]);
    let state = WorkspaceState {
        packages: vec![PackageState {
            package,
            index: WorkspaceIndex {
                docs,
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    };

    let fence_references = find_references(
        source_text,
        Position {
            line: 15,
            character: 18,
        },
        &source,
        &state,
    );
    assert!(
        fence_references.is_none(),
        "references should abstain inside code fence content: {fence_references:?}"
    );

    let references = find_references(
        source_text,
        Position {
            line: 8,
            character: 8,
        },
        &source,
        &state,
    )
    .expect("references");

    let caller_uri = Url::from_file_path(&caller).unwrap();
    let noise_uri = Url::from_file_path(&noise).unwrap();
    assert!(
        references.iter().any(|location| location.uri == caller_uri),
        "structured reference should be returned: {references:?}"
    );
    assert!(
        !references.iter().any(|location| location.uri == noise_uri),
        "heuristic noise should be suppressed when structured refs exist: {references:?}"
    );
}

#[test]
fn test_find_references_uses_package_descriptor_root_for_multi_part_suffix_docs() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source = root.join(".mds/source/pkg/index.d.ts.md");
    let caller = root.join(".mds/source/pkg/caller.ts.md");
    let noise = root.join(".mds/source/pkg/noise.ts.md");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    write_multi_part_suffix_descriptor(&root);

    let source_text = concat!(
        "# pkg.index\n\n",
        "## Purpose\n\n",
        "Fixture.\n\n",
        "## Exports\n\n",
        "##### Thing\n\n",
        "Shared export.\n\n",
        "## Source\n\n",
        "```ts\n",
        "export interface Thing { value: string }\n",
        "```\n",
    );
    let caller_text = concat!(
        "# pkg.caller\n\n",
        "## Purpose\n\n",
        "Uses [[pkg.index#Thing]].\n\n",
        "## Source\n\n",
        "```ts\n",
        "export const caller = true;\n",
        "```\n",
    );
    let noise_text = concat!(
        "# pkg.noise\n\n",
        "## Purpose\n\n",
        "This prose mentions pkg.index and Thing only as text.\n\n",
        "## Source\n\n",
        "```ts\n",
        "export const note = 'Thing';\n",
        "```\n",
    );

    fs::write(&source, source_text).unwrap();
    fs::write(&caller, caller_text).unwrap();
    fs::write(&noise, noise_text).unwrap();

    let package = Package {
        root: root.clone(),
        config: Config::default(),
        package_manager_id: "npm".to_string(),
    };
    let docs = HashMap::from([
        (
            source.clone(),
            parse_doc_for_test(
                &package,
                &source,
                source_text,
                DocKind::Source,
                Lang::Other("dts".to_string()),
            ),
        ),
        (
            caller.clone(),
            parse_doc_for_test(
                &package,
                &caller,
                caller_text,
                DocKind::Source,
                Lang::Other("ts".to_string()),
            ),
        ),
        (
            noise.clone(),
            parse_doc_for_test(
                &package,
                &noise,
                noise_text,
                DocKind::Source,
                Lang::Other("ts".to_string()),
            ),
        ),
    ]);
    let state = WorkspaceState {
        packages: vec![PackageState {
            package,
            index: WorkspaceIndex {
                docs,
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    };

    let references = find_references(
        source_text,
        Position {
            line: 8,
            character: 8,
        },
        &source,
        &state,
    )
    .expect("references");

    let caller_uri = Url::from_file_path(&caller).unwrap();
    let noise_uri = Url::from_file_path(&noise).unwrap();
    assert!(
        references.iter().any(|location| location.uri == caller_uri),
        "structured reference should be returned: {references:?}"
    );
    assert!(
        !references.iter().any(|location| location.uri == noise_uri),
        "descriptor-aware structured refs should suppress heuristic noise: {references:?}"
    );
}

#[test]
fn test_find_references_keeps_heuristic_fallback_without_structured_refs() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source = root.join(".mds/source/app/greet.ts.md");
    let note = root.join(".mds/source/app/note.ts.md");
    fs::create_dir_all(source.parent().unwrap()).unwrap();

    let source_text = concat!(
        "# app.greet\n\n",
        "## Purpose\n\n",
        "A module.\n\n",
        "## Exports\n\n",
        "##### greet\n\n",
        "Shared entrypoint.\n\n",
        "## Source\n\n",
        "```ts\n",
        "export function greet(): string { return 'hi'; }\n",
        "```\n",
    );
    let note_text = concat!(
        "# app.note\n\n",
        "## Purpose\n\n",
        "This prose mentions greet only as plain text.\n\n",
        "## Source\n\n",
        "```ts\n",
        "export const note = 'greet';\n",
        "```\n",
    );

    fs::write(&source, source_text).unwrap();
    fs::write(&note, note_text).unwrap();

    let package = Package {
        root: root.clone(),
        config: Config::default(),
        package_manager_id: "npm".to_string(),
    };
    let docs = HashMap::from([
        (
            source.clone(),
            parse_doc_for_test(
                &package,
                &source,
                source_text,
                DocKind::Source,
                Lang::Other("ts".to_string()),
            ),
        ),
        (
            note.clone(),
            parse_doc_for_test(
                &package,
                &note,
                note_text,
                DocKind::Source,
                Lang::Other("ts".to_string()),
            ),
        ),
    ]);
    let state = WorkspaceState {
        packages: vec![PackageState {
            package,
            index: WorkspaceIndex {
                docs,
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    };

    let fence_references = find_references(
        source_text,
        Position {
            line: 15,
            character: 18,
        },
        &source,
        &state,
    );
    assert!(
        fence_references.is_none(),
        "references should abstain inside code fence content: {fence_references:?}"
    );

    let references = find_references(
        source_text,
        Position {
            line: 8,
            character: 8,
        },
        &source,
        &state,
    )
    .expect("references");

    assert!(
        references
            .iter()
            .any(|location| location.uri == Url::from_file_path(&note).unwrap()),
        "heuristic fallback should remain available: {references:?}"
    );
}

#[test]
fn test_workspace_symbols_preserve_module_and_shared_definition_structure() {
    let root = std::env::temp_dir().join(format!("mds-lsp-symbols-{}", std::process::id()));
    let source = root.join(".mds/source/app/greet.ts.md");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "# app.greet\n\n## Exports\n\n##### greet\n").unwrap();

    let state = WorkspaceState {
        packages: vec![PackageState {
            package: Package {
                root: root.clone(),
                config: Config::default(),
                package_manager_id: "npm".to_string(),
            },
            index: WorkspaceIndex {
                module_index: HashMap::from([("app.greet".to_string(), vec![source.clone()])]),
                symbol_index: HashMap::from([(
                    ("app.greet".to_string(), "greet".to_string()),
                    vec![source.clone()],
                )]),
                ..WorkspaceIndex::default()
            },
        }],
        ..WorkspaceState::default()
    };

    let symbols = workspace_symbols("greet", &state);
    assert!(
        symbols
            .iter()
            .any(|item| { item.name == "app.greet" && item.kind == SymbolKind::MODULE }),
        "module result should stay visible: {symbols:?}"
    );
    assert!(
        symbols.iter().any(|item| {
            item.name == "greet"
                && item.kind == SymbolKind::FUNCTION
                && item.container_name.as_deref() == Some("app.greet")
                && item.location.uri == Url::from_file_path(&source).unwrap()
        }),
        "shared definition should keep module context in workspace symbols: {symbols:?}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_code_action_missing_sections() {
    let text = r#"## Purpose

A module.
"#;
    let uri = Url::parse("file:///test/example.ts.md").unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions(&uri, text, &config);
    assert!(
        !actions.is_empty(),
        "should provide code actions for missing sections"
    );
}

#[test]
fn test_code_action_legacy_sections_keep_authoring_actions_without_migration_fix() {
    let text = r#"## Uses

| From | Target | Expose | Summary |
| --- | --- | --- | --- |
"#;
    let uri = Url::parse("file:///test/example.ts.md").unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions(&uri, text, &config);
    let titles: Vec<String> = actions
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => Some(action.title.clone()),
            CodeActionOrCommand::Command(command) => Some(command.title.clone()),
        })
        .collect();
    assert!(
        titles.iter().any(|title| title.contains("Add missing")),
        "should still provide useful authoring actions: {titles:?}"
    );
    assert!(
        !titles.iter().any(|title| title.contains("Rename ##")),
        "should not provide legacy migration quick fixes: {titles:?}"
    );
}

#[test]
fn test_word_at_position() {
    let text = "hello world foo-bar";
    let word = word_at_position(
        text,
        Position {
            line: 0,
            character: 7,
        },
    );
    assert_eq!(word, Some("world".to_string()));
}

#[test]
fn test_word_at_position_with_path() {
    let text = "| internal | utils/helper | Name | desc |";
    let word = word_at_position(
        text,
        Position {
            line: 0,
            character: 14,
        },
    );
    assert_eq!(word, Some("utils/helper".to_string()));
}

#[test]
fn test_table_cell_at_position() {
    let text = "| internal | utils/helper | Name | desc |";
    let cell = table_cell_at_position(
        text,
        Position {
            line: 0,
            character: 14,
        },
    );
    assert_eq!(cell, Some("utils/helper".to_string()));
}

#[test]
fn test_line_at() {
    let text = "line 0\nline 1\nline 2";
    assert_eq!(line_at(text, 0), Some("line 0"));
    assert_eq!(line_at(text, 1), Some("line 1"));
    assert_eq!(line_at(text, 2), Some("line 2"));
    assert_eq!(line_at(text, 3), None);
}

#[test]
fn test_code_action_empty_document() {
    let text = "";
    let uri = Url::parse("file:///test/empty.ts.md").unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions(&uri, text, &config);
    assert!(
        !actions.is_empty(),
        "should offer to add all missing sections"
    );
}

#[test]
fn test_code_action_source_doc_prefers_tableless_authoring_sections() {
    let text = r#"## Purpose

Source module.
"#;
    let uri = Url::parse("file:///workspace/pkg/.mds/source/example.ts.md").unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions(&uri, text, &config);
    let inserted = action_texts(&actions).join("\n");

    assert!(
        inserted.contains("## Contract"),
        "source quick fix should add Contract: {inserted:?}"
    );
    assert!(
        inserted.contains("## Source"),
        "source quick fix should add Source: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Imports"),
        "source quick fix should not add Imports: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Exports"),
        "source quick fix should not add Exports: {inserted:?}"
    );
    assert!(
        !inserted.contains("| From |"),
        "source quick fix should stay tableless: {inserted:?}"
    );
}

#[test]
fn test_code_action_test_doc_prefers_tableless_test_sections() {
    let text = r#"## Purpose

Verification module.
"#;
    let uri = Url::parse("file:///workspace/pkg/.mds/test/example.md").unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions(&uri, text, &config);
    let inserted = action_texts(&actions).join("\n");

    assert!(
        inserted.contains("## Covers"),
        "test quick fix should add Covers: {inserted:?}"
    );
    assert!(
        inserted.contains("## Cases"),
        "test quick fix should add Cases: {inserted:?}"
    );
    assert!(
        inserted.contains("## Test"),
        "test quick fix should add Test: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Contract"),
        "test quick fix should not add Contract: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Source"),
        "test quick fix should not add Source: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Imports"),
        "test quick fix should not add Imports: {inserted:?}"
    );
}

#[test]
fn test_code_action_source_overview_doc_prefers_overview_sections() {
    let text = r#"## Purpose

Source hierarchy.
"#;
    let uri = Url::parse("file:///workspace/pkg/.mds/source/overview.md").unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions(&uri, text, &config);
    let inserted = action_texts(&actions).join("\n");

    assert!(
        inserted.contains("## Architecture"),
        "source overview quick fix should add Architecture: {inserted:?}"
    );
    assert!(
        inserted.contains("### Package Summary"),
        "source overview quick fix should add managed headings: {inserted:?}"
    );
    assert!(
        inserted.contains("### Dependencies"),
        "source overview quick fix should add dependency heading: {inserted:?}"
    );
    assert!(
        inserted.contains("### Dev Dependencies"),
        "source overview quick fix should add dev dependency heading: {inserted:?}"
    );
    assert!(
        inserted.contains("## Rules"),
        "source overview quick fix should add Rules: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Contract"),
        "source overview quick fix should not add Contract: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Source"),
        "source overview quick fix should not add Source: {inserted:?}"
    );
}

#[test]
fn test_code_action_test_overview_doc_uses_generic_overview_sections() {
    let text = r#"## Purpose

Test hierarchy.
"#;
    let uri = Url::parse("file:///workspace/pkg/.mds/test/overview.md").unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions(&uri, text, &config);
    let inserted = action_texts(&actions).join("\n");

    assert!(
        inserted.contains("## Architecture"),
        "test overview quick fix should add Architecture: {inserted:?}"
    );
    assert!(
        inserted.contains("## Rules"),
        "test overview quick fix should add Rules: {inserted:?}"
    );
    assert!(
        !inserted.contains("### Package Summary"),
        "test overview quick fix should stay generic: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Covers"),
        "test overview quick fix should not add test sections: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Test"),
        "test overview quick fix should not add test code section: {inserted:?}"
    );
}

#[test]
fn test_code_action_root_module_doc_keeps_source_optional() {
    let text = r#"## Purpose

Package root.
"#;
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let state = package_state_for_root(&root);
    let path = root.join(".mds/source/index.ts.md");
    let uri = Url::from_file_path(&path).unwrap();
    let config = mds_core::Config::default();
    let actions = provide_code_actions_with_state(&uri, text, &config, Some(&state));
    let inserted = action_texts(&actions).join("\n");

    assert!(
        inserted.contains("## Contract"),
        "root module quick fix should add Contract: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Source"),
        "root module quick fix should keep Source optional: {inserted:?}"
    );
}

#[test]
fn test_code_action_uses_effective_config_label_overrides() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let mut state = package_state_for_root(&root);
    let config_path = root.join("mds.config.toml");
    let path = root.join(".mds/source/app/greet.ts.md");
    let uri = Url::from_file_path(&path).unwrap();

    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: concat!(
                "[package]\n",
                "enabled = true\n\n",
                "[labels]\n",
                "purpose = \"目的\"\n",
                "contract = \"契約\"\n",
                "source = \"実装\"\n",
            )
            .to_string(),
            version: 1,
            lang: None,
        },
    );

    let actions = provide_code_actions_with_state(&uri, "", &Config::default(), Some(&state));
    let inserted = action_texts(&actions).join("\n");
    let titles: Vec<String> = actions
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => Some(action.title.clone()),
            CodeActionOrCommand::Command(command) => Some(command.title.clone()),
        })
        .collect();

    assert!(
        inserted.contains("## 目的"),
        "missing custom purpose label: {inserted:?}"
    );
    assert!(
        inserted.contains("## 契約"),
        "missing custom contract label: {inserted:?}"
    );
    assert!(
        inserted.contains("## 実装"),
        "missing custom source label: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Purpose"),
        "stale default labels should not win: {inserted:?}"
    );
    assert!(
        titles.contains(&"Add missing sections: 目的, 契約, 実装".to_string()),
        "summary title should use effective labels: {titles:?}"
    );
    assert!(
        titles.contains(&"Add missing ## 目的 section".to_string()),
        "purpose title should use effective labels: {titles:?}"
    );
    assert!(
        titles.contains(&"Add missing ## 契約 section".to_string()),
        "contract title should use effective labels: {titles:?}"
    );
    assert!(
        titles.contains(&"Add missing ## 実装 section".to_string()),
        "source title should use effective labels: {titles:?}"
    );
}

#[test]
fn test_code_action_source_overview_uses_label_overrides_except_managed_headings() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let mut state = package_state_for_root(&root);
    let config_path = root.join("mds.config.toml");
    let path = root.join(".mds/source/overview.md");
    let uri = Url::from_file_path(&path).unwrap();

    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: concat!(
                "[package]\n",
                "enabled = true\n\n",
                "[labels]\n",
                "purpose = \"目的\"\n",
                "architecture = \"構成\"\n",
                "rules = \"規約\"\n",
            )
            .to_string(),
            version: 1,
            lang: None,
        },
    );

    let actions = provide_code_actions_with_state(
        &uri,
        "## 目的\n\nSource hierarchy.\n",
        &Config::default(),
        Some(&state),
    );
    let inserted = action_texts(&actions).join("\n");
    let titles: Vec<String> = actions
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => Some(action.title.clone()),
            CodeActionOrCommand::Command(command) => Some(command.title.clone()),
        })
        .collect();

    assert!(
        inserted.contains("## 構成"),
        "missing custom architecture label: {inserted:?}"
    );
    assert!(
        inserted.contains("## 規約"),
        "missing custom rules label: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Architecture"),
        "stale architecture label should not win: {inserted:?}"
    );
    assert!(
        !inserted.contains("## Rules"),
        "stale rules label should not win: {inserted:?}"
    );
    assert!(
        inserted.contains("### Package Summary"),
        "managed heading should stay fixed: {inserted:?}"
    );
    assert!(
        inserted.contains("### Dependencies"),
        "managed heading should stay fixed: {inserted:?}"
    );
    assert!(
        inserted.contains("### Dev Dependencies"),
        "managed heading should stay fixed: {inserted:?}"
    );
    assert!(
        titles.contains(&"Add missing sections: 構成, 規約".to_string()),
        "summary title should use overridden overview labels: {titles:?}"
    );
    assert!(
        titles.contains(&"Add missing ## 構成 section".to_string()),
        "architecture title should use overridden overview label: {titles:?}"
    );
    assert!(
        titles.contains(&"Add missing ## 規約 section".to_string()),
        "rules title should use overridden overview label: {titles:?}"
    );
}

#[test]
fn test_document_symbols_empty() {
    let symbols = document_symbols("");
    assert!(symbols.is_empty(), "empty document should have no symbols");
}

#[test]
fn test_document_symbols_with_h4() {
    let text = "## Purpose\n\n#### Detail\n\nContent.";
    let symbols = document_symbols(text);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Purpose"), "should have Purpose");
    // H4 headings should NOT appear in symbols (only ## and ###)
    assert!(!names.contains(&"Detail"), "should not have H4 heading");
}

#[test]
fn test_document_symbols_with_h5_shared_definition() {
    let text = "## Exports\n\n##### greet\n\nShared entrypoint.";
    let symbols = document_symbols(text);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Exports"), "should have Exports");
    assert!(names.contains(&"greet"), "should have H5 shared definition");
}

#[test]
fn test_word_at_position_boundary() {
    let text = "hello";
    // At beginning
    assert_eq!(
        word_at_position(
            text,
            Position {
                line: 0,
                character: 0
            }
        ),
        Some("hello".to_string())
    );
    // Beyond end
    assert_eq!(
        word_at_position(
            text,
            Position {
                line: 0,
                character: 100
            }
        ),
        None
    );
}

#[test]
fn test_table_cell_not_a_table() {
    let text = "This is not a table line";
    let cell = table_cell_at_position(
        text,
        Position {
            line: 0,
            character: 5,
        },
    );
    assert_eq!(cell, None);
}

#[test]
fn test_source_doc_overview_snippet_is_generic() {
    let text = "module";
    let position = Position {
        line: 0,
        character: 0,
    };
    let config = mds_core::Config::default();
    let path = std::path::Path::new("/workspace/.mds/source/app/greet.ts.md");
    let items = provide_completions(text, position, Some(path), &config, None);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(
        labels.contains(&"mds: New Impl Document"),
        "should provide impl snippet: {labels:?}"
    );
    assert!(
        labels.contains(&"mds: New Overview Document"),
        "should provide overview snippet: {labels:?}"
    );
    assert!(
        labels.contains(&"mds: New Root Module Document"),
        "should provide root-module snippet: {labels:?}"
    );
    assert!(
        !labels.contains(&"mds: New Spec Document"),
        "obsolete spec snippet should be removed: {labels:?}"
    );
    assert!(
        !labels.contains(&"mds: Imports Table Row"),
        "should not provide legacy imports row snippet: {labels:?}"
    );

    let source = items
        .iter()
        .find(|item| item.label == "mds: New Impl Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("source snippet insert text");
    assert!(
        source.contains("## API"),
        "unexpected source snippet: {source}"
    );
    assert!(
        source.contains("## Source"),
        "unexpected source snippet: {source}"
    );
    assert!(
        !source.contains("## Imports"),
        "unexpected source snippet: {source}"
    );
    assert!(
        !source.contains("## Exports"),
        "unexpected source snippet: {source}"
    );
    assert!(
        !source.contains("## Types"),
        "unexpected source snippet: {source}"
    );

    let overview = items
        .iter()
        .find(|item| item.label == "mds: New Overview Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("overview snippet insert text");
    assert!(
        overview.contains("## Architecture"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        overview.contains("## Rules"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        !overview.contains("### Package Summary"),
        "source doc overview snippet should stay generic: {overview}"
    );
    assert!(
        !overview.contains("### Dependencies"),
        "source doc overview snippet should stay generic: {overview}"
    );
    assert!(
        !overview.contains("### Dev Dependencies"),
        "source doc overview snippet should stay generic: {overview}"
    );

    let root_module = items
        .iter()
        .find(|item| item.label == "mds: New Root Module Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("root-module snippet insert text");
    assert!(
        root_module.contains("## Contract"),
        "unexpected root-module snippet: {root_module}"
    );
    assert!(
        root_module.contains("## API"),
        "unexpected root-module snippet: {root_module}"
    );
    assert!(
        !root_module.contains("## Source"),
        "root-module snippet should keep source section optional: {root_module}"
    );
    assert!(
        root_module.contains("Add a `Source` section only when this root module owns runtime behavior, and keep import or export declarations in that code block instead of duplicate metadata tables."),
        "root-module snippet should keep source guidance as prose: {root_module}"
    );
}

#[test]
fn test_source_overview_snippet_completion_uses_managed_headings_only_for_exact_path() {
    let text = "module";
    let position = Position {
        line: 0,
        character: 0,
    };
    let config = mds_core::Config::default();

    let source_overview = provide_completions(
        text,
        position,
        Some(std::path::Path::new("/workspace/.mds/source/overview.md")),
        &config,
        None,
    )
    .into_iter()
    .find(|item| item.label == "mds: New Overview Document")
    .and_then(|item| item.insert_text)
    .expect("source overview snippet insert text");
    assert!(
        source_overview.contains("### Package Summary"),
        "exact source overview should use managed headings: {source_overview}"
    );
    assert!(
        source_overview.contains("### Dependencies"),
        "exact source overview should use dependency heading: {source_overview}"
    );
    assert!(
        source_overview.contains("### Dev Dependencies"),
        "exact source overview should use dev dependency heading: {source_overview}"
    );

    let nested_overview = provide_completions(
        text,
        position,
        Some(std::path::Path::new(
            "/workspace/.mds/source/pkg/overview.md",
        )),
        &config,
        None,
    )
    .into_iter()
    .find(|item| item.label == "mds: New Overview Document")
    .and_then(|item| item.insert_text)
    .expect("nested overview snippet insert text");
    assert!(
        !nested_overview.contains("### Package Summary"),
        "nested overview should stay generic: {nested_overview}"
    );
    assert!(
        nested_overview.contains("## Architecture"),
        "nested overview should keep Architecture: {nested_overview}"
    );
    assert!(
        nested_overview.contains("## Rules"),
        "nested overview should keep Rules: {nested_overview}"
    );

    let test_overview = provide_completions(
        text,
        position,
        Some(std::path::Path::new("/workspace/.mds/test/overview.md")),
        &config,
        None,
    )
    .into_iter()
    .find(|item| item.label == "mds: New Overview Document")
    .and_then(|item| item.insert_text)
    .expect("test overview snippet insert text");
    assert!(
        !test_overview.contains("### Package Summary"),
        "test overview should stay generic: {test_overview}"
    );
    assert!(
        test_overview.contains("## Architecture"),
        "test overview should keep Architecture: {test_overview}"
    );
    assert!(
        test_overview.contains("## Rules"),
        "test overview should keep Rules: {test_overview}"
    );
}

#[test]
fn test_source_snippet_completion_uses_effective_config_label_overrides() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let mut state = package_state_for_root(&root);
    let config_path = root.join("mds.config.toml");
    let path = root.join(".mds/source/app/greet.ts.md");

    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: concat!(
                "[package]\n",
                "enabled = true\n\n",
                "[labels]\n",
                "purpose = \"目的\"\n",
                "contract = \"契約\"\n",
                "source = \"実装\"\n",
                "cases = \"確認項目\"\n",
                "architecture = \"構成\"\n",
                "rules = \"規約\"\n",
            )
            .to_string(),
            version: 1,
            lang: None,
        },
    );

    let items = provide_completions(
        "module",
        Position {
            line: 0,
            character: 0,
        },
        Some(&path),
        &Config::default(),
        Some(&state),
    );

    let source = items
        .iter()
        .find(|item| item.label == "mds: New Impl Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("impl snippet insert text");
    assert!(
        source.contains("## 目的"),
        "unexpected impl snippet: {source}"
    );
    assert!(
        source.contains("## 契約"),
        "unexpected impl snippet: {source}"
    );
    assert!(
        source.contains("## 実装"),
        "unexpected impl snippet: {source}"
    );
    assert!(
        source.contains("## 確認項目"),
        "unexpected impl snippet: {source}"
    );

    let overview = items
        .iter()
        .find(|item| item.label == "mds: New Overview Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("overview snippet insert text");
    assert!(
        overview.contains("## 目的"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        !overview.contains("### Package Summary"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        !overview.contains("### Dependencies"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        !overview.contains("### Dev Dependencies"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        overview.contains("## 構成"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        overview.contains("## 規約"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        !overview.contains("## Architecture"),
        "stale overview label should not win: {overview}"
    );
    assert!(
        !overview.contains("## Rules"),
        "stale overview label should not win: {overview}"
    );

    let root_module = items
        .iter()
        .find(|item| item.label == "mds: New Root Module Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("root-module snippet insert text");
    assert!(
        root_module.contains("## 目的"),
        "unexpected root-module snippet: {root_module}"
    );
    assert!(
        root_module.contains("## 契約"),
        "unexpected root-module snippet: {root_module}"
    );
    assert!(
        root_module
            .contains("Add a `実装` section only when this root module owns runtime behavior"),
        "unexpected root-module snippet: {root_module}"
    );
}

#[test]
fn test_source_overview_snippet_completion_uses_label_overrides_except_managed_headings() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let mut state = package_state_for_root(&root);
    let config_path = root.join("mds.config.toml");
    let path = root.join(".mds/source/overview.md");

    state.open_files.insert(
        "file:///config".to_string(),
        OpenFile {
            uri: "file:///config".to_string(),
            path: config_path,
            text: concat!(
                "[package]\n",
                "enabled = true\n\n",
                "[labels]\n",
                "purpose = \"目的\"\n",
                "architecture = \"構成\"\n",
                "rules = \"規約\"\n",
            )
            .to_string(),
            version: 1,
            lang: None,
        },
    );

    let overview = provide_completions(
        "module",
        Position {
            line: 0,
            character: 0,
        },
        Some(&path),
        &Config::default(),
        Some(&state),
    )
    .into_iter()
    .find(|item| item.label == "mds: New Overview Document")
    .and_then(|item| item.insert_text)
    .expect("source overview snippet insert text");

    assert!(
        overview.contains("## 目的"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        overview.contains("## 構成"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        overview.contains("## 規約"),
        "unexpected overview snippet: {overview}"
    );
    assert!(
        !overview.contains("## Architecture"),
        "stale overview label should not win: {overview}"
    );
    assert!(
        !overview.contains("## Rules"),
        "stale overview label should not win: {overview}"
    );
    assert!(
        overview.contains("### Package Summary"),
        "managed heading should stay fixed: {overview}"
    );
    assert!(
        overview.contains("### Dependencies"),
        "managed heading should stay fixed: {overview}"
    );
    assert!(
        overview.contains("### Dev Dependencies"),
        "managed heading should stay fixed: {overview}"
    );
}

#[test]
fn test_test_snippet_completion_uses_covers_and_test_sections() {
    let text = "module";
    let position = Position {
        line: 0,
        character: 0,
    };
    let config = mds_core::Config::default();
    let path = std::path::Path::new("/workspace/.mds/test/app/greet.ts.md");
    let items = provide_completions(text, position, Some(path), &config, None);

    let test_snippet = items
        .iter()
        .find(|item| item.label == "mds: New Test Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("test snippet insert text");
    assert!(
        test_snippet.contains("## Covers"),
        "unexpected test snippet: {test_snippet}"
    );
    assert!(
        test_snippet.contains("## Test"),
        "unexpected test snippet: {test_snippet}"
    );
    assert!(
        !test_snippet.contains("## Source"),
        "unexpected test snippet: {test_snippet}"
    );
    assert!(
        !test_snippet.contains("## Imports"),
        "unexpected test snippet: {test_snippet}"
    );
    assert!(
        !test_snippet.contains("## Exports"),
        "unexpected test snippet: {test_snippet}"
    );
}

#[test]
fn test_code_block_language_completion_includes_config_runtime_language() {
    let text = "```";
    let position = Position {
        line: 0,
        character: 3,
    };
    let mut config = Config::default();
    config.adapters.insert(Lang::Other("foo".to_string()), true);
    let path = std::path::Path::new("/test/example.foo.md");
    let items = provide_completions(text, position, Some(path), &config, None);
    let foo = items
        .iter()
        .find(|item| item.label == "foo")
        .expect("config runtime language completion");

    assert_eq!(foo.preselect, Some(true), "unexpected item: {foo:?}");
}

#[test]
fn test_code_block_language_completion_uses_workspace_descriptor_aliases() {
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

    let path = source_root.join("example.foo.md");
    let items = provide_completions(
        "```",
        Position {
            line: 0,
            character: 3,
        },
        Some(&path),
        &Config::default(),
        None,
    );
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();

    assert!(
        labels.contains(&"foo"),
        "missing descriptor label: {labels:?}"
    );
    assert!(
        labels.contains(&"fooscript"),
        "missing descriptor alias: {labels:?}"
    );
}

#[test]
fn test_unsuffixed_doc_snippets_do_not_prefill_arbitrary_language() {
    let items = provide_completions(
        "module",
        Position {
            line: 0,
            character: 0,
        },
        Some(std::path::Path::new("/workspace/.mds/source/app/greet.md")),
        &Config::default(),
        None,
    );

    let source = items
        .iter()
        .find(|item| item.label == "mds: New Impl Document")
        .and_then(|item| item.insert_text.as_deref())
        .expect("source snippet insert text");
    assert!(
        source.contains("## Source\n\n```\n${4:// implementation}\n```"),
        "unexpected source snippet: {source}"
    );
    assert!(
        !source.contains("```text") && !source.contains("```typescript"),
        "source snippet should stay neutral without active language: {source}"
    );

    let code_block = items
        .iter()
        .find(|item| item.label == "mds: Code Block")
        .and_then(|item| item.insert_text.as_deref())
        .expect("code block snippet insert text");
    assert_eq!(code_block, "```\n${1:// code}\n```\n");
}

#[test]
fn test_internal_definition_prefers_current_suffix_for_multi_suffix_language() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source_root = root.join(".mds/source");
    fs::create_dir_all(source_root.join("lib")).unwrap();
    write_multi_suffix_descriptor(&root);

    let primary = source_root.join("lib/thing.foo.md");
    let sibling = source_root.join("lib/thing.fooscript.md");
    fs::write(&primary, "## Purpose\n\nPrimary.\n").unwrap();
    fs::write(&sibling, "## Purpose\n\nSibling.\n").unwrap();

    let state = package_state_for_root(&root);
    let response = goto_definition(
        "| internal | lib/thing | Name | desc |",
        Position {
            line: 0,
            character: 15,
        },
        &source_root.join("app/caller.fooscript.md"),
        &state,
    );

    let Some(GotoDefinitionResponse::Scalar(location)) = response else {
        panic!("expected scalar definition response, got: {response:?}");
    };
    assert_eq!(location.uri.to_file_path().unwrap(), sibling);
}

#[test]
fn test_hover_uses_allowed_suffixes_when_current_suffix_target_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("pkg");
    let source_root = root.join(".mds/source");
    fs::create_dir_all(source_root.join("lib")).unwrap();
    write_multi_suffix_descriptor(&root);

    let sibling = source_root.join("lib/thing.fooscript.md");
    fs::write(&sibling, "## Purpose\n\nSibling purpose.\n").unwrap();

    let state = package_state_for_root(&root);
    let hover = provide_hover(
        "| internal | lib/thing | Name | desc |",
        Position {
            line: 0,
            character: 15,
        },
        &source_root.join("app/caller.foo.md"),
        &state,
    )
    .expect("hover result");

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(
        markup.value.contains("Sibling purpose."),
        "unexpected hover content: {}",
        markup.value
    );
}
