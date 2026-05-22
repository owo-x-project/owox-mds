use crate::descriptor;
use crate::diagnostics::Diagnostic;
use crate::diagnostics::RunState;
use crate::fs_utils::is_mds_managed_file;
use crate::model::AgentKitCategory;
use crate::model::AiTarget;
use crate::model::Config;
use crate::model::InitOptions;
use crate::model::InitQualityCommands;
use crate::model::Lang;
use crate::model::QualityConfig;
use crate::quality;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
};
mod template_registry {
    include!(concat!(env!("OUT_DIR"), "/template_registry.rs"));
}

const INIT_BOOTSTRAP_TS_DESCRIPTOR: &str = include_str!("bootstrap/ts.toml");
const INIT_BOOTSTRAP_NPM_MANIFEST: &str = include_str!("bootstrap/npm.toml");

#[derive(Clone, Default)]
struct InitRuntimeContext {
    package_managers: Vec<descriptor::PackageManagerManifest>,
    descriptors: Vec<descriptor::Descriptor>,
    missing_descriptors: Vec<String>,
}

impl InitRuntimeContext {
    fn detect(root: &Path) -> Self {
        let package_managers = init_runtime_package_managers(root);
        let mut descriptors = Vec::new();
        let mut missing_descriptors = Vec::new();
        for manager in &package_managers {
            let lang = manager.resolved_lang();
            let Some(descriptor) = descriptor::descriptor_for_key(lang.key())
                .or_else(|| init_bootstrap_descriptor(root, &lang))
            else {
                let lang_key = lang.key().to_string();
                if !missing_descriptors
                    .iter()
                    .any(|current| current == &lang_key)
                {
                    missing_descriptors.push(lang_key);
                }
                continue;
            };
            if descriptors
                .iter()
                .any(|current: &descriptor::Descriptor| current.id == descriptor.id)
            {
                continue;
            }
            descriptors.push(descriptor);
        }
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        missing_descriptors.sort();
        Self {
            package_managers,
            descriptors,
            missing_descriptors,
        }
    }

    fn primary_package_manager(&self) -> Option<&descriptor::PackageManagerManifest> {
        self.package_managers.first()
    }

    fn descriptor_for_lang(&self, lang: &Lang) -> Option<&descriptor::Descriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor::descriptor_matches_lang(descriptor, lang))
    }

    fn bootstrap_files(&self, root: &Path) -> Vec<PlannedFile> {
        let mut files = Vec::new();

        if self
            .package_managers
            .iter()
            .any(|manager| manager.id == "npm")
            && root.join("package.json").exists()
        {
            let path = root.join(".mds/descriptors/package-managers/npm.toml");
            if !path.exists() {
                files.push(PlannedFile {
                    path,
                    content: INIT_BOOTSTRAP_NPM_MANIFEST.to_string(),
                });
            }
        }

        if self
            .descriptors
            .iter()
            .any(|descriptor| descriptor.id == "ts")
            && root.join("package.json").exists()
        {
            let path = root.join(".mds/descriptors/languages/ts.toml");
            if !path.exists() {
                files.push(PlannedFile {
                    path,
                    content: INIT_BOOTSTRAP_TS_DESCRIPTOR.to_string(),
                });
            }
        }

        files
    }
}

pub(crate) fn run_init(
    cwd: &Path,
    package: Option<&Path>,
    options: &InitOptions,
    verbose: bool,
    state: &mut RunState,
) -> Result<(), String> {
    let root = package.map_or_else(|| cwd.to_path_buf(), |path| cwd.join(path));
    descriptor::with_workspace_descriptor_root(Some(&root), || {
        let runtime = InitRuntimeContext::detect(&root);

        let files = match collect_planned_files(&root, options, &runtime) {
            Ok(files) => files,
            Err(error) => {
                state
                    .diagnostics
                    .push(Diagnostic::error(Some(root.clone()), &error.message()));
                return Ok(());
            }
        };

        state.stdout.push_str("Init plan:\n");
        for file in &files {
            state
                .stdout
                .push_str(&format!("- write {}\n", file.path.display()));
        }

        let setup_actions = setup_actions(&root, options, &runtime);
        if !setup_actions.is_empty() {
            state.stdout.push_str("Setup plan:\n");
            for action in &setup_actions {
                state.stdout.push_str(&format!(
                    "- {}: {}{}\n",
                    action.kind,
                    action.command.join(" "),
                    action
                        .install_hint
                        .map(|hint| format!(" (install: {hint})"))
                        .unwrap_or_default()
                ));
            }
        }

        if !options.yes {
            state
                .stdout
                .push_str("No changes written. Re-run with --yes to apply the init plan.\n");
            return Ok(());
        }

        for file in files {
            write_planned_file(&file, options.force, state)?;
        }

        for action in setup_actions {
            run_setup_action(&root, &action, verbose, state);
        }

        if !state.has_errors() && !state.environment_missing {
            if !options.targets.is_empty() {
                state.stdout.push_str(&ai_post_init_guide(&options.targets));
            }
            state.stdout.push_str("init ok\n");
        }
        Ok(())
    })
}

pub fn planned_init_write_paths(root: &Path, options: &InitOptions) -> Vec<PathBuf> {
    descriptor::with_workspace_descriptor_root(Some(root), || {
        let runtime = InitRuntimeContext::detect(root);
        collect_planned_files(root, options, &runtime)
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|file| file.path)
            .collect()
    })
}

