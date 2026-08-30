use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use eframe::egui;
use il2cpp_core::analysis::{Il2CppProject, TypeResolver};
use il2cpp_export::{DumpCsExporter, DumpCsOptions, ExportContext, Exporter};

use crate::actions::{self, TargetFile};
use crate::state::{LoadState, MethodTab, ProjectData, SearchMatch, SearchResult, search};
use crate::views::explorer::{self, ExplorerAction};
use crate::views::welcome::{self, WelcomeAction};

pub struct Il2CppExplorerApp {
    load_state: LoadState,
    binary_path: Option<PathBuf>,
    metadata_path: Option<PathBuf>,
    selected_type: Option<il2cpp_core::model::TypeId>,
    selected_method: Option<il2cpp_core::model::MethodId>,
    search_query: String,
    search_results: Vec<SearchMatch>,
    search_limited: bool,
    active_method_tab: MethodTab,
    export_status: Option<String>,
}

impl Default for Il2CppExplorerApp {
    fn default() -> Self {
        Self {
            load_state: LoadState::Empty,
            binary_path: None,
            metadata_path: None,
            selected_type: None,
            selected_method: None,
            search_query: String::new(),
            search_results: Vec::new(),
            search_limited: false,
            active_method_tab: MethodTab::Overview,
            export_status: None,
        }
    }
}

impl Il2CppExplorerApp {
    fn start_load(&mut self, binary: PathBuf, metadata: PathBuf) {
        self.selected_type = None;
        self.selected_method = None;
        self.search_query.clear();
        self.search_results.clear();
        let (sender, receiver) = mpsc::channel();
        let worker_binary = binary.clone();
        let worker_metadata = metadata.clone();
        std::thread::spawn(move || {
            let result = load_project(&worker_binary, &worker_metadata)
                .map(|project| ProjectData::new(project, worker_binary, worker_metadata));
            let _ = sender.send(result);
        });
        self.load_state = LoadState::Loading {
            receiver,
            binary,
            metadata,
        };
    }

    fn poll_load(&mut self) {
        let completed = match &self.load_state {
            LoadState::Loading {
                receiver,
                binary,
                metadata,
            } => receiver
                .try_recv()
                .ok()
                .map(|result| (result, binary.clone(), metadata.clone())),
            _ => None,
        };
        if let Some((result, binary, metadata)) = completed {
            self.load_state = match result {
                Ok(data) => {
                    self.binary_path = Some(data.binary_path.clone());
                    self.metadata_path = Some(data.metadata_path.clone());
                    LoadState::Loaded(data)
                }
                Err(message) => LoadState::Failed {
                    message,
                    binary,
                    metadata,
                },
            };
        }
    }

    fn update_search(&mut self) {
        let LoadState::Loaded(data) = &self.load_state else {
            return;
        };
        let (results, limited) = search(&data.search_entries, &self.search_query);
        self.search_results = results;
        self.search_limited = limited;
    }

    fn select(&mut self, result: SearchResult) {
        match result {
            SearchResult::Type(type_id) => {
                self.selected_type = Some(type_id);
                self.selected_method = None;
            }
            SearchResult::Method(method_id) => {
                let data = match &self.load_state {
                    LoadState::Loaded(data) => data,
                    _ => return,
                };
                self.selected_type =
                    Some(data.project.metadata().methods[method_id.0].declaring_type);
                self.selected_method = Some(method_id);
            }
            SearchResult::Field(field_id) => {
                let data = match &self.load_state {
                    LoadState::Loaded(data) => data,
                    _ => return,
                };
                self.selected_type =
                    Some(data.project.metadata().fields[field_id.0].declaring_type);
                self.selected_method = None;
            }
        }
        self.search_query.clear();
        self.search_results.clear();
    }

    fn handle_drops(&mut self, context: &egui::Context) {
        for file in context.input(|input| input.raw.dropped_files.clone()) {
            let Some(path) = file.path else {
                continue;
            };
            match actions::dropped_target(&path) {
                Some(TargetFile::Binary(path)) => self.binary_path = Some(path),
                Some(TargetFile::Metadata(path)) => self.metadata_path = Some(path),
                None => {}
            }
        }
    }

