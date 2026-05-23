include!("../src/wizard.rs");

const TS_DESCRIPTOR: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/languages/ts.toml");
const NPM_PACKAGE_MANAGER: &str =
    include_str!("../../../examples/minimal-ts/.mds/descriptors/package-managers/npm.toml");

static TEMP_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct TestDir {
    path: std::path::PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("mds-cli-wizard-test-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("package.json"),
            r#"{
  "name": "wizard-test",
  "scripts": {
    "typecheck": "tsc --noEmit",
    "lint": "eslint .",
    "test": "vitest run"
  }
}
"#,
        )
        .unwrap();
        std::fs::write(path.join("package-lock.json"), "{}\n").unwrap();
        std::fs::create_dir_all(path.join(".mds/descriptors/languages")).unwrap();
        std::fs::create_dir_all(path.join(".mds/descriptors/package-managers")).unwrap();
        std::fs::write(
            path.join(".mds/descriptors/languages/ts.toml"),
            TS_DESCRIPTOR,
        )
        .unwrap();
        std::fs::write(
            path.join(".mds/descriptors/package-managers/npm.toml"),
            NPM_PACKAGE_MANAGER,
        )
        .unwrap();
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn visible_titles(state: &mut WizardState) -> Vec<String> {
    let mut titles = vec![state.steps[state.current_step].title(state)];
    while !state.is_final_step() {
        state.advance();
        titles.push(state.steps[state.current_step].title(state));
    }
    titles
}

#[test]
fn default_flow_skips_conditional_screens() {
    let temp = TestDir::new();
    let mut state = WizardState::new(temp.path());
    let titles = visible_titles(&mut state);

    assert_eq!(
        &titles[..5],
        [
            "What to initialize".to_string(),
            "Welcome".to_string(),
            "Section Profile Preset".to_string(),
            "Link Policy".to_string(),
            "Quality Summary".to_string(),
        ]
    );
    assert!(!titles.iter().any(|title| title == "Custom Section Labels"));
    assert!(!titles.iter().any(|title| title == "Quality Advanced"));
    assert_eq!(titles.last().unwrap(), "Confirm");
}

#[test]
fn custom_flow_enters_custom_labels_and_quality_advanced() {
    let temp = TestDir::new();
    let mut state = WizardState::new(temp.path());

    state.advance();
    state.advance();
    state.list_state.select(Some(2));
    state.apply_current_selection();
    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Custom Section Labels"
    );

    state.advance();
    assert_eq!(state.steps[state.current_step].title(&state), "Link Policy");

    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Quality Summary"
    );

    state.list_state.select(Some(1));
    state.apply_current_selection();
    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Quality Advanced"
    );
}

#[test]
fn wizard_summaries_cover_required_contracts() {
    let temp = TestDir::new();
    let mut state = WizardState::new(temp.path());

    state.current_step = state
        .steps
        .iter()
        .position(|step| matches!(step, Step::Welcome))
        .unwrap();
    let welcome = state.selection_intro_lines().join("\n");
    assert!(welcome.contains(".mds/source"));
    assert!(welcome.contains(".mds/test"));
    assert!(welcome.contains("Source overview"));
    assert!(welcome.contains("Link policy"));
    assert!(welcome.contains("Quality commands"));

    state.current_step = state
        .steps
        .iter()
        .position(|step| matches!(step, Step::QualitySummary))
        .unwrap();
    let quality = state.selection_intro_lines().join("\n");
    assert!(quality.contains("typecheck"));
    assert!(quality.contains("lint"));
    assert!(quality.contains("fix"));
    assert!(quality.contains("test"));
    assert!(quality.contains("remap"));

    state.current_step = state
        .steps
        .iter()
        .position(|step| matches!(step, Step::Confirm))
        .unwrap();
    let confirm = state.selection_intro_lines().join("\n");
    assert!(confirm.contains("mds.config.toml"));
    assert!(confirm.contains("Section profile"));
    assert!(confirm.contains("Link policy"));
    assert!(confirm.contains("AI kit"));
}