pub fn detect_init_quality_toolchains(root: &Path) -> Vec<InitQualityToolchainSummary> {
    descriptor::with_workspace_descriptor_root(Some(root), || {
        let runtime = InitRuntimeContext::detect(root);
        let config = load_init_config(root);
        runtime
            .package_managers
            .iter()
            .map(|manager| {
                let lang = manager.resolved_lang();
                let descriptor = runtime.descriptor_for_lang(&lang);
                let metadata = manager
                    .metadata_path(root)
                    .and_then(|path| {
                        path.file_name()
                            .and_then(|value| value.to_str())
                            .map(ToOwned::to_owned)
                    })
                    .unwrap_or_else(|| manager.metadata_files.first().cloned().unwrap_or_default());
                let scripts = descriptor::load_package_manager_scripts(&manager, root);
                InitQualityToolchainSummary {
                    lang: lang.clone(),
                    display_name: manager.display_name.clone(),
                    metadata,
                    type_check: detect_init_quality_slot(
                        config.as_ref(),
                        manager,
                        descriptor,
                        &lang,
                        &scripts,
                        InitQualityField::TypeCheck,
                    ),
                    lint: detect_init_quality_slot(
                        config.as_ref(),
                        manager,
                        descriptor,
                        &lang,
                        &scripts,
                        InitQualityField::Lint,
                    ),
                    fix: detect_init_quality_slot(
                        config.as_ref(),
                        manager,
                        descriptor,
                        &lang,
                        &scripts,
                        InitQualityField::Fix,
                    ),
                    test: detect_init_quality_slot(
                        config.as_ref(),
                        manager,
                        descriptor,
                        &lang,
                        &scripts,
                        InitQualityField::Test,
                    ),
                }
            })
            .collect()
    })
}

fn collect_planned_files(
    root: &Path,
    options: &InitOptions,
    runtime: &InitRuntimeContext,
) -> Result<Vec<PlannedFile>, InitPlanError> {
    if !runtime.missing_descriptors.is_empty() {
        return Err(InitPlanError::MissingLanguageDescriptors(
            runtime.missing_descriptors.clone(),
        ));
    }

    if !options.ai_only && runtime.primary_package_manager().is_none() {
        return Err(InitPlanError::MissingPackageManagerMetadata);
    }

    let mut files = Vec::new();
    if !options.ai_only {
        files.extend(runtime.bootstrap_files(root));
        files.extend(project_files(root, options, runtime));
    }
    files.extend(ai_files(root, options, runtime));
    Ok(files)
}

fn load_init_config(root: &Path) -> Option<Config> {
    let path = root.join("mds.config.toml");
    if !path.exists() {
        return None;
    }

    let mut config = Config::default();
    let mut state = RunState::default();
    crate::config::merge_config_file(&mut config, &path, &mut state)?;
    Some(config)
}

fn detect_init_quality_slot(
    config: Option<&Config>,
    manager: &descriptor::PackageManagerManifest,
    descriptor: Option<&descriptor::Descriptor>,
    lang: &Lang,
    scripts: &HashMap<String, String>,
    field: InitQualityField,
) -> InitQualitySlotSummary {
    let config = config
        .and_then(|config| config.quality.get(lang))
        .cloned()
        .unwrap_or_default();
    let resolution = resolve_init_quality_resolution(
        &config,
        Some(manager),
        descriptor,
        lang,
        scripts,
        field.quality_slot(),
    );

    InitQualitySlotSummary {
        command: resolution.command,
        source: init_quality_source(resolution.source),
    }
}

struct PlannedFile {
    path: PathBuf,
    content: String,
}

enum InitPlanError {
    MissingPackageManagerMetadata,
    MissingLanguageDescriptors(Vec<String>),
}

impl InitPlanError {
    fn message(&self) -> String {
        match self {
            Self::MissingPackageManagerMetadata => {
                "mds init requires an existing recognized package manager metadata file"
                    .to_string()
            }
            Self::MissingLanguageDescriptors(langs) => format!(
                "mds init requires package-local language descriptors for active package manager languages: {}",
                langs.join(", ")
            ),
        }
    }
}

