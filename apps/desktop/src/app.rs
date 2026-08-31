use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use eframe::egui;
use il2cpp_core::analysis::{Il2CppProject, TypeResolver};
use il2cpp_diff::{DiffEngine, DiffOptions, DiffStatus, ProjectDiff};
use il2cpp_export::{DumpCsExporter, DumpCsOptions, ExportContext, Exporter};

use crate::actions::{self, TargetFile};
use crate::history::NavigationHistory;
use crate::navigation::{AddressTarget, NavigationTarget, TabState, parse_address};
use crate::recent::RecentProjects;
use crate::state::{LoadState, MethodTab, ProjectData, SearchMatch, SearchResult, search};
use crate::views::explorer::{self, ExplorerAction};
use crate::views::strings_view::{self, StringFilter};
use crate::views::welcome::{self, WelcomeAction};

pub struct Il2CppExplorerApp {
    load_state: LoadState,
    binary_path: Option<PathBuf>,
    metadata_path: Option<PathBuf>,
    selected_type: Option<il2cpp_core::model::TypeId>,
    selected_method: Option<il2cpp_core::model::MethodId>,
    tree_focus: Option<il2cpp_core::model::TypeId>,
    search_query: String,
    search_results: Vec<SearchMatch>,
    search_limited: bool,
    active_method_tab: MethodTab,
    export_status: Option<String>,
    history: NavigationHistory,
    tabs: TabState,
    selected_address: Option<AddressTarget>,
    address_input: String,
    address_error: Option<String>,
    show_go_to: bool,
    address_kind: AddressKind,
    recent: RecentProjects,
    tree_filter: String,
    member_filter: String,
    string_query: String,
    string_filter: StringFilter,
    selected_string: Option<il2cpp_core::model::StringLiteralId>,
    pending_restore: Option<StableTarget>,
    mode: MainMode,
    compare: CompareState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MainMode {
    Explorer,
    Strings,
    Compare,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum AddressKind {
    Rva,
    Va,
    FileOffset,
}
enum StableTarget {
    Type {
        namespace: String,
        name: String,
    },
    Method {
        namespace: String,
        type_name: String,
        name: String,
        parameters: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompareFilter {
    All,
    Added,
    Removed,
    Changed,
    Moved,
}

struct CompareState {
    old_binary: Option<PathBuf>,
    old_metadata: Option<PathBuf>,
    new_binary: Option<PathBuf>,
    new_metadata: Option<PathBuf>,
    receiver: Option<mpsc::Receiver<Result<ProjectDiff, String>>>,
    report: Option<ProjectDiff>,
    error: Option<String>,
    search: String,
    filter: CompareFilter,
    selected_type: Option<usize>,
}

impl Default for CompareState {
    fn default() -> Self {
        Self {
            old_binary: None,
            old_metadata: None,
            new_binary: None,
            new_metadata: None,
            receiver: None,
            report: None,
            error: None,
            search: String::new(),
            filter: CompareFilter::All,
            selected_type: None,
        }
    }
}

impl Default for Il2CppExplorerApp {
    fn default() -> Self {
        Self {
            load_state: LoadState::Empty,
            binary_path: None,
            metadata_path: None,
            selected_type: None,
            selected_method: None,
            tree_focus: None,
            search_query: String::new(),
            search_results: Vec::new(),
            search_limited: false,
            active_method_tab: MethodTab::Overview,
            export_status: None,
            history: NavigationHistory::default(),
            tabs: TabState::default(),
            selected_address: None,
            address_input: String::new(),
            address_error: None,
            show_go_to: false,
            address_kind: AddressKind::Rva,
            recent: RecentProjects::load(),
            tree_filter: String::new(),
            member_filter: String::new(),
            string_query: String::new(),
            string_filter: StringFilter::All,
            selected_string: None,
            pending_restore: None,
            mode: MainMode::Explorer,
            compare: CompareState::default(),
        }
    }
}

impl Il2CppExplorerApp {
    fn navigate_to(&mut self, target: NavigationTarget) {
        if self.history.navigate(target) {
            self.apply_navigation(target);
        }
    }
    fn apply_navigation(&mut self, target: NavigationTarget) {
        let title = self.navigation_title(target);
        self.tabs.replace_active(target, title);
        self.apply_selection(target);
    }
    fn apply_selection(&mut self, target: NavigationTarget) {
        self.selected_address = None;
        match target {
            NavigationTarget::ProjectOverview | NavigationTarget::Assembly(_) => {
                self.selected_type = None;
                self.selected_method = None;
                self.tree_focus = None;
            }
            NavigationTarget::Type(id) => {
                self.selected_type = Some(id);
                self.selected_method = None;
                self.tree_focus = Some(id);
            }
            NavigationTarget::Method(id) => {
                if let LoadState::Loaded(data) = &self.load_state {
                    if let Some(method) = data.project.metadata().methods.get(id.0) {
                        self.selected_type = Some(method.declaring_type);
                        self.selected_method = Some(id);
                        self.tree_focus = self.selected_type;
                    }
                }
            }
            NavigationTarget::Field(id) => {
                if let LoadState::Loaded(data) = &self.load_state {
                    if let Some(field) = data.project.metadata().fields.get(id.0) {
                        self.selected_type = Some(field.declaring_type);
                        self.selected_method = None;
                        self.tree_focus = self.selected_type;
                    }
                }
            }
            NavigationTarget::Property(id) => {
                if let LoadState::Loaded(data) = &self.load_state {
                    if let Some(property) = data.project.metadata().properties.get(id.0) {
                        self.selected_type = Some(property.declaring_type);
                        self.selected_method = None;
                        self.tree_focus = self.selected_type;
                    }
                }
            }
            NavigationTarget::Address(address) => {
                self.selected_type = None;
                self.selected_method = None;
                self.tree_focus = None;
                self.selected_address = Some(address);
            }
            NavigationTarget::StringLiteral(id) => {
                self.mode = MainMode::Strings;
                self.selected_string = Some(id);
            }
        }
    }
    fn navigation_title(&self, target: NavigationTarget) -> String {
        match target {
            NavigationTarget::ProjectOverview => "Project".into(),
            NavigationTarget::Assembly(id) => self
                .loaded()
                .and_then(|data| data.project.metadata().assemblies.get(id.0))
                .map_or_else(|| "Assembly".into(), |item| item.name.clone()),
            NavigationTarget::Type(id) => self.loaded().map_or_else(
                || "Type".into(),
                |data| crate::state::type_name(&data.project, id),
            ),
            NavigationTarget::Method(id) => self
                .loaded()
                .and_then(|data| data.project.metadata().methods.get(id.0))
                .map_or_else(|| "Method".into(), |item| item.name.clone()),
            NavigationTarget::Field(id) => self
                .loaded()
                .and_then(|data| data.project.metadata().fields.get(id.0))
                .map_or_else(|| "Field".into(), |item| item.name.clone()),
            NavigationTarget::Property(id) => self
                .loaded()
                .and_then(|data| data.project.metadata().properties.get(id.0))
                .map_or_else(|| "Property".into(), |item| item.name.clone()),
            NavigationTarget::Address(value) => format!("{value:?}"),
            NavigationTarget::StringLiteral(id) => format!("String #{}", id.0),
        }
    }
    fn loaded(&self) -> Option<&ProjectData> {
        if let LoadState::Loaded(data) = &self.load_state {
            Some(data)
        } else {
            None
        }
    }
    fn start_load(&mut self, binary: PathBuf, metadata: PathBuf) {
        self.pending_restore = self.stable_target();
        self.selected_type = None;
        self.selected_method = None;
        self.tree_focus = None;
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
            let loaded = result.is_ok();
            let recent_paths = (binary.clone(), metadata.clone());
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
            if loaded {
                self.recent.remember(recent_paths.0, recent_paths.1);
                self.history = NavigationHistory::default();
                self.tabs = TabState::default();
                let target = self
                    .pending_restore
                    .take()
                    .and_then(|target| self.restore_target(target))
                    .unwrap_or(NavigationTarget::ProjectOverview);
                self.navigate_to(target);
            }
        }
    }

    fn update_search(&mut self) {
        let LoadState::Loaded(data) = &self.load_state else {
            return;
        };
        if let Some(value) = self.search_query.strip_prefix("addr:") {
            if let Ok(address) = parse_address(value) {
                self.search_results = vec![SearchMatch {
                    result: SearchResult::Address(address),
                    label: format!("RVA 0x{address:08X}"),
                    kind: "Address",
                    searchable: String::new(),
                }];
                self.search_limited = false;
                return;
            }
        }
        let (results, limited) = search(&data.search_entries, &self.search_query);
        self.search_results = results;
        self.search_limited = limited;
        if let Some(value) = self.search_query.strip_prefix("addr:") {
            if let Ok(address) = parse_address(value) {
                self.search_results = vec![SearchMatch {
                    result: SearchResult::Address(address),
                    label: format!("RVA 0x{address:08X}"),
                    kind: "Address",
                    searchable: String::new(),
                }];
                self.search_limited = false;
                return;
            }
        }
        let (results, limited) = search(&data.search_entries, &self.search_query);
        self.search_results = results;
        self.search_limited = limited;
    }

    fn select(&mut self, result: SearchResult) {
        let target = match result {
            SearchResult::StringLiteral(id) => NavigationTarget::StringLiteral(id),
            SearchResult::Address(value) => NavigationTarget::Address(AddressTarget::Rva(value)),
            SearchResult::Assembly(id) => NavigationTarget::Assembly(id),
            SearchResult::Type(id) => NavigationTarget::Type(id),
            SearchResult::Method(id) => NavigationTarget::Method(id),
            SearchResult::Field(id) => NavigationTarget::Field(id),
        };
        self.navigate_to(target);
        self.search_query.clear();
        self.search_results.clear();
    }

    fn stable_target(&self) -> Option<StableTarget> {
        let data = self.loaded()?;
        if let Some(method) = self
            .selected_method
            .and_then(|id| data.project.metadata().methods.get(id.0))
        {
            let ty = &data.project.metadata().types[method.declaring_type.0];
            return Some(StableTarget::Method {
                namespace: ty.namespace.clone(),
                type_name: ty.name.clone(),
                name: method.name.clone(),
                parameters: method.parameters.len(),
            });
        }
        self.selected_type
            .and_then(|id| data.project.metadata().types.get(id.0))
            .map(|ty| StableTarget::Type {
                namespace: ty.namespace.clone(),
                name: ty.name.clone(),
            })
    }
    fn restore_target(&self, target: StableTarget) -> Option<NavigationTarget> {
        let data = self.loaded()?;
        match target {
            StableTarget::Type { namespace, name } => data
                .project
                .metadata()
                .types
                .iter()
                .find(|ty| ty.namespace == namespace && ty.name == name)
                .map(|ty| NavigationTarget::Type(ty.id)),
            StableTarget::Method {
                namespace,
                type_name,
                name,
                parameters,
            } => data
                .project
                .metadata()
                .methods
                .iter()
                .find(|method| {
                    let ty = &data.project.metadata().types[method.declaring_type.0];
                    ty.namespace == namespace
                        && ty.name == type_name
                        && method.name == name
                        && method.parameters.len() == parameters
                })
                .map(|method| NavigationTarget::Method(method.id)),
        }
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

    fn start_compare(&mut self) {
        let old_binary = self.compare.old_binary.clone();
        let new_binary = self.compare.new_binary.clone();
        let (Some(old_metadata), Some(new_metadata)) = (
            self.compare.old_metadata.clone(),
            self.compare.new_metadata.clone(),
        ) else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<ProjectDiff, String> {
                let mut old = old_binary
                    .as_ref()
                    .map_or_else(
                        || Il2CppProject::load_metadata_only(&old_metadata),
                        |binary| Il2CppProject::load(binary, &old_metadata),
                    )
                    .map_err(|error| error.to_string())?;
                let mut new = new_binary
                    .as_ref()
                    .map_or_else(
                        || Il2CppProject::load_metadata_only(&new_metadata),
                        |binary| Il2CppProject::load(binary, &new_metadata),
                    )
                    .map_err(|error| error.to_string())?;
                let _ = old.prepare_analysis();
                let _ = new.prepare_analysis();
                DiffEngine::new(&old, &new)
                    .with_options(DiffOptions::default())
                    .compare()
                    .map_err(|error| error.to_string())
            })();
            let _ = sender.send(result);
        });
        self.compare.receiver = Some(receiver);
        self.compare.report = None;
        self.compare.error = None;
        self.compare.selected_type = None;
    }

    fn poll_compare(&mut self) {
        let result = self
            .compare
            .receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        if let Some(result) = result {
            self.compare.receiver = None;
            match result {
                Ok(report) => self.compare.report = Some(report),
                Err(error) => self.compare.error = Some(error),
            }
        }
    }

    fn show_go_to_address(&mut self, context: &egui::Context) {
        if !self.show_go_to {
            return;
        }
        let mut open = true;
        egui::Window::new("Go to Address")
            .open(&mut open)
            .collapsible(false)
            .show(context, |ui| {
                ui.label("Address");
                ui.text_edit_singleline(&mut self.address_input);
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.address_kind, AddressKind::Rva, "RVA");
                    ui.radio_value(&mut self.address_kind, AddressKind::Va, "VA");
                    ui.radio_value(
                        &mut self.address_kind,
                        AddressKind::FileOffset,
                        "File Offset",
                    );
                });
                if let Some(error) = &self.address_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                if ui.button("Go").clicked() {
                    match parse_address(&self.address_input) {
                        Ok(value) => {
                            let address = match self.address_kind {
                                AddressKind::Rva => AddressTarget::Rva(value),
                                AddressKind::Va => AddressTarget::Va(value),
                                AddressKind::FileOffset => AddressTarget::FileOffset(value),
                            };
                            self.navigate_to(NavigationTarget::Address(address));
                            self.show_go_to = false;
                            self.address_error = None;
                        }
                        Err(error) => self.address_error = Some(error.to_owned()),
                    }
                }
            });
        self.show_go_to = open;
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
        self.poll_compare();
        egui::TopBottomPanel::top("main_navigation").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, MainMode::Explorer, "Explorer");
                ui.selectable_value(&mut self.mode, MainMode::Strings, "Strings");
                ui.selectable_value(&mut self.mode, MainMode::Compare, "Compare Builds");
            });
        });
        if self.mode == MainMode::Compare {
            self.show_compare(context);
            return;
        }
        if self.mode == MainMode::Strings {
            self.show_strings(context);
            return;
        }
        self.handle_drops(context);
        if context.input(|input| input.modifiers.command && input.key_pressed(egui::Key::O)) {
            if let Some(path) = actions::select_binary() {
                self.binary_path = Some(path);
            }
        }
        if context.input(|input| input.modifiers.command && input.key_pressed(egui::Key::F)) {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("global_search")));
        }
        if context.input(|input| input.modifiers.command && input.key_pressed(egui::Key::G)) {
            self.show_go_to = true;
            self.address_error = None;
        }
        if context.input(|input| input.modifiers.alt && input.key_pressed(egui::Key::ArrowLeft)) {
            if let Some(target) = self.history.back() {
                self.apply_navigation(target);
            }
        }
        if context.input(|input| input.modifiers.alt && input.key_pressed(egui::Key::ArrowRight)) {
            if let Some(target) = self.history.forward() {
                self.apply_navigation(target);
            }
        }
        if context.input(|input| input.modifiers.command && input.key_pressed(egui::Key::W)) {
            if let Some(index) = self.tabs.active {
                self.tabs.close(index);
                if let Some(tab) = self.tabs.active.and_then(|index| self.tabs.tabs.get(index)) {
                    self.apply_selection(tab.target);
                } else {
                    self.navigate_to(NavigationTarget::ProjectOverview);
                }
            }
        }
        if context.input(|input| input.modifiers.command && input.key_pressed(egui::Key::R)) {
            if let (Some(binary), Some(metadata)) =
                (self.binary_path.clone(), self.metadata_path.clone())
            {
                self.start_load(binary, metadata);
            }
        }
        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.search_query.clear();
            self.search_results.clear();
        }
        self.show_go_to_address(context);
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
        if self.loaded().is_some() {
            let mut activate_tab = None;
            egui::TopBottomPanel::top("explorer_tabs").show(context, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(self.history.can_back(), egui::Button::new("←"))
                        .on_hover_text("Back (Alt+Left)")
                        .clicked()
                    {
                        activate_tab = self
                            .history
                            .back()
                            .map(|target| (self.tabs.active.unwrap_or(0), target));
                    }
                    if ui
                        .add_enabled(self.history.can_forward(), egui::Button::new("→"))
                        .on_hover_text("Forward (Alt+Right)")
                        .clicked()
                    {
                        activate_tab = self
                            .history
                            .forward()
                            .map(|target| (self.tabs.active.unwrap_or(0), target));
                    }
                    for (index, tab) in self.tabs.tabs.iter().enumerate() {
                        if ui
                            .selectable_label(self.tabs.active == Some(index), &tab.title)
                            .clicked()
                        {
                            activate_tab = Some((index, tab.target));
                        }
                    }
                });
            });
            if let Some((index, target)) = activate_tab {
                self.tabs.active = Some(index);
                self.apply_selection(target);
            }
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
                    &self.recent.projects,
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
                        WelcomeAction::Recent(binary, metadata) => {
                            self.start_load(binary, metadata)
                        }
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
                        tree_focus: &mut self.tree_focus,
                        export_status: self.export_status.as_deref(),
                        address: self.selected_address,
                        tree_filter: &mut self.tree_filter,
                        member_filter: &mut self.member_filter,
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