    fn export_dump(&mut self) {
        let LoadState::Loaded(data) = &self.load_state else {
            return;
        };
        let Some(path) = actions::select_dump_destination() else {
            return;
        };
        let project = data.project.clone();
        self.export_status = Some(format!("Exporting {}", path.display()));
        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let resolver = project.runtime_metadata().map_or_else(
                    || TypeResolver::metadata_only(project.metadata()),
                    |runtime| {
                        TypeResolver::with_runtime(project.metadata(), project.binary(), runtime)
                    },
                );
                let context = ExportContext {
                    metadata: project.metadata(),
                    types: &resolver,
                    native_methods: project.native_methods(),
                };
                let mut file = std::fs::File::create(&path).map_err(|error| error.to_string())?;
                DumpCsExporter::new(DumpCsOptions {
                    include_file_offsets: true,
                    ..DumpCsOptions::default()
                })
                .export(&context, &mut file)
                .map_err(|error| error.to_string())?;
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("dump.cs export failed: {error}");
            }
        });
    }
}

fn load_project(binary: &Path, metadata: &Path) -> Result<Arc<Il2CppProject>, String> {
    let mut project = Il2CppProject::load(binary, metadata).map_err(|error| error.to_string())?;
    project
        .prepare_analysis()
        .map_err(|error| error.to_string())?;
    Ok(Arc::new(project))
}

impl eframe::App for Il2CppExplorerApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_load();
        self.handle_drops(context);
        if context.input(|input| input.modifiers.command && input.key_pressed(egui::Key::O)) {
            if let Some(path) = actions::select_binary() {
                self.binary_path = Some(path);
            }
        }
        if context.input(|input| input.modifiers.command && input.key_pressed(egui::Key::F)) {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("global_search")));
        }
        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.search_query.clear();
            self.search_results.clear();
        }
        if let LoadState::Failed {
            message,
            binary,
            metadata,
        } = &self.load_state
        {
            let action = welcome::failed(context, message);
            let paths = (binary.clone(), metadata.clone());
            if let Some(retry) = action {
                if retry {
                    self.start_load(paths.0, paths.1);
                } else {
                    self.load_state = LoadState::Empty;
                }
            }
            return;
        }
        match &mut self.load_state {
            LoadState::Empty => {
                context
                    .send_viewport_cmd(egui::ViewportCommand::Title("IL2CPP Explorer".to_owned()));
                if let Some(action) = welcome::show(
                    context,
                    self.binary_path.as_deref(),
                    self.metadata_path.as_deref(),
                    Path::new("libil2cpp.so").is_file()
                        && Path::new("global-metadata.dat").is_file(),
                ) {
                    match action {
                        WelcomeAction::SelectBinary => self.binary_path = actions::select_binary(),
                        WelcomeAction::SelectMetadata => {
                            self.metadata_path = actions::select_metadata()
                        }
                        WelcomeAction::Analyze => {
                            if let (Some(binary), Some(metadata)) =
                                (self.binary_path.clone(), self.metadata_path.clone())
                            {
                                self.start_load(binary, metadata);
                            }
                        }
                        WelcomeAction::OpenLocal => self.start_load(
                            PathBuf::from("libil2cpp.so"),
                            PathBuf::from("global-metadata.dat"),
                        ),
                    }
                }
            }
            LoadState::Loading { .. } => {
                welcome::loading(context);
                context.request_repaint_after(std::time::Duration::from_millis(50));
            }
            LoadState::Failed { .. } => unreachable!("failed state is handled above"),
            LoadState::Loaded(data) => {
                let title = data
                    .binary_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map_or_else(
                        || "IL2CPP Explorer".to_owned(),
                        |name| format!("IL2CPP Explorer — {name}"),
                    );
                context.send_viewport_cmd(egui::ViewportCommand::Title(title));
                let old_query = self.search_query.clone();
                if let Some(action) = explorer::show(
                    context,
                    data,
                    explorer::ExplorerState {
                        selected_type: &mut self.selected_type,
                        selected_method: &mut self.selected_method,
                        search_query: &mut self.search_query,
                        search_results: &self.search_results,
                        search_limited: self.search_limited,
                        tab: &mut self.active_method_tab,
                        export_status: self.export_status.as_deref(),
                    },
                ) {
                    match action {
                        ExplorerAction::Open => self.load_state = LoadState::Empty,
                        ExplorerAction::Export => self.export_dump(),
                        ExplorerAction::Select(result) => self.select(result),
                    }
                }
                if self.search_query != old_query {
                    self.update_search();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires local proprietary IL2CPP target files"]
    fn worker_loads_local_target_with_native_mappings() {
        let project = load_project(
            Path::new("../../libil2cpp.so"),
            Path::new("../../global-metadata.dat"),
        )
        .expect("local target loads");
        let data = ProjectData::new(
            project,
            PathBuf::from("../../libil2cpp.so"),
            PathBuf::from("../../global-metadata.dat"),
        );
        assert!(!data.project.metadata().assemblies.is_empty());
        assert!(!data.search_entries.is_empty());
        assert!(
            data.project
                .native_methods()
                .is_some_and(|index| index.mapped_method_count() > 0)
        );
    }
}