struct SetupAction {
    kind: &'static str,
    command: Vec<String>,
    install_hint: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InitQualitySource {
    Config,
    ExistingScripts,
    PackageManager,
    Schema,
    None,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InitQualitySlotSummary {
    pub command: Option<String>,
    pub source: InitQualitySource,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InitQualityToolchainSummary {
    pub lang: Lang,
    pub display_name: String,
    pub metadata: String,
    pub type_check: InitQualitySlotSummary,
    pub lint: InitQualitySlotSummary,
    pub fix: InitQualitySlotSummary,
    pub test: InitQualitySlotSummary,
}

#[derive(Clone, Copy)]
enum InitQualityField {
    TypeCheck,
    Lint,
    Fix,
    Test,
}

impl InitQualityField {
    fn quality_slot(self) -> quality::QualitySlot {
        match self {
            Self::TypeCheck => quality::QualitySlot::Typecheck,
            Self::Lint => quality::QualitySlot::Lint,
            Self::Fix => quality::QualitySlot::Fix,
            Self::Test => quality::QualitySlot::Test,
        }
    }
}

fn init_quality_source(source: quality::QualityCommandSource) -> InitQualitySource {
    match source {
        quality::QualityCommandSource::Config | quality::QualityCommandSource::Disabled => {
            InitQualitySource::Config
        }
        quality::QualityCommandSource::PackageManagerScript => InitQualitySource::ExistingScripts,
        quality::QualityCommandSource::PackageManager => InitQualitySource::PackageManager,
        quality::QualityCommandSource::DescriptorDefault => InitQualitySource::Schema,
        quality::QualityCommandSource::None => InitQualitySource::None,
    }
}

fn project_files(
    root: &Path,
    options: &InitOptions,
    runtime: &InitRuntimeContext,
) -> Vec<PlannedFile> {
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("mds-project");
    let l_purpose = section_label(options, "purpose");
    let l_contract = section_label(options, "contract");
    let l_source = section_label(options, "source");
    let l_covers = section_label(options, "covers");
    let l_cases = section_label(options, "cases");
    let l_test = section_label(options, "test");
    let pkg_name = sanitize_package_name(name);
    let default_config = Config::default();
    let root_module = package_root_markdown_name(runtime);
    let overview_path = descriptor::package_source_overview_markdown_path(root, &default_config);
    let overview_content = init_source_overview_content(
        root,
        runtime.primary_package_manager(),
        &pkg_name,
        &root_module,
        &format!(
            "<!-- Generated by mds init. -->\n# Source Overview\n\n## {l_purpose}\n\nDescribe the `.mds/source/` hierarchy.\n\n## Architecture\n\nMarkdown files in this directory describe package and source hierarchy rules.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| {pkg_name} | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Keep package or directory root API notes in `{root_module}`.\n- Add source code blocks there only when the root module owns runtime behavior.\n- Keep one source md per feature.\n",
        ),
    );
    let files = vec![
        PlannedFile {
            path: root.join("mds.config.toml"),
            content: project_config(root, options, runtime),
        },
        PlannedFile {
            path: overview_path,
            content: overview_content,
        },
        PlannedFile {
            path: root.join(".mds/source").join(&root_module),
            content: format!(
                "<!-- Generated by mds init. -->\n# Package Root\n\n## {l_purpose}\n\nDescribe the package root module surface.\n\n## {l_contract}\n\n- Describe stable exports, re-exports, and entrypoint constraints.\n\n## API\n\nDescribe the public API and re-exports in prose. Add a `{l_source}` section only when this root module owns runtime behavior, and keep import and export declarations in that code block instead of duplicate tables.\n",
            ),
        },
        PlannedFile {
            path: descriptor::package_test_overview_markdown_path(root, &default_config),
            content: format!(
                "<!-- Generated by mds init. -->\n# Test Overview\n\n## {l_purpose}\n\nDescribe the `.mds/test/` hierarchy.\n\n## Architecture\n\nMarkdown files in this directory define test intent and executable test code.\n\n## Rules\n\n- Keep one test md per verification target.\n- Use `{l_covers}` to point at the source module id being verified.\n",
            ),
        },
        PlannedFile {
            path: root.join(".mds/reference/overview.md"),
            content: format!(
                "# Source Overview Example\n\n## {l_purpose}\n\nDescribe the source hierarchy and package-level intent.\n\n## Architecture\n\nSummarize how source markdown maps to generated code.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| example-package | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Keep package or directory root docs focused on API prose and add code blocks there only when the root module owns runtime behavior.\n- Keep one source md per feature.\n"
            ),
        },
        PlannedFile {
            path: root.join(".mds/reference/root-module.md"),
            content: format!(
                "# Package Root Example\n\n## {l_purpose}\n\nDescribe the package root module surface.\n\n## {l_contract}\n\n- Re-export the package entrypoints from the root module.\n\n## API\n\nSummarize public exports and re-exports in prose. Keep import and export declarations in the root module code block instead of duplicate metadata tables.\n\n## {l_source}\n\n```ts\nexport {{ greet }} from './greet';\nexport type {{ Greeting }} from './greet';\n```\n"
            ),
        },
        PlannedFile {
            path: root.join(".mds/reference/impl.md"),
            content: format!(
                "# greet\n\n## {l_purpose}\n\nProvide the greeting entrypoint.\n\n## {l_contract}\n\n- Return a greeting string for the supplied name.\n\n## API\n\n`greet` is the public greeting function. Import helpers in the source block with normal language imports.\n\n## {l_source}\n\n```ts\nimport {{ formatName }} from './format-name';\n\nexport type Greeting = string;\n\nexport function greet(name: string): Greeting {{\n  return `Hello, ${{formatName(name)}}`;\n}}\n```\n\n## {l_cases}\n\n- Returns a stable greeting for a valid name.\n"
            ),
        },
        PlannedFile {
            path: root.join(".mds/reference/test.md"),
            content: format!(
                "# greet test\n\n## {l_purpose}\n\nVerify the greeting behavior.\n\n## {l_covers}\n\n- greet\n\n## {l_cases}\n\n- Returns `Hello, Ada` for `Ada`.\n\n## {l_test}\n\n```ts\nimport {{ describe, expect, it }} from 'vitest';\n\nimport {{ greet }} from './greet';\n\ndescribe('greet', () => {{\n  it('returns a greeting', () => {{\n    expect(greet('Ada')).toBe('Hello, Ada');\n  }});\n}});\n```\n"
            ),
        },
    ];
    files
}

fn package_root_markdown_name(runtime: &InitRuntimeContext) -> String {
    runtime
        .primary_package_manager()
        .and_then(|manager| runtime.descriptor_for_lang(&manager.resolved_lang()))
        .or_else(|| runtime.descriptors.first())
        .map(descriptor::Descriptor::default_root_module_markdown_name)
        .unwrap_or_else(|| "index.ts.md".to_string())
}

fn project_config(root: &Path, options: &InitOptions, runtime: &InitRuntimeContext) -> String {
    let mut content = format!(
        "# Generated by mds init.\n[package]\nenabled = true\nallow_raw_source = false\n\n[roots]\nsource_md = \".mds/source\"\ntest_md = \".mds/test\"\nsource_out = \"src\"\ntest_out = \"tests\"\n\n[authoring]\nlink_policy = \"{}\"\n",
        options.link_policy.as_str()
    );
    for descriptor in &runtime.descriptors {
        content.push_str(&render_quality(
            &descriptor.id,
            project_config_quality_for_descriptor(root, options, runtime, descriptor),
        ));
    }
    let labels = label_entries(options);
    if !labels.is_empty() {
        content.push_str("\n[labels]\n");
        for (key, value) in labels {
            content.push_str(&format!("{key} = \"{value}\"\n"));
        }
    }
    content
}

fn section_label(options: &InitOptions, canonical: &str) -> String {
    if let Some(value) = options
        .label_overrides
        .get(canonical)
        .filter(|value| !value.trim().is_empty())
    {
        return value.clone();
    }
    options.label_preset.section_label(canonical)
}

fn label_entries(options: &InitOptions) -> Vec<(String, String)> {
    let mut labels = BTreeMap::new();
    for (key, value) in options.label_preset.labels() {
        if crate::config::is_supported_label(key) {
            labels.insert((*key).to_string(), (*value).to_string());
        }
    }
    for (key, value) in &options.label_overrides {
        if !value.trim().is_empty() {
            labels.insert(key.to_string(), value.to_string());
        }
    }
    labels.into_iter().collect()
}

struct InitQuality {
    type_check: Option<String>,
    lint: Option<String>,
    fix: Option<String>,
    test: Option<String>,
    required: Vec<String>,
    optional: Vec<String>,
}

fn project_config_quality_for_descriptor(
    root: &Path,
    options: &InitOptions,
    runtime: &InitRuntimeContext,
    descriptor: &descriptor::Descriptor,
) -> InitQuality {
    let lang = Lang::Other(descriptor.id.clone());
    if options
        .quality_commands
        .iter()
        .any(|commands| commands.lang.key() == lang.key())
    {
        return quality_for_descriptor(options, descriptor);
    }
    let package_manager = runtime
        .package_managers
        .iter()
        .find(|manager| descriptor::langs_match(&manager.resolved_lang(), &lang));
    init_quality_from_effective_resolution(root, package_manager, descriptor)
}

fn quality_for_descriptor(
    options: &InitOptions,
    descriptor: &descriptor::Descriptor,
) -> InitQuality {
    let lang = Lang::Other(descriptor.id.clone());
    if let Some(commands) = options
        .quality_commands
        .iter()
        .find(|commands| commands.lang.key() == lang.key())
    {
        return custom_quality(descriptor, commands);
    }
    init_quality_from_defaults(descriptor)
}

fn custom_quality(
    descriptor: &descriptor::Descriptor,
    commands: &InitQualityCommands,
) -> InitQuality {
    let mut quality = InitQuality {
        type_check: commands.type_check.clone(),
        lint: commands.lint.clone(),
        fix: commands.fix.clone(),
        test: commands.test.clone(),
        required: Vec::new(),
        optional: Vec::new(),
    };
    add_profile_tools_for_selected_commands(descriptor, &mut quality);
    quality
}

fn init_quality_from_defaults(descriptor: &descriptor::Descriptor) -> InitQuality {
    let mut quality = InitQuality {
        type_check: descriptor.default_typecheck_command().map(str::to_string),
        lint: descriptor.default_lint_command().map(str::to_string),
        fix: descriptor.default_fix_command().map(str::to_string),
        test: descriptor.default_test_command().map(str::to_string),
        required: Vec::new(),
        optional: Vec::new(),
    };
    add_profile_tools_for_selected_commands(descriptor, &mut quality);
    quality
}

fn init_quality_from_effective_resolution(
    root: &Path,
    package_manager: Option<&descriptor::PackageManagerManifest>,
    descriptor: &descriptor::Descriptor,
) -> InitQuality {
    let lang = Lang::Other(descriptor.id.clone());
    let scripts = package_manager
        .map(|manager| descriptor::load_package_manager_scripts(manager, root))
        .unwrap_or_default();
    let config = QualityConfig::default();
    let type_check = resolve_init_quality_resolution(
        &config,
        package_manager,
        Some(descriptor),
        &lang,
        &scripts,
        quality::QualitySlot::Typecheck,
    );
    let lint = resolve_init_quality_resolution(
        &config,
        package_manager,
        Some(descriptor),
        &lang,
        &scripts,
        quality::QualitySlot::Lint,
    );
    let fix = resolve_init_quality_resolution(
        &config,
        package_manager,
        Some(descriptor),
        &lang,
        &scripts,
        quality::QualitySlot::Fix,
    );
    let test = resolve_init_quality_resolution(
        &config,
        package_manager,
        Some(descriptor),
        &lang,
        &scripts,
        quality::QualitySlot::Test,
    );
    let mut quality = InitQuality {
        type_check: type_check.command.clone(),
        lint: lint.command.clone(),
        fix: fix.command.clone(),
        test: test.command.clone(),
        required: Vec::new(),
        optional: Vec::new(),
    };
    add_profile_tools_for_effective_slot(
        descriptor,
        &scripts,
        quality::QualitySlot::Typecheck,
        &type_check,
        &mut quality,
    );
    add_profile_tools_for_effective_slot(
        descriptor,
        &scripts,
        quality::QualitySlot::Lint,
        &lint,
        &mut quality,
    );
    add_profile_tools_for_effective_slot(
        descriptor,
        &scripts,
        quality::QualitySlot::Fix,
        &fix,
        &mut quality,
    );
    add_profile_tools_for_effective_slot(
        descriptor,
        &scripts,
        quality::QualitySlot::Test,
        &test,
        &mut quality,
    );
    quality
}

fn resolve_init_quality_resolution(
    config: &QualityConfig,
    package_manager: Option<&descriptor::PackageManagerManifest>,
    descriptor: Option<&descriptor::Descriptor>,
    lang: &Lang,
    scripts: &HashMap<String, String>,
    slot: quality::QualitySlot,
) -> quality::QualityCommandResolution {
    quality::resolve_quality_command_for_config_with_descriptor(
        config,
        descriptor,
        lang,
        slot,
        package_manager,
        scripts,
    )
}

fn add_profile_tools_for_selected_commands(
    descriptor: &descriptor::Descriptor,
    quality: &mut InitQuality,
) {
    if let Some(command) = quality.type_check.clone() {
        if let Some(profile) =
            slot_profile_for_command(descriptor, quality::QualitySlot::Typecheck, &command)
        {
            add_profile_tools(quality, profile);
        }
    }
    if let Some(command) = quality.lint.clone() {
        if let Some(profile) =
            slot_profile_for_command(descriptor, quality::QualitySlot::Lint, &command)
        {
            add_profile_tools(quality, profile);
        }
    }
    if let Some(command) = quality.fix.clone() {
        if let Some(profile) =
            slot_profile_for_command(descriptor, quality::QualitySlot::Fix, &command)
        {
            add_profile_tools(quality, profile);
        }
    }
    if let Some(command) = quality.test.clone() {
        if let Some(profile) =
            slot_profile_for_command(descriptor, quality::QualitySlot::Test, &command)
        {
            add_profile_tools(quality, profile);
        }
    }
}

fn add_profile_tools(quality: &mut InitQuality, profile: &descriptor::ToolProfile) {
    for tool in &profile.required {
        add_unique(&mut quality.required, tool);
    }
    for tool in &profile.optional {
        add_unique(&mut quality.optional, tool);
    }
}

fn add_profile_tools_for_effective_slot(
    descriptor: &descriptor::Descriptor,
    scripts: &HashMap<String, String>,
    slot: quality::QualitySlot,
    resolution: &quality::QualityCommandResolution,
    quality: &mut InitQuality,
) {
    if let Some(profile) = effective_slot_profile(descriptor, scripts, slot, resolution) {
        add_profile_tools(quality, profile);
    }
}

fn effective_slot_profile<'a>(
    descriptor: &'a descriptor::Descriptor,
    scripts: &HashMap<String, String>,
    slot: quality::QualitySlot,
    resolution: &quality::QualityCommandResolution,
) -> Option<&'a descriptor::ToolProfile> {
    if resolution.source == quality::QualityCommandSource::PackageManagerScript {
        if let Some(script_text) =
            matched_quality_script_text(scripts, init_quality_slot_kind(slot))
        {
            if let Some(profile) = slot_profile_for_command(descriptor, slot, script_text) {
                return Some(profile);
            }
        }
    }

    resolution
        .command
        .as_deref()
        .and_then(|command| slot_profile_for_command(descriptor, slot, command))
}

fn matched_quality_script_text<'a>(
    scripts: &'a HashMap<String, String>,
    kind: &str,
) -> Option<&'a str> {
    init_quality_slot_script_names(kind)
        .iter()
        .find_map(|name| scripts.get(*name).map(String::as_str))
}