#[test]
fn descriptor_flow_asks_kind_specific_minimal_fields() {
    let temp = TestDir::new();
    let mut state = WizardState::new(temp.path());

    state.list_state.select(Some(1));
    state.apply_current_selection();
    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Descriptor Kind"
    );

    state.list_state.select(Some(0));
    state.apply_current_selection();
    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Descriptor Basics"
    );

    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Language Descriptor"
    );
    let language_rows = state.form_rows();
    assert!(language_rows
        .iter()
        .any(|row| row.label == "Match suffixes"));
    assert!(language_rows.iter().any(|row| row.label == "Fence label"));

    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Language Quality"
    );

    state.advance();
    assert_eq!(
        state.steps[state.current_step].title(&state),
        "Confirm Descriptor"
    );
    let confirm = state.selection_intro_lines().join("\n");
    assert!(confirm.contains(".mds/descriptors/languages/ts.toml"));
}

#[test]
fn descriptor_options_include_kind_specific_answers() {
    let temp = TestDir::new();
    let mut state = WizardState::new(temp.path());
    state.mode = InitMode::Descriptor;
    state.descriptor_kind = DescriptorKind::PackageManager;
    state.apply_descriptor_kind_defaults();
    state.descriptor_fields.package_display_name = "Bun".to_string();
    state.descriptor_fields.package_metadata_files = "package.json,bunfig.toml".to_string();
    state.descriptor_fields.package_lockfiles = "bun.lockb".to_string();
    state.descriptor_fields.package_test = "bun test".to_string();

    let options = state.to_descriptor_options();

    assert_eq!(options.kind, DescriptorKind::PackageManager);
    assert_eq!(options.name, "npm");
    let package_manager = options.package_manager.unwrap();
    assert_eq!(package_manager.display_name, "Bun");
    assert_eq!(
        package_manager.metadata_files,
        vec!["package.json".to_string(), "bunfig.toml".to_string()]
    );
    assert_eq!(package_manager.lockfiles, vec!["bun.lockb".to_string()]);
    assert_eq!(package_manager.test.as_deref(), Some("bun test"));
}

