use mds_core::config::{merge_config_file, merge_config_text};
use mds_core::descriptor::{
    fence_labels_for_lang, is_overview_markdown_path, lang_for_markdown_path,
    markdown_suffix_for_lang, markdown_suffixes_for_lang, with_workspace_descriptor_root,
};
use mds_core::markdown::{
    discover_source_doc_lang, discover_source_doc_lang_from_text, discover_test_doc_lang_from_text,
    is_discoverable_authoring_doc_path, source_doc_lang_rejection_from_text, source_markdown_root,
    test_markdown_root,
};
use mds_core::Config;
use mds_core::ImplDoc;
use mds_core::Lang;
use mds_core::Package;
use mds_core::RunState;
use mds_core::{SourceMap, SourceSpan};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::{Location, Position, Range, TextEdit, Url};
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OpenFile {
    pub uri: String,
    pub path: PathBuf,
    pub text: String,
    pub version: i32,
    pub lang: Option<Lang>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct WorkspaceIndex {
    pub docs: HashMap<PathBuf, ImplDoc>,
    pub expose_index: HashMap<String, Vec<PathBuf>>,
    pub file_exposes: HashMap<PathBuf, Vec<String>>,
    pub module_index: HashMap<String, Vec<PathBuf>>,
    pub symbol_index: HashMap<(String, String), Vec<PathBuf>>,
    pub source_map: SourceMap,
    pub generated_files: HashSet<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PackageState {
    pub package: Package,
    pub index: WorkspaceIndex,
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct WorkspaceState {
    pub workspace_folders: Vec<PathBuf>,
    pub open_files: HashMap<String, OpenFile>,
    pub packages: Vec<PackageState>,
    pub configs: HashMap<PathBuf, Config>,
}

pub type SharedState = Arc<RwLock<WorkspaceState>>;

#[derive(Debug, Clone)]
pub struct ResolvedAuthoringDoc {
    pub config: Config,
}

pub fn descriptor_root_for_path(
    path: &Path,
    workspace_state: Option<&WorkspaceState>,
) -> Option<PathBuf> {
    if let Some(workspace_state) = workspace_state {
        if let Some(package_state) = workspace_state.package_for_path(path) {
            return Some(package_state.package.root.clone());
        }
    }

    if matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("mds.config.toml")
    ) {
        return path.parent().map(Path::to_path_buf);
    }

    let mut current = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };
    while let Some(dir) = current {
        if dir.join("mds.config.toml").is_file() || dir.join(".mds").is_dir() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }

    None
}

pub fn with_descriptor_root_for_path<T>(
    path: Option<&Path>,
    workspace_state: Option<&WorkspaceState>,
    action: impl FnOnce() -> T,
) -> T {
    let root = path.and_then(|path| descriptor_root_for_path(path, workspace_state));
    with_workspace_descriptor_root(root.as_deref(), action)
}

fn effective_package_for_path(path: &Path, workspace_state: &WorkspaceState) -> Option<Package> {
    let package_state = workspace_state.package_for_path(path)?;
    let config = workspace_state
        .effective_config_for_path(path)
        .unwrap_or_else(|| package_state.package.config.clone());
    Some(Package {
        root: package_state.package.root.clone(),
        config,
        package_manager_id: package_state.package.package_manager_id.clone(),
    })
}

fn resolve_source_doc_language(package: &Package, path: &Path, text: Option<&str>) -> Option<Lang> {
    if !path.starts_with(source_markdown_root(package).as_path()) {
        return None;
    }

    match text {
        Some(text) => discover_source_doc_lang_from_text(
            path,
            text,
            &package.config.label_overrides,
            package.config.check.implementation_section_only,
        ),
        None => discover_source_doc_lang(package, path),
    }
}

fn resolve_test_doc_language(
    package: &Package,
    path: &Path,
    text: Option<&str>,
    workspace_state: Option<&WorkspaceState>,
) -> Option<Lang> {
    if !path.starts_with(test_markdown_root(package).as_path()) {
        return None;
    }

    let workspace_state = workspace_state?;
    let package_state = workspace_state.package_for_path(path)?;
    let source_root = source_markdown_root(package);
    let source_docs = package_state
        .index
        .docs
        .values()
        .filter(|doc| doc.path.starts_with(source_root.as_path()))
        .filter(|doc| !is_overview_markdown_path(&doc.path))
        .cloned()
        .collect::<Vec<_>>();

    let disk_text;
    let text = match text {
        Some(text) => text,
        None => {
            disk_text = std::fs::read_to_string(path).ok()?;
            disk_text.as_str()
        }
    };

    let mut run_state = RunState::default();
    discover_test_doc_lang_from_text(package, path, text, &source_docs, &mut run_state)
}

fn resolve_active_language_impl(
    path: &Path,
    text: Option<&str>,
    workspace_state: Option<&WorkspaceState>,
) -> Option<Lang> {
    let package = workspace_state
        .and_then(|workspace_state| effective_package_for_path(path, workspace_state));
    with_descriptor_root_for_path(Some(path), workspace_state, || {
        let rejected_source_candidate = package.as_ref().is_some_and(|package| {
            path.starts_with(source_markdown_root(package).as_path())
                && text.is_some_and(|text| {
                    source_doc_lang_rejection_from_text(
                        path,
                        text,
                        &package.config.label_overrides,
                        package.config.check.implementation_section_only,
                    )
                    .is_some()
                })
        });

        lang_for_markdown_path(path)
            .or_else(|| {
                package
                    .as_ref()
                    .and_then(|package| resolve_source_doc_language(package, path, text))
            })
            .or_else(|| {
                package.as_ref().and_then(|package| {
                    resolve_test_doc_language(package, path, text, workspace_state)
                })
            })
            .or_else(|| {
                if rejected_source_candidate {
                    None
                } else {
                    Lang::from_path(path)
                }
            })
    })
}

pub fn resolve_active_language_for_text(
    path: &Path,
    text: &str,
    workspace_state: Option<&WorkspaceState>,
) -> Option<Lang> {
    resolve_active_language_impl(path, Some(text), workspace_state)
}

pub fn resolve_active_language(
    path: &Path,
    workspace_state: Option<&WorkspaceState>,
) -> Option<Lang> {
    let text =
        workspace_state.and_then(|workspace_state| workspace_state.open_file_text_for_path(path));
    resolve_active_language_impl(path, text, workspace_state)
}

fn matched_markdown_suffix_for_path(path: &Path, lang: &Lang) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    markdown_suffixes_for_lang(lang)
        .into_iter()
        .find(|suffix| name.ends_with(&format!(".{suffix}.md")))
        .map(|suffix| format!(".{suffix}.md"))
}