impl Il2CppExplorerApp {
    fn show_strings(&mut self, context: &egui::Context) {
        let LoadState::Loaded(data) = &self.load_state else {
            egui::CentralPanel::default().show(context, |ui| {
                ui.heading("String Literals");
                ui.weak("Open a project first.");
            });
            return;
        };
        egui::CentralPanel::default().show(context, |ui| {
            strings_view::show(
                ui,
                data,
                &mut self.string_query,
                &mut self.string_filter,
                &mut self.selected_string,
            )
        });
    }
    fn show_compare(&mut self, context: &egui::Context) {
        let comparing = self.compare.receiver.is_some();
        if self.compare.report.is_none() {
            egui::CentralPanel::default().show(context, |ui| {
                ui.heading("Compare Builds");
                ui.weak("Managed identity first. Native RVA movement is not code change.");
                ui.add_space(12.0);
                path_picker(
                    ui,
                    "Old Build",
                    "libil2cpp.so",
                    &mut self.compare.old_binary,
                    actions::select_binary,
                );
                path_picker(
                    ui,
                    "",
                    "global-metadata.dat",
                    &mut self.compare.old_metadata,
                    actions::select_metadata,
                );
                ui.add_space(8.0);
                path_picker(
                    ui,
                    "New Build",
                    "libil2cpp.so",
                    &mut self.compare.new_binary,
                    actions::select_binary,
                );
                path_picker(
                    ui,
                    "",
                    "global-metadata.dat",
                    &mut self.compare.new_metadata,
                    actions::select_metadata,
                );
                ui.add_space(10.0);
                if ui
                    .add_enabled(
                        !comparing
                            && self.compare.old_metadata.is_some()
                            && self.compare.new_metadata.is_some(),
                        egui::Button::new("Compare"),
                    )
                    .clicked()
                {
                    self.start_compare();
                }
                if comparing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Comparing builds...");
                    });
                    context.request_repaint_after(std::time::Duration::from_millis(50));
                }
                if let Some(error) = &self.compare.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            });
            return;
        }
        let report = self.compare.report.as_ref().expect("report was checked");
        egui::TopBottomPanel::bottom("diff_summary").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.monospace(format!(
                    "Types: +{} -{} ~{}",
                    report.summary.types_added,
                    report.summary.types_removed,
                    report.summary.types_changed
                ));
                ui.separator();
                ui.monospace(format!(
                    "Methods: +{} -{} ~{} >{}",
                    report.summary.methods_added,
                    report.summary.methods_removed,
                    report.summary.methods_changed,
                    report.summary.methods_moved
                ));
                if !report.native_available {
                    ui.separator();
                    ui.weak("Native comparison unavailable");
                }
            });
        });
        egui::SidePanel::left("diff_changes")
            .resizable(true)
            .default_width(320.0)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Changes");
                });
                ui.text_edit_singleline(&mut self.compare.search);
                ui.horizontal_wrapped(|ui| {
                    for (filter, label) in [
                        (CompareFilter::All, "All"),
                        (CompareFilter::Added, "Added"),
                        (CompareFilter::Removed, "Removed"),
                        (CompareFilter::Changed, "Changed"),
                        (CompareFilter::Moved, "Moved"),
                    ] {
                        ui.selectable_value(&mut self.compare.filter, filter, label);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (index, item) in report.types.iter().enumerate().filter(|(_, item)| {
                        diff_type_visible(item, &self.compare.search, self.compare.filter)
                    }) {
                        if ui
                            .selectable_label(
                                self.compare.selected_type == Some(index),
                                format!("{} {}", item.status.marker(), item.identity),
                            )
                            .clicked()
                        {
                            self.compare.selected_type = Some(index);
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(context, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let Some(index) = self.compare.selected_type else {
                    ui.heading("Select changed type");
                    return;
                };
                let item = &report.types[index];
                ui.heading(item.identity.to_string());
                ui.label(format!("Status: {:?}", item.status));
                ui.columns(2, |columns| {
                    columns[0].strong("Old");
                    columns[0].label(
                        item.old_base
                            .as_ref()
                            .map_or_else(|| "Base: —".to_owned(), |value| format!("Base: {value}")),
                    );
                    columns[1].strong("New");
                    columns[1].label(
                        item.new_base
                            .as_ref()
                            .map_or_else(|| "Base: —".to_owned(), |value| format!("Base: {value}")),
                    );
                });
                show_diff_fields(
                    ui,
                    "Fields",
                    item.fields
                        .iter()
                        .filter(|value| value.status.is_changed())
                        .map(|value| {
                            format!(
                                "{} {}  {} → {}",
                                value.status.marker(),
                                value.name,
                                value
                                    .old_offset
                                    .map_or_else(|| "—".to_owned(), |v| format!("{v:#x}")),
                                value
                                    .new_offset
                                    .map_or_else(|| "—".to_owned(), |v| format!("{v:#x}"))
                            )
                        }),
                );
                show_diff_fields(
                    ui,
                    "Properties",
                    item.properties
                        .iter()
                        .filter(|value| value.status.is_changed())
                        .map(|value| format!("{} {}", value.status.marker(), value.name)),
                );
                ui.separator();
                ui.strong("Methods");
                for method in item
                    .methods
                    .iter()
                    .filter(|value| value.status.is_changed())
                {
                    ui.group(|ui| {
                        ui.label(format!("{} {}", method.status.marker(), method.identity));
                        ui.label(format!(
                            "Old RVA: {}",
                            method
                                .old_rva
                                .map_or_else(|| "—".to_owned(), |v| format!("{v:#010X}"))
                        ));
                        ui.label(format!(
                            "New RVA: {}",
                            method
                                .new_rva
                                .map_or_else(|| "—".to_owned(), |v| format!("{v:#010X}"))
                        ));
                        if let Some(native) = &method.native {
                            if native.equivalent == Some(true) {
                                ui.label("Native body: Equivalent");
                            }
                            if let Some(similarity) = native.similarity {
                                ui.label(format!("Native similarity: {:.1}%", similarity * 100.0));
                            }
                            if let (Some(old), Some(new)) =
                                (&native.old_instructions, &native.new_instructions)
                            {
                                egui::CollapsingHeader::new("Disassembly Diff").show(ui, |ui| {
                                    ui.columns(2, |columns| {
                                        columns[0].strong("OLD");
                                        for instruction in old {
                                            columns[0].monospace(format!(
                                                "{:#x}: {} {}",
                                                instruction.address,
                                                instruction.mnemonic,
                                                instruction.operands
                                            ));
                                        }
                                        columns[1].strong("NEW");
                                        for instruction in new {
                                            columns[1].monospace(format!(
                                                "{:#x}: {} {}",
                                                instruction.address,
                                                instruction.mnemonic,
                                                instruction.operands
                                            ));
                                        }
                                    });
                                });
                            }
                        }
                    });
                }
            });
        });
    }
}