#[test]
fn quality_summary_distinguishes_config_script_and_package_manager_sources() {
    let temp = TestDir::new();
    std::fs::write(
        temp.path().join("package.json"),
        r#"{
  "name": "wizard-test",
  "scripts": {
    "lint": "eslint .",
    "format": "prettier --write ."
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        temp.path().join("mds.config.toml"),
        "[quality.ts]\ntype_checker = \"npm run verify\"\n",
    )
    .unwrap();

    let state = WizardState::new(temp.path());
    let toolchain = &state.toolchains[0];

    assert_eq!(toolchain.type_check.source, InitQualitySource::Config);
    assert_eq!(
        toolchain.type_check.detected.as_deref(),
        Some("npm run verify")
    );
    assert_eq!(toolchain.lint.source, InitQualitySource::ExistingScripts);
    assert_eq!(toolchain.lint.detected.as_deref(), Some("npm run lint"));
    assert_eq!(toolchain.fix.source, InitQualitySource::ExistingScripts);
    assert_eq!(toolchain.fix.detected.as_deref(), Some("npm run format"));
    assert_eq!(toolchain.test.source, InitQualitySource::PackageManager);
    assert_eq!(toolchain.test.detected.as_deref(), Some("npm test"));

    let summary = state.quality_summary_lines().join("\n");
    assert!(summary
        .contains("detected command: npm run verify [detection source: config; override: no]"));
    assert!(summary.contains(
        "detected command: npm run lint [detection source: existing scripts; override: no]"
    ));
    assert!(summary
        .contains("detected command: npm test [detection source: package manager; override: no]"));
}

#[test]
fn quality_summary_distinguishes_schema_and_unresolved_states() {
    let schema = TestDir::new();
    std::fs::write(
        schema.path().join("package.json"),
        "{\"name\":\"wizard-test\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();
    let schema_state = WizardState::new(schema.path());
    assert_eq!(
        schema_state.toolchains[0].fix.source,
        InitQualitySource::Schema
    );
    assert_eq!(
        schema_state.toolchains[0].fix.detected.as_deref(),
        Some("prettier --write")
    );
    assert!(schema_state
        .quality_summary_lines()
        .join("\n")
        .contains("detected command: prettier --write [detection source: schema; override: no]"));

    let none = TestDir::new();
    let _ = std::fs::remove_file(none.path().join("package.json"));
    std::fs::write(none.path().join("build.zig"), "").unwrap();
    let none_state = WizardState::new(none.path());
    assert!(none_state.toolchains.is_empty());
    assert!(none_state
        .quality_summary_lines()
        .join("\n")
        .contains("unresolved: no recognized package manager metadata"));
}

#[test]
fn wizard_defaults_package_json_without_lockfile_to_single_npm_toolchain() {
    let temp = TestDir::new();
    std::fs::remove_file(temp.path().join("package-lock.json")).unwrap();
    std::fs::write(
        temp.path().join("package.json"),
        "{\"name\":\"wizard-test\",\"version\":\"0.1.0\"}\n",
    )
    .unwrap();

    let state = WizardState::new(temp.path());

    assert_eq!(state.toolchains.len(), 1);
    assert_eq!(state.toolchains[0].profile.name, "Node.js (npm)");
    assert_eq!(
        state.toolchains[0].type_check.detected.as_deref(),
        Some("npm run typecheck")
    );
    assert!(state.quality_summary_lines().join("\n").contains(
        "detected command: npm run typecheck [detection source: package manager; override: no]"
    ));
}

#[test]
fn confirm_lists_actual_write_set_including_selected_ai_outputs() {
    let temp = TestDir::new();
    let mut state = WizardState::new(temp.path());
    state.ai_targets = vec![false; AiTarget::all().len()];
    state.ai_categories = vec![[false; 3]; AiTarget::all().len()];

    let codex_index = AiTarget::all()
        .iter()
        .position(|target| *target == AiTarget::CodexCli)
        .unwrap();
    state.ai_targets[codex_index] = true;
    state.ai_categories[codex_index][0] = true;
    state.current_step = state
        .steps
        .iter()
        .position(|step| matches!(step, Step::Confirm))
        .unwrap();

    let confirm = state.selection_intro_lines().join("\n");
    assert!(confirm.contains(".codex/instructions.md"));
    assert!(!confirm.contains(".codex/skills/mds.md"));
}

#[test]
fn to_options_includes_link_policy_custom_labels_and_fix_slot() {
    let temp = TestDir::new();
    let mut state = WizardState::new(temp.path());
    assert!(!state.toolchains.is_empty());

    state.section_profile = SectionProfile::Custom;
    state.custom_labels[0].value = "Goal".to_string();
    state.custom_labels[1].value = "Guarantee".to_string();
    state.custom_labels[2].value = "Implementation".to_string();
    state.custom_labels[3].value = "Verifies".to_string();
    state.custom_labels[4].value = "Scenarios".to_string();
    state.custom_labels[5].value = "Checks".to_string();
    state.link_policy = LinkPolicy::MarkdownOnly;
    state.toolchains[0].fix.current = "npm run fix".to_string();

    let options = state.to_options();

    assert_eq!(options.label_preset, LabelPreset::English);
    assert_eq!(options.link_policy, LinkPolicy::MarkdownOnly);
    assert_eq!(
        options.label_overrides.get("purpose"),
        Some(&"Goal".to_string())
    );
    assert_eq!(
        options.label_overrides.get("contract"),
        Some(&"Guarantee".to_string())
    );
    assert_eq!(
        options.label_overrides.get("source"),
        Some(&"Implementation".to_string())
    );
    assert_eq!(
        options.label_overrides.get("covers"),
        Some(&"Verifies".to_string())
    );
    assert_eq!(
        options.label_overrides.get("cases"),
        Some(&"Scenarios".to_string())
    );
    assert_eq!(
        options.label_overrides.get("test"),
        Some(&"Checks".to_string())
    );
    assert_eq!(
        options.quality_commands[0].fix.as_deref(),
        Some("npm run fix")
    );
}
