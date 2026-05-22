use crate::capabilities;
use crate::state::resolve_active_language;
use crate::state::resolve_authoring_doc;
use crate::state::with_descriptor_root_for_path;
use crate::state::OpenFile;
use crate::state::PackageState;
use crate::state::SharedState;
use crate::state::WorkspaceIndex;
use crate::state::WorkspaceState;
use mds_core::config::merge_config_file;
use mds_core::descriptor::{
    detect_package_manager, is_overview_markdown_path, markdown_module_id_for_lang,
    markdown_module_path_for_lang, with_workspace_descriptor_root,
};
use mds_core::diagnostics::RunState;
use mds_core::markdown::{
    is_authoring_doc_candidate_path, load_workspace_index_docs, parse_impl_doc_text,
    source_markdown_root, test_markdown_root,
};
use mds_core::package::discover_package_roots;
use mds_core::{DocKind, Lang, Package};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::Client;
use tower_lsp::LanguageServer;
use tracing::error;
use tracing::info;

const RESOLVE_GENERATED_POSITION_COMMAND: &str = "mds.resolveGeneratedPosition";
const REMAP_GENERATED_LOCATIONS_COMMAND: &str = "mds.remapGeneratedLocations";
const REMAP_GENERATED_RANGE_COMMAND: &str = "mds.remapGeneratedRange";
const REMAP_GENERATED_TEXT_EDITS_COMMAND: &str = "mds.remapGeneratedTextEdits";
const REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND: &str = "mds.remapGeneratedTextDocumentEdits";

#[derive(Debug, Deserialize)]
struct ResolveGeneratedPositionParams {
    markdown_uri: Url,
    position: Position,
}

#[derive(Debug, Deserialize)]
struct RemapGeneratedRangeParams {
    uri: Url,
    range: Range,
}

#[derive(Debug, Deserialize)]
struct RemapGeneratedLocationsParams {
    locations: Vec<Location>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BridgeTextDocumentEdits {
    uri: Url,
    edits: Vec<TextEdit>,
}

#[derive(Debug, Deserialize)]
struct RemapGeneratedTextDocumentEditsParams {
    documents: Vec<BridgeTextDocumentEdits>,
}

pub struct MdsLanguageServer {
    pub client: Client,
    pub state: SharedState,
}

fn bridge_commands() -> Vec<String> {
    vec![
        RESOLVE_GENERATED_POSITION_COMMAND.to_string(),
        REMAP_GENERATED_LOCATIONS_COMMAND.to_string(),
        REMAP_GENERATED_RANGE_COMMAND.to_string(),
        REMAP_GENERATED_TEXT_EDITS_COMMAND.to_string(),
        REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND.to_string(),
    ]
}

fn remap_generated_text_document_edits(
    state: &WorkspaceState,
    document: &BridgeTextDocumentEdits,
) -> Option<BridgeTextDocumentEdits> {
    state
        .remap_generated_text_edits(&document.uri, &document.edits)
        .map(|(uri, edits)| BridgeTextDocumentEdits { uri, edits })
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(true),
                })),
                ..Default::default()
            },
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![
                "#".to_string(),
                "|".to_string(),
                "`".to_string(),
                "[".to_string(),
            ]),
            resolve_provider: Some(false),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: bridge_commands(),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        ..Default::default()
    }
}

fn invalid_params(message: impl Into<String>) -> Error {
    Error::invalid_params(message.into())
}

fn parse_single_argument<T>(arguments: Vec<Value>) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut arguments = arguments.into_iter();
    let value = arguments
        .next()
        .ok_or_else(|| invalid_params("expected a single command argument"))?;
    if arguments.next().is_some() {
        return Err(invalid_params("expected exactly one command argument"));
    }
    serde_json::from_value(value)
        .map_err(|err| invalid_params(format!("failed to decode command arguments: {err}")))
}

fn execute_bridge_command(
    state: &WorkspaceState,
    params: ExecuteCommandParams,
) -> Result<Option<Value>> {
    let result = match params.command.as_str() {
        RESOLVE_GENERATED_POSITION_COMMAND => {
            let command =
                parse_single_argument::<ResolveGeneratedPositionParams>(params.arguments)?;
            serde_json::to_value(
                state.resolve_generated_position(&command.markdown_uri, command.position),
            )
        }
        REMAP_GENERATED_LOCATIONS_COMMAND => {
            let command = parse_single_argument::<RemapGeneratedLocationsParams>(params.arguments)?;
            serde_json::to_value(state.remap_generated_locations(&command.locations))
        }
        REMAP_GENERATED_RANGE_COMMAND => {
            let command = parse_single_argument::<RemapGeneratedRangeParams>(params.arguments)?;
            serde_json::to_value(state.remap_generated_range(&command.uri, command.range))
        }
        REMAP_GENERATED_TEXT_EDITS_COMMAND => {
            let command = parse_single_argument::<BridgeTextDocumentEdits>(params.arguments)?;
            serde_json::to_value(remap_generated_text_document_edits(state, &command))
        }
        REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND => {
            let command =
                parse_single_argument::<RemapGeneratedTextDocumentEditsParams>(params.arguments)?;
            let remapped = command
                .documents
                .iter()
                .map(|document| remap_generated_text_document_edits(state, document))
                .collect::<Vec<_>>();
            serde_json::to_value(remapped)
        }
        _ => {
            return Err(invalid_params(format!(
                "unsupported executeCommand `{}`",
                params.command
            )));
        }
    }
    .map_err(|err| invalid_params(format!("failed to encode command result: {err}")))?;

    Ok(Some(result))
}

fn is_workspace_authoring_doc_path(state: &WorkspaceState, path: &Path) -> bool {
    resolve_authoring_doc(path, state).is_some()
}

fn effective_package_for_path(state: &WorkspaceState, path: &Path) -> Option<Package> {
    let package_state = state.package_for_path(path)?;
    let config = state
        .effective_config_for_path(path)
        .unwrap_or_else(|| package_state.package.config.clone());
    Some(Package {
        root: package_state.package.root.clone(),
        config,
        package_manager_id: package_state.package.package_manager_id.clone(),
    })
}

fn is_workspace_authoring_doc_candidate_path(state: &WorkspaceState, path: &Path) -> bool {
    effective_package_for_path(state, path)
        .is_some_and(|package| is_authoring_doc_candidate_path(&package, path))
}

fn is_descriptor_refresh_path(path: &Path) -> bool {
    let mut components = path.components();
    while let Some(component) = components.next() {
        if component.as_os_str() != ".mds" {
            continue;
        }

        if matches!(components.next(), Some(next) if next.as_os_str() == "descriptors") {
            return true;
        }
    }

    false
}

fn path_requires_workspace_refresh(state: &WorkspaceState, path: &Path) -> bool {
    if matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("mds.config.toml")
    ) {
        return true;
    }

    if is_descriptor_refresh_path(path) {
        return true;
    }

    is_workspace_authoring_doc_path(state, path)
        || is_workspace_authoring_doc_candidate_path(state, path)
}

fn path_requires_workspace_refresh_on_change(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("mds.config.toml")
    )
}

fn path_requires_package_refresh_on_change(state: &WorkspaceState, path: &Path) -> bool {
    is_workspace_authoring_doc_path(state, path)
        || is_workspace_authoring_doc_candidate_path(state, path)
}

fn watched_changes_require_workspace_refresh(
    state: &WorkspaceState,
    changes: &[FileEvent],
) -> bool {
    changes.iter().any(|change| {
        change
            .uri
            .to_file_path()
            .ok()
            .is_some_and(|path| path_requires_workspace_refresh(state, &path))
    })
}

fn open_documents_for_validation(state: &WorkspaceState) -> Vec<(Url, String)> {
    let mut documents = state
        .open_files
        .values()
        .filter_map(|file| {
            Url::from_file_path(&file.path)
                .ok()
                .map(|uri| (uri, file.text.clone()))
        })
        .collect::<Vec<_>>();
    documents.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
    documents
}

fn open_documents_for_validation_in_package(
    state: &WorkspaceState,
    package_root: &Path,
) -> Vec<(Url, String)> {
    let mut documents = state
        .open_files
        .values()
        .filter(|file| file.path.starts_with(package_root))
        .filter_map(|file| {
            Url::from_file_path(&file.path)
                .ok()
                .map(|uri| (uri, file.text.clone()))
        })
        .collect::<Vec<_>>();
    documents.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
    documents
}

fn is_primary_workspace_index_doc(doc: &mds_core::ImplDoc) -> bool {
    !is_overview_markdown_path(&doc.path)
}

fn authoring_doc_kind(package: &mds_core::Package, path: &Path) -> Option<DocKind> {
    if path.starts_with(source_markdown_root(package).as_path()) {
        Some(DocKind::Source)
    } else if path.starts_with(test_markdown_root(package).as_path()) {
        Some(DocKind::Test)
    } else {
        None
    }
}

fn overlay_open_authoring_docs(
    package: &mds_core::Package,
    state: &WorkspaceState,
    docs: Vec<mds_core::ImplDoc>,
) -> Vec<mds_core::ImplDoc> {
    let mut run_state = RunState::default();
    let mut docs_by_path = docs
        .into_iter()
        .map(|doc| (doc.path.clone(), doc))
        .collect::<HashMap<_, _>>();

    for file in state
        .open_files
        .values()
        .filter(|file| file.path.starts_with(&package.root))
    {
        let Some(doc_kind) = authoring_doc_kind(package, &file.path) else {
            continue;
        };

        let is_discoverable = resolve_authoring_doc(&file.path, state).is_some();
        if !is_discoverable {
            docs_by_path.remove(&file.path);
            continue;
        }

        let lang = if is_overview_markdown_path(&file.path) {
            Lang::Other("md".to_string())
        } else {
            let Some(lang) = resolve_active_language(&file.path, Some(state)) else {
                docs_by_path.remove(&file.path);
                continue;
            };
            lang
        };

        match parse_impl_doc_text(
            package,
            doc_kind,
            lang,
            &file.path,
            &file.text,
            &mut run_state,
        ) {
            Some(doc) => {
                docs_by_path.insert(file.path.clone(), doc);
            }
            None => {
                docs_by_path.remove(&file.path);
            }
        }
    }

    let mut docs = docs_by_path.into_values().collect::<Vec<_>>();
    docs.sort_by(|left, right| left.path.cmp(&right.path));
    docs
}

fn load_disk_package_config(
    workspace_root: &Path,
    package_root: &Path,
) -> Option<mds_core::Config> {
    let mut config = mds_core::Config::default();
    let mut run_state = RunState::default();
    let workspace_config_path = workspace_root.join("mds.config.toml");
    if workspace_config_path.exists()
        && merge_config_file(&mut config, &workspace_config_path, &mut run_state).is_none()
    {
        return None;
    }

    let package_config_path = package_root.join("mds.config.toml");
    if package_config_path != workspace_config_path
        && merge_config_file(&mut config, &package_config_path, &mut run_state).is_none()
    {
        return None;
    }

    Some(config)
}