fn merge_config_for_workspace_state(
    config: &mut Config,
    path: &Path,
    workspace_state: &WorkspaceState,
    state: &mut RunState,
) -> Option<()> {
    if let Some(text) = workspace_state.open_file_text_for_path(path) {
        merge_config_text(config, path, text, state)
    } else if path.exists() {
        merge_config_file(config, path, state)
    } else {
        Some(())
    }
}

pub fn resolve_markdown_suffixes(
    path: &Path,
    workspace_state: Option<&WorkspaceState>,
) -> Vec<String> {
    let Some(lang) = resolve_active_language(path, workspace_state) else {
        return Vec::new();
    };
    with_descriptor_root_for_path(Some(path), workspace_state, || {
        let mut suffixes = Vec::new();

        if let Some(current_suffix) = matched_markdown_suffix_for_path(path, &lang) {
            suffixes.push(current_suffix);
        }

        for suffix in markdown_suffixes_for_lang(&lang) {
            let suffix = format!(".{suffix}.md");
            if !suffixes.contains(&suffix) {
                suffixes.push(suffix);
            }
        }

        if suffixes.is_empty() {
            if let Some(default_suffix) = markdown_suffix_for_lang(&lang) {
                suffixes.push(default_suffix);
            } else {
                suffixes.push(format!(".{}.md", lang.key()));
            }
        }

        suffixes
    })
}

pub fn resolve_markdown_suffix(
    path: &Path,
    workspace_state: Option<&WorkspaceState>,
) -> Option<String> {
    resolve_markdown_suffixes(path, workspace_state)
        .into_iter()
        .next()
}

pub fn resolve_fence_labels_for_path(
    path: &Path,
    workspace_state: Option<&WorkspaceState>,
) -> Vec<String> {
    let Some(lang) = resolve_active_language(path, workspace_state) else {
        return Vec::new();
    };
    with_descriptor_root_for_path(Some(path), workspace_state, || fence_labels_for_lang(&lang))
}