fn init_quality_slot_script_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "typecheck" => &["typecheck"],
        "lint" => &["lint"],
        "fix" => &["fix", "format"],
        "test" => &["test"],
        _ => &[],
    }
}

fn init_quality_slot_kind(slot: quality::QualitySlot) -> &'static str {
    match slot {
        quality::QualitySlot::Typecheck => "typecheck",
        quality::QualitySlot::Lint => "lint",
        quality::QualitySlot::Fix => "fix",
        quality::QualitySlot::Test => "test",
    }
}

fn slot_profile_for_command<'a>(
    descriptor: &'a descriptor::Descriptor,
    slot: quality::QualitySlot,
    command: &str,
) -> Option<&'a descriptor::ToolProfile> {
    match slot {
        quality::QualitySlot::Typecheck => descriptor
            .tool_profiles
            .typecheck
            .values()
            .find(|profile| command_text_matches_profile(command, &profile.command)),
        quality::QualitySlot::Lint => descriptor
            .tool_profiles
            .lint
            .values()
            .find(|profile| command_text_matches_profile(command, &profile.command)),
        quality::QualitySlot::Fix => descriptor
            .tool_profiles
            .fix
            .values()
            .find(|profile| command_text_matches_profile(command, &profile.command)),
        quality::QualitySlot::Test => descriptor
            .tool_profiles
            .test
            .values()
            .find(|profile| command_text_matches_profile(command, &profile.command)),
    }
}