fn rebuild_workspace_packages(state: &WorkspaceState) -> Vec<PackageState> {
    let mut effective_packages = Vec::new();

    for folder in &state.workspace_folders {
        match discover_package_roots(folder, None) {
            Ok(package_roots) => {
                for package_root in package_roots {
                    let Some(disk_config) = load_disk_package_config(folder, &package_root) else {
                        error!(
                            "failed to load package config for {}",
                            package_root.display()
                        );
                        continue;
                    };
                    let package = mds_core::Package {
                        root: package_root.clone(),
                        config: disk_config.clone(),
                        package_manager_id: String::new(),
                    };
                    let effective_config = state
                        .effective_config_for_package(&package)
                        .unwrap_or(disk_config);
                    if !effective_config.enabled {
                        continue;
                    }

                    let Some(package_manager) =
                        with_descriptor_root_for_path(Some(&package_root), Some(state), || {
                            detect_package_manager(&package_root)
                        })
                    else {
                        error!(
                            "enabled package requires a recognized package manager metadata file: {}",
                            package_root.display()
                        );
                        continue;
                    };

                    effective_packages.push(mds_core::Package {
                        root: package_root,
                        config: effective_config,
                        package_manager_id: package_manager.id,
                    });
                }
            }
            Err(error) => {
                error!(
                    "failed to discover packages in {}: {}",
                    folder.display(),
                    error
                );
            }
        }
    }

    let mut overlay_state = state.clone();
    overlay_state.packages = effective_packages
        .iter()
        .cloned()
        .map(|package| PackageState {
            package,
            index: WorkspaceIndex::default(),
        })
        .collect();

    effective_packages
        .into_iter()
        .map(|package| PackageState {
            index: build_workspace_index_with_state(&package, &overlay_state),
            package,
        })
        .collect()
}

fn rebuild_package_index(state: &WorkspaceState, package_root: &Path) -> Option<WorkspaceIndex> {
    let package_state = state
        .packages
        .iter()
        .find(|package| package.package.root == package_root)?;
    Some(build_workspace_index_with_state(
        &package_state.package,
        state,
    ))
}

fn index_doc_text(doc: &mds_core::ImplDoc) -> &str {
    doc.normalized_input
        .split_once('\n')
        .map(|(_, text)| text)
        .unwrap_or_default()
}

impl MdsLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Default::default(),
        }
    }
}

impl MdsLanguageServer {
    pub async fn reindex_workspace(&self) {
        let snapshot = {
            let state = self.state.read().await;
            WorkspaceState {
                workspace_folders: state.workspace_folders.clone(),
                open_files: state.open_files.clone(),
                packages: Vec::new(),
                configs: state.configs.clone(),
            }
        };
        let packages = rebuild_workspace_packages(&snapshot);

        let mut state = self.state.write().await;
        state.packages = packages;
        info!("indexed {} packages", state.packages.len());
    }

    async fn refresh_workspace(&self) {
        self.reindex_workspace().await;
        self.revalidate_open_documents().await;
    }

    async fn refresh_package_for_path(&self, path: &Path) {
        let (package_root, open_documents, index) = {
            let state = self.state.read().await;
            let package_root = state
                .package_for_path(path)
                .map(|package| package.package.root.clone());
            let Some(package_root) = package_root else {
                return;
            };
            let open_documents = open_documents_for_validation_in_package(&state, &package_root);
            let index = rebuild_package_index(&state, &package_root);
            (package_root, open_documents, index)
        };

        if let Some(index) = index {
            let mut state = self.state.write().await;
            if let Some(package) = state
                .packages
                .iter_mut()
                .find(|package| package.package.root == package_root)
            {
                package.index = index;
            }
        }

        for (uri, text) in open_documents {
            self.validate_document(&uri, &text).await;
        }
    }

    async fn revalidate_open_documents(&self) {
        let open_documents = {
            let state = self.state.read().await;
            open_documents_for_validation(&state)
        };

        for (uri, text) in open_documents {
            self.validate_document(&uri, &text).await;
        }
    }
}

impl MdsLanguageServer {
    pub async fn validate_document(&self, uri: &Url, text: &str) {
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        let state = self.state.read().await;
        let authoring_doc = resolve_authoring_doc(&path, &state);

        let diagnostics = if path.ends_with("mds.config.toml") {
            capabilities::diagnostics::validate_config_text(&path, text)
        } else if let Some(authoring_doc) = authoring_doc {
            capabilities::diagnostics::validate_impl_md_text_with_state(
                &path,
                text,
                &authoring_doc.config,
                Some(&state),
            )
        } else {
            vec![]
        };

        drop(state);

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}

fn build_workspace_index_from_docs(
    package: &mds_core::Package,
    docs_vec: Vec<mds_core::ImplDoc>,
) -> WorkspaceIndex {
    with_workspace_descriptor_root(Some(&package.root), || {
        let mut run_state = RunState::default();
        let generation_plan =
            mds_core::plan_generation_with_source_map(package, &docs_vec, &mut run_state);

        let mut docs = HashMap::new();
        let mut expose_index: HashMap<String, Vec<PathBuf>> = HashMap::new();
        let mut file_exposes: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut module_index: HashMap<String, Vec<PathBuf>> = HashMap::new();
        let mut symbol_index: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();
        let generated_files: HashSet<PathBuf> = generation_plan
            .generated
            .into_iter()
            .map(|file| file.path)
            .collect();

        for doc in docs_vec {
            let path = doc.path.clone();
            if is_primary_workspace_index_doc(&doc) {
                let module_path =
                    markdown_module_path_for_lang(&doc.lang, &doc.markdown_relative_path);
                let module_id = markdown_module_id_for_lang(&doc.lang, &doc.markdown_relative_path);
                let mut module_keys = vec![module_path.clone()];
                if module_id != module_path {
                    module_keys.push(module_id.clone());
                }

                for module_key in module_keys {
                    module_index
                        .entry(module_key.clone())
                        .or_default()
                        .push(path.clone());
                    expose_index
                        .entry(module_key.clone())
                        .or_default()
                        .push(path.clone());
                    file_exposes
                        .entry(path.clone())
                        .or_default()
                        .push(module_key);
                }

                let exported_names = exported_names_from_text(index_doc_text(&doc));
                for exported in &exported_names {
                    symbol_index
                        .entry((module_id.clone(), exported.clone()))
                        .or_default()
                        .push(path.clone());
                }
                for exposed in exported_names {
                    expose_index
                        .entry(exposed.clone())
                        .or_default()
                        .push(path.clone());
                    file_exposes.entry(path.clone()).or_default().push(exposed);
                }
            }

            docs.insert(path, doc);
        }

        WorkspaceIndex {
            docs,
            expose_index,
            file_exposes,
            module_index,
            symbol_index,
            source_map: generation_plan.source_map,
            generated_files,
        }
    })
}

fn build_workspace_index_with_state(
    package: &mds_core::Package,
    state: &WorkspaceState,
) -> WorkspaceIndex {
    let mut run_state = RunState::default();
    let docs = load_workspace_index_docs(package, &mut run_state).unwrap_or_default();
    let docs = overlay_open_authoring_docs(package, state, docs);
    build_workspace_index_from_docs(package, docs)
}

#[cfg(test)]
fn build_workspace_index(package: &mds_core::Package) -> WorkspaceIndex {
    let mut run_state = RunState::default();
    let docs = load_workspace_index_docs(package, &mut run_state).unwrap_or_default();
    build_workspace_index_from_docs(package, docs)
}

fn exported_names_from_text(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_exports = false;

    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            let title = title.trim();
            in_exports = matches!(title, "Exports" | "Expose" | "Exposes" | "公開" | "公開面");
            continue;
        }

        if let Some(title) = line.strip_prefix("##### ") {
            let name = title.trim();
            if !name.is_empty() {
                names.push(name.to_string());
            }
            continue;
        }

        if in_exports && line.trim_start().starts_with('|') {
            let cells: Vec<&str> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            if let Some(name) = cells.first() {
                if !name.is_empty()
                    && *name != "Name"
                    && *name != "名前"
                    && !name.chars().all(|c| c == '-')
                {
                    names.push((*name).to_string());
                }
            }
        }
    }

    names.sort();
    names.dedup();
    names
}

#[tower_lsp::async_trait]