fn path_picker(
    ui: &mut egui::Ui,
    group: &str,
    label: &str,
    path: &mut Option<PathBuf>,
    select: fn() -> Option<PathBuf>,
) {
    ui.horizontal(|ui| {
        if !group.is_empty() {
            ui.strong(group);
        }
        ui.label(label);
        ui.monospace(path.as_ref().map_or_else(
            || "Not selected".to_owned(),
            |value| value.display().to_string(),
        ));
        if ui.button("Select").clicked() {
            *path = select();
        }
    });
}
fn diff_type_visible(item: &il2cpp_diff::TypeDiff, query: &str, filter: CompareFilter) -> bool {
    let status_match = match filter {
        CompareFilter::All => {
            item.status.is_changed()
                || item.methods.iter().any(|method| method.status.is_changed())
                || item.fields.iter().any(|field| field.status.is_changed())
                || item
                    .properties
                    .iter()
                    .any(|property| property.status.is_changed())
        }
        CompareFilter::Added => {
            item.status == DiffStatus::Added
                || item
                    .methods
                    .iter()
                    .any(|method| method.status == DiffStatus::Added)
        }
        CompareFilter::Removed => {
            item.status == DiffStatus::Removed
                || item
                    .methods
                    .iter()
                    .any(|method| method.status == DiffStatus::Removed)
        }
        CompareFilter::Changed => {
            item.status == DiffStatus::Changed
                || item
                    .methods
                    .iter()
                    .any(|method| method.status == DiffStatus::Changed)
        }
        CompareFilter::Moved => item
            .methods
            .iter()
            .any(|method| method.status == DiffStatus::Moved),
    };
    status_match
        && item
            .identity
            .to_string()
            .to_lowercase()
            .contains(&query.to_lowercase())
}
fn show_diff_fields(ui: &mut egui::Ui, title: &str, values: impl Iterator<Item = String>) {
    ui.separator();
    ui.strong(title);
    for value in values {
        ui.monospace(value);
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