pub fn resolve_authoring_doc(
    path: &Path,
    workspace_state: &WorkspaceState,
) -> Option<ResolvedAuthoringDoc> {
    let package = effective_package_for_path(path, workspace_state)?;
    let open_text = workspace_state.open_file_text_for_path(path);
    let disk_text = if open_text.is_none() {
        std::fs::read_to_string(path).ok()
    } else {
        None
    };
    let resolution_text = open_text.or(disk_text.as_deref());
    let source_root = source_markdown_root(&package);
    let config = package.config.clone();

    with_descriptor_root_for_path(Some(path), Some(workspace_state), || {
        if is_discoverable_authoring_doc_path(&package, path) {
            return Some(ResolvedAuthoringDoc {
                config: config.clone(),
            });
        }

        if path.starts_with(source_root.as_path())
            && resolution_text.is_some_and(|text| {
                source_doc_lang_rejection_from_text(
                    path,
                    text,
                    &package.config.label_overrides,
                    package.config.check.implementation_section_only,
                )
                .is_some()
            })
        {
            return Some(ResolvedAuthoringDoc {
                config: config.clone(),
            });
        }

        resolution_text
            .filter(|_| path.starts_with(source_root.as_path()))
            .and_then(|text| resolve_active_language_impl(path, Some(text), Some(workspace_state)))
            .filter(|lang| package.config.adapters.get(lang).copied().unwrap_or(true))
            .map(|_| ResolvedAuthoringDoc {
                config: config.clone(),
            })
    })
}

#[allow(dead_code)]
impl WorkspaceState {
    pub fn package_for_path(&self, path: &Path) -> Option<&PackageState> {
        self.packages
            .iter()
            .filter(|pkg| path.starts_with(&pkg.package.root))
            .max_by_key(|pkg| pkg.package.root.components().count())
    }

    pub fn open_file_for_path(&self, path: &Path) -> Option<&OpenFile> {
        self.open_files
            .values()
            .find(|file| file.path.as_path() == path)
    }

    pub fn open_file_text_for_path(&self, path: &Path) -> Option<&str> {
        self.open_file_for_path(path).map(|file| file.text.as_str())
    }

    pub fn package_for_generated_path(&self, path: &Path) -> Option<&PackageState> {
        self.packages
            .iter()
            .find(|pkg| pkg.index.generated_files.contains(path))
    }

    pub fn effective_config_for_package(&self, package: &Package) -> Option<Config> {
        let fallback = package.config.clone();
        let mut config = Config::default();
        let mut state = RunState::default();
        let mut applied = false;

        let workspace_root = self
            .workspace_folders
            .iter()
            .filter(|folder| package.root.starts_with(folder.as_path()))
            .max_by_key(|folder| folder.as_path().components().count())
            .cloned()
            .unwrap_or_else(|| package.root.clone());
        let workspace_config_path = workspace_root.join("mds.config.toml");
        if workspace_config_path.exists()
            || self.open_file_for_path(&workspace_config_path).is_some()
        {
            if merge_config_for_workspace_state(
                &mut config,
                &workspace_config_path,
                self,
                &mut state,
            )
            .is_none()
            {
                return Some(fallback);
            }
            applied = true;
        }

        let package_config_path = package.root.join("mds.config.toml");
        if package_config_path != workspace_config_path
            && (package_config_path.exists()
                || self.open_file_for_path(&package_config_path).is_some())
        {
            if merge_config_for_workspace_state(&mut config, &package_config_path, self, &mut state)
                .is_none()
            {
                return Some(fallback);
            }
            applied = true;
        }

        if applied {
            Some(config)
        } else {
            Some(fallback)
        }
    }

    pub fn effective_config_for_path(&self, path: &Path) -> Option<Config> {
        let package_state = self.package_for_path(path)?;
        self.effective_config_for_package(&package_state.package)
    }
}

#[allow(dead_code)]
impl WorkspaceState {
    pub fn find_doc(&self, path: &Path) -> Option<&ImplDoc> {
        for pkg in &self.packages {
            if let Some(doc) = pkg.index.docs.get(path) {
                return Some(doc);
            }
        }
        None
    }
}

#[allow(dead_code)]
impl WorkspaceState {
    pub fn find_expose_locations(&self, name: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();
        for pkg in &self.packages {
            if let Some(paths) = pkg.index.expose_index.get(name) {
                results.extend(paths.iter().cloned());
            }
        }
        results
    }
}

#[allow(dead_code)]
impl WorkspaceState {
    pub fn find_module_locations(&self, module: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();
        for pkg in &self.packages {
            if let Some(paths) = pkg.index.module_index.get(module) {
                results.extend(paths.iter().cloned());
            }
        }
        results
    }
}

#[allow(dead_code)]
impl WorkspaceState {
    pub fn find_symbol_locations(&self, module: &str, symbol: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();
        for pkg in &self.packages {
            if let Some(paths) = pkg
                .index
                .symbol_index
                .get(&(module.to_string(), symbol.to_string()))
            {
                results.extend(paths.iter().cloned());
            }
        }
        results
    }
}

fn source_map_line(line: u32) -> usize {
    line as usize + 1
}

fn lsp_line(line: usize) -> u32 {
    line.saturating_sub(1) as u32
}

fn collapsed_range(position: Position) -> Range {
    Range {
        start: position,
        end: position,
    }
}