fn command_text_matches_profile(command_text: &str, profile_command: &str) -> bool {
    for segment in command_text
        .split(['\n', ';'])
        .flat_map(|segment| segment.split("&&"))
        .flat_map(|segment| segment.split("||"))
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
    {
        if command_prefix_matches_profile(segment, profile_command) {
            return true;
        }
    }
    false
}

fn command_prefix_matches_profile(command: &str, profile_command: &str) -> bool {
    for prefix in [
        "",
        "npx ",
        "npx --no-install ",
        "npm exec ",
        "npm exec -- ",
        "pnpm exec ",
        "pnpm exec -- ",
        "yarn ",
        "yarn exec ",
        "yarn exec -- ",
        "bunx ",
    ] {
        let candidate = format!("{prefix}{profile_command}");
        if command == candidate {
            return true;
        }
        if let Some(rest) = command.strip_prefix(&candidate) {
            if rest.chars().next().is_some_and(char::is_whitespace) {
                return true;
            }
        }
    }
    false
}

fn add_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|current| current == value) {
        values.push(value.to_string());
    }
}

fn render_quality(lang: &str, quality: InitQuality) -> String {
    let mut content = format!("\n[quality.{lang}]\n");
    if quality.type_check.is_some() {
        content.push_str(&format!(
            "type_checker = {}\n",
            render_optional_owned_command(quality.type_check.as_deref())
        ));
    }
    content.push_str(&format!(
        "linter = {}\nfixer = {}\ntest_runner = {}\nrequired = [{}]\noptional = [{}]\n",
        render_optional_command(quality.lint.as_deref()),
        render_optional_command(quality.fix.as_deref()),
        render_optional_command(quality.test.as_deref()),
        render_string_array(&quality.required),
        render_string_array(&quality.optional),
    ));
    content
}

