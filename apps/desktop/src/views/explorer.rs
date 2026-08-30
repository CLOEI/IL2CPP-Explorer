use crate::state::{MethodTab, ProjectData, SearchMatch, SearchResult};
use crate::views::{method_view, project_summary, type_view};
use crate::widgets::{assembly_tree, search};

pub enum ExplorerAction {
    Open,
    Export,
    Select(SearchResult),
}

pub struct ExplorerState<'a> {
    pub selected_type: &'a mut Option<il2cpp_core::model::TypeId>,
    pub selected_method: &'a mut Option<il2cpp_core::model::MethodId>,
    pub search_query: &'a mut String,
    pub search_results: &'a [SearchMatch],
    pub search_limited: bool,
    pub tab: &'a mut MethodTab,
    pub tree_focus: &'a mut Option<il2cpp_core::model::TypeId>,
    pub export_status: Option<&'a str>,
}

pub fn show(
    context: &egui::Context,
    data: &mut ProjectData,
    state: ExplorerState<'_>,
) -> Option<ExplorerAction> {
    let ExplorerState {
        selected_type,
        selected_method,
        search_query,
        search_results,
        search_limited,
        tab,
        tree_focus,
        export_status,
    } = state;
    let mut action = None;
    egui::TopBottomPanel::top("toolbar").show(context, |ui| {
        ui.horizontal(|ui| {
            ui.heading("IL2CPP Explorer");
            if ui.button("Open").clicked() {
                action = Some(ExplorerAction::Open);
            }
            if ui.button("Export dump.cs").clicked() {
                action = Some(ExplorerAction::Export);
            }
            ui.separator();
            let response = ui.add_sized(
                [300.0, 24.0],
                egui::TextEdit::singleline(search_query)
                    .id(egui::Id::new("global_search"))
                    .hint_text("Search types, methods, fields..."),
            );
            show_search_dropdown(
                context,
                response.rect.left_bottom(),
                search_query,
                search_results,
                search_limited,
                &mut action,
            );
        });
    });
    egui::TopBottomPanel::bottom("status").show(context, |ui| {
        let project = &data.project;
        let metadata = project.metadata();
        ui.horizontal(|ui| {
            ui.monospace(project.architecture().to_string());
            ui.separator();
            ui.monospace(format!("Metadata v{}", project.metadata_version().raw()));
            ui.separator();
            ui.monospace(format!("{} Types", metadata.types.len()));
            ui.separator();
            ui.monospace(format!("{} Methods", metadata.methods.len()));
            if let Some(status) = export_status {
                ui.separator();
                ui.weak(status);
            }
        });
    });
    egui::SidePanel::left("explorer")
        .resizable(true)
        .default_width(280.0)
        .show(context, |ui| {
            ui.strong("Assemblies / Types");
            ui.separator();
            if let Some(type_id) = assembly_tree::show(
                ui,
                &data.project,
                &data.navigation.assemblies,
                *selected_type,
                tree_focus,
            ) {
                action = Some(ExplorerAction::Select(SearchResult::Type(type_id)));
            }
        });
    egui::CentralPanel::default().show(context, |ui| {
        if let Some(method_id) = *selected_method {
            method_view::show(ui, data, method_id, tab);
            if ui.button("Back to type").clicked() {
                *selected_method = None;
            }
        } else if let Some(type_id) = *selected_type {
            if let Some(method_id) = type_view::show(ui, data, type_id) {
                *selected_method = Some(method_id);
            }
            ui.add_space(12.0);
            egui::CollapsingHeader::new("C# representation").show(ui, |ui| {
                method_view::type_csharp(ui, data, type_id);
            });
        } else {
            project_summary::show(ui, data);
        }
    });
    action
}

fn show_search_dropdown(
    context: &egui::Context,
    position: egui::Pos2,
    query: &str,
    results: &[SearchMatch],
    limited: bool,
    action: &mut Option<ExplorerAction>,
) {
    if query.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("global_search_results"))
        .order(egui::Order::Foreground)
        .fixed_pos(position)
        .show(context, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(420.0);
                if results.is_empty() {
                    ui.weak("No matches.");
                } else if let Some(result) = search::show(ui, results, limited) {
                    *action = Some(ExplorerAction::Select(result));
                }
            });
        });
}
