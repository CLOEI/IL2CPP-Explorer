use il2cpp_core::analysis::TypeResolver;
use il2cpp_core::model::TypeId;
use il2cpp_export::CSharpTypeRenderer;

use crate::state::{ProjectData, type_name};

pub fn show(
    ui: &mut egui::Ui,
    data: &ProjectData,
    type_id: TypeId,
    member_filter: &mut String,
) -> Option<il2cpp_core::model::MethodId> {
    let project = &data.project;
    let metadata = project.metadata();
    let ty = &metadata.types[type_id.0];
    let resolver = resolver(project);
    let names = CSharpTypeRenderer::new(metadata, true);
    ui.heading(type_name(project, type_id));
    let base = ty
        .parent
        .and_then(|parent| resolver.resolve(parent).ok())
        .map(|parent| names.format(&parent));
    ui.label(format!(
        "{:?} {}",
        ty.kind(),
        base.map_or_else(String::new, |base| format!(": {base}"))
    ));
    ui.add_space(10.0);

    section(ui, "Fields");
    ui.text_edit_singleline(member_filter);
    egui::Grid::new(("fields", type_id.0))
        .striped(true)
        .show(ui, |ui| {
            header(ui, ["Name", "Type", "Offset"]);
            for field_id in &ty.fields {
                let field = &metadata.fields[field_id.0];
                if !member_filter.is_empty()
                    && !field
                        .name
                        .to_lowercase()
                        .contains(&member_filter.to_lowercase())
                {
                    continue;
                }
                let signature = resolver.field_signature(field).ok();
                ui.monospace(&field.name).context_menu(|ui| {
                    if ui.button("Copy Name").clicked() {
                        ui.ctx().copy_text(field.name.clone());
                        ui.close_menu();
                    }
                });
                ui.monospace(
                    signature
                        .as_ref()
                        .map_or_else(|| "-".to_owned(), |signature| names.format(&signature.ty)),
                );
                ui.monospace(
                    signature
                        .and_then(|signature| signature.offset)
                        .map_or_else(|| "-".to_owned(), |offset| format!("0x{offset:X}")),
                );
                ui.end_row();
            }
        });

    ui.add_space(12.0);
    section(ui, "Properties");
    egui::Grid::new(("properties", type_id.0))
        .striped(true)
        .show(ui, |ui| {
            header(ui, ["Name", "Type", "Access"]);
            for property_id in &ty.properties {
                let property = &metadata.properties[property_id.0];
                let ty = property_type(property, metadata, &resolver, &names);
                let access = match (property.getter, property.setter) {
                    (Some(_), Some(_)) => "get/set",
                    (Some(_), None) => "get",
                    (None, Some(_)) => "set",
                    (None, None) => "-",
                };
                ui.monospace(&property.name);
                ui.monospace(ty);
                ui.monospace(access);
                ui.end_row();
            }
        });

    ui.add_space(12.0);
    section(ui, "Methods");
    let mut selected = None;
    egui::Grid::new(("methods", type_id.0))
        .striped(true)
        .show(ui, |ui| {
            header(ui, ["Method", "RVA", ""]);
            for method_id in &ty.methods {
                let method = &metadata.methods[method_id.0];
                if !member_filter.is_empty()
                    && !method
                        .name
                        .to_lowercase()
                        .contains(&member_filter.to_lowercase())
                {
                    continue;
                }
                let label = method_label(method, &resolver, &names);
                let response = ui.selectable_label(false, label);
                response.context_menu(|ui| {
                    if ui.button("Copy Name").clicked() {
                        ui.ctx().copy_text(method.name.clone());
                        ui.close_menu();
                    }
                });
                if response.clicked() {
                    selected = Some(*method_id);
                }
                let address = project
                    .native_methods()
                    .and_then(|index| index.address_of(*method_id));
                ui.monospace(address.map_or_else(
                    || "-".to_owned(),
                    |address| format!("0x{:08X}", address.relative_address),
                ));
                ui.end_row();
            }
        });
    selected
}

pub fn resolver(project: &il2cpp_core::analysis::Il2CppProject) -> TypeResolver<'_> {
    project.runtime_metadata().map_or_else(
        || TypeResolver::metadata_only(project.metadata()),
        |runtime| TypeResolver::with_runtime(project.metadata(), project.binary(), runtime),
    )
}

pub fn method_label(
    method: &il2cpp_core::model::Method,
    resolver: &TypeResolver<'_>,
    names: &CSharpTypeRenderer<'_>,
) -> String {
    let parameters = resolver
        .method_signature(method)
        .map(|signature| {
            signature
                .parameters
                .iter()
                .map(|parameter| format!("{} {}", names.format(&parameter.ty), parameter.name))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|_| "?".to_owned());
    format!("{}({parameters})", method.name)
}

fn property_type(
    property: &il2cpp_core::model::Property,
    metadata: &il2cpp_core::metadata::Metadata,
    resolver: &TypeResolver<'_>,
    names: &CSharpTypeRenderer<'_>,
) -> String {
    let type_index = property
        .getter
        .map(|getter| metadata.methods[getter.0].return_type)
        .or_else(|| {
            property.setter.and_then(|setter| {
                metadata.methods[setter.0]
                    .parameters
                    .last()
                    .map(|parameter| metadata.parameters[parameter.0].parameter_type)
            })
        });
    type_index
        .and_then(|type_index| resolver.resolve(type_index).ok())
        .map_or_else(|| "-".to_owned(), |ty| names.format(&ty))
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.strong(title);
    ui.separator();
}

fn header(ui: &mut egui::Ui, values: [&str; 3]) {
    for value in values {
        ui.strong(value);
    }
    ui.end_row();
}