fn render_optional_command(command: Option<&str>) -> String {
    command
        .map(|command| format!("\"{command}\""))
        .unwrap_or_else(|| "false".to_string())
}

fn render_optional_owned_command(command: Option<&str>) -> String {
    command
        .filter(|command| !command.trim().is_empty())
        .map(|command| format!("\"{command}\""))
        .unwrap_or_else(|| "false".to_string())
}

fn render_string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn ai_files(root: &Path, options: &InitOptions, runtime: &InitRuntimeContext) -> Vec<PlannedFile> {
    let mut files = Vec::new();
    if options.target_categories.is_empty() {
        for target in &options.targets {
            for file in ai_target_files(root, *target, &options.categories, options, runtime) {
                files.push(file);
            }
        }
        return files;
    }
    for target in &options.target_categories {
        for file in ai_target_files(root, target.target, &target.categories, options, runtime) {
            files.push(file);
        }
    }
    files
}

fn ai_target_files(
    root: &Path,
    target: AiTarget,
    categories: &[AgentKitCategory],
    options: &InitOptions,
    runtime: &InitRuntimeContext,
) -> Vec<PlannedFile> {
    let entries = template_registry::templates_for_target(target.key());
    entries
        .into_iter()
        .filter(|entry| {
            AgentKitCategory::parse(entry.category).is_some_and(|cat| categories.contains(&cat))
        })
        .flat_map(|entry| expand_template_entry(root, entry, options, runtime))
        .collect()
}

fn descriptor_match_suffixes(descriptor: &descriptor::Descriptor) -> Vec<String> {
    let mut suffixes = descriptor.match_suffixes.clone();
    if !suffixes.contains(&descriptor.language.primary_ext) {
        suffixes.push(descriptor.language.primary_ext.clone());
    }
    suffixes
}

fn expand_template_entry(
    root: &Path,
    entry: &template_registry::TemplateEntry,
    options: &InitOptions,
    runtime: &InitRuntimeContext,
) -> Vec<PlannedFile> {
    if !entry.output_path.contains("{{LANG_ID}}") && !entry.content.contains("{{LANG_ID}}") {
        return vec![PlannedFile {
            path: root.join(entry.output_path),
            content: apply_label_policy(entry.content, options),
        }];
    }

    runtime
        .descriptors
        .iter()
        .map(|descriptor| PlannedFile {
            path: root.join(render_language_template(
                entry.output_path,
                descriptor,
                options,
            )),
            content: render_language_template(entry.content, &descriptor, options),
        })
        .collect()
}

fn render_language_template(
    template: &str,
    descriptor: &descriptor::Descriptor,
    options: &InitOptions,
) -> String {
    let import_row = language_import_row();
    let import_statement = descriptor
        .render_import(&import_row)
        .unwrap_or_else(|| "// No generated import for this descriptor.".to_string());
    template
        .replace("{{LANG_ID}}", &descriptor.id)
        .replace("{{LANG_SUFFIXES}}", &descriptor_match_suffixes(descriptor).join(","))
        .replace("{{FENCE_LANG}}", &descriptor.language.primary_ext)
        .replace("{{IMPORT_STYLE}}", &descriptor.imports.style)
        .replace("{{IMPORT_ROW}}", "| internal | ./dep | ImportedSymbol | - | Dependency symbol. | [./dep.md#imported-symbol](./dep.md#imported-symbol) |")
        .replace("{{GENERATED_IMPORT}}", &import_statement)
        .replace("{{IMPORTS}}", &section_label(options, "imports"))
        .replace("{{EXPORTS}}", &section_label(options, "exports"))
        .replace("{{FROM}}", &section_label(options, "from"))
        .replace("{{TARGET}}", &section_label(options, "target"))
        .replace("{{SYMBOLS}}", &section_label(options, "symbols"))
        .replace("{{VIA}}", &section_label(options, "via"))
        .replace("{{SUMMARY}}", &section_label(options, "summary"))
        .replace("{{REFERENCE}}", &section_label(options, "reference"))
        .replace("{{NAME}}", &section_label(options, "name"))
        .replace("{{VISIBILITY}}", &section_label(options, "visibility"))
}

fn language_import_row() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::from([
        ("from".to_string(), "internal".to_string()),
        ("target".to_string(), "./dep".to_string()),
        ("symbols".to_string(), "ImportedSymbol".to_string()),
        ("via".to_string(), "-".to_string()),
    ])
}

