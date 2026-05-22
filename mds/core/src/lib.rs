mod adapter;
pub mod config;
pub mod descriptor;
mod descriptor_authoring;
pub mod diagnostics;
mod diff;
mod doctor;
mod fs_utils;
mod generation;
mod hash;
mod init;
mod manifest;
pub mod markdown;
pub mod model;
mod new;
pub mod package;
mod package_sync;
mod quality;
mod runner;
pub mod table;

pub use diagnostics::{Diagnostic, RunState, Severity};
pub use generation::{plan_generation_with_source_map, GenerationPlan};
pub use init::{
    detect_init_quality_toolchains, planned_init_write_paths, InitQualitySlotSummary,
    InitQualitySource, InitQualityToolchainSummary,
};
pub use model::{
    AgentKitCategory, AiTarget, BuildMode, CliRequest, CliResult, CodeFenceBlock, Command, Config,
    DescriptorCommand, DescriptorKind, DescriptorSchemaFormat, DocKind, DoctorConfig, DoctorFormat,
    DoctorVersionFloor, GeneratedFile, GeneratedKind, ImplDoc, InitDescriptorOptions,
    InitLanguageDescriptorOptions, InitOptions, InitPackageManagerDescriptorOptions,
    InitQualityCommands, InitTargetCategories, InitToolDescriptorOptions, LabelPreset, Lang,
    LinkPolicy, NewOptions, OutputKind, Package, PackageMetadata, QualityConfig, Roots, SourceMap,
    SourceSpan,
};
pub use new::validate_new_options;
pub use package_sync::{validate_source_overview_required_sections, validate_source_overview_text};
pub use runner::execute;