fn file_location(path: &Path, range: Range) -> Option<Location> {
    Some(Location {
        uri: Url::from_file_path(path).ok()?,
        range,
    })
}

fn generated_line_for_markdown(span: &SourceSpan, line: usize) -> Option<usize> {
    span.contains_markdown_line(line)
        .then_some(span.generated_start_line + (line - span.markdown_start_line))
}

#[allow(dead_code)]
impl PackageState {
    pub fn contains_generated_path(&self, path: &Path) -> bool {
        self.index.generated_files.contains(path)
    }

    pub fn remap_generated_position(&self, path: &Path, position: Position) -> Option<Location> {
        let line = source_map_line(position.line);
        let span = self.index.source_map.find_generated(path, line)?;
        let markdown_line = span.markdown_line_for_generated(line)?;
        file_location(
            &span.markdown_path,
            collapsed_range(Position {
                line: lsp_line(markdown_line),
                character: position.character,
            }),
        )
    }

    pub fn remap_generated_range(&self, path: &Path, range: Range) -> Option<Location> {
        let start_line = source_map_line(range.start.line);
        let end_line = source_map_line(range.end.line);
        let start_span = self.index.source_map.find_generated(path, start_line)?;
        let end_span = self.index.source_map.find_generated(path, end_line)?;
        if start_span != end_span {
            return None;
        }

        file_location(
            &start_span.markdown_path,
            Range {
                start: Position {
                    line: lsp_line(start_span.markdown_line_for_generated(start_line)?),
                    character: range.start.character,
                },
                end: Position {
                    line: lsp_line(end_span.markdown_line_for_generated(end_line)?),
                    character: range.end.character,
                },
            },
        )
    }

    pub fn remap_generated_location(&self, location: &Location) -> Option<Location> {
        let path = location.uri.to_file_path().ok()?;
        self.remap_generated_range(&path, location.range)
    }

    pub fn resolve_generated_position(&self, path: &Path, position: Position) -> Option<Location> {
        let line = source_map_line(position.line);
        let span = self.index.source_map.find_markdown(path, line)?;
        let generated_line = generated_line_for_markdown(span, line)?;
        file_location(
            &span.generated_path,
            collapsed_range(Position {
                line: lsp_line(generated_line),
                character: position.character,
            }),
        )
    }
}

#[allow(dead_code)]
impl WorkspaceState {
    pub fn remap_generated_position(&self, uri: &Url, position: Position) -> Option<Location> {
        let path = uri.to_file_path().ok()?;
        self.package_for_generated_path(&path)?
            .remap_generated_position(&path, position)
    }

    pub fn remap_generated_range(&self, uri: &Url, range: Range) -> Option<Location> {
        let path = uri.to_file_path().ok()?;
        self.package_for_generated_path(&path)?
            .remap_generated_range(&path, range)
    }

    pub fn remap_generated_location(&self, location: &Location) -> Option<Location> {
        let path = match location.uri.to_file_path() {
            Ok(path) => path,
            Err(_) => return Some(location.clone()),
        };
        match self.package_for_generated_path(&path) {
            Some(package) => package.remap_generated_location(location),
            None => Some(location.clone()),
        }
    }

    pub fn remap_generated_locations(&self, locations: &[Location]) -> Vec<Option<Location>> {
        locations
            .iter()
            .map(|location| self.remap_generated_location(location))
            .collect()
    }

    pub fn remap_generated_text_edits(
        &self,
        uri: &Url,
        edits: &[TextEdit],
    ) -> Option<(Url, Vec<TextEdit>)> {
        if edits.is_empty() {
            return None;
        }

        let path = match uri.to_file_path() {
            Ok(path) => path,
            Err(_) => return Some((uri.clone(), edits.to_vec())),
        };
        let Some(package) = self.package_for_generated_path(&path) else {
            return Some((uri.clone(), edits.to_vec()));
        };
        let mut markdown_uri = None;
        let mut markdown_edits = Vec::with_capacity(edits.len());

        for edit in edits {
            let remapped = package.remap_generated_range(&path, edit.range)?;
            match &markdown_uri {
                Some(existing_uri) if *existing_uri != remapped.uri => return None,
                None => markdown_uri = Some(remapped.uri.clone()),
                _ => {}
            }
            markdown_edits.push(TextEdit {
                range: remapped.range,
                new_text: edit.new_text.clone(),
            });
        }

        Some((markdown_uri?, markdown_edits))
    }

    pub fn resolve_generated_position(
        &self,
        markdown_uri: &Url,
        position: Position,
    ) -> Option<Location> {
        let path = markdown_uri.to_file_path().ok()?;
        self.package_for_path(&path)?
            .resolve_generated_position(&path, position)
    }
}