fn apply_label_policy(content: &str, options: &InitOptions) -> String {
    let mut result = content.to_string();
    let replacements: &[(&str, &str)] = &[
        ("{{PURPOSE}}", "purpose"),
        ("{{CONTRACT}}", "contract"),
        ("{{SOURCE}}", "source"),
        ("{{CASES}}", "cases"),
        ("{{TEST}}", "test"),
        ("{{COVERS}}", "covers"),
        ("{{IMPORTS}}", "imports"),
        ("{{EXPORTS}}", "exports"),
        ("{{EXPOSE}}", "expose"),
        ("{{FROM}}", "from"),
        ("{{TARGET}}", "target"),
        ("{{SYMBOLS}}", "symbols"),
        ("{{VIA}}", "via"),
        ("{{SUMMARY}}", "summary"),
        ("{{REFERENCE}}", "reference"),
        ("{{NAME}}", "name"),
        ("{{VISIBILITY}}", "visibility"),
    ];
    for (placeholder, key) in replacements {
        let label = section_label(options, key);
        result = result.replace(placeholder, &label);
    }
    result
}

fn ai_post_init_guide(targets: &[AiTarget]) -> String {
    let mut guide = String::new();
    guide.push_str("\nIntegration guide:\n");
    for target in targets {
        guide.push_str(&format!(
            "  {}: generated files follow template manifest output paths\n",
            target.key()
        ));
    }
    guide
}

fn write_planned_file(file: &PlannedFile, force: bool, state: &mut RunState) -> Result<(), String> {
    if file.path.exists() && !is_mds_managed_file(&file.path) && !force {
        state.diagnostics.push(Diagnostic::error(
            Some(file.path.clone()),
            "refusing to overwrite non-managed file during init; re-run with --force after reviewing the diff",
        ));
        return Ok(());
    }
    if let Some(parent) = file.path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&file.path, &file.content)
        .map_err(|error| format!("failed to write {}: {error}", file.path.display()))?;
    state.generated.push(file.path.clone());
    Ok(())
}

fn setup_actions(
    root: &Path,
    options: &InitOptions,
    runtime: &InitRuntimeContext,
) -> Vec<SetupAction> {
    let mut actions = Vec::new();
    if options.install_project_deps {
        if let Some(command) = runtime
            .primary_package_manager()
            .and_then(|manager| manager.command("install"))
        {
            actions.push(SetupAction {
                kind: "project-deps",
                command: split_command(command),
                install_hint: None,
            });
        }
    }
    if options.install_toolchains {
        for (tool, hint) in [
            (
                "node",
                "install Node.js 24+ from nodejs.org or a trusted version manager",
            ),
            ("rustc", "rustup toolchain install stable"),
            ("cargo", "rustup toolchain install stable"),
            (
                "python3",
                "install Python 3.13+ from python.org or a trusted package manager",
            ),
            ("uv", "python3 -m pip install --user uv"),
        ] {
            push_toolchain_check(&mut actions, tool, hint);
        }
        for tool in selected_quality_tools(root, options, runtime) {
            push_toolchain_check(&mut actions, &tool, quality_tool_hint(&tool));
        }
    }
    if options.install_ai_cli {
        for (tool, hint) in [
            ("claude", "npm install -g @anthropic-ai/claude-code"),
            ("codex", "npm install -g @openai/codex"),
            ("opencode", "npm install -g opencode-ai"),
            (
                "gh",
                "install GitHub CLI, then gh extension install github/gh-copilot",
            ),
        ] {
            actions.push(SetupAction {
                kind: "ai-cli-check",
                command: vec![tool.to_string(), "--version".to_string()],
                install_hint: Some(hint),
            });
        }
    }
    actions
}

fn selected_quality_tools(
    root: &Path,
    options: &InitOptions,
    runtime: &InitRuntimeContext,
) -> Vec<String> {
    let mut tools = Vec::new();
    for descriptor in &runtime.descriptors {
        let quality = project_config_quality_for_descriptor(root, options, runtime, descriptor);
        for tool in quality.required.into_iter().chain(quality.optional) {
            add_unique(&mut tools, &tool);
        }
    }
    tools
}

fn push_toolchain_check(actions: &mut Vec<SetupAction>, tool: &str, hint: &'static str) {
    if actions.iter().any(|action| {
        action.kind == "toolchain-check" && action.command.first().is_some_and(|name| name == tool)
    }) {
        return;
    }
    actions.push(SetupAction {
        kind: "toolchain-check",
        command: vec![tool.to_string(), "--version".to_string()],
        install_hint: Some(hint),
    });
}

fn quality_tool_hint(tool: &str) -> &'static str {
    match tool {
        "tsc" => "npm install --save-dev typescript",
        "eslint" => "npm install --save-dev eslint",
        "prettier" => "npm install --save-dev prettier",
        "biome" => "npm install --save-dev @biomejs/biome",
        "vitest" => "npm install --save-dev vitest",
        "jest" => "npm install --save-dev jest",
        "mypy" => "uv add --dev mypy",
        "ruff" => "uv add --dev ruff",
        "black" => "uv add --dev black",
        "pytest" => "uv add --dev pytest",
        "rustfmt" => "rustup component add rustfmt",
        "clippy-driver" => "rustup component add clippy",
        "cargo-nextest" => "cargo install cargo-nextest",
        "node" => "install Node.js 24+ from nodejs.org or a trusted version manager",
        "python3" => "install Python 3.13+ from python.org or a trusted package manager",
        "rustc" | "cargo" => "rustup toolchain install stable",
        "uv" => "python3 -m pip install --user uv",
        _ => "install the selected quality tool from its official source",
    }
}