impl LanguageServer for MdsLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!("mds-lsp initializing");

        if let Some(folders) = params.workspace_folders {
            let mut state = self.state.write().await;
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    state.workspace_folders.push(path);
                }
            }
        } else if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                let mut state = self.state.write().await;
                state.workspace_folders.push(path);
            }
        }

        Ok(InitializeResult {
            capabilities: server_capabilities(),
            server_info: Some(ServerInfo {
                name: "mds-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("mds-lsp initialized");
        self.reindex_workspace().await;
    }

    async fn shutdown(&self) -> Result<()> {
        info!("mds-lsp shutting down");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        let version = params.text_document.version;
        let mut opened_path = None;

        if let Ok(path) = uri.to_file_path() {
            let lang = resolve_active_language(&path, None);
            let mut state = self.state.write().await;
            state.open_files.insert(
                uri.to_string(),
                OpenFile {
                    uri: uri.to_string(),
                    path: path.clone(),
                    text: text.clone(),
                    version,
                    lang,
                },
            );
            opened_path = Some(path);
        }

        if let Some(path) = opened_path {
            let refresh_kind = {
                let state = self.state.read().await;
                if path_requires_workspace_refresh_on_change(&path) {
                    2u8
                } else if path_requires_package_refresh_on_change(&state, &path) {
                    1u8
                } else {
                    0u8
                }
            };

            match refresh_kind {
                2 => self.refresh_workspace().await,
                1 => self.refresh_package_for_path(&path).await,
                _ => self.validate_document(&uri, &text).await,
            }
        } else {
            self.validate_document(&uri, &text).await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().next_back() {
            let text = change.text.clone();
            let changed_path = uri.to_file_path().ok();
            {
                let mut state = self.state.write().await;
                if let Some(file) = state.open_files.get_mut(&uri.to_string()) {
                    file.text = text.clone();
                    file.version = params.text_document.version;
                }
            }
            let refresh_kind = if let Some(path) = changed_path.as_deref() {
                let state = self.state.read().await;
                if path_requires_workspace_refresh_on_change(path) {
                    2u8
                } else if path_requires_package_refresh_on_change(&state, path) {
                    1u8
                } else {
                    0u8
                }
            } else {
                0u8
            };

            match refresh_kind {
                2 => self.refresh_workspace().await,
                1 => {
                    if let Some(path) = changed_path.as_deref() {
                        self.refresh_package_for_path(path).await;
                    } else {
                        self.validate_document(&uri, &text).await;
                    }
                }
                _ => self.validate_document(&uri, &text).await,
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Ok(path) = uri.to_file_path() {
            // Use provided text or fall back to reading from disk
            let text = if let Some(text) = params.text {
                text
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(t) => t,
                    Err(_) => return,
                }
            };

            let needs_refresh = {
                let state = self.state.read().await;
                path_requires_workspace_refresh(&state, &path)
            };

            if needs_refresh {
                self.refresh_workspace().await;
            } else {
                self.validate_document(&uri, &text).await;
            }
        }
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let should_refresh = {
            let state = self.state.read().await;
            watched_changes_require_workspace_refresh(&state, &params.changes)
        };

        if should_refresh {
            self.refresh_workspace().await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let (closed_path, refresh_kind) = {
            let state = self.state.read().await;
            let closed_path = uri.to_file_path().ok();
            let refresh_kind = closed_path
                .as_deref()
                .map(|path| {
                    if path_requires_workspace_refresh_on_change(path) {
                        2u8
                    } else if path_requires_package_refresh_on_change(&state, path) {
                        1u8
                    } else {
                        0u8
                    }
                })
                .unwrap_or(0u8);
            (closed_path, refresh_kind)
        };

        let mut state = self.state.write().await;
        state.open_files.remove(&uri.to_string());

        // Clear diagnostics for closed file
        drop(state);
        self.client.publish_diagnostics(uri, vec![], None).await;

        match (refresh_kind, closed_path.as_deref()) {
            (2, _) => self.refresh_workspace().await,
            (1, Some(path)) => self.refresh_package_for_path(path).await,
            _ => {}
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let state = self.state.read().await;
        let text = state
            .open_files
            .get(&uri.to_string())
            .map(|f| f.text.clone());
        let path = uri.to_file_path().ok();
        let config = path
            .as_ref()
            .and_then(|p| state.package_for_path(p))
            .map(|p| p.package.config.clone())
            .unwrap_or_default();
        if let Some(text) = text {
            let items = capabilities::completion::provide_completions(
                &text,
                position,
                path.as_deref(),
                &config,
                Some(&state),
            );
            Ok(Some(CompletionResponse::Array(items)))
        } else {
            Ok(None)
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let state = self.state.read().await;
        let text = state
            .open_files
            .get(&uri.to_string())
            .map(|f| f.text.clone());
        let path = uri.to_file_path().ok();
        drop(state);

        if let (Some(text), Some(path)) = (text, path) {
            let state = self.state.read().await;
            let hover = capabilities::hover::provide_hover(&text, position, &path, &state);
            Ok(hover)
        } else {
            Ok(None)
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let state = self.state.read().await;
        let text = state
            .open_files
            .get(&uri.to_string())
            .map(|f| f.text.clone());
        let path = uri.to_file_path().ok();
        drop(state);

        if let (Some(text), Some(path)) = (text, path) {
            let state = self.state.read().await;
            let result = capabilities::navigation::goto_definition(&text, position, &path, &state);
            Ok(result)
        } else {
            Ok(None)
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let state = self.state.read().await;
        let text = state
            .open_files
            .get(&uri.to_string())
            .map(|f| f.text.clone());
        let path = uri.to_file_path().ok();
        drop(state);

        if let (Some(text), Some(path)) = (text, path) {
            let state = self.state.read().await;
            let result = capabilities::navigation::find_references(&text, position, &path, &state);
            Ok(result)
        } else {
            Ok(None)
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let state = self.state.read().await;
        let text = state
            .open_files
            .get(&uri.to_string())
            .map(|f| f.text.clone());
        drop(state);

        if let Some(text) = text {
            let symbols = capabilities::symbols::document_symbols(&text);
            Ok(Some(DocumentSymbolResponse::Flat(symbols)))
        } else {
            Ok(None)
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let state = self.state.read().await;
        let symbols = capabilities::symbols::workspace_symbols(&query, &state);
        Ok(Some(symbols))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let state = self.state.read().await;
        let text = state
            .open_files
            .get(&uri.to_string())
            .map(|f| f.text.clone());
        let path = uri.to_file_path().ok();
        let config = path
            .as_ref()
            .and_then(|p| state.effective_config_for_path(p))
            .unwrap_or_default();

        if let Some(text) = text {
            let actions = capabilities::code_action::provide_code_actions_with_state(
                &uri,
                &text,
                &config,
                Some(&state),
            );
            Ok(Some(actions))
        } else {
            Ok(None)
        }
    }

    async fn formatting(&self, _params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        Ok(None)
    }

    async fn rename(&self, _params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        let state = self.state.read().await;
        execute_bridge_command(&state, params)
    }
}

#[cfg(test)]
#[path = "../../test_support/fixture_copy.rs"]
mod fixture_copy;

#[cfg(test)]
mod tests {
    use super::build_workspace_index;
    use super::build_workspace_index_with_state;
    use super::execute_bridge_command;
    use super::fixture_copy::copy_example_package_to;
    use super::is_workspace_authoring_doc_path;
    use super::open_documents_for_validation;
    use super::open_documents_for_validation_in_package;
    use super::path_requires_package_refresh_on_change;
    use super::path_requires_workspace_refresh;
    use super::path_requires_workspace_refresh_on_change;
    use super::rebuild_package_index;
    use super::rebuild_workspace_packages;
    use super::server_capabilities;
    use super::watched_changes_require_workspace_refresh;
    use super::BridgeTextDocumentEdits;
    use super::REMAP_GENERATED_LOCATIONS_COMMAND;
    use super::REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND;
    use super::REMAP_GENERATED_TEXT_EDITS_COMMAND;
    use crate::state::{
        resolve_active_language, resolve_authoring_doc, OpenFile, PackageState, WorkspaceIndex,
        WorkspaceState,
    };
    use mds_core::{Config, Lang, Package};
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;
    use tower_lsp::lsp_types::{
        CodeActionProviderCapability, ExecuteCommandParams, FileChangeType, FileEvent,
        GotoDefinitionResponse, Location, OneOf, Position, Range, TextEdit, Url,
    };

    struct BridgeFixture {
        _temp: TempDir,
        package_state: PackageState,
        markdown_path: PathBuf,
        source_generated_path: PathBuf,
    }

    struct OverviewIndexFixture {
        _temp: TempDir,
        package: Package,
        index: WorkspaceIndex,
        source_path: PathBuf,
        source_overview_path: PathBuf,
        nested_overview_path: PathBuf,
        test_overview_path: PathBuf,
    }

    const TS_DESCRIPTOR: &str =
        include_str!("../../../examples/minimal-ts/.mds/descriptors/languages/ts.toml");
    const NPM_PACKAGE_MANAGER: &str =
        include_str!("../../../examples/minimal-ts/.mds/descriptors/package-managers/npm.toml");

    fn write_npm_runtime_descriptors(root: &Path) {
        let language_root = root.join(".mds/descriptors/languages");
        let package_manager_root = root.join(".mds/descriptors/package-managers");
        std::fs::create_dir_all(&language_root).unwrap();
        std::fs::create_dir_all(&package_manager_root).unwrap();
        std::fs::write(language_root.join("ts.toml"), TS_DESCRIPTOR).unwrap();
        std::fs::write(package_manager_root.join("npm.toml"), NPM_PACKAGE_MANAGER).unwrap();
    }

    #[test]
    fn test_initialize_capability_surface_declares_bridge_providers() {
        let capabilities = server_capabilities();

        assert!(
            matches!(
                capabilities.code_action_provider,
                Some(CodeActionProviderCapability::Simple(true))
            ),
            "code action capability missing: {:?}",
            capabilities.code_action_provider
        );
        assert!(
            capabilities.rename_provider.is_none(),
            "rename capability should stay disabled until editor bridge owns it: {:?}",
            capabilities.rename_provider
        );
        assert!(
            capabilities.document_formatting_provider.is_none(),
            "document formatting capability should stay disabled until editor bridge owns it: {:?}",
            capabilities.document_formatting_provider
        );
        let execute_command_provider = capabilities
            .execute_command_provider
            .expect("bridge execute command capability missing");
        assert!(
            execute_command_provider
                .commands
                .contains(&REMAP_GENERATED_TEXT_EDITS_COMMAND.to_string()),
            "text edit remap command missing: {:?}",
            execute_command_provider.commands
        );
        assert!(
            execute_command_provider
                .commands
                .contains(&REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND.to_string()),
            "batch text edit remap command missing: {:?}",
            execute_command_provider.commands
        );
    }

    fn bridge_fixture() -> BridgeFixture {
        let temp = TempDir::new().unwrap();
        let root = copy_example_package_to(temp.path(), "broken-remap-ts", "pkg");
        let markdown_path = root.join(".mds/source/foo/source-map.ts.md");
        let mut run_state = mds_core::RunState::default();
        let package = mds_core::package::load_package(&root, &Config::default(), &mut run_state)
            .expect("fixture package should load");
        assert!(
            run_state.diagnostics.is_empty(),
            "{:?}",
            run_state.diagnostics
        );
        let index = build_workspace_index(&package);

        BridgeFixture {
            _temp: temp,
            package_state: PackageState { package, index },
            markdown_path,
            source_generated_path: root.join("src/foo/source-map.ts"),
        }
    }

    fn overview_index_fixture() -> OverviewIndexFixture {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let source_path = root.join(".mds/source/pkg/greet.ts.md");
        let source_overview_path = root.join(".mds/source/overview.md");
        let nested_overview_path = root.join(".mds/source/pkg/overview.md");
        let test_overview_path = root.join(".mds/test/overview.md");

        write_npm_runtime_descriptors(&root);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(test_overview_path.parent().unwrap()).unwrap();
        std::fs::write(
            &source_path,
            "# greet\n\n## Purpose\n\nGreet users.\n\n## Contract\n\n- Stable.\n\n## Exports\n\n##### greet\n\nShared entrypoint.\n\n## Source\n\n```ts\nexport function greet(): string {\n  return 'hi';\n}\n```\n",
        )
        .unwrap();

        for (path, export_name) in [
            (&source_overview_path, "sourceOverviewShared"),
            (&nested_overview_path, "nestedOverviewShared"),
            (&test_overview_path, "testOverviewShared"),
        ] {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(
                path,
                format!(
                    "## Purpose\n\nOverview.\n\n## Architecture\n\nArchitecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Exports\n\n##### {export_name}\n\nShould never enter primary indexes.\n\n## Rules\n\n- Keep notes here.\n"
                ),
            )
            .unwrap();
        }

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let index = build_workspace_index(&package);

        OverviewIndexFixture {
            _temp: temp,
            package,
            index,
            source_path,
            source_overview_path,
            nested_overview_path,
            test_overview_path,
        }
    }

    fn write_custom_descriptor(root: &std::path::Path) {
        let descriptor_root = root.join(".mds/descriptors/languages");
        std::fs::create_dir_all(&descriptor_root).unwrap();
        std::fs::write(
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

    fn write_multi_part_descriptor(root: &std::path::Path, root_module_name: &str) {
        let descriptor_root = root.join(".mds/descriptors/languages");
        std::fs::create_dir_all(&descriptor_root).unwrap();
        std::fs::write(
            descriptor_root.join("dts.toml"),
            format!(
                concat!(
                    "id = \"dts\"\n",
                    "match_suffixes = [\"d.ts\"]\n\n",
                    "[language]\n",
                    "primary_ext = \"ts\"\n",
                    "root_module_markdown_names = [\"{}\"]\n\n",
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
                root_module_name,
            ),
        )
        .unwrap();
    }

    fn write_custom_package_manager_descriptor(root: &std::path::Path) {
        let descriptor_root = root.join(".mds/descriptors/package-managers");
        std::fs::create_dir_all(&descriptor_root).unwrap();
        std::fs::write(
            descriptor_root.join("schema-pm.toml"),
            concat!(
                "id = \"schema-pm\"\n",
                "display_name = \"Schema PM\"\n",
                "lang = \"schema-lang\"\n",
                "metadata_files = [\"schema.pkg\"]\n",
                "metadata_reader = \"plain-text\"\n",
            ),
        )
        .unwrap();
    }

    #[test]
    fn build_workspace_index_uses_markdown_exports_for_symbol_index() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mds-lsp-exports-index-{}-{}",
            std::process::id(),
            suffix,
        ));
        let source = root.join(".mds/source/app/greet.ts.md");
        write_npm_runtime_descriptors(&root);
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            "# app.greet\n\n## Purpose\n\nA module.\n\n## Contract\n\n- Stable.\n\n## Exports\n\n##### greet\n\nShared entrypoint.\n\n## Source\n\n```ts\nexport function greet(): string { return 'hi'; }\n```\n",
        )
        .unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };

        let index = build_workspace_index(&package);
        let locations = index
            .symbol_index
            .get(&("app.greet".to_string(), "greet".to_string()))
            .cloned()
            .unwrap_or_default();

        assert_eq!(locations, vec![source.clone()]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn build_workspace_index_normalizes_multi_part_suffix_module_ids() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let source = root.join(".mds/source/pkg/index.d.ts.md");
        write_multi_part_descriptor(&root, "index.d.ts.md");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            "# pkg.index\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Normalize multi-part suffix module ids.\n\n## Exports\n\n##### Thing\n\nShared export.\n\n## Source\n\n```ts\nexport interface Thing { value: string }\n```\n",
        )
        .unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };

        let index = build_workspace_index(&package);
        assert!(
            index
                .module_index
                .get("pkg/index")
                .is_some_and(|paths| paths == &vec![source.clone()]),
            "slash module key should use shared normalization: {:?}",
            index.module_index
        );
        assert!(
            index
                .module_index
                .get("pkg.index")
                .is_some_and(|paths| paths == &vec![source.clone()]),
            "dotted module key should use shared normalization: {:?}",
            index.module_index
        );
        assert!(
            !index.module_index.contains_key("pkg.index.d"),
            "stale single-segment suffix stripping must not leak into index: {:?}",
            index.module_index
        );
        assert!(
            index
                .symbol_index
                .contains_key(&("pkg.index".to_string(), "Thing".to_string())),
            "symbol index should anchor to the normalized dotted module id"
        );
    }

    #[test]
    fn build_workspace_index_tracks_source_maps_and_generated_files() {
        let fixture = bridge_fixture();

        assert!(fixture
            .package_state
            .contains_generated_path(&fixture.source_generated_path));
        let remapped = fixture
            .package_state
            .index
            .source_map
            .remap_generated_line(&fixture.source_generated_path, 5)
            .expect("expected generated line to map back to markdown");
        assert_eq!(remapped.0, fixture.markdown_path.as_path());
        assert_eq!(remapped.1, 18);
    }

    #[test]
    fn remap_generated_range_returns_markdown_range_for_code_fence_lines() {
        let fixture = bridge_fixture();
        let markdown_uri = Url::from_file_path(&fixture.markdown_path).unwrap();

        let remapped = fixture
            .package_state
            .remap_generated_range(
                &fixture.source_generated_path,
                Range {
                    start: Position {
                        line: 4,
                        character: 1,
                    },
                    end: Position {
                        line: 5,
                        character: 10,
                    },
                },
            )
            .expect("expected generated range to remap");

        assert_eq!(remapped.uri, markdown_uri);
        assert_eq!(
            remapped.range,
            Range {
                start: Position {
                    line: 17,
                    character: 1,
                },
                end: Position {
                    line: 18,
                    character: 10,
                },
            }
        );
        assert!(fixture
            .package_state
            .remap_generated_range(
                &fixture.source_generated_path,
                Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
            )
            .is_none());
    }

    #[test]
    fn resolve_generated_position_returns_generated_location_for_markdown_code_fence() {
        let fixture = bridge_fixture();
        let markdown_uri = Url::from_file_path(&fixture.markdown_path).unwrap();
        let generated_uri = Url::from_file_path(&fixture.source_generated_path).unwrap();
        let state = WorkspaceState {
            packages: vec![fixture.package_state.clone()],
            ..WorkspaceState::default()
        };

        let resolved = state
            .resolve_generated_position(
                &markdown_uri,
                Position {
                    line: 17,
                    character: 6,
                },
            )
            .expect("expected markdown position to resolve to generated output");

        assert_eq!(resolved.uri, generated_uri);
        assert_eq!(
            resolved.range,
            Range {
                start: Position {
                    line: 4,
                    character: 6,
                },
                end: Position {
                    line: 4,
                    character: 6,
                },
            }
        );
        assert!(state
            .resolve_generated_position(
                &markdown_uri,
                Position {
                    line: 0,
                    character: 0,
                },
            )
            .is_none());
    }

    #[test]
    fn execute_command_remaps_generated_locations_via_json_bridge() {
        let fixture = bridge_fixture();
        let markdown_uri = Url::from_file_path(&fixture.markdown_path).unwrap();
        let generated_uri = Url::from_file_path(&fixture.source_generated_path).unwrap();
        let unmanaged_uri =
            Url::from_file_path(fixture.package_state.package.root.join("src/unmanaged.ts"))
                .unwrap();
        let state = WorkspaceState {
            packages: vec![fixture.package_state],
            ..WorkspaceState::default()
        };

        let value = execute_bridge_command(
            &state,
            ExecuteCommandParams {
                command: REMAP_GENERATED_LOCATIONS_COMMAND.to_string(),
                arguments: vec![json!({
                    "locations": [
                        {
                            "uri": generated_uri,
                            "range": {
                                "start": { "line": 4, "character": 2 },
                                "end": { "line": 4, "character": 9 }
                            }
                        },
                        {
                            "uri": unmanaged_uri,
                            "range": {
                                "start": { "line": 1, "character": 3 },
                                "end": { "line": 1, "character": 8 }
                            }
                        },
                        {
                            "uri": generated_uri,
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 1 }
                            }
                        }
                    ]
                })],
                work_done_progress_params: Default::default(),
            },
        )
        .expect("expected execute command to succeed")
        .expect("expected execute command payload");

        let remapped: Vec<Option<Location>> = serde_json::from_value(value).unwrap();
        assert_eq!(remapped.len(), 3);
        assert_eq!(
            remapped[0],
            Some(Location {
                uri: markdown_uri,
                range: Range {
                    start: Position {
                        line: 17,
                        character: 2,
                    },
                    end: Position {
                        line: 17,
                        character: 9,
                    },
                },
            })
        );
        assert_eq!(
            remapped[1],
            Some(Location {
                uri: unmanaged_uri,
                range: Range {
                    start: Position {
                        line: 1,
                        character: 3,
                    },
                    end: Position {
                        line: 1,
                        character: 8,
                    },
                },
            })
        );
        assert_eq!(remapped[2], None);
    }

    #[test]
    fn execute_command_remaps_generated_text_edits_via_json_bridge() {
        let fixture = bridge_fixture();
        let markdown_uri = Url::from_file_path(&fixture.markdown_path).unwrap();
        let generated_uri = Url::from_file_path(&fixture.source_generated_path).unwrap();
        let state = WorkspaceState {
            packages: vec![fixture.package_state],
            ..WorkspaceState::default()
        };

        let value = execute_bridge_command(
            &state,
            ExecuteCommandParams {
                command: REMAP_GENERATED_TEXT_EDITS_COMMAND.to_string(),
                arguments: vec![json!({
                    "uri": generated_uri,
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 4, "character": 2 },
                                "end": { "line": 4, "character": 9 }
                            },
                            "newText": "const one = 2;"
                        }
                    ]
                })],
                work_done_progress_params: Default::default(),
            },
        )
        .expect("expected execute command to succeed")
        .expect("expected execute command payload");

        let remapped: Option<BridgeTextDocumentEdits> = serde_json::from_value(value).unwrap();
        assert_eq!(
            remapped,
            Some(BridgeTextDocumentEdits {
                uri: markdown_uri,
                edits: vec![TextEdit {
                    range: Range {
                        start: Position {
                            line: 17,
                            character: 2,
                        },
                        end: Position {
                            line: 17,
                            character: 9,
                        },
                    },
                    new_text: "const one = 2;".to_string(),
                }],
            })
        );
    }

    #[test]
    fn execute_command_passes_through_unmanaged_text_edits_via_json_bridge() {
        let fixture = bridge_fixture();
        let unmanaged_uri =
            Url::from_file_path(fixture.package_state.package.root.join("src/unmanaged.ts"))
                .unwrap();
        let state = WorkspaceState {
            packages: vec![fixture.package_state],
            ..WorkspaceState::default()
        };

        let edits = vec![TextEdit {
            range: Range {
                start: Position {
                    line: 2,
                    character: 1,
                },
                end: Position {
                    line: 2,
                    character: 4,
                },
            },
            new_text: "next".to_string(),
        }];

        let value = execute_bridge_command(
            &state,
            ExecuteCommandParams {
                command: REMAP_GENERATED_TEXT_EDITS_COMMAND.to_string(),
                arguments: vec![serde_json::to_value(BridgeTextDocumentEdits {
                    uri: unmanaged_uri.clone(),
                    edits: edits.clone(),
                })
                .unwrap()],
                work_done_progress_params: Default::default(),
            },
        )
        .expect("expected execute command to succeed")
        .expect("expected execute command payload");

        let remapped: Option<BridgeTextDocumentEdits> = serde_json::from_value(value).unwrap();
        assert_eq!(
            remapped,
            Some(BridgeTextDocumentEdits {
                uri: unmanaged_uri,
                edits,
            })
        );
    }

    #[test]
    fn execute_command_remaps_generated_text_document_edits_via_json_bridge() {
        let fixture = bridge_fixture();
        let markdown_uri = Url::from_file_path(&fixture.markdown_path).unwrap();
        let generated_uri = Url::from_file_path(&fixture.source_generated_path).unwrap();
        let unmanaged_uri =
            Url::from_file_path(fixture.package_state.package.root.join("src/unmanaged.ts"))
                .unwrap();
        let state = WorkspaceState {
            packages: vec![fixture.package_state],
            ..WorkspaceState::default()
        };

        let value = execute_bridge_command(
            &state,
            ExecuteCommandParams {
                command: REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND.to_string(),
                arguments: vec![json!({
                    "documents": [
                        {
                            "uri": generated_uri,
                            "edits": [
                                {
                                    "range": {
                                        "start": { "line": 4, "character": 2 },
                                        "end": { "line": 4, "character": 9 }
                                    },
                                    "newText": "const one = 2;"
                                }
                            ]
                        },
                        {
                            "uri": generated_uri,
                            "edits": [
                                {
                                    "range": {
                                        "start": { "line": 0, "character": 0 },
                                        "end": { "line": 0, "character": 0 }
                                    },
                                    "newText": "noop"
                                }
                            ]
                        },
                        {
                            "uri": unmanaged_uri,
                            "edits": [
                                {
                                    "range": {
                                        "start": { "line": 2, "character": 1 },
                                        "end": { "line": 2, "character": 4 }
                                    },
                                    "newText": "next"
                                }
                            ]
                        }
                    ]
                })],
                work_done_progress_params: Default::default(),
            },
        )
        .expect("expected execute command to succeed")
        .expect("expected execute command payload");

        let remapped: Vec<Option<BridgeTextDocumentEdits>> = serde_json::from_value(value).unwrap();
        assert_eq!(remapped.len(), 3);
        assert_eq!(
            remapped[0],
            Some(BridgeTextDocumentEdits {
                uri: markdown_uri,
                edits: vec![TextEdit {
                    range: Range {
                        start: Position {
                            line: 17,
                            character: 2,
                        },
                        end: Position {
                            line: 17,
                            character: 9,
                        },
                    },
                    new_text: "const one = 2;".to_string(),
                }],
            })
        );
        assert_eq!(remapped[1], None);
        assert_eq!(
            remapped[2],
            Some(BridgeTextDocumentEdits {
                uri: unmanaged_uri,
                edits: vec![TextEdit {
                    range: Range {
                        start: Position {
                            line: 2,
                            character: 1,
                        },
                        end: Position {
                            line: 2,
                            character: 4,
                        },
                    },
                    new_text: "next".to_string(),
                }],
            })
        );
    }

    #[test]
    fn build_workspace_index_includes_source_test_and_overview_docs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let source_path = root.join(".mds/source/pkg/greet.ts.md");
        let test_path = root.join(".mds/test/pkg/greet.test.ts.md");
        let source_overview_path = root.join(".mds/source/overview.md");
        let nested_overview_path = root.join(".mds/source/pkg/overview.md");
        let test_overview_path = root.join(".mds/test/overview.md");

        write_npm_runtime_descriptors(&root);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(test_path.parent().unwrap()).unwrap();
        std::fs::write(
            &source_path,
            "# greet\n\n## Purpose\n\nGreet users.\n\n## Contract\n\n- Stable.\n\n## Source\n\n```ts\nexport function greet(): string {\n  return 'hi';\n}\n```\n",
        )
        .unwrap();
        std::fs::write(
            &test_path,
            "# greet test\n\n## Purpose\n\nTest greet.\n\n## Covers\n\n- [Greet](pkg.greet)\n\n## Cases\n\n- Returns greeting.\n\n## Test\n\n```ts\nexpect(greet()).toBe('hi');\n```\n",
        )
        .unwrap();
        for overview_path in [
            &source_overview_path,
            &nested_overview_path,
            &test_overview_path,
        ] {
            if let Some(parent) = overview_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(
                overview_path,
                "## Purpose\n\nOverview.\n\n## Architecture\n\nArchitecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Keep notes here.\n",
            )
            .unwrap();
        }

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };

        let index = build_workspace_index(&package);

        for path in [
            source_path,
            test_path,
            source_overview_path,
            nested_overview_path,
            test_overview_path,
        ] {
            assert!(
                index.docs.contains_key(&path),
                "workspace index should contain {}",
                path.display()
            );
        }
    }

    #[test]
    fn build_workspace_index_keeps_overview_docs_out_of_primary_indexes() {
        let fixture = overview_index_fixture();

        for path in [
            &fixture.source_path,
            &fixture.source_overview_path,
            &fixture.nested_overview_path,
            &fixture.test_overview_path,
        ] {
            assert!(
                fixture.index.docs.contains_key(path),
                "workspace docs index should contain {}",
                path.display()
            );
        }

        assert!(fixture.index.module_index.contains_key("pkg.greet"));
        for key in ["overview", "pkg.overview", "pkg/overview"] {
            assert!(
                !fixture.index.module_index.contains_key(key),
                "overview should stay out of module index: {key}"
            );
            assert!(
                !fixture.index.expose_index.contains_key(key),
                "overview should stay out of expose index: {key}"
            );
        }
        for symbol_key in [
            ("overview", "sourceOverviewShared"),
            ("pkg.overview", "nestedOverviewShared"),
            ("overview", "testOverviewShared"),
        ] {
            assert!(
                !fixture
                    .index
                    .symbol_index
                    .contains_key(&(symbol_key.0.to_string(), symbol_key.1.to_string())),
                "overview should stay out of symbol index: {symbol_key:?}"
            );
        }
        for path in [
            &fixture.source_overview_path,
            &fixture.nested_overview_path,
            &fixture.test_overview_path,
        ] {
            assert!(
                !fixture.index.file_exposes.contains_key(path),
                "overview should stay out of file expose index: {}",
                path.display()
            );
        }
    }

    #[test]
    fn overview_docs_do_not_appear_in_completion_or_workspace_symbols() {
        let fixture = overview_index_fixture();
        let state = WorkspaceState {
            packages: vec![PackageState {
                package: fixture.package.clone(),
                index: fixture.index.clone(),
            }],
            ..WorkspaceState::default()
        };
        let config = Config::default();

        let package_modules = crate::capabilities::completion::provide_completions(
            "[[pkg.",
            Position {
                line: 0,
                character: 6,
            },
            None,
            &config,
            Some(&state),
        );
        assert!(
            package_modules.iter().any(|item| item.label == "pkg.greet"),
            "real modules should stay available in completions: {package_modules:?}"
        );
        assert!(
            !package_modules
                .iter()
                .any(|item| item.label == "pkg.overview"),
            "nested overview should stay out of completions: {package_modules:?}"
        );

        let root_modules = crate::capabilities::completion::provide_completions(
            "[[o",
            Position {
                line: 0,
                character: 3,
            },
            None,
            &config,
            Some(&state),
        );
        assert!(
            !root_modules.iter().any(|item| item.label == "overview"),
            "root overview should stay out of completions: {root_modules:?}"
        );

        let symbols = crate::capabilities::symbols::workspace_symbols("overview", &state);
        assert!(
            symbols.is_empty(),
            "overview docs should stay out of workspace symbols: {symbols:?}"
        );

        let greet_symbols = crate::capabilities::symbols::workspace_symbols("greet", &state);
        assert!(
            greet_symbols.iter().any(|item| item.name == "pkg.greet"),
            "real modules should stay visible in workspace symbols: {greet_symbols:?}"
        );
    }

    #[test]
    fn overview_docs_remain_unresolved_wiki_link_targets() {
        let fixture = overview_index_fixture();
        let state = WorkspaceState {
            packages: vec![PackageState {
                package: fixture.package.clone(),
                index: fixture.index.clone(),
            }],
            ..WorkspaceState::default()
        };
        let caller_path = fixture.package.root.join(".mds/source/pkg/caller.ts.md");
        let text = "## Purpose\n\nLink validation.\n\n## Contract\n\n- Keep overview out of module targets.\n- Valid module target: [[pkg.greet]]\n- Invalid overview targets: [[overview]] and [[pkg.overview]]\n\n## Source\n\n```ts\nexport function caller(): string {\n  return greet();\n}\n```\n";

        let diagnostics = crate::capabilities::diagnostics::validate_impl_md_text_with_state(
            &caller_path,
            text,
            &Config::default(),
            Some(&state),
        );

        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("wiki link target `[[overview]]` does not resolve to a module")),
            "root overview should stay unresolved: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("wiki link target `[[pkg.overview]]` does not resolve to a module")),
            "nested overview should stay unresolved: {diagnostics:?}"
        );
        assert!(
            !diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("wiki link target `[[pkg.greet]]`")),
            "real modules should still resolve: {diagnostics:?}"
        );
    }

    #[test]
    fn build_workspace_index_skips_template_and_excluded_overviews() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let source_overview_path = root.join(".mds/source/overview.md");
        let test_overview_path = root.join(".mds/test/overview.md");
        let template_source_overview_path = root.join(".mds/source/templates/overview.md");
        let template_test_overview_path = root.join(".mds/test/templates/overview.md");
        let excluded_source_overview_path = root.join(".mds/source/excluded/overview.md");
        let excluded_test_overview_path = root.join(".mds/test/excluded/overview.md");

        for overview_path in [
            &source_overview_path,
            &test_overview_path,
            &template_source_overview_path,
            &template_test_overview_path,
            &excluded_source_overview_path,
            &excluded_test_overview_path,
        ] {
            if let Some(parent) = overview_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(
                overview_path,
                "## Purpose\n\nOverview.\n\n## Architecture\n\nArchitecture.\n\n### Package Summary\n\n| Name | Version |\n| --- | --- |\n| fixture | 0.1.0 |\n\n### Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n### Dev Dependencies\n\n| Name | Version | Summary |\n| --- | --- | --- |\n\n## Rules\n\n- Keep notes here.\n",
            )
            .unwrap();
        }

        let mut config = Config::default();
        config.excludes = vec![
            ".mds/source/excluded/overview.md".to_string(),
            ".mds/test/excluded/overview.md".to_string(),
        ];
        let package = Package {
            root: root.clone(),
            config,
            package_manager_id: "npm".to_string(),
        };

        let index = build_workspace_index(&package);

        for path in [source_overview_path, test_overview_path] {
            assert!(
                index.docs.contains_key(&path),
                "workspace index should contain {}",
                path.display()
            );
        }

        for path in [
            template_source_overview_path,
            template_test_overview_path,
            excluded_source_overview_path,
            excluded_test_overview_path,
        ] {
            assert!(
                !index.docs.contains_key(&path),
                "workspace index should skip {}",
                path.display()
            );
        }
    }

    #[test]
    fn workspace_authoring_doc_path_skips_template_and_excluded_overviews() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let mut config = Config::default();
        config.excludes = vec![
            ".mds/source/excluded/overview.md".to_string(),
            ".mds/test/excluded/overview.md".to_string(),
        ];
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: Package {
                    root: root.clone(),
                    config,
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        for path in [
            root.join(".mds/source/overview.md"),
            root.join(".mds/test/overview.md"),
        ] {
            assert!(
                is_workspace_authoring_doc_path(&state, &path),
                "{} should remain a diagnostics target",
                path.display()
            );
        }

        for path in [
            root.join(".mds/source/templates/overview.md"),
            root.join(".mds/test/templates/overview.md"),
            root.join(".mds/source/excluded/overview.md"),
            root.join(".mds/test/excluded/overview.md"),
        ] {
            assert!(
                !is_workspace_authoring_doc_path(&state, &path),
                "{} should not be a diagnostics target",
                path.display()
            );
        }
    }

    #[test]
    fn workspace_authoring_doc_path_uses_effective_config_from_open_config_buffer() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let config_path = root.join("mds.config.toml");
        let source_path = root.join(".mds/source/pkg/greet.ts.md");
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: Package {
                    root: root.clone(),
                    config: Config::default(),
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            }],
            open_files: HashMap::from([(
                "file:///config".to_string(),
                OpenFile {
                    uri: "file:///config".to_string(),
                    path: config_path,
                    text: concat!(
                        "[package]\n",
                        "enabled = true\n\n",
                        "[roots]\n",
                        "exclude = [\".mds/source/pkg/greet.ts.md\"]\n",
                    )
                    .to_string(),
                    version: 1,
                    lang: None,
                },
            )]),
            ..WorkspaceState::default()
        };

        assert!(
            !is_workspace_authoring_doc_path(&state, &source_path),
            "{} should respect excludes from the open config buffer",
            source_path.display()
        );
        assert!(
            !path_requires_workspace_refresh(&state, &source_path),
            "{} should stop refreshing once the open config excludes it",
            source_path.display()
        );
    }

    #[test]
    fn path_requires_workspace_refresh_for_config_and_authoring_docs() {
        let temp = TempDir::new().unwrap();
        let workspace_root = temp.path().join("workspace");
        let package_root = workspace_root.join("packages/pkg");
        let undiscovered_root = workspace_root.join("packages/descriptor-only");
        let mut config = Config::default();
        config.excludes = vec![
            ".mds/source/excluded/overview.md".to_string(),
            ".mds/test/excluded/overview.md".to_string(),
        ];
        let state = WorkspaceState {
            workspace_folders: vec![workspace_root.clone()],
            packages: vec![PackageState {
                package: Package {
                    root: package_root.clone(),
                    config,
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        for path in [
            workspace_root.join("mds.config.toml"),
            package_root.join("mds.config.toml"),
            package_root.join(".mds/source/overview.md"),
            package_root.join(".mds/source/pkg/greet.ts.md"),
            package_root.join(".mds/test/overview.md"),
            package_root.join(".mds/test/pkg/greet.test.ts.md"),
            package_root.join(".mds/descriptors/languages/foo.toml"),
            package_root.join(".mds/descriptors/package-managers/schema-pm.toml"),
            package_root.join(".mds/descriptors/tools/lint.toml"),
            undiscovered_root.join(".mds/descriptors/package-managers/schema-pm.toml"),
        ] {
            assert!(
                path_requires_workspace_refresh(&state, &path),
                "{} should trigger workspace refresh",
                path.display()
            );
        }

        for path in [
            package_root.join("src/greet.ts"),
            package_root.join("README.md"),
            package_root.join("Cargo.toml"),
            package_root.join(".mds/source/templates/overview.md"),
            package_root.join(".mds/test/templates/overview.md"),
            package_root.join(".mds/source/excluded/overview.md"),
            package_root.join(".mds/test/excluded/overview.md"),
            package_root.join(".mds/descriptor-notes/foo.toml"),
        ] {
            assert!(
                !path_requires_workspace_refresh(&state, &path),
                "{} should not trigger workspace refresh",
                path.display()
            );
        }
    }

    #[test]
    fn path_requires_workspace_refresh_on_change_only_for_config_buffers() {
        let temp = TempDir::new().unwrap();
        let workspace_root = temp.path().join("workspace");

        for path in [
            workspace_root.join("mds.config.toml"),
            workspace_root.join("packages/pkg/mds.config.toml"),
        ] {
            assert!(
                path_requires_workspace_refresh_on_change(&path),
                "{} should refresh during didChange",
                path.display()
            );
        }

        for path in [
            workspace_root.join(".mds/source/overview.md"),
            workspace_root.join(".mds/source/pkg/greet.ts.md"),
            workspace_root.join(".mds/test/pkg/greet.test.ts.md"),
            workspace_root.join("README.md"),
        ] {
            assert!(
                !path_requires_workspace_refresh_on_change(&path),
                "{} should stay local until save or watcher refresh",
                path.display()
            );
        }
    }

    #[test]
    fn path_requires_package_refresh_on_change_for_authoring_docs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: Package {
                    root: root.clone(),
                    config: Config::default(),
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        for path in [
            root.join(".mds/source/pkg/greet.ts.md"),
            root.join(".mds/source/pkg/feature.md"),
            root.join(".mds/source/overview.md"),
            root.join(".mds/test/pkg/greet.test.ts.md"),
        ] {
            assert!(
                path_requires_package_refresh_on_change(&state, &path),
                "{} should refresh package index during didChange",
                path.display()
            );
        }

        for path in [
            root.join("mds.config.toml"),
            root.join("README.md"),
            root.join("src/greet.ts"),
        ] {
            assert!(
                !path_requires_package_refresh_on_change(&state, &path),
                "{} should not use package refresh during didChange",
                path.display()
            );
        }
    }

    #[test]
    fn watched_changes_require_workspace_refresh_when_batch_contains_tracked_path() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let mut config = Config::default();
        config.excludes = vec![
            ".mds/source/excluded/overview.md".to_string(),
            ".mds/test/excluded/overview.md".to_string(),
        ];
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: Package {
                    root: root.clone(),
                    config,
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        let tracked_change = FileEvent {
            uri: Url::from_file_path(root.join(".mds/source/overview.md")).unwrap(),
            typ: FileChangeType::CHANGED,
        };
        let untracked_change = FileEvent {
            uri: Url::from_file_path(root.join("src/greet.ts")).unwrap(),
            typ: FileChangeType::CHANGED,
        };
        let excluded_change = FileEvent {
            uri: Url::from_file_path(root.join(".mds/source/excluded/overview.md")).unwrap(),
            typ: FileChangeType::CHANGED,
        };
        let template_change = FileEvent {
            uri: Url::from_file_path(root.join(".mds/source/templates/overview.md")).unwrap(),
            typ: FileChangeType::CHANGED,
        };

        assert!(watched_changes_require_workspace_refresh(
            &state,
            &[untracked_change.clone(), tracked_change]
        ));
        assert!(!watched_changes_require_workspace_refresh(
            &state,
            &[untracked_change]
        ));
        assert!(!watched_changes_require_workspace_refresh(
            &state,
            &[excluded_change, template_change]
        ));
    }

    #[test]
    fn custom_suffix_source_candidates_trigger_refresh_on_save_add_and_remove() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        write_custom_descriptor(&root);
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: Package {
                    root: root.clone(),
                    config: Config::default(),
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        let path = root.join(".mds/source/pkg/feature.foo.md");

        assert!(
            !is_workspace_authoring_doc_path(&state, &path),
            "{} should stay a candidate path until it exists on disk or in an open buffer",
            path.display()
        );
        assert!(
            path_requires_workspace_refresh(&state, &path),
            "{} should trigger save refresh",
            path.display()
        );

        for change_type in [
            FileChangeType::CHANGED,
            FileChangeType::CREATED,
            FileChangeType::DELETED,
        ] {
            let change = FileEvent {
                uri: Url::from_file_path(&path).unwrap(),
                typ: change_type,
            };
            assert!(
                watched_changes_require_workspace_refresh(&state, &[change]),
                "{} should trigger watcher refresh for {:?}",
                path.display(),
                change_type
            );
        }
    }

    #[test]
    fn descriptor_paths_trigger_workspace_refresh_for_watched_create_change_delete() {
        let temp = TempDir::new().unwrap();
        let workspace_root = temp.path().join("workspace");
        let state = WorkspaceState {
            workspace_folders: vec![workspace_root.clone()],
            ..WorkspaceState::default()
        };

        for path in [
            workspace_root.join("packages/pkg/.mds/descriptors/languages/foo.toml"),
            workspace_root.join("packages/pkg/.mds/descriptors/package-managers/schema-pm.toml"),
            workspace_root.join("packages/pkg/.mds/descriptors/tools/lint.toml"),
        ] {
            for change_type in [
                FileChangeType::CHANGED,
                FileChangeType::CREATED,
                FileChangeType::DELETED,
            ] {
                let change = FileEvent {
                    uri: Url::from_file_path(&path).unwrap(),
                    typ: change_type,
                };
                assert!(
                    watched_changes_require_workspace_refresh(&state, &[change]),
                    "{} should trigger watcher refresh for {:?}",
                    path.display(),
                    change_type
                );
            }
        }
    }

    #[test]
    fn open_documents_for_validation_includes_open_config_and_authoring_docs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let config_path = root.join("mds.config.toml");
        let source_path = root.join(".mds/source/pkg/greet.ts.md");
        let test_path = root.join(".mds/test/pkg/greet.test.ts.md");
        let overview_path = root.join(".mds/source/overview.md");
        let mut state = WorkspaceState::default();

        for (uri, path, text) in [
            (
                "file:///config",
                config_path.clone(),
                "[package]\nenabled = true\n".to_string(),
            ),
            ("file:///source", source_path.clone(), "source".to_string()),
            ("file:///test", test_path.clone(), "test".to_string()),
            (
                "file:///overview",
                overview_path.clone(),
                "overview".to_string(),
            ),
        ] {
            state.open_files.insert(
                uri.to_string(),
                OpenFile {
                    uri: uri.to_string(),
                    path,
                    text,
                    version: 1,
                    lang: None,
                },
            );
        }

        let mut refreshed_paths = open_documents_for_validation(&state)
            .into_iter()
            .map(|(uri, _)| uri.to_file_path().unwrap())
            .collect::<Vec<_>>();
        refreshed_paths.sort();

        let mut expected_paths = vec![config_path, overview_path, source_path, test_path];
        expected_paths.sort();

        assert_eq!(refreshed_paths, expected_paths);
    }

    #[test]
    fn open_documents_for_validation_in_package_scopes_revalidation() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("workspace");
        let left_root = root.join("packages/left");
        let right_root = root.join("packages/right");
        let left_path = left_root.join(".mds/source/pkg/left.ts.md");
        let right_path = right_root.join(".mds/source/pkg/right.ts.md");
        let mut state = WorkspaceState::default();

        for (uri, path) in [
            ("file:///left", left_path.clone()),
            ("file:///right", right_path.clone()),
        ] {
            state.open_files.insert(
                uri.to_string(),
                OpenFile {
                    uri: uri.to_string(),
                    path,
                    text: "doc".to_string(),
                    version: 1,
                    lang: None,
                },
            );
        }

        let refreshed_paths = open_documents_for_validation_in_package(&state, &left_root)
            .into_iter()
            .map(|(uri, _)| uri.to_file_path().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(refreshed_paths, vec![left_path]);
    }

    #[test]
    fn rebuild_package_index_uses_open_authoring_buffers_for_cross_file_features() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let target_path = root.join(".mds/source/pkg/greet.ts.md");
        let caller_path = root.join(".mds/source/pkg/caller.ts.md");
        write_npm_runtime_descriptors(&root);
        std::fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        std::fs::write(
            &target_path,
            "# greet\n\n## Purpose\n\nDisk version.\n\n## Contract\n\n- Stable.\n\n## Exports\n\n##### greet\n\nDisk export.\n\n## Source\n\n```ts\nexport function greet(): string {\n  return 'hi';\n}\n```\n",
        )
        .unwrap();
        std::fs::write(
            &caller_path,
            "## Purpose\n\nCaller.\n\n## Contract\n\n- Uses target.\n\n## Source\n\n##### callRenamed\n\nCalls renamed export.\n\n```ts\nexport function callRenamed(): string {\n  return renamed();\n}\n```\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| workspace | pkg.greet | renamed | - | Renamed symbol | [[pkg.greet#renamed]] |\n",
        )
        .unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let disk_index = build_workspace_index(&package);
        let open_target_text = "# greet\n\n## Purpose\n\nBuffer version.\n\n## Contract\n\n- Stable.\n\n## Exports\n\n##### renamed\n\nBuffer export.\n\n## Source\n\n##### renamed\n\nBuffer shared definition.\n\n```ts\nexport function renamed(): string {\n  return 'hi';\n}\n```\n";
        let open_caller_text = "## Purpose\n\nCaller.\n\n## Contract\n\n- Uses target.\n\n## Source\n\n##### callRenamed\n\nCalls renamed export.\n\n```ts\nexport function callRenamed(): string {\n  return renamed();\n}\n```\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| workspace | pkg.greet | renamed | - | Renamed symbol | [[pkg.greet#renamed]] |\n";
        let mut state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: package.clone(),
                index: disk_index,
            }],
            ..WorkspaceState::default()
        };
        state.open_files.insert(
            "file:///target".to_string(),
            OpenFile {
                uri: "file:///target".to_string(),
                path: target_path.clone(),
                text: open_target_text.to_string(),
                version: 2,
                lang: Some(Lang::Other("ts".to_string())),
            },
        );
        state.open_files.insert(
            "file:///caller".to_string(),
            OpenFile {
                uri: "file:///caller".to_string(),
                path: caller_path.clone(),
                text: open_caller_text.to_string(),
                version: 2,
                lang: Some(Lang::Other("ts".to_string())),
            },
        );

        let rebuilt_index = rebuild_package_index(&state, &root).expect("package index");
        state.packages[0].index = rebuilt_index;
        let config = Config::default();

        let symbol_completions = crate::capabilities::completion::provide_completions(
            "[[pkg.greet#r",
            Position {
                line: 0,
                character: 13,
            },
            Some(&caller_path),
            &config,
            Some(&state),
        );
        assert!(
            symbol_completions.iter().any(|item| item.label == "renamed"),
            "completion should see the renamed symbol from the open target buffer: {symbol_completions:?}"
        );
        assert!(
            !symbol_completions.iter().any(|item| item.label == "greet"),
            "stale symbol should disappear after package refresh: {symbol_completions:?}"
        );

        let definition = crate::capabilities::navigation::goto_definition(
            "See [[pkg.greet#renamed]]",
            Position {
                line: 0,
                character: 18,
            },
            &caller_path,
            &state,
        );
        match definition {
            Some(GotoDefinitionResponse::Scalar(location)) => {
                assert_eq!(location.uri, Url::from_file_path(&target_path).unwrap());
                assert_eq!(location.range.start.line, 12);
            }
            other => panic!("expected scalar definition result, got {other:?}"),
        }

        let symbols = crate::capabilities::symbols::workspace_symbols("renamed", &state);
        assert!(
            symbols.iter().any(|item| item.name == "renamed"),
            "workspace symbols should use the refreshed open-buffer export list: {symbols:?}"
        );
        assert!(
            !symbols.iter().any(|item| item.name == "greet"),
            "stale workspace symbol should disappear: {symbols:?}"
        );

        let references = crate::capabilities::navigation::find_references(
            open_target_text,
            Position {
                line: 18,
                character: 10,
            },
            &target_path,
            &state,
        )
        .expect("references");
        assert!(
            references
                .iter()
                .any(|location| location.uri == Url::from_file_path(&caller_path).unwrap()),
            "references should scan open caller buffers, not stale disk text: {references:?}"
        );
    }

    #[test]
    fn find_references_returns_none_inside_code_fence_content() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let target_path = root.join(".mds/source/pkg/greet.ts.md");
        let caller_path = root.join(".mds/source/pkg/caller.ts.md");
        std::fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        let target_text = "# greet\n\n## Purpose\n\nTarget.\n\n## Contract\n\n- Stable.\n\n## Exports\n\n##### greet\n\nShared export.\n\n## Source\n\n```ts\nexport function greet(): string {\n  return 'hi';\n}\n```\n";
        std::fs::write(&target_path, target_text).unwrap();
        std::fs::write(
            &caller_path,
            "# caller\n\n## Purpose\n\nCaller.\n\n## Contract\n\n- Uses target.\n\n## Imports\n\n| From | Target | Symbols | Via | Summary | Reference |\n| --- | --- | --- | --- | --- | --- |\n| workspace | pkg.greet | greet | - | Shared symbol | [[pkg.greet#greet]] |\n",
        )
        .unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: package.clone(),
                index: build_workspace_index(&package),
            }],
            ..WorkspaceState::default()
        };

        let references = crate::capabilities::navigation::find_references(
            target_text,
            Position {
                line: 15,
                character: 18,
            },
            &target_path,
            &state,
        );

        assert_eq!(
            references, None,
            "references inside code fences should defer to the embedded bridge"
        );
    }

    #[test]
    fn rebuild_package_index_drops_closed_unsaved_authoring_docs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let unsaved_path = root.join(".mds/source/pkg/unsaved.ts.md");
        write_npm_runtime_descriptors(&root);
        std::fs::create_dir_all(unsaved_path.parent().unwrap()).unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let open_text = "# unsaved\n\n## Purpose\n\nBuffer only.\n\n## Contract\n\n- Ephemeral.\n\n## Exports\n\n##### ephemeral\n\nBuffer export.\n\n## Source\n\n```ts\nexport function ephemeral(): string {\n  return 'hi';\n}\n```\n";
        let mut state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            open_files: HashMap::from([(
                "file:///unsaved".to_string(),
                OpenFile {
                    uri: "file:///unsaved".to_string(),
                    path: unsaved_path.clone(),
                    text: open_text.to_string(),
                    version: 1,
                    lang: Some(Lang::Other("ts".to_string())),
                },
            )]),
            packages: vec![PackageState {
                package,
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        let open_index = rebuild_package_index(&state, &root).expect("package index");
        assert!(open_index.docs.contains_key(&unsaved_path));
        assert!(open_index
            .module_index
            .get("pkg.unsaved")
            .is_some_and(|paths| paths.contains(&unsaved_path)));
        assert!(open_index
            .symbol_index
            .get(&("pkg.unsaved".to_string(), "ephemeral".to_string()))
            .is_some_and(|paths| paths.contains(&unsaved_path)));

        state.packages[0].index = open_index;
        state.open_files.clear();

        let rebuilt_index = rebuild_package_index(&state, &root).expect("package index");
        assert!(
            !rebuilt_index.docs.contains_key(&unsaved_path),
            "closed unsaved doc should be removed from docs index"
        );
        assert!(
            rebuilt_index
                .module_index
                .values()
                .all(|paths| !paths.contains(&unsaved_path)),
            "closed unsaved doc should be removed from module index"
        );
        assert!(
            rebuilt_index
                .symbol_index
                .values()
                .all(|paths| !paths.contains(&unsaved_path)),
            "closed unsaved doc should be removed from symbol index"
        );
        assert!(
            rebuilt_index
                .expose_index
                .values()
                .all(|paths| !paths.contains(&unsaved_path)),
            "closed unsaved doc should be removed from expose index"
        );
    }

    #[test]
    fn build_workspace_index_with_state_uses_open_fence_only_source_buffer() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let path = root.join(".mds/source/pkg/feature.md");
        write_custom_descriptor(&root);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let text = "# feature\n\n## Purpose\n\nBuffer only.\n\n## Contract\n\n- Keep custom source docs indexed.\n\n## Exports\n\n##### feature\n\nBuffer export.\n\n## Source\n\n```foo\nexport const feature = 1;\n```\n";
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            open_files: HashMap::from([(
                "file:///feature".to_string(),
                OpenFile {
                    uri: "file:///feature".to_string(),
                    path: path.clone(),
                    text: text.to_string(),
                    version: 1,
                    lang: None,
                },
            )]),
            packages: vec![PackageState {
                package: package.clone(),
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        assert!(
            resolve_authoring_doc(&path, &state).is_some(),
            "open fence-only source doc should remain a diagnostics target"
        );
        assert_eq!(
            resolve_active_language(&path, Some(&state)),
            Some(Lang::Other("foo".to_string()))
        );

        let index = build_workspace_index_with_state(&package, &state);
        let doc = index
            .docs
            .get(&path)
            .expect("open fence-only source doc should be indexed");
        assert_eq!(doc.lang, Lang::Other("foo".to_string()));
        assert!(
            index
                .module_index
                .get("pkg.feature")
                .is_some_and(|paths| paths.contains(&path)),
            "open fence-only source doc should populate module index"
        );
    }

    #[test]
    fn build_workspace_index_with_state_uses_open_unsuffixed_test_buffer_language() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let source_path = root.join(".mds/source/pkg/feature.foo.md");
        let test_path = root.join(".mds/test/pkg/feature.md");
        write_custom_descriptor(&root);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(test_path.parent().unwrap()).unwrap();
        std::fs::write(
            &source_path,
            "# feature\n\n## Purpose\n\nSource fixture.\n\n## Contract\n\n- Support custom source docs.\n\n## Source\n\n```foo\nexport const feature = 1;\n```\n",
        )
        .unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            open_files: HashMap::from([(
                "file:///feature-test".to_string(),
                OpenFile {
                    uri: "file:///feature-test".to_string(),
                    path: test_path.clone(),
                    text: "# feature test\n\n## Purpose\n\nBuffer only.\n\n## Covers\n\n- [feature](pkg.feature)\n\n## Cases\n\n- Keeps language-aware indexing.\n\n## Test\n\n```foo\nexpect(feature).toBe(1);\n```\n".to_string(),
                    version: 1,
                    lang: None,
                },
            )]),
            packages: vec![PackageState {
                package: package.clone(),
                index: build_workspace_index(&package),
            }],
            ..WorkspaceState::default()
        };

        assert_eq!(
            resolve_active_language(&test_path, Some(&state)),
            Some(Lang::Other("foo".to_string()))
        );

        let index = build_workspace_index_with_state(&package, &state);
        let doc = index
            .docs
            .get(&test_path)
            .expect("open unsuffixed test doc should be indexed");
        assert_eq!(doc.lang, Lang::Other("foo".to_string()));
        assert!(
            index
                .module_index
                .get("pkg.feature")
                .is_some_and(|paths| paths.contains(&test_path)),
            "open unsuffixed test doc should populate module index"
        );
    }

    #[test]
    fn build_workspace_index_with_state_uses_open_unsuffixed_source_buffer_language_from_noncanonical_generated_fence(
    ) {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let path = root.join(".mds/source/pkg/feature.md");
        write_npm_runtime_descriptors(&root);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let mut config = Config::default();
        config.check.implementation_section_only = false;
        let package = Package {
            root: root.clone(),
            config: config.clone(),
            package_manager_id: "npm".to_string(),
        };
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            open_files: HashMap::from([(
                "file:///feature-source".to_string(),
                OpenFile {
                    uri: "file:///feature-source".to_string(),
                    path: path.clone(),
                    text: "# feature\n\n## Purpose\n\nBuffer only.\n\n## Contract\n\n- Allow generated fences to drive language discovery.\n\n## Exports\n\n##### feature\n\nBuffer export.\n\n## Cases\n\n```ts\nexport const feature = 1;\n```\n"
                        .to_string(),
                    version: 1,
                    lang: None,
                },
            )]),
            packages: vec![PackageState {
                package: package.clone(),
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        assert!(
            resolve_authoring_doc(&path, &state).is_some(),
            "open unsuffixed source doc should remain a diagnostics target"
        );
        assert_eq!(
            resolve_active_language(&path, Some(&state)),
            Some(Lang::Other("ts".to_string()))
        );

        let index = build_workspace_index_with_state(&package, &state);
        let doc = index
            .docs
            .get(&path)
            .expect("open unsuffixed source doc should be indexed");
        assert_eq!(doc.lang, Lang::Other("ts".to_string()));
        assert!(
            index
                .module_index
                .get("pkg.feature")
                .is_some_and(|paths| paths.contains(&path)),
            "open unsuffixed source doc should populate module index"
        );
    }

    #[test]
    fn resolve_authoring_doc_keeps_rejected_fence_only_source_doc_as_diagnostics_target() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let path = root.join(".mds/source/pkg/rejected.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "# Rejected\n\n## Purpose\n\nFixture.\n\n## Contract\n\n- Keep rejected source docs diagnosable.\n\n## Source\n\n```unknown\ncontent\n```\n",
        )
        .unwrap();

        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            packages: vec![PackageState {
                package: Package {
                    root: root.clone(),
                    config: Config::default(),
                    package_manager_id: "npm".to_string(),
                },
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        assert!(
            resolve_authoring_doc(&path, &state).is_some(),
            "rejected fence-only source doc should remain a diagnostics target"
        );
        assert_eq!(resolve_active_language(&path, Some(&state)), None);
    }

    #[test]
    fn build_workspace_index_with_state_skips_rejected_dotted_source_buffer() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let path = root.join(".mds/source/pkg/rejected.foo.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            open_files: HashMap::from([(
                "file:///rejected".to_string(),
                OpenFile {
                    uri: "file:///rejected".to_string(),
                    path: path.clone(),
                    text: "# Rejected\n\n## Purpose\n\nBuffer only.\n\n## Contract\n\n- Keep diagnostics, not index entries.\n\n## Exports\n\n##### leaked\n\nShould not be indexed.\n\n## Source\n\n```unknown\ncontent\n```\n".to_string(),
                    version: 1,
                    lang: None,
                },
            )]),
            packages: vec![PackageState {
                package: package.clone(),
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        assert!(
            resolve_authoring_doc(&path, &state).is_some(),
            "rejected dotted source doc should remain a diagnostics target"
        );
        assert_eq!(resolve_active_language(&path, Some(&state)), None);

        let index = build_workspace_index_with_state(&package, &state);
        assert!(
            !index.docs.contains_key(&path),
            "rejected dotted source doc should not be indexed"
        );
        assert!(
            index
                .module_index
                .values()
                .all(|paths| !paths.contains(&path)),
            "rejected dotted source doc should not populate module index"
        );
        assert!(
            index
                .symbol_index
                .values()
                .all(|paths| !paths.contains(&path)),
            "rejected dotted source doc should not populate symbol index"
        );
        assert!(
            index
                .expose_index
                .values()
                .all(|paths| !paths.contains(&path)),
            "rejected dotted source doc should not populate expose index"
        );
    }

    #[test]
    fn build_workspace_index_with_state_uses_open_buffer_exports() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("pkg");
        let path = root.join(".mds/source/pkg/greet.ts.md");
        write_npm_runtime_descriptors(&root);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "# greet\n\n## Purpose\n\nDisk.\n\n## Contract\n\n- Stable.\n\n## Exports\n\n##### greet\n\nDisk export.\n\n## Source\n\n```ts\nexport function greet(): string {\n  return 'hi';\n}\n```\n",
        )
        .unwrap();

        let package = Package {
            root: root.clone(),
            config: Config::default(),
            package_manager_id: "npm".to_string(),
        };
        let state = WorkspaceState {
            workspace_folders: vec![root.clone()],
            open_files: HashMap::from([(
                "file:///target".to_string(),
                OpenFile {
                    uri: "file:///target".to_string(),
                    path: path.clone(),
                    text: "# greet\n\n## Purpose\n\nBuffer.\n\n## Contract\n\n- Stable.\n\n## Exports\n\n##### renamed\n\nBuffer export.\n\n## Source\n\n##### renamed\n\nBuffer shared definition.\n\n```ts\nexport function renamed(): string {\n  return 'hi';\n}\n```\n".to_string(),
                    version: 2,
                    lang: Some(Lang::Other("ts".to_string())),
                },
            )]),
            packages: vec![PackageState {
                package: package.clone(),
                index: WorkspaceIndex::default(),
            }],
            ..WorkspaceState::default()
        };

        let index = build_workspace_index_with_state(&package, &state);

        assert!(
            index
                .symbol_index
                .contains_key(&("pkg.greet".to_string(), "renamed".to_string())),
            "workspace index should use the open buffer export list"
        );
        assert!(
            !index
                .symbol_index
                .contains_key(&("pkg.greet".to_string(), "greet".to_string())),
            "disk export should not survive the open-buffer overlay"
        );
    }

    #[test]
    fn rebuild_workspace_packages_uses_open_config_excludes() {
        let temp = TempDir::new().unwrap();
        let workspace_root = temp.path().join("workspace");
        let package_root = workspace_root.join("packages/pkg");
        let disk_source_path = package_root.join(".mds/source/pkg/greet.ts.md");
        write_npm_runtime_descriptors(&package_root);
        std::fs::create_dir_all(disk_source_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&package_root).unwrap();
        std::fs::write(
            workspace_root.join("package.json"),
            "{\"name\":\"workspace\",\"private\":true}",
        )
        .unwrap();
        std::fs::write(
            package_root.join("package.json"),
            "{\"name\":\"pkg\",\"version\":\"0.1.0\"}",
        )
        .unwrap();
        std::fs::write(
            package_root.join("mds.config.toml"),
            "[package]\nenabled = true\n",
        )
        .unwrap();
        std::fs::write(
            &disk_source_path,
            "# greet\n\n## Purpose\n\nDisk.\n\n## Contract\n\n- Stable.\n\n## Source\n\n```ts\nexport function greet(): string {\n  return 'hi';\n}\n```\n",
        )
        .unwrap();

        let state = WorkspaceState {
            workspace_folders: vec![workspace_root.clone()],
            open_files: HashMap::from([(
                "file:///config".to_string(),
                OpenFile {
                    uri: "file:///config".to_string(),
                    path: package_root.join("mds.config.toml"),
                    text: concat!(
                        "[package]\n",
                        "enabled = true\n\n",
                        "[roots]\n",
                        "source_md = \".mds/source\"\n",
                        "exclude = [\".mds/source/pkg/greet.ts.md\"]\n",
                    )
                    .to_string(),
                    version: 2,
                    lang: None,
                },
            )]),
            ..WorkspaceState::default()
        };

        let packages = rebuild_workspace_packages(&state);
        let package_state = packages
            .iter()
            .find(|package| package.package.root == package_root)
            .expect("package state");

        assert!(
            !package_state.index.docs.contains_key(&disk_source_path),
            "open config excludes should remove docs from the refreshed index"
        );
        assert_eq!(
            package_state.package.config.excludes,
            vec![".mds/source/pkg/greet.ts.md".to_string()]
        );
    }

    #[test]
    fn rebuild_workspace_packages_uses_package_root_descriptor_context_for_package_manager() {
        let temp = TempDir::new().unwrap();
        let workspace_root = temp.path().join("workspace");
        let package_root = workspace_root.join("packages/pkg");
        std::fs::create_dir_all(&package_root).unwrap();
        std::fs::write(
            package_root.join("mds.config.toml"),
            "[package]\nenabled = true\n",
        )
        .unwrap();
        std::fs::write(package_root.join("schema.pkg"), "name = \"pkg\"\n").unwrap();
        write_custom_package_manager_descriptor(&package_root);

        let state = WorkspaceState {
            workspace_folders: vec![workspace_root.clone()],
            ..WorkspaceState::default()
        };

        let packages = rebuild_workspace_packages(&state);
        let package_state = packages
            .iter()
            .find(|package| package.package.root == package_root)
            .expect("package state");

        assert_eq!(package_state.package.package_manager_id, "schema-pm");
    }

    #[test]
    fn rebuild_workspace_packages_uses_open_config_enabled_for_package_membership() {
        let temp = TempDir::new().unwrap();
        let workspace_root = temp.path().join("workspace");
        let package_root = workspace_root.join("packages/pkg");
        let config_path = package_root.join("mds.config.toml");
        let disk_source_path = package_root.join(".mds/source/pkg/greet.ts.md");
        write_npm_runtime_descriptors(&package_root);
        std::fs::create_dir_all(disk_source_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&package_root).unwrap();
        std::fs::write(
            workspace_root.join("package.json"),
            "{\"name\":\"workspace\",\"private\":true}",
        )
        .unwrap();
        std::fs::write(
            package_root.join("package.json"),
            "{\"name\":\"pkg\",\"version\":\"0.1.0\"}",
        )
        .unwrap();
        std::fs::write(
            &disk_source_path,
            "# greet\n\n## Purpose\n\nDisk.\n\n## Contract\n\n- Stable.\n\n## Source\n\n```ts\nexport function greet(): string {\n  return 'hi';\n}\n```\n",
        )
        .unwrap();

        std::fs::write(&config_path, "[package]\nenabled = true\n").unwrap();
        let disabled_overlay = WorkspaceState {
            workspace_folders: vec![workspace_root.clone()],
            open_files: HashMap::from([(
                "file:///config-disabled".to_string(),
                OpenFile {
                    uri: "file:///config-disabled".to_string(),
                    path: config_path.clone(),
                    text: "[package]\nenabled = false\n".to_string(),
                    version: 2,
                    lang: None,
                },
            )]),
            ..WorkspaceState::default()
        };

        let disabled_packages = rebuild_workspace_packages(&disabled_overlay);
        assert!(
            disabled_packages.is_empty(),
            "open config enabled=false should remove package from workspace state"
        );

        let disabled_state = WorkspaceState {
            workspace_folders: vec![workspace_root.clone()],
            open_files: disabled_overlay.open_files.clone(),
            packages: disabled_packages,
            ..WorkspaceState::default()
        };
        assert!(
            resolve_authoring_doc(&disk_source_path, &disabled_state).is_none(),
            "disabled package should stop being a diagnostics target"
        );
        assert!(
            disabled_state.find_module_locations("pkg.greet").is_empty(),
            "disabled package should disappear from navigation indexes"
        );

        std::fs::write(&config_path, "[package]\nenabled = false\n").unwrap();
        let enabled_overlay = WorkspaceState {
            workspace_folders: vec![workspace_root.clone()],
            open_files: HashMap::from([(
                "file:///config-enabled".to_string(),
                OpenFile {
                    uri: "file:///config-enabled".to_string(),
                    path: config_path,
                    text: "[package]\nenabled = true\n".to_string(),
                    version: 3,
                    lang: None,
                },
            )]),
            ..WorkspaceState::default()
        };

        let enabled_packages = rebuild_workspace_packages(&enabled_overlay);
        let enabled_package = enabled_packages
            .iter()
            .find(|package| package.package.root == package_root)
            .expect("package restored by open config");
        assert!(
            enabled_package.index.docs.contains_key(&disk_source_path),
            "open config enabled=true should restore package docs into the workspace index"
        );

        let enabled_state = WorkspaceState {
            workspace_folders: vec![workspace_root],
            open_files: enabled_overlay.open_files.clone(),
            packages: enabled_packages,
            ..WorkspaceState::default()
        };
        assert!(
            resolve_authoring_doc(&disk_source_path, &enabled_state).is_some(),
            "re-enabled package should return to diagnostics targets"
        );
        assert!(
            !enabled_state.find_module_locations("pkg.greet").is_empty(),
            "re-enabled package should return to navigation indexes"
        );
    }
}
