use il2cpp_core::analysis::Il2CppProject;
use il2cpp_core::model::TypeId;

use crate::state::{AssemblyNode, NamespaceNode};

pub fn show(
    ui: &mut egui::Ui,
    project: &Il2CppProject,
    assemblies: &[AssemblyNode],
    selected_type: Option<TypeId>,
    focus: &mut Option<TypeId>,
    filter: &str,
) -> Option<TypeId> {
    let mut selected = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        let filter = filter.trim().to_lowercase();
        if !filter.is_empty() {
            for assembly in assemblies {
                for type_id in &project.metadata().images
                    [project.metadata().assemblies[assembly.id.0].image.0]
                    .types
                {
                    let ty = &project.metadata().types[type_id.0];
                    let full = format!("{}.{}", ty.namespace, ty.name).to_lowercase();
                    if full.contains(&filter)
                        && ui
                            .selectable_label(
                                selected_type == Some(*type_id),
                                format!("{}: {}", assembly.name, full),
                            )
                            .clicked()
                    {
                        selected = Some(*type_id);
                    }
                }
            }
            return;
        }
        for assembly in assemblies {
            let focused = focus.is_some_and(|type_id| contains(&assembly.namespaces, type_id));
            let id = ui.make_persistent_id(("assembly", assembly.id.0));
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                false,
            );
            if focused {
                state.set_open(true);
            }
            state
                .show_header(ui, |ui| ui.label(&assembly.name))
                .body(|ui| {
                    show_namespace(
                        ui,
                        project,
                        &assembly.namespaces,
                        &mut selected,
                        selected_type,
                        focus,
                        "",
                    )
                });
        }
    });
    selected
}

fn show_namespace(
    ui: &mut egui::Ui,
    project: &Il2CppProject,
    node: &NamespaceNode,
    selected: &mut Option<TypeId>,
    selected_type: Option<TypeId>,
    focus: &mut Option<TypeId>,
    path: &str,
) {
    for (name, child) in &node.children {
        let child_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        let id = ui.make_persistent_id(("namespace", &child_path));
        let mut state =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
        if focus.is_some_and(|type_id| contains(child, type_id)) {
            state.set_open(true);
        }
        state.show_header(ui, |ui| ui.label(name)).body(|ui| {
            show_namespace(
                ui,
                project,
                child,
                selected,
                selected_type,
                focus,
                &child_path,
            )
        });
    }
    for type_id in &node.types {
        let ty = &project.metadata().types[type_id.0];
        let focused = *focus == Some(*type_id);
        let response = ui.selectable_label(selected_type == Some(*type_id), &ty.name);
        if focused {
            response.scroll_to_me(Some(egui::Align::Center));
            *focus = None;
        }
        if response.clicked() {
            *selected = Some(*type_id);
        }
    }
}

fn contains(node: &NamespaceNode, type_id: TypeId) -> bool {
    node.types.contains(&type_id) || node.children.values().any(|child| contains(child, type_id))
}
