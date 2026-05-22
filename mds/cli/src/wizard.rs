use std::collections::HashMap;
use std::io::{self, stdout};
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use mds_core::{
    detect_init_quality_toolchains, planned_init_write_paths, AgentKitCategory, AiTarget,
    DescriptorKind, InitDescriptorOptions, InitLanguageDescriptorOptions, InitOptions,
    InitPackageManagerDescriptorOptions, InitQualityCommands, InitQualitySource,
    InitTargetCategories, InitToolDescriptorOptions, LabelPreset, Lang, LinkPolicy,
};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
};

#[derive(Clone, Copy)]
enum Step {
    InitMode,
    Welcome,
    SectionProfilePreset,
    CustomSectionLabels,
    LinkPolicy,
    QualitySummary,
    QualityAdvanced,
    AiNeeded,
    AiTargets,
    AiCategories { target: usize },
    Confirm,
    DescriptorKind,
    DescriptorCommon,
    DescriptorLanguage,
    DescriptorLanguageQuality,
    DescriptorTool,
    DescriptorPackageManager,
    DescriptorConfirm,
}

impl Step {
    fn title(&self, state: &WizardState) -> String {
        let japanese = state.is_japanese();
        match self {
            Step::InitMode => localized(japanese, "What to initialize", "初期化対象").into(),
            Step::Welcome => localized(japanese, "Welcome", "ようこそ").into(),
            Step::SectionProfilePreset => {
                localized(japanese, "Section Profile Preset", "セクションプロファイル").into()
            }
            Step::CustomSectionLabels => localized(
                japanese,
                "Custom Section Labels",
                "カスタムセクションラベル",
            )
            .into(),
            Step::LinkPolicy => localized(japanese, "Link Policy", "リンクポリシー").into(),
            Step::QualitySummary => localized(japanese, "Quality Summary", "品質サマリー").into(),
            Step::QualityAdvanced => localized(japanese, "Quality Advanced", "品質詳細").into(),
            Step::AiNeeded => localized(japanese, "AI Kit", "AI キット").into(),
            Step::AiTargets => localized(japanese, "AI CLI Targets", "AI CLI 対象").into(),
            Step::AiCategories { target } => {
                if japanese {
                    format!(
                        "{} の生成カテゴリ",
                        ai_target_label(AiTarget::all()[*target])
                    )
                } else {
                    format!("{} Categories", ai_target_label(AiTarget::all()[*target]))
                }
            }
            Step::Confirm => localized(japanese, "Confirm", "確認").into(),
            Step::DescriptorKind => {
                localized(japanese, "Descriptor Kind", "descriptor 種別").into()
            }
            Step::DescriptorCommon => {
                localized(japanese, "Descriptor Basics", "descriptor 基本").into()
            }
            Step::DescriptorLanguage => {
                localized(japanese, "Language Descriptor", "language descriptor").into()
            }
            Step::DescriptorLanguageQuality => {
                localized(japanese, "Language Quality", "language quality").into()
            }
            Step::DescriptorTool => {
                localized(japanese, "Tool Descriptor", "tool descriptor").into()
            }
            Step::DescriptorPackageManager => localized(
                japanese,
                "Package Manager Descriptor",
                "package-manager descriptor",
            )
            .into(),
            Step::DescriptorConfirm => {
                localized(japanese, "Confirm Descriptor", "descriptor 確認").into()
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum InitMode {
    Package,
    Descriptor,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SectionProfile {
    English,
    Japanese,
    Custom,
}

impl SectionProfile {
    fn label_preset(self) -> LabelPreset {
        match self {
            Self::Japanese => LabelPreset::Japanese,
            Self::English | Self::Custom => LabelPreset::English,
        }
    }

    fn is_japanese_ui(self) -> bool {
        matches!(self, Self::Japanese)
    }
}

#[derive(Clone, Copy)]
enum QualityField {
    TypeCheck,
    Lint,
    Fix,
    Test,
}

impl QualityField {
    fn all() -> [Self; 4] {
        [Self::TypeCheck, Self::Lint, Self::Fix, Self::Test]
    }

    fn key(self) -> &'static str {
        match self {
            Self::TypeCheck => "typecheck",
            Self::Lint => "lint",
            Self::Fix => "fix",
            Self::Test => "test",
        }
    }
}

struct QualitySlotState {
    detected: Option<String>,
    current: String,
    source: InitQualitySource,
}

impl QualitySlotState {
    fn new(detected: Option<String>, source: InitQualitySource) -> Self {
        let current = detected.clone().unwrap_or_default();
        Self {
            detected,
            current,
            source,
        }
    }

    fn is_override(&self) -> bool {
        self.current.trim() != self.detected.as_deref().unwrap_or_default().trim()
    }

    fn display_command(&self, japanese: bool) -> String {
        let trimmed = self.current.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        if self.detected.is_some() {
            localized(japanese, "disabled", "無効").to_string()
        } else {
            localized(japanese, "unresolved", "未解決").to_string()
        }
    }

    fn effective_command(&self) -> Option<String> {
        non_empty_command(&self.current)
    }
}

struct ToolchainProfile {
    lang: Lang,
    name: String,
}

struct ToolchainState {
    profile: ToolchainProfile,
    type_check: QualitySlotState,
    lint: QualitySlotState,
    fix: QualitySlotState,
    test: QualitySlotState,
}

impl ToolchainState {
    fn slot(&self, field: QualityField) -> &QualitySlotState {
        match field {
            QualityField::TypeCheck => &self.type_check,
            QualityField::Lint => &self.lint,
            QualityField::Fix => &self.fix,
            QualityField::Test => &self.test,
        }
    }

    fn slot_mut(&mut self, field: QualityField) -> &mut QualitySlotState {
        match field {
            QualityField::TypeCheck => &mut self.type_check,
            QualityField::Lint => &mut self.lint,
            QualityField::Fix => &mut self.fix,
            QualityField::Test => &mut self.test,
        }
    }
}

struct LabelField {
    canonical: &'static str,
    title: &'static str,
    value: String,
}

struct SelectItem {
    label: String,
    description: String,
    selected: bool,
}

struct FormRow {
    label: String,
    value: String,
    detail: String,
}

#[derive(Clone, Copy)]
struct QualityRow {
    toolchain: usize,
    field: QualityField,
}

struct WizardState {
    root: PathBuf,
    steps: Vec<Step>,
    current_step: usize,
    mode: InitMode,
    section_profile: SectionProfile,
    custom_labels: Vec<LabelField>,
    link_policy: LinkPolicy,
    toolchains: Vec<ToolchainState>,
    quality_advanced: bool,
    ai_enabled: bool,
    ai_targets: Vec<bool>,
    ai_categories: Vec<[bool; 3]>,
    confirmed: bool,
    descriptor_kind: DescriptorKind,
    descriptor_fields: DescriptorFields,
    list_state: ListState,
    cancelled: bool,
    message: Option<String>,
}

struct DescriptorFields {
    name: String,
    id: String,
    aliases: String,
    output: String,
    language_match_suffixes: String,
    language_fence_lang: String,
    language_primary_ext: String,
    language_root_modules: String,
    language_source_strip: String,
    language_source_prefix: String,
    language_source_suffix: String,
    language_source_extension: String,
    language_test_strip: String,
    language_test_prefix: String,
    language_test_suffix: String,
    language_test_extension: String,
    quality_typecheck: String,
    quality_lint: String,
    quality_fix: String,
    quality_test: String,
    tool_match_prefixes: String,
    tool_input: String,
    tool_output: String,
    tool_append_file_arg: String,
    package_display_name: String,
    package_lang: String,
    package_metadata_files: String,
    package_lockfiles: String,
    package_metadata_reader: String,
    package_install: String,
    package_build: String,
    package_typecheck: String,
    package_lint: String,
    package_test: String,
}

impl DescriptorFields {
    fn new() -> Self {
        Self {
            name: "TypeScript".to_string(),
            id: "ts".to_string(),
            aliases: "typescript".to_string(),
            output: String::new(),
            language_match_suffixes: "ts".to_string(),
            language_fence_lang: "ts".to_string(),
            language_primary_ext: "ts".to_string(),
            language_root_modules: "index.ts.md".to_string(),
            language_source_strip: "false".to_string(),
            language_source_prefix: String::new(),
            language_source_suffix: String::new(),
            language_source_extension: "ts".to_string(),
            language_test_strip: "true".to_string(),
            language_test_prefix: String::new(),
            language_test_suffix: ".test".to_string(),
            language_test_extension: "ts".to_string(),
            quality_typecheck: String::new(),
            quality_lint: String::new(),
            quality_fix: String::new(),
            quality_test: String::new(),
            tool_match_prefixes: "npm run lint".to_string(),
            tool_input: "stdin".to_string(),
            tool_output: "stdout".to_string(),
            tool_append_file_arg: "false".to_string(),
            package_display_name: "Node.js (npm)".to_string(),
            package_lang: "ts".to_string(),
            package_metadata_files: "package.json".to_string(),
            package_lockfiles: "package-lock.json".to_string(),
            package_metadata_reader: "node-package-json".to_string(),
            package_install: "npm install".to_string(),
            package_build: "npm run build".to_string(),
            package_typecheck: "npm run typecheck".to_string(),
            package_lint: "npm run lint".to_string(),
            package_test: "npm test".to_string(),
        }
    }
}

pub enum WizardOutcome {
    Init(InitOptions),
    InitDescriptor(InitDescriptorOptions),
}

impl WizardState {
    fn new(root: &Path) -> Self {
        let toolchains = detect_toolchains(root);
        let mut steps = vec![
            Step::InitMode,
            Step::Welcome,
            Step::SectionProfilePreset,
            Step::CustomSectionLabels,
            Step::LinkPolicy,
            Step::QualitySummary,
            Step::QualityAdvanced,
            Step::AiNeeded,
            Step::AiTargets,
        ];
        for target in 0..AiTarget::all().len() {
            steps.push(Step::AiCategories { target });
        }
        steps.push(Step::Confirm);
        steps.extend([
            Step::DescriptorKind,
            Step::DescriptorCommon,
            Step::DescriptorLanguage,
            Step::DescriptorLanguageQuality,
            Step::DescriptorTool,
            Step::DescriptorPackageManager,
            Step::DescriptorConfirm,
        ]);

        let mut state = Self {
            root: root.to_path_buf(),
            steps,
            current_step: 0,
            mode: InitMode::Package,
            section_profile: SectionProfile::English,
            custom_labels: default_label_fields(),
            link_policy: LinkPolicy::WikiOnly,
            toolchains,
            quality_advanced: false,
            ai_enabled: true,
            ai_targets: vec![true; AiTarget::all().len()],
            ai_categories: vec![[true; 3]; AiTarget::all().len()],
            confirmed: false,
            descriptor_kind: DescriptorKind::Language,
            descriptor_fields: DescriptorFields::new(),
            list_state: ListState::default(),
            cancelled: false,
            message: None,
        };
        state.list_state.select(Some(0));
        state
    }

    fn current_items(&self) -> Vec<SelectItem> {
        let japanese = self.is_japanese();
        match self.steps[self.current_step] {
            Step::InitMode => vec![
                SelectItem {
                    label: localized(japanese, "Set up package", "package を初期化").into(),
                    description: localized(
                        japanese,
                        "Create mds.config.toml, source/test roots, quality settings, and optional AI kit.",
                        "mds.config.toml、source/test root、quality 設定、任意の AI キットを作ります。",
                    )
                    .into(),
                    selected: self.mode == InitMode::Package,
                },
                SelectItem {
                    label: localized(japanese, "Create descriptor", "descriptor を作成").into(),
                    description: localized(
                        japanese,
                        "Create a package-local language, tool, or package-manager descriptor.",
                        "package-local な language / tool / package-manager descriptor を作ります。",
                    )
                    .into(),
                    selected: self.mode == InitMode::Descriptor,
                },
            ],
            Step::Welcome => vec![SelectItem {
                label: localized(japanese, "Start initialization", "初期化を開始").into(),
                description: localized(
                    japanese,
                    "Review the fixed contract, then choose the package policies that vary by project.",
                    "固定 contract を確認し、project ごとの差分になる policy だけ選びます。",
                )
                .into(),
                selected: true,
            }],
            Step::SectionProfilePreset => vec![
                SelectItem {
                    label: "English".into(),
                    description: localized(
                        japanese,
                        "Use English visible section titles such as Purpose and Source.",
                        "Purpose や Source などの英語見出しを使います。",
                    )
                    .into(),
                    selected: self.section_profile == SectionProfile::English,
                },
                SelectItem {
                    label: "Japanese".into(),
                    description: localized(
                        japanese,
                        "Use Japanese visible section titles such as 目的 and 実装.",
                        "目的 や 実装 などの日本語見出しを使います。",
                    )
                    .into(),
                    selected: self.section_profile == SectionProfile::Japanese,
                },
                SelectItem {
                    label: "Custom".into(),
                    description: localized(
                        japanese,
                        "Edit only the required semantic labels: Purpose, Contract, Source, Covers, Cases, and Test.",
                        "必須 semantic だけ編集します: Purpose, Contract, Source, Covers, Cases, Test。",
                    )
                    .into(),
                    selected: self.section_profile == SectionProfile::Custom,
                },
            ],
            Step::LinkPolicy => vec![
                SelectItem {
                    label: "wiki-only".into(),
                    description: localized(
                        japanese,
                        "Allow only wiki links such as [[module]].",
                        "[[module]] のような wiki link だけを許可します。",
                    )
                    .into(),
                    selected: self.link_policy == LinkPolicy::WikiOnly,
                },
                SelectItem {
                    label: "markdown-only".into(),
                    description: localized(
                        japanese,
                        "Allow only Markdown links such as [module](./module.md).",
                        "[module](./module.md) のような Markdown link だけを許可します。",
                    )
                    .into(),
                    selected: self.link_policy == LinkPolicy::MarkdownOnly,
                },
                SelectItem {
                    label: "mixed".into(),
                    description: localized(
                        japanese,
                        "Allow both wiki links and Markdown links.",
                        "wiki link と Markdown link の両方を許可します。",
                    )
                    .into(),
                    selected: self.link_policy == LinkPolicy::Mixed,
                },
            ],
            Step::QualitySummary => vec![
                SelectItem {
                    label: localized(japanese, "Use detected commands", "自動検出を使う").into(),
                    description: localized(
                        japanese,
                        "Keep the detected slot commands and continue.",
                        "検出済み slot command をそのまま使います。",
                    )
                    .into(),
                    selected: !self.quality_advanced,
                },
                SelectItem {
                    label: localized(japanese, "Edit slot commands", "slot command を編集").into(),
                    description: localized(
                        japanese,
                        "Open the advanced screen to override typecheck, lint, fix, and test commands.",
                        "typecheck, lint, fix, test command を詳細画面で上書きします。",
                    )
                    .into(),
                    selected: self.quality_advanced,
                },
            ],
            Step::AiNeeded => vec![
                SelectItem {
                    label: localized(japanese, "Add AI kit", "AI キットを追加").into(),
                    description: localized(
                        japanese,
                        "Generate AI instructions, skills, or commands after package init.",
                        "package init 後に AI instructions / skills / commands を生成します。",
                    )
                    .into(),
                    selected: self.ai_enabled,
                },
                SelectItem {
                    label: localized(japanese, "Skip AI kit", "AI キットを追加しない").into(),
                    description: localized(
                        japanese,
                        "Initialize only the package files and skip AI output.",
                        "package 初期化だけ行い、AI 出力は生成しません。",
                    )
                    .into(),
                    selected: !self.ai_enabled,
                },
            ],
            Step::AiTargets => AiTarget::all()
                .iter()
                .enumerate()
                .map(|(index, target)| SelectItem {
                    label: ai_target_label(*target),
                    description: ai_target_description(*target, japanese),
                    selected: self.ai_targets[index],
                })
                .collect(),
            Step::AiCategories { target } => AgentKitCategory::all()
                .iter()
                .enumerate()
                .map(|(index, category)| SelectItem {
                    label: category_label(*category, japanese).into(),
                    description: category_description(*category, japanese).into(),
                    selected: self.ai_categories[target][index],
                })
                .collect(),
            Step::Confirm => vec![
                SelectItem {
                    label: localized(japanese, "Apply initialization", "初期化を実行").into(),
                    description: localized(
                        japanese,
                        "Write the generated files using the current configuration.",
                        "現在の設定で生成ファイルを書き込みます。",
                    )
                    .into(),
                    selected: true,
                },
                SelectItem {
                    label: localized(japanese, "Cancel", "キャンセル").into(),
                    description: localized(
                        japanese,
                        "Exit without writing any files.",
                        "ファイルを書き込まず終了します。",
                    )
                    .into(),
                    selected: false,
                },
            ],
            Step::DescriptorKind => vec![
                SelectItem {
                    label: "language".into(),
                    description: localized(
                        japanese,
                        "Map Markdown files and code fences to generated source/test files.",
                        "Markdown file と code fence を生成 source/test file へ写像します。",
                    )
                    .into(),
                    selected: self.descriptor_kind == DescriptorKind::Language,
                },
                SelectItem {
                    label: "tool".into(),
                    description: localized(
                        japanese,
                        "Describe how a command runs and how mds recognizes it.",
                        "command の実行方法と mds の認識方法を定義します。",
                    )
                    .into(),
                    selected: self.descriptor_kind == DescriptorKind::Tool,
                },
                SelectItem {
                    label: "package-manager".into(),
                    description: localized(
                        japanese,
                        "Describe metadata files, lockfiles, and fallback commands.",
                        "metadata file、lockfile、fallback command を定義します。",
                    )
                    .into(),
                    selected: self.descriptor_kind == DescriptorKind::PackageManager,
                },
            ],
            Step::DescriptorConfirm => vec![
                SelectItem {
                    label: localized(japanese, "Create descriptor", "descriptor を作成").into(),
                    description: localized(
                        japanese,
                        "Write the descriptor file using the current answers.",
                        "現在の入力内容で descriptor file を書き込みます。",
                    )
                    .into(),
                    selected: true,
                },
                SelectItem {
                    label: localized(japanese, "Cancel", "キャンセル").into(),
                    description: localized(
                        japanese,
                        "Exit without writing any files.",
                        "ファイルを書き込まず終了します。",
                    )
                    .into(),
                    selected: false,
                },
            ],
            Step::CustomSectionLabels
            | Step::QualityAdvanced
            | Step::DescriptorCommon
            | Step::DescriptorLanguage
            | Step::DescriptorLanguageQuality
            | Step::DescriptorTool
            | Step::DescriptorPackageManager => Vec::new(),
        }
    }

    fn selection_intro_lines(&self) -> Vec<String> {
        let japanese = self.is_japanese();
        match self.steps[self.current_step] {
            Step::InitMode => vec![localized(
                japanese,
                "Choose whether this run initializes the package or creates one descriptor first. Descriptor setup stays package-local and does not copy language templates.",
                "package 初期化か descriptor 作成かを選びます。descriptor は package-local に作り、言語 template はコピーしません。",
            )
            .into()],
            Step::Welcome => {
                let mut lines = vec![
                    localized(japanese, "Fixed contract:", "固定 contract:").into(),
                    format!(
                        "- {} .mds/source  .mds/test",
                        localized(japanese, "Canonical roots:", "canonical root:")
                    ),
                    format!(
                        "- {}",
                        localized(
                            japanese,
                            "Source overview is always generated and required.",
                            "Source overview は常に生成し、必須です。",
                        )
                    ),
                    format!(
                        "- {}",
                        localized(
                            japanese,
                            "Section semantics stay canonical; only visible labels change by project profile.",
                            "section semantic は固定で、見出し表示名だけ project profile で変わります。",
                        )
                    ),
                    format!(
                        "- {}",
                        localized(
                            japanese,
                            "Link policy is selected next and written package-wide.",
                            "link policy を次で選び、package 単位で書き込みます。",
                        )
                    ),
                    format!(
                        "- {}",
                        localized(
                            japanese,
                            "Quality commands are detected per semantic slot.",
                            "quality command は semantic slot 単位で自動検出します。",
                        )
                    ),
                ];
                if self.toolchains.is_empty() {
                    lines.push(format!(
                        "- {}",
                        localized(
                            japanese,
                            "No recognized package manager metadata detected. Apply will fail until one exists.",
                            "認識済み package manager metadata がありません。作成されるまで apply は失敗します。",
                        )
                    ));
                }
                lines
            }
            Step::SectionProfilePreset => vec![localized(
                japanese,
                "Choose the visible section title profile. Semantic meaning stays canonical and templates still target the same required sections.",
                "表示見出し profile を選びます。semantic 自体は固定で、template も同じ必須 section を対象にします。",
            )
            .into()],
            Step::LinkPolicy => vec![localized(
                japanese,
                "Choose the package-wide link syntax policy used by lint and fix.",
                "lint と fix が使う package 単位の link syntax policy を選びます。",
            )
            .into()],
            Step::QualitySummary => {
                let mut lines = vec![localized(
                    japanese,
                    "Review the detected command and detection source for each slot. Diagnostics remap back to markdown files after tool execution.",
                    "slot ごとの検出コマンドと検出元を確認します。diagnostics は tool 実行後に markdown file へ remap されます。",
                )
                .into()];
                lines.push(localized(
                    japanese,
                    "Detection source labels show whether a command came from package manager, existing scripts, config, schema, or none.",
                    "検出元ラベルは command の由来が package manager / existing scripts / config / schema / none のどれかを示します。",
                )
                .into());
                lines.extend(self.quality_summary_lines());
                lines
            }
            Step::AiNeeded => vec![localized(
                japanese,
                "AI kit is optional and stays after the package init flow.",
                "AI キットは optional branch で、package init 本線の後ろに置きます。",
            )
            .into()],
            Step::AiTargets => vec![localized(
                japanese,
                "Select one or more AI CLIs to generate project guidance for.",
                "project guidance を生成する AI CLI を 1 つ以上選びます。",
            )
            .into()],
            Step::AiCategories { target } => {
                let target = ai_target_label(AiTarget::all()[target]);
                vec![if japanese {
                    format!("{target} 向けに生成するカテゴリを選びます。")
                } else {
                    format!("Choose which categories to generate for {target}.")
                }]
            }
            Step::Confirm => {
                let mut lines = vec![localized(japanese, "Files to write:", "書き込む file:").into()];
                for path in self.planned_files() {
                    lines.push(format!("- {path}"));
                }
                lines.push(String::new());
                lines.push(format!(
                    "{}: {}",
                    localized(japanese, "Section profile", "セクションプロファイル"),
                    self.section_profile_summary(),
                ));
                lines.push(format!(
                    "{}: {}",
                    localized(japanese, "Link policy", "リンクポリシー"),
                    self.link_policy.as_str(),
                ));
                lines.push(format!(
                    "{}: {}",
                    localized(japanese, "AI kit", "AI キット"),
                    self.ai_summary(),
                ));
                lines.push(localized(japanese, "Quality summary:", "品質サマリー:").into());
                for line in self.quality_summary_lines() {
                    lines.push(format!("  {line}"));
                }
                lines
            }
            Step::DescriptorKind => vec![localized(
                japanese,
                "Pick the descriptor type. The next screens only ask for fields needed by that type.",
                "descriptor 種別を選びます。次の画面ではその種別に必要な項目だけ入力します。",
            )
            .into()],
            Step::DescriptorConfirm => {
                let options = self.to_descriptor_options();
                let mut lines = vec![
                    localized(japanese, "Descriptor to write:", "書き込む descriptor:").into(),
                    format!("- kind: {}", options.kind.key()),
                    format!("- id: {}", descriptor_id_from_name(&options.name)),
                    format!("- path: {}", self.descriptor_output_display()),
                    String::new(),
                    localized(
                        japanese,
                        "After writing, run `mds descriptor check --package .` to validate collisions and capture rules.",
                        "書き込み後、`mds descriptor check --package .` で衝突と capture rule を検証します。",
                    )
                    .into(),
                ];
                match options.kind {
                    DescriptorKind::Language => {
                        let fields = &self.descriptor_fields;
                        lines.push(format!("- match suffixes: {}", fields.language_match_suffixes));
                        lines.push(format!("- fence label: {}", fields.language_fence_lang));
                        lines.push(format!("- source extension: {}", fields.language_source_extension));
                        lines.push(format!("- test suffix: {}", fields.language_test_suffix));
                    }
                    DescriptorKind::Tool => {
                        lines.push(format!(
                            "- match prefixes: {}",
                            self.descriptor_fields.tool_match_prefixes
                        ));
                        lines.push(format!(
                            "- behavior: input={}, output={}, append_file_arg={}",
                            self.descriptor_fields.tool_input,
                            self.descriptor_fields.tool_output,
                            self.descriptor_fields.tool_append_file_arg
                        ));
                    }
                    DescriptorKind::PackageManager => {
                        lines.push(format!(
                            "- metadata files: {}",
                            self.descriptor_fields.package_metadata_files
                        ));
                        lines.push(format!(
                            "- lockfiles: {}",
                            self.descriptor_fields.package_lockfiles
                        ));
                    }
                }
                lines
            }
            Step::CustomSectionLabels
            | Step::QualityAdvanced
            | Step::DescriptorCommon
            | Step::DescriptorLanguage
            | Step::DescriptorLanguageQuality
            | Step::DescriptorTool
            | Step::DescriptorPackageManager => Vec::new(),
        }
    }

    fn form_intro_lines(&self) -> Vec<String> {
        let japanese = self.is_japanese();
        match self.steps[self.current_step] {
            Step::CustomSectionLabels => vec![localized(
                japanese,
                "Edit only the required semantic labels. Empty values are not allowed.",
                "必須 semantic の表示名だけ編集します。空欄は不可です。",
            )
            .into()],
            Step::QualityAdvanced => vec![localized(
                japanese,
                "Override commands per semantic slot. Leave a value empty to disable that slot.",
                "semantic slot ごとに command を上書きします。空欄にすると slot を無効化します。",
            )
            .into()],
            Step::DescriptorCommon => vec![localized(
                japanese,
                "Name and id decide the descriptor file name and lookup key. Leave output path empty to use .mds/descriptors/<kind>/<id>.toml.",
                "name と id が descriptor file 名と lookup key になります。output path 空欄なら .mds/descriptors/<kind>/<id>.toml を使います。",
            )
            .into()],
            Step::DescriptorLanguage => vec![localized(
                japanese,
                "Use comma-separated lists. Defaults match common file naming: feature.ts.md -> feature.ts and feature.test.ts.md -> feature.test.ts.",
                "comma 区切りで入力します。既定は feature.ts.md -> feature.ts、feature.test.ts.md -> feature.test.ts の命名です。",
            )
            .into()],
            Step::DescriptorLanguageQuality => vec![localized(
                japanese,
                "Optional quality default commands. Leave a command empty when the package manager or config should provide it later.",
                "任意の quality default command。package manager や config 側で後から決める項目は空欄にします。",
            )
            .into()],
            Step::DescriptorTool => vec![localized(
                japanese,
                "Describe how mds recognizes this command. Diagnostics capture can be added later by editing the TOML.",
                "mds が command を認識する方法を定義します。diagnostics capture は後で TOML 編集で追加できます。",
            )
            .into()],
            Step::DescriptorPackageManager => vec![localized(
                japanese,
                "Describe package metadata and fallback commands. Commands are optional; empty values are omitted.",
                "package metadata と fallback command を定義します。command は任意で、空欄なら出力しません。",
            )
            .into()],
            _ => Vec::new(),
        }
    }

    fn form_rows(&self) -> Vec<FormRow> {
        let japanese = self.is_japanese();
        match self.steps[self.current_step] {
            Step::CustomSectionLabels => self
                .custom_labels
                .iter()
                .map(|field| FormRow {
                    label: field.title.to_string(),
                    value: field.value.clone(),
                    detail: if japanese {
                        format!("{} semantic の表示見出し", field.title)
                    } else {
                        format!("Visible heading for the {} semantic.", field.title)
                    },
                })
                .collect(),
            Step::QualityAdvanced => self
                .quality_rows()
                .into_iter()
                .map(|row| {
                    let toolchain = &self.toolchains[row.toolchain];
                    let slot = toolchain.slot(row.field);
                    let detected = slot
                        .detected
                        .as_deref()
                        .unwrap_or_else(|| localized(japanese, "unresolved", "未解決"));
                    let source = slot_source_detail(slot, &toolchain.profile, japanese);
                    FormRow {
                        label: format!("{} / {}", toolchain.profile.name, row.field.key()),
                        value: slot.current.clone(),
                        detail: if japanese {
                            format!("検出コマンド: {detected} [検出元: {source}]")
                        } else {
                            format!("Detected command: {detected} [detection source: {source}]")
                        },
                    }
                })
                .collect(),
            Step::DescriptorCommon => vec![
                self.descriptor_row(
                    "Name",
                    &self.descriptor_fields.name,
                    "Human-readable seed. Example: TypeScript or npm.",
                ),
                self.descriptor_row(
                    "ID",
                    &self.descriptor_fields.id,
                    "Stable lookup key. Example: ts, eslint, npm.",
                ),
                self.descriptor_row(
                    "Aliases",
                    &self.descriptor_fields.aliases,
                    "Comma-separated aliases. Example: typescript,nodejs.",
                ),
                self.descriptor_row(
                    "Output path",
                    &self.descriptor_fields.output,
                    "Optional package-local TOML path. Empty uses the default path.",
                ),
            ],
            Step::DescriptorLanguage => vec![
                self.descriptor_row(
                    "Match suffixes",
                    &self.descriptor_fields.language_match_suffixes,
                    "Comma-separated Markdown suffixes. Example: ts,tsx.",
                ),
                self.descriptor_row(
                    "Fence label",
                    &self.descriptor_fields.language_fence_lang,
                    "Code fence label to scaffold. Example: ts.",
                ),
                self.descriptor_row(
                    "Primary extension",
                    &self.descriptor_fields.language_primary_ext,
                    "Generated source file extension. Example: ts.",
                ),
                self.descriptor_row(
                    "Root module md names",
                    &self.descriptor_fields.language_root_modules,
                    "Comma-separated root module Markdown names.",
                ),
                self.descriptor_row(
                    "Source strip lang ext",
                    &self.descriptor_fields.language_source_strip,
                    "true/false. Remove .ts from source Markdown file name before output.",
                ),
                self.descriptor_row(
                    "Source prefix",
                    &self.descriptor_fields.language_source_prefix,
                    "Optional output file prefix.",
                ),
                self.descriptor_row(
                    "Source suffix",
                    &self.descriptor_fields.language_source_suffix,
                    "Optional output file suffix.",
                ),
                self.descriptor_row(
                    "Source extension",
                    &self.descriptor_fields.language_source_extension,
                    "Output extension for source docs.",
                ),
                self.descriptor_row(
                    "Test strip lang ext",
                    &self.descriptor_fields.language_test_strip,
                    "true/false. Remove .ts from test Markdown file name before output.",
                ),
                self.descriptor_row(
                    "Test prefix",
                    &self.descriptor_fields.language_test_prefix,
                    "Optional test output prefix.",
                ),
                self.descriptor_row(
                    "Test suffix",
                    &self.descriptor_fields.language_test_suffix,
                    "Optional test output suffix. Example: .test.",
                ),
                self.descriptor_row(
                    "Test extension",
                    &self.descriptor_fields.language_test_extension,
                    "Output extension for test docs.",
                ),
            ],
            Step::DescriptorLanguageQuality => vec![
                self.descriptor_row(
                    "typecheck",
                    &self.descriptor_fields.quality_typecheck,
                    "Optional default typecheck command.",
                ),
                self.descriptor_row(
                    "lint",
                    &self.descriptor_fields.quality_lint,
                    "Optional default lint command.",
                ),
                self.descriptor_row(
                    "fix",
                    &self.descriptor_fields.quality_fix,
                    "Optional default fix command.",
                ),
                self.descriptor_row(
                    "test",
                    &self.descriptor_fields.quality_test,
                    "Optional default test command.",
                ),
            ],
            Step::DescriptorTool => vec![
                self.descriptor_row(
                    "Match prefixes",
                    &self.descriptor_fields.tool_match_prefixes,
                    "Comma-separated command prefixes. Example: eslint,npm run lint.",
                ),
                self.descriptor_row(
                    "Input",
                    &self.descriptor_fields.tool_input,
                    "Tool input mode. Example: stdin.",
                ),
                self.descriptor_row(
                    "Output",
                    &self.descriptor_fields.tool_output,
                    "Tool output mode. Example: stdout.",
                ),
                self.descriptor_row(
                    "Append file arg",
                    &self.descriptor_fields.tool_append_file_arg,
                    "true/false. Add file path to the command.",
                ),
            ],
            Step::DescriptorPackageManager => vec![
                self.descriptor_row(
                    "Display name",
                    &self.descriptor_fields.package_display_name,
                    "Human-readable package manager name.",
                ),
                self.descriptor_row(
                    "Language id",
                    &self.descriptor_fields.package_lang,
                    "Language descriptor id this package manager usually drives.",
                ),
                self.descriptor_row(
                    "Metadata files",
                    &self.descriptor_fields.package_metadata_files,
                    "Comma-separated metadata files. Example: package.json.",
                ),
                self.descriptor_row(
                    "Lockfiles",
                    &self.descriptor_fields.package_lockfiles,
                    "Comma-separated lockfiles.",
                ),
                self.descriptor_row(
                    "Metadata reader",
                    &self.descriptor_fields.package_metadata_reader,
                    "Metadata reader id. Example: node-package-json.",
                ),
                self.descriptor_row(
                    "install",
                    &self.descriptor_fields.package_install,
                    "Optional install command.",
                ),
                self.descriptor_row(
                    "build",
                    &self.descriptor_fields.package_build,
                    "Optional build command.",
                ),
                self.descriptor_row(
                    "typecheck",
                    &self.descriptor_fields.package_typecheck,
                    "Optional typecheck command.",
                ),
                self.descriptor_row(
                    "lint",
                    &self.descriptor_fields.package_lint,
                    "Optional lint command.",
                ),
                self.descriptor_row(
                    "test",
                    &self.descriptor_fields.package_test,
                    "Optional test command.",
                ),
            ],
            _ => Vec::new(),
        }
    }

    fn descriptor_row(&self, label: &str, value: &str, detail: &str) -> FormRow {
        FormRow {
            label: label.to_string(),
            value: value.to_string(),
            detail: detail.to_string(),
        }
    }

    fn quality_rows(&self) -> Vec<QualityRow> {
        let mut rows = Vec::new();
        for toolchain in 0..self.toolchains.len() {
            for field in QualityField::all() {
                rows.push(QualityRow { toolchain, field });
            }
        }
        rows
    }

    fn quality_row(&self, index: usize) -> Option<QualityRow> {
        let fields = QualityField::all();
        let field_count = fields.len();
        let toolchain = index / field_count;
        if toolchain >= self.toolchains.len() {
            return None;
        }
        Some(QualityRow {
            toolchain,
            field: fields[index % field_count],
        })
    }

    fn current_form_value_mut(&mut self) -> Option<&mut String> {
        let index = self.list_state.selected().unwrap_or(0);
        match self.steps[self.current_step] {
            Step::CustomSectionLabels => self
                .custom_labels
                .get_mut(index)
                .map(|field| &mut field.value),
            Step::QualityAdvanced => {
                let row = self.quality_row(index)?;
                Some(&mut self.toolchains[row.toolchain].slot_mut(row.field).current)
            }
            Step::DescriptorCommon => match index {
                0 => Some(&mut self.descriptor_fields.name),
                1 => Some(&mut self.descriptor_fields.id),
                2 => Some(&mut self.descriptor_fields.aliases),
                3 => Some(&mut self.descriptor_fields.output),
                _ => None,
            },
            Step::DescriptorLanguage => match index {
                0 => Some(&mut self.descriptor_fields.language_match_suffixes),
                1 => Some(&mut self.descriptor_fields.language_fence_lang),
                2 => Some(&mut self.descriptor_fields.language_primary_ext),
                3 => Some(&mut self.descriptor_fields.language_root_modules),
                4 => Some(&mut self.descriptor_fields.language_source_strip),
                5 => Some(&mut self.descriptor_fields.language_source_prefix),
                6 => Some(&mut self.descriptor_fields.language_source_suffix),
                7 => Some(&mut self.descriptor_fields.language_source_extension),
                8 => Some(&mut self.descriptor_fields.language_test_strip),
                9 => Some(&mut self.descriptor_fields.language_test_prefix),
                10 => Some(&mut self.descriptor_fields.language_test_suffix),
                11 => Some(&mut self.descriptor_fields.language_test_extension),
                _ => None,
            },
            Step::DescriptorLanguageQuality => match index {
                0 => Some(&mut self.descriptor_fields.quality_typecheck),
                1 => Some(&mut self.descriptor_fields.quality_lint),
                2 => Some(&mut self.descriptor_fields.quality_fix),
                3 => Some(&mut self.descriptor_fields.quality_test),
                _ => None,
            },
            Step::DescriptorTool => match index {
                0 => Some(&mut self.descriptor_fields.tool_match_prefixes),
                1 => Some(&mut self.descriptor_fields.tool_input),
                2 => Some(&mut self.descriptor_fields.tool_output),
                3 => Some(&mut self.descriptor_fields.tool_append_file_arg),
                _ => None,
            },
            Step::DescriptorPackageManager => match index {
                0 => Some(&mut self.descriptor_fields.package_display_name),
                1 => Some(&mut self.descriptor_fields.package_lang),
                2 => Some(&mut self.descriptor_fields.package_metadata_files),
                3 => Some(&mut self.descriptor_fields.package_lockfiles),
                4 => Some(&mut self.descriptor_fields.package_metadata_reader),
                5 => Some(&mut self.descriptor_fields.package_install),
                6 => Some(&mut self.descriptor_fields.package_build),
                7 => Some(&mut self.descriptor_fields.package_typecheck),
                8 => Some(&mut self.descriptor_fields.package_lint),
                9 => Some(&mut self.descriptor_fields.package_test),
                _ => None,
            },
            _ => None,
        }
    }

    fn cursor_len(&self) -> usize {
        if self.is_text_form() {
            self.form_rows().len()
        } else {
            self.current_items().len()
        }
    }

    fn is_text_form(&self) -> bool {
        matches!(
            self.steps[self.current_step],
            Step::CustomSectionLabels
                | Step::QualityAdvanced
                | Step::DescriptorCommon
                | Step::DescriptorLanguage
                | Step::DescriptorLanguageQuality
                | Step::DescriptorTool
                | Step::DescriptorPackageManager
        )
    }

    fn is_japanese(&self) -> bool {
        self.section_profile.is_japanese_ui()
    }

    fn is_multi_select(&self) -> bool {
        matches!(
            self.steps[self.current_step],
            Step::AiTargets | Step::AiCategories { .. }
        )
    }

    fn step_is_visible(&self, step: Step) -> bool {
        match step {
            Step::Welcome
            | Step::SectionProfilePreset
            | Step::CustomSectionLabels
            | Step::LinkPolicy
            | Step::QualitySummary
            | Step::QualityAdvanced
            | Step::AiNeeded
            | Step::AiTargets
            | Step::AiCategories { .. }
            | Step::Confirm => {
                if self.mode != InitMode::Package {
                    return false;
                }
            }
            Step::DescriptorKind
            | Step::DescriptorCommon
            | Step::DescriptorLanguage
            | Step::DescriptorLanguageQuality
            | Step::DescriptorTool
            | Step::DescriptorPackageManager
            | Step::DescriptorConfirm => {
                if self.mode != InitMode::Descriptor {
                    return false;
                }
            }
            Step::InitMode => {}
        }
        match step {
            Step::CustomSectionLabels => self.section_profile == SectionProfile::Custom,
            Step::QualityAdvanced => self.quality_advanced,
            Step::AiTargets => self.ai_enabled,
            Step::AiCategories { target } => self.ai_enabled && self.ai_targets[target],
            Step::DescriptorLanguage | Step::DescriptorLanguageQuality => {
                self.descriptor_kind == DescriptorKind::Language
            }
            Step::DescriptorTool => self.descriptor_kind == DescriptorKind::Tool,
            Step::DescriptorPackageManager => {
                self.descriptor_kind == DescriptorKind::PackageManager
            }
            _ => true,
        }
    }

    fn should_skip_step(&self) -> bool {
        !self.step_is_visible(self.steps[self.current_step])
    }

    fn visible_step_indices(&self) -> Vec<usize> {
        self.steps
            .iter()
            .enumerate()
            .filter_map(|(index, step)| self.step_is_visible(*step).then_some(index))
            .collect()
    }

    fn apply_current_selection(&mut self) {
        let index = self.list_state.selected().unwrap_or(0);
        match self.steps[self.current_step] {
            Step::InitMode => {
                self.mode = if index == 1 {
                    InitMode::Descriptor
                } else {
                    InitMode::Package
                }
            }
            Step::Welcome => {}
            Step::SectionProfilePreset => {
                self.section_profile = match index {
                    1 => SectionProfile::Japanese,
                    2 => SectionProfile::Custom,
                    _ => SectionProfile::English,
                };
            }
            Step::CustomSectionLabels => {}
            Step::LinkPolicy => {
                self.link_policy = match index {
                    1 => LinkPolicy::MarkdownOnly,
                    2 => LinkPolicy::Mixed,
                    _ => LinkPolicy::WikiOnly,
                };
            }
            Step::QualitySummary => self.quality_advanced = index == 1,
            Step::QualityAdvanced => {}
            Step::AiNeeded => self.ai_enabled = index == 0,
            Step::AiTargets => {
                if let Some(selected) = self.ai_targets.get_mut(index) {
                    *selected = !*selected;
                }
            }
            Step::AiCategories { target } => {
                if index < AgentKitCategory::all().len() {
                    self.ai_categories[target][index] = !self.ai_categories[target][index];
                }
            }
            Step::Confirm => self.confirmed = index == 0,
            Step::DescriptorKind => {
                self.descriptor_kind = match index {
                    1 => DescriptorKind::Tool,
                    2 => DescriptorKind::PackageManager,
                    _ => DescriptorKind::Language,
                };
                self.apply_descriptor_kind_defaults();
            }
            Step::DescriptorConfirm => self.confirmed = index == 0,
            Step::DescriptorCommon
            | Step::DescriptorLanguage
            | Step::DescriptorLanguageQuality
            | Step::DescriptorTool
            | Step::DescriptorPackageManager => {}
        }
        self.message = None;
    }

    fn apply_descriptor_kind_defaults(&mut self) {
        match self.descriptor_kind {
            DescriptorKind::Language => {
                self.descriptor_fields.name = "TypeScript".to_string();
                self.descriptor_fields.id = "ts".to_string();
                self.descriptor_fields.aliases = "typescript".to_string();
            }
            DescriptorKind::Tool => {
                self.descriptor_fields.name = "ESLint".to_string();
                self.descriptor_fields.id = "eslint".to_string();
                self.descriptor_fields.aliases = String::new();
            }
            DescriptorKind::PackageManager => {
                self.descriptor_fields.name = "npm".to_string();
                self.descriptor_fields.id = "npm".to_string();
                self.descriptor_fields.aliases = "node,nodejs".to_string();
            }
        }
    }

    fn advance(&mut self) {
        loop {
            if self.current_step >= self.steps.len() - 1 {
                break;
            }
            self.current_step += 1;
            if !self.should_skip_step() {
                break;
            }
        }
        self.list_state.select(Some(0));
        self.message = None;
    }

    fn go_back(&mut self) {
        loop {
            if self.current_step == 0 {
                break;
            }
            self.current_step -= 1;
            if !self.should_skip_step() {
                break;
            }
        }
        self.list_state.select(Some(0));
        self.message = None;
    }

    fn is_final_step(&self) -> bool {
        self.visible_step_indices()
            .last()
            .map(|index| *index == self.current_step)
            .unwrap_or(false)
    }

    fn validate_current_step(&self) -> Result<(), String> {
        match self.steps[self.current_step] {
            Step::CustomSectionLabels => {
                for field in &self.custom_labels {
                    if field.value.trim().is_empty() {
                        return Err(format!("{} label cannot be empty", field.title));
                    }
                }
                Ok(())
            }
            Step::DescriptorCommon => {
                if self.descriptor_fields.name.trim().is_empty() {
                    return Err("descriptor name cannot be empty".to_string());
                }
                if descriptor_id_from_name(&self.descriptor_fields.id).is_empty() {
                    return Err("descriptor id must contain ASCII letters or numbers".to_string());
                }
                Ok(())
            }
            Step::DescriptorLanguage => {
                require_bool_text(
                    "Source strip lang ext",
                    &self.descriptor_fields.language_source_strip,
                )?;
                require_bool_text(
                    "Test strip lang ext",
                    &self.descriptor_fields.language_test_strip,
                )?;
                for (label, value) in [
                    (
                        "Match suffixes",
                        &self.descriptor_fields.language_match_suffixes,
                    ),
                    ("Fence label", &self.descriptor_fields.language_fence_lang),
                    (
                        "Primary extension",
                        &self.descriptor_fields.language_primary_ext,
                    ),
                    (
                        "Source extension",
                        &self.descriptor_fields.language_source_extension,
                    ),
                    (
                        "Test extension",
                        &self.descriptor_fields.language_test_extension,
                    ),
                ] {
                    if value.trim().is_empty() {
                        return Err(format!("{label} cannot be empty"));
                    }
                }
                Ok(())
            }
            Step::DescriptorTool => {
                require_bool_text(
                    "Append file arg",
                    &self.descriptor_fields.tool_append_file_arg,
                )?;
                if self.descriptor_fields.tool_match_prefixes.trim().is_empty() {
                    return Err("Match prefixes cannot be empty".to_string());
                }
                Ok(())
            }
            Step::DescriptorPackageManager => {
                if self
                    .descriptor_fields
                    .package_display_name
                    .trim()
                    .is_empty()
                {
                    return Err("Display name cannot be empty".to_string());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn section_profile_summary(&self) -> String {
        match self.section_profile {
            SectionProfile::English => "English".into(),
            SectionProfile::Japanese => "Japanese".into(),
            SectionProfile::Custom => {
                let labels = self
                    .custom_labels
                    .iter()
                    .map(|field| format!("{}={}", field.title, field.value.trim()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Custom ({labels})")
            }
        }
    }

    fn selected_ai_targets(&self) -> Vec<AiTarget> {
        if !self.ai_enabled {
            return Vec::new();
        }
        self.ai_targets
            .iter()
            .enumerate()
            .filter(|(_, selected)| **selected)
            .map(|(index, _)| AiTarget::all()[index])
            .collect()
    }

    fn ai_summary(&self) -> String {
        let japanese = self.is_japanese();
        if !self.ai_enabled {
            return localized(japanese, "not included", "追加しない").into();
        }

        let mut parts = Vec::new();
        for (index, target) in AiTarget::all().iter().enumerate() {
            if !self.ai_targets[index] {
                continue;
            }
            let categories = AgentKitCategory::all()
                .iter()
                .enumerate()
                .filter(|(category_index, _)| self.ai_categories[index][*category_index])
                .map(|(_, category)| category_label(*category, japanese).to_string())
                .collect::<Vec<_>>();
            parts.push(format!(
                "{} [{}]",
                ai_target_label(*target),
                categories.join(", ")
            ));
        }

        if parts.is_empty() {
            localized(japanese, "enabled, no targets", "有効だが対象なし").into()
        } else {
            parts.join("; ")
        }
    }

    fn quality_summary_lines(&self) -> Vec<String> {
        let japanese = self.is_japanese();
        if self.toolchains.is_empty() {
            return vec![format!(
                "- {}",
                localized(
                    japanese,
                    "unresolved: no recognized package manager metadata",
                    "未解決: 認識済み package manager metadata がありません",
                )
            )];
        }

        let mut lines = Vec::new();
        for toolchain in &self.toolchains {
            for field in QualityField::all() {
                let slot = toolchain.slot(field);
                let command = slot.display_command(japanese);
                let source = slot_source_detail(slot, &toolchain.profile, japanese);
                let override_state = if slot.is_override() {
                    localized(japanese, "yes", "あり")
                } else {
                    localized(japanese, "no", "なし")
                };
                lines.push(format!(
                    "- {} {}: {}: {} [{}: {}; {}: {}]",
                    toolchain.profile.name,
                    field.key(),
                    localized(japanese, "detected command", "検出コマンド"),
                    command,
                    localized(japanese, "detection source", "検出元"),
                    source,
                    localized(japanese, "override", "上書き"),
                    override_state,
                ));
            }
        }
        lines
    }

    fn planned_files(&self) -> Vec<String> {
        planned_init_write_paths(&self.root, &self.to_options())
            .into_iter()
            .map(|path| {
                path.strip_prefix(&self.root)
                    .unwrap_or(path.as_path())
                    .display()
                    .to_string()
            })
            .collect()
    }

    fn to_options(&self) -> InitOptions {
        let quality_commands = self
            .toolchains
            .iter()
            .map(|toolchain| InitQualityCommands {
                lang: toolchain.profile.lang.clone(),
                type_check: toolchain.type_check.effective_command(),
                lint: toolchain.lint.effective_command(),
                fix: toolchain.fix.effective_command(),
                test: toolchain.test.effective_command(),
            })
            .collect();

        let targets = self.selected_ai_targets();
        let target_categories = targets
            .iter()
            .map(|target| {
                let target_index = AiTarget::all()
                    .iter()
                    .position(|known| known == target)
                    .unwrap_or(0);
                InitTargetCategories {
                    target: *target,
                    categories: AgentKitCategory::all()
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| self.ai_categories[target_index][*index])
                        .map(|(_, category)| *category)
                        .collect(),
                }
            })
            .collect();

        let label_overrides = if self.section_profile == SectionProfile::Custom {
            self.custom_labels
                .iter()
                .map(|field| (field.canonical.to_string(), field.value.trim().to_string()))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };

        InitOptions {
            ai_only: false,
            yes: true,
            force: false,
            targets,
            categories: Vec::new(),
            install_project_deps: false,
            install_toolchains: false,
            install_ai_cli: false,
            label_preset: self.section_profile.label_preset(),
            link_policy: self.link_policy,
            label_overrides,
            quality_commands,
            target_categories,
        }
    }

    fn to_descriptor_options(&self) -> InitDescriptorOptions {
        let fields = &self.descriptor_fields;
        let aliases = split_csv(&fields.aliases);
        let output = non_empty_command(&fields.output);
        let name = if fields.id.trim().is_empty() {
            fields.name.trim().to_string()
        } else {
            fields.id.trim().to_string()
        };

        let language = (self.descriptor_kind == DescriptorKind::Language).then(|| {
            InitLanguageDescriptorOptions {
                match_suffixes: split_csv(&fields.language_match_suffixes),
                fence_lang: fields.language_fence_lang.trim().to_string(),
                primary_ext: fields.language_primary_ext.trim().to_string(),
                root_module_markdown_names: split_csv(&fields.language_root_modules),
                source_strip_lang_ext: parse_bool_text(&fields.language_source_strip)
                    .unwrap_or(false),
                source_prefix: fields.language_source_prefix.trim().to_string(),
                source_suffix: fields.language_source_suffix.trim().to_string(),
                source_extension: fields.language_source_extension.trim().to_string(),
                test_strip_lang_ext: parse_bool_text(&fields.language_test_strip).unwrap_or(true),
                test_prefix: fields.language_test_prefix.trim().to_string(),
                test_suffix: fields.language_test_suffix.trim().to_string(),
                test_extension: fields.language_test_extension.trim().to_string(),
                quality_typecheck: non_empty_command(&fields.quality_typecheck),
                quality_lint: non_empty_command(&fields.quality_lint),
                quality_fix: non_empty_command(&fields.quality_fix),
                quality_test: non_empty_command(&fields.quality_test),
            }
        });
        let tool =
            (self.descriptor_kind == DescriptorKind::Tool).then(|| InitToolDescriptorOptions {
                match_prefixes: split_csv(&fields.tool_match_prefixes),
                input: fields.tool_input.trim().to_string(),
                output: fields.tool_output.trim().to_string(),
                append_file_arg: parse_bool_text(&fields.tool_append_file_arg).unwrap_or(false),
            });
        let package_manager = (self.descriptor_kind == DescriptorKind::PackageManager).then(|| {
            InitPackageManagerDescriptorOptions {
                display_name: fields.package_display_name.trim().to_string(),
                lang: fields.package_lang.trim().to_string(),
                metadata_files: split_csv(&fields.package_metadata_files),
                lockfiles: split_csv(&fields.package_lockfiles),
                metadata_reader: fields.package_metadata_reader.trim().to_string(),
                install: non_empty_command(&fields.package_install),
                build: non_empty_command(&fields.package_build),
                typecheck: non_empty_command(&fields.package_typecheck),
                lint: non_empty_command(&fields.package_lint),
                test: non_empty_command(&fields.package_test),
            }
        });

        InitDescriptorOptions {
            kind: self.descriptor_kind,
            name,
            output,
            yes: true,
            force: false,
            aliases,
            language,
            tool,
            package_manager,
        }
    }

    fn descriptor_output_display(&self) -> String {
        if let Some(output) = non_empty_command(&self.descriptor_fields.output) {
            return output;
        }
        let id = descriptor_id_from_name(&self.descriptor_fields.id);
        let dir = match self.descriptor_kind {
            DescriptorKind::Language => "languages",
            DescriptorKind::Tool => "tools",
            DescriptorKind::PackageManager => "package-managers",
        };
        format!(".mds/descriptors/{dir}/{id}.toml")
    }
}

fn render(frame: &mut Frame, state: &mut WizardState) {
    let area = frame.size();
    let shell = centered_rect(92, 88, area);
    frame.render_widget(Clear, shell);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(12),
            Constraint::Length(4),
        ])
        .split(shell);

    let visible_steps = state.visible_step_indices();
    let current_visible = visible_steps
        .iter()
        .position(|index| *index == state.current_step)
        .unwrap_or(0);
    let progress_bar: String = visible_steps
        .iter()
        .enumerate()
        .map(|(index, _)| match index.cmp(&current_visible) {
            std::cmp::Ordering::Less => '*',
            std::cmp::Ordering::Equal => '>',
            std::cmp::Ordering::Greater => '.',
        })
        .collect();

    let japanese = state.is_japanese();
    let mut header_lines = vec![
        Line::from(vec![
            Span::styled(
                " mds init ",
                Style::default().fg(Color::Black).bg(Color::Cyan).bold(),
            ),
            Span::raw("  "),
            Span::styled(
                if japanese {
                    format!("ステップ {}/{}", current_visible + 1, visible_steps.len())
                } else {
                    format!("Step {} of {}", current_visible + 1, visible_steps.len())
                },
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                state.steps[state.current_step].title(state),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw("  "),
            Span::styled(progress_bar, Style::default().fg(Color::DarkGray)),
        ]),
    ];
    if let Some(message) = &state.message {
        header_lines.push(Line::from(Span::styled(
            message.clone(),
            Style::default().fg(Color::LightRed),
        )));
    }

    let header = Paragraph::new(header_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .padding(Padding::horizontal(2)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(header, chunks[0]);

    if state.is_text_form() {
        render_form(frame, state, chunks[1]);
    } else {
        render_selection(frame, state, chunks[1]);
    }

    let footer_text = if state.is_text_form() {
        localized(
            japanese,
            " Up/Down Move  Type Edit  Enter/Right Next  Left/Esc Back  q Quit ",
            " Up/Down 移動  入力 編集  Enter/Right 次へ  Left/Esc 戻る  q 終了 ",
        )
    } else if state.is_multi_select() {
        localized(
            japanese,
            " Up/Down Move  Enter/Space Toggle  Right Next  Left/Esc Back  q Quit ",
            " Up/Down 移動  Enter/Space 切替  Right 次へ  Left/Esc 戻る  q 終了 ",
        )
    } else {
        localized(
            japanese,
            " Up/Down Move  Enter Select+Next  Right Next  Left/Esc Back  q Quit ",
            " Up/Down 移動  Enter 選択+次へ  Right 次へ  Left/Esc 戻る  q 終了 ",
        )
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            localized(japanese, "Keys", "キー"),
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(footer_text, Style::default().fg(Color::Gray)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .padding(Padding::horizontal(2)),
    );
    frame.render_widget(footer, chunks[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn render_selection(frame: &mut Frame, state: &mut WizardState, area: Rect) {
    let inner = centered_rect(88, 92, area);
    frame.render_widget(Clear, inner);
    let japanese = state.is_japanese();
    let intro_lines = state.selection_intro_lines();
    let intro_height = if intro_lines.is_empty() {
        0
    } else {
        (intro_lines.len() as u16)
            .saturating_add(2)
            .min(inner.height.saturating_sub(6))
            .max(3)
    };

    let chunks = if intro_height == 0 {
        vec![inner]
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(intro_height), Constraint::Min(6)])
            .split(inner)
            .to_vec()
    };

    let list_area = if intro_height == 0 {
        chunks[0]
    } else {
        chunks[1]
    };
    if intro_height != 0 {
        let intro = Paragraph::new(intro_lines.join("\n"))
            .block(
                Block::default()
                    .title(Span::styled(
                        localized(japanese, " Summary ", " 要約 "),
                        Style::default().fg(Color::Cyan).bold(),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .padding(Padding::new(1, 1, 0, 0)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(intro, chunks[0]);
    }

    let items: Vec<ListItem> = state
        .current_items()
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let is_cursor = state.list_state.selected() == Some(index);
            let marker = if item.selected { "[x]" } else { "[ ]" };
            let base_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan).bold()
            } else if item.selected {
                Style::default().fg(Color::Green).bold()
            } else {
                Style::default().fg(Color::Gray)
            };
            let detail_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let cursor = if is_cursor { ">" } else { " " };
            ListItem::new(vec![
                Line::from(Span::styled(
                    format!("{cursor} {marker} {}", item.label),
                    base_style,
                )),
                Line::from(Span::styled(
                    if item.description.is_empty() {
                        String::new()
                    } else {
                        format!("    {}", item.description)
                    },
                    detail_style,
                )),
                Line::raw(""),
            ])
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                format!(" {} ", state.steps[state.current_step].title(state)),
                Style::default().fg(Color::Cyan).bold(),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::new(1, 1, 0, 0)),
    );
    frame.render_stateful_widget(list, list_area, &mut state.list_state);
}

fn render_form(frame: &mut Frame, state: &mut WizardState, area: Rect) {
    let inner = centered_rect(88, 92, area);
    frame.render_widget(Clear, inner);
    let japanese = state.is_japanese();
    let intro_lines = state.form_intro_lines();
    let rows = state.form_rows();
    let intro_height = (intro_lines.len() as u16)
        .saturating_add(2)
        .min(inner.height.saturating_sub(6))
        .max(3);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(intro_height), Constraint::Min(6)])
        .split(inner);

    let intro = Paragraph::new(intro_lines.join("\n"))
        .block(
            Block::default()
                .title(Span::styled(
                    localized(japanese, " Summary ", " 要約 "),
                    Style::default().fg(Color::Yellow).bold(),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(intro, chunks[0]);

    let items: Vec<ListItem> = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let is_cursor = state.list_state.selected() == Some(index);
            let label_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Yellow).bold()
            } else {
                Style::default().fg(Color::White).bold()
            };
            let value_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };
            let detail_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let cursor = if is_cursor { ">" } else { " " };
            let value = if row.value.is_empty() {
                localized(japanese, "<empty>", "<empty>").to_string()
            } else {
                row.value
            };
            ListItem::new(vec![
                Line::from(Span::styled(format!("{cursor} {}", row.label), label_style)),
                Line::from(Span::styled(format!("    {value}"), value_style)),
                Line::from(Span::styled(format!("    {}", row.detail), detail_style)),
                Line::raw(""),
            ])
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                format!(" {} ", state.steps[state.current_step].title(state)),
                Style::default().fg(Color::Yellow).bold(),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .padding(Padding::new(1, 1, 0, 0)),
    );
    frame.render_stateful_widget(list, chunks[1], &mut state.list_state);
}

fn default_label_fields() -> Vec<LabelField> {
    [
        ("purpose", "Purpose"),
        ("contract", "Contract"),
        ("source", "Source"),
        ("covers", "Covers"),
        ("cases", "Cases"),
        ("test", "Test"),
    ]
    .into_iter()
    .map(|(canonical, title)| LabelField {
        canonical,
        title,
        value: LabelPreset::English.section_label(canonical),
    })
    .collect()
}

fn slot_source_detail(
    slot: &QualitySlotState,
    _profile: &ToolchainProfile,
    japanese: bool,
) -> String {
    match slot.source {
        InitQualitySource::Config => localized(japanese, "config", "config").to_string(),
        InitQualitySource::ExistingScripts => {
            localized(japanese, "existing scripts", "existing scripts").to_string()
        }
        InitQualitySource::PackageManager => {
            localized(japanese, "package manager", "package manager").to_string()
        }
        InitQualitySource::Schema => localized(japanese, "schema", "schema").to_string(),
        InitQualitySource::None => localized(japanese, "none", "none").to_string(),
    }
}

pub fn run_interactive_init(cwd: &Path, package: Option<&Path>) -> Result<WizardOutcome, String> {
    run_interactive_init_with_mode(cwd, package, InitMode::Package)
}

pub fn run_interactive_init_descriptor(
    cwd: &Path,
    package: Option<&Path>,
) -> Result<WizardOutcome, String> {
    run_interactive_init_with_mode(cwd, package, InitMode::Descriptor)
}

fn run_interactive_init_with_mode(
    cwd: &Path,
    package: Option<&Path>,
    mode: InitMode,
) -> Result<WizardOutcome, String> {
    enable_raw_mode().map_err(|error| format!("failed to enable raw mode: {error}"))?;
    stdout()
        .execute(EnterAlternateScreen)
        .map_err(|error| format!("failed to enter alternate screen: {error}"))?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))
        .map_err(|error| format!("failed to create terminal: {error}"))?;

    let root = package.map_or_else(|| cwd.to_path_buf(), |path| cwd.join(path));
    let mut state = WizardState::new(&root);
    state.mode = mode;
    if mode == InitMode::Descriptor {
        state.current_step = state
            .steps
            .iter()
            .position(|step| matches!(step, Step::DescriptorKind))
            .unwrap_or(0);
    }
    let result = run_loop(&mut terminal, &mut state);

    disable_raw_mode().ok();
    stdout().execute(LeaveAlternateScreen).ok();

    match result {
        Ok(()) if state.cancelled => Err("init cancelled by user".to_string()),
        Ok(()) if !state.confirmed => Err("init cancelled by user".to_string()),
        Ok(()) => match state.mode {
            InitMode::Package => Ok(WizardOutcome::Init(state.to_options())),
            InitMode::Descriptor => {
                Ok(WizardOutcome::InitDescriptor(state.to_descriptor_options()))
            }
        },
        Err(error) => Err(error),
    }
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut WizardState,
) -> Result<(), String> {
    loop {
        terminal
            .draw(|frame| render(frame, state))
            .map_err(|error| format!("draw error: {error}"))?;

        if let Event::Key(key) = event::read().map_err(|error| format!("event error: {error}"))? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') if !state.is_text_form() => {
                    state.cancelled = true;
                    return Ok(());
                }
                KeyCode::Esc => {
                    if state.current_step == 0 {
                        state.cancelled = true;
                        return Ok(());
                    }
                    state.go_back();
                }
                KeyCode::Left => state.go_back(),
                KeyCode::Right => {
                    if state.is_text_form() {
                        match state.validate_current_step() {
                            Ok(()) => state.advance(),
                            Err(message) => state.message = Some(message),
                        }
                    } else if state.is_final_step() {
                        state.apply_current_selection();
                        return Ok(());
                    } else if state.is_multi_select() {
                        state.advance();
                    } else {
                        state.apply_current_selection();
                        state.advance();
                    }
                }
                KeyCode::Up => move_cursor(state, -1),
                KeyCode::Down => move_cursor(state, 1),
                KeyCode::Backspace if state.is_text_form() => {
                    if let Some(value) = state.current_form_value_mut() {
                        value.pop();
                    }
                    state.message = None;
                }
                KeyCode::Char(ch) if state.is_text_form() => {
                    if let Some(value) = state.current_form_value_mut() {
                        value.push(ch);
                    }
                    state.message = None;
                }
                KeyCode::Char(' ') if state.is_multi_select() => state.apply_current_selection(),
                KeyCode::Enter => {
                    if state.is_text_form() {
                        match state.validate_current_step() {
                            Ok(()) => state.advance(),
                            Err(message) => state.message = Some(message),
                        }
                    } else if state.is_multi_select() {
                        state.apply_current_selection();
                    } else if state.is_final_step() {
                        state.apply_current_selection();
                        return Ok(());
                    } else {
                        state.apply_current_selection();
                        state.advance();
                    }
                }
                _ => {}
            }
        }
    }
}

fn move_cursor(state: &mut WizardState, direction: isize) {
    let len = state.cursor_len();
    if len == 0 {
        return;
    }
    let current = state.list_state.selected().unwrap_or(0) as isize;
    let next = (current + direction).rem_euclid(len as isize) as usize;
    state.list_state.select(Some(next));
}

fn detect_toolchains(root: &Path) -> Vec<ToolchainState> {
    detect_init_quality_toolchains(root)
        .into_iter()
        .map(|toolchain| ToolchainState {
            profile: ToolchainProfile {
                lang: toolchain.lang,
                name: toolchain.display_name,
            },
            type_check: QualitySlotState::new(
                toolchain.type_check.command,
                toolchain.type_check.source,
            ),
            lint: QualitySlotState::new(toolchain.lint.command, toolchain.lint.source),
            fix: QualitySlotState::new(toolchain.fix.command, toolchain.fix.source),
            test: QualitySlotState::new(toolchain.test.command, toolchain.test.source),
        })
        .collect()
}

fn non_empty_command(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_bool_text(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" => Some(true),
        "false" | "no" | "n" | "0" => Some(false),
        _ => None,
    }
}

fn require_bool_text(label: &str, value: &str) -> Result<(), String> {
    parse_bool_text(value)
        .map(|_| ())
        .ok_or_else(|| format!("{label} must be true or false"))
}

fn descriptor_id_from_name(name: &str) -> String {
    let mut id = String::new();
    let mut previous_dash = false;
    for ch in name.trim().chars() {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '-' | '_' | ' ') {
            Some('-')
        } else {
            None
        };
        let Some(next) = next else {
            continue;
        };
        if next == '-' {
            if id.is_empty() || previous_dash {
                continue;
            }
            previous_dash = true;
            id.push(next);
        } else {
            previous_dash = false;
            id.push(next);
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    id
}

fn localized<'a>(japanese: bool, english: &'a str, japanese_text: &'a str) -> &'a str {
    if japanese {
        japanese_text
    } else {
        english
    }
}

fn ai_target_label(target: AiTarget) -> String {
    target
        .key()
        .split('-')
        .map(|part| {
            if part == "cli" {
                "CLI".to_string()
            } else {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn ai_target_description(target: AiTarget, japanese: bool) -> String {
    let label = ai_target_label(target);
    if japanese {
        format!("{label} 向けの project guidance を生成します。")
    } else {
        format!("Generate project guidance for {label}.")
    }
}

fn category_label(category: AgentKitCategory, japanese: bool) -> &'static str {
    match (category, japanese) {
        (AgentKitCategory::Instructions, false) => "Instructions",
        (AgentKitCategory::Skills, false) => "Skills",
        (AgentKitCategory::Commands, false) => "Commands",
        (AgentKitCategory::Instructions, true) => "Instructions",
        (AgentKitCategory::Skills, true) => "Skills",
        (AgentKitCategory::Commands, true) => "Commands",
    }
}

fn category_description(category: AgentKitCategory, japanese: bool) -> &'static str {
    match (category, japanese) {
        (AgentKitCategory::Instructions, false) => "Always-on project rules and coding guidance.",
        (AgentKitCategory::Skills, false) => "Reusable workflows for common project tasks.",
        (AgentKitCategory::Commands, false) => {
            "Executable prompts or commands for repeatable tasks."
        }
        (AgentKitCategory::Instructions, true) => {
            "常に参照する project rule と coding guidance です。"
        }
        (AgentKitCategory::Skills, true) => "共通 task で再利用する workflow です。",
        (AgentKitCategory::Commands, true) => {
            "繰り返し task 向けの executable prompt / command です。"
        }
    }
}