fn split_command(command: &str) -> Vec<String> {
    command.split_whitespace().map(ToOwned::to_owned).collect()
}

fn init_runtime_package_managers(root: &Path) -> Vec<descriptor::PackageManagerManifest> {
    let package_managers = descriptor::package_managers_for_root(root);
    if !package_managers.is_empty() {
        return package_managers;
    }
    init_bootstrap_package_manager(root).into_iter().collect()
}

fn init_bootstrap_package_manager(root: &Path) -> Option<descriptor::PackageManagerManifest> {
    let package_json = root.join("package.json");
    if !package_json.exists() || has_non_npm_lockfile(root) {
        return None;
    }

    let explicit = read_package_manager_field(&package_json)?;
    if !explicit.is_empty() && explicit != "npm" {
        return None;
    }

    toml::from_str(INIT_BOOTSTRAP_NPM_MANIFEST).ok()
}

fn init_bootstrap_descriptor(root: &Path, lang: &Lang) -> Option<descriptor::Descriptor> {
    if lang.key() != "ts" || init_bootstrap_package_manager(root).is_none() {
        return None;
    }
    toml::from_str(INIT_BOOTSTRAP_TS_DESCRIPTOR).ok()
}

fn has_non_npm_lockfile(root: &Path) -> bool {
    ["pnpm-lock.yaml", "yarn.lock", "bun.lock", "bun.lockb"]
        .into_iter()
        .any(|lockfile| root.join(lockfile).exists())
}

fn read_package_manager_field(package_json: &Path) -> Option<String> {
    let text = fs::read_to_string(package_json).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let explicit = value
        .get("packageManager")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    Some(
        explicit
            .split('@')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string(),
    )
}

fn init_source_overview_content(
    root: &Path,
    package_manager: Option<&descriptor::PackageManagerManifest>,
    fallback_name: &str,
    _root_module: &str,
    template: &str,
) -> String {
    let Some(metadata) = init_source_overview_metadata(root, package_manager) else {
        return template.to_string();
    };

    let name = if metadata.name.trim().is_empty() {
        fallback_name
    } else {
        metadata.name.as_str()
    };

    let content = replace_overview_section(
        template,
        "Package Summary",
        &render_package_summary_table(name, &metadata.version),
    );
    let content = replace_overview_section(
        &content,
        "Dependencies",
        &render_dependency_table(&metadata.dependencies),
    );
    replace_overview_section(
        &content,
        "Dev Dependencies",
        &render_dependency_table(&metadata.dev_dependencies),
    )
}

fn init_source_overview_metadata(
    root: &Path,
    package_manager: Option<&descriptor::PackageManagerManifest>,
) -> Option<crate::model::PackageMetadata> {
    let package_manager = package_manager?;
    let mut state = RunState::default();
    crate::package::read_package_metadata_for_manager(root, package_manager, &mut state)
}

fn render_package_summary_table(name: &str, version: &str) -> String {
    format!("| Name | Version |\n| --- | --- |\n| {name} | {version} |")
}

fn render_dependency_table(dependencies: &HashMap<String, String>) -> String {
    let mut sorted = dependencies.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    let mut output = String::from("| Name | Version | Summary |\n| --- | --- | --- |");
    for (name, version) in sorted {
        output.push_str(&format!("\n| {name} | {version} |  |"));
    }
    output
}

fn replace_overview_section(text: &str, heading: &str, replacement: &str) -> String {
    let heading = format!("### {heading}");
    let lines = text.lines().collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| line.trim() == heading) else {
        return text.to_string();
    };

    let mut end = start + 1;
    while end < lines.len() {
        let trimmed = lines[end].trim();
        let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
        if hashes > 0 && hashes <= 3 && trimmed.as_bytes().get(hashes) == Some(&b' ') {
            break;
        }
        end += 1;
    }

    let mut output = String::new();
    output.push_str(&lines[..=start].join("\n"));
    output.push_str("\n\n");
    output.push_str(replacement.trim_end());
    output.push('\n');
    if end < lines.len() {
        output.push('\n');
        output.push_str(&lines[end..].join("\n"));
    }
    if text.ends_with('\n') && !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn run_setup_action(root: &Path, action: &SetupAction, verbose: bool, state: &mut RunState) {
    if action.command.is_empty() {
        return;
    }
    if verbose {
        state.stdout.push_str(&format!(
            "running setup action: {}\n",
            action.command.join(" ")
        ));
    }
    let mut command = ProcessCommand::new(&action.command[0]);
    command.args(&action.command[1..]).current_dir(root);
    match command.output() {
        Ok(output) if output.status.success() => {
            state
                .stdout
                .push_str(&format!("setup ok: {}\n", action.command.join(" ")));
        }
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr);
            state.diagnostics.push(Diagnostic::error(
                None,
                format!(
                    "setup action failed: {}{}{}",
                    action.command.join(" "),
                    if detail.trim().is_empty() {
                        String::new()
                    } else {
                        format!(": {}", detail.trim())
                    },
                    action
                        .install_hint
                        .map(|hint| format!("; install hint: {hint}"))
                        .unwrap_or_default()
                ),
            ));
        }
        Err(error) => {
            state.environment_missing = true;
            state.diagnostics.push(Diagnostic::error(
                None,
                format!(
                    "setup action could not start: {}: {error}{}",
                    action.command.join(" "),
                    action
                        .install_hint
                        .map(|hint| format!("; install hint: {hint}"))
                        .unwrap_or_default()
                ),
            ));
        }
    }
}

fn sanitize_package_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
