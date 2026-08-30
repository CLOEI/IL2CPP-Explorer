use il2cpp_core::analysis::Il2CppProject;
use il2cpp_core::model::TypeId;

use crate::state::{AssemblyNode, NamespaceNode};

pub fn show(
    ui: &mut egui::Ui,
    project: &Il2CppProject,
    assemblies: &[AssemblyNode],
) -> Option<TypeId> {
    let mut selected = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for assembly in assemblies {
            egui::CollapsingHeader::new(&assembly.name)
                .id_salt(("assembly", assembly.id.0))
                .show(ui, |ui| {
                    show_namespace(ui, project, &assembly.namespaces, &mut selected)
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
) {
    for (name, child) in &node.children {
        egui::CollapsingHeader::new(name)
            .id_salt(("namespace", name, child as *const NamespaceNode))
            .show(ui, |ui| show_namespace(ui, project, child, selected));
    }
    for type_id in &node.types {
        let ty = &project.metadata().types[type_id.0];
        if ui.selectable_label(false, &ty.name).clicked() {
            *selected = Some(*type_id);
        }
    }
}
