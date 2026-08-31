use il2cpp_core::model::MethodId;
use il2cpp_disasm::{Arm64Disassembler, FunctionInspector};
use il2cpp_export::{ExportContext, render_method, render_type};

use crate::navigation::AddressTarget;
use crate::state::{MethodTab, ProjectData, format_hex, format_token, type_name};
use crate::views::type_view::{method_label, resolver};
use crate::widgets::address;
use crate::widgets::code_view;

pub fn show(
    ui: &mut egui::Ui,
    data: &mut ProjectData,
    method_id: MethodId,
    tab: &mut MethodTab,
) -> Option<()> {
    let project = data.project.clone();
    let metadata = project.metadata();
    let method = &metadata.methods[method_id.0];
    let types = resolver(&project);
    let names = il2cpp_export::CSharpTypeRenderer::new(metadata, true);
    ui.horizontal(|ui| {
        ui.heading(format!(
            "{}::{}",
            type_name(&project, method.declaring_type),
            method_label(method, &types, &names)
        ));
    });
    ui.horizontal(|ui| {
        for (value, label) in [
            (MethodTab::Overview, "Overview"),
            (MethodTab::CSharp, "C#"),
            (MethodTab::Disassembly, "Disassembly"),
            (MethodTab::Raw, "Raw"),
        ] {
            if ui.selectable_label(*tab == value, label).clicked() {
                *tab = value;
            }
        }
    });
    ui.separator();
    match tab {
        MethodTab::Overview => overview(ui, &project, method_id),
        MethodTab::CSharp => csharp(ui, data, method_id),
        MethodTab::Disassembly => disassembly(ui, data, method_id),
        MethodTab::Raw => raw(ui, &project, method_id),
    }
    None
}

fn overview(
    ui: &mut egui::Ui,
    project: &il2cpp_core::analysis::Il2CppProject,
    method_id: MethodId,
) {
    let metadata = project.metadata();
    let method = &metadata.methods[method_id.0];
    let owner = &metadata.types[method.declaring_type.0];
    let address = project
        .native_methods()
        .and_then(|index| index.address_of(method_id));
    egui::Grid::new(("method_overview", method_id.0))
        .num_columns(2)
        .show(ui, |ui| {
            row(ui, "Assembly", &metadata.images[owner.image.0].name);
            row(ui, "Type", &type_name(project, owner.id));
            row(ui, "Namespace", &owner.namespace);
            row(ui, "Method", &method.name);
            row(ui, "Token", &format_token(method.token));
            row(ui, "Method ID", &method.id.0.to_string());
            ui.strong("RVA");
            let _ = address::show(
                ui,
                address.map(|address| address.relative_address),
                AddressTarget::Rva(address.map_or(0, |address| address.relative_address)),
            );
            ui.end_row();
            ui.strong("VA");
            let _ = address::show(
                ui,
                address.map(|address| address.virtual_address),
                AddressTarget::Va(address.map_or(0, |address| address.virtual_address)),
            );
            ui.end_row();
            ui.strong("File offset");
            let _ = address::show(
                ui,
                address.map(|address| address.file_offset),
                AddressTarget::FileOffset(address.map_or(0, |address| address.file_offset)),
            );
            ui.end_row();
        });
}

fn csharp(ui: &mut egui::Ui, data: &mut ProjectData, method_id: MethodId) {
    if !data.csharp_methods.contains_key(&method_id) {
        let project = data.project.clone();
        let resolver = resolver(&project);
        let context = ExportContext {
            metadata: project.metadata(),
            types: &resolver,
            native_methods: project.native_methods(),
        };
        let mut output = Vec::new();
        let result = render_method(&context, method_id, &mut output)
            .map(|()| String::from_utf8_lossy(&output).into_owned())
            .unwrap_or_else(|error| format!("// C# rendering failed: {error}"));
        data.csharp_methods.insert(method_id, result);
    }
    if let Some(code) = data.csharp_methods.get(&method_id) {
        code_view::show(ui, code);
    }
}

fn disassembly(ui: &mut egui::Ui, data: &mut ProjectData, method_id: MethodId) {
    if !data.disassembly.contains_key(&method_id) {
        let project = data.project.clone();
        let result = if let Some(index) = project.native_methods() {
            Arm64Disassembler::new()
                .map_err(|error| error.to_string())
                .and_then(|backend| {
                    FunctionInspector::new(project.binary(), index, &backend)
                        .inspect(method_id)
                        .map_err(|error| error.to_string())
                })
        } else {
            Err("This method has no resolved native body.".to_owned())
        };
        data.disassembly.insert(method_id, result);
    }
    match data.disassembly.get(&method_id) {
        Some(Ok(inspection)) => {
            ui.monospace(format!(
                "{} instructions, window ending {}",
                inspection.instructions.len(),
                format_hex(Some(inspection.window_end))
            ));
            egui::ScrollArea::vertical().show_rows(
                ui,
                ui.text_style_height(&egui::TextStyle::Monospace),
                inspection.instructions.len(),
                |ui, rows| {
                    for instruction in &inspection.instructions[rows] {
                        let bytes = instruction
                            .bytes
                            .iter()
                            .map(|byte| format!("{byte:02X}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        ui.monospace(format!(
                            "{:08X}  {:<20}  {} {}",
                            instruction.address, bytes, instruction.mnemonic, instruction.operands
                        ));
                    }
                },
            );
        }
        Some(Err(error)) => {
            ui.weak(error);
        }
        None => {}
    }
}

fn raw(ui: &mut egui::Ui, project: &il2cpp_core::analysis::Il2CppProject, method_id: MethodId) {
    let metadata = project.metadata();
    let method = &metadata.methods[method_id.0];
    let address = project
        .native_methods()
        .and_then(|index| index.address_of(method_id));
    let text = format!(
        "Method ID: {}\nType ID: {}\nToken: {}\nMethod slot: {}\nParameter count: {}\nReturn type index: {}\nCodegen module: {}\nRVA: {}\nVA: {}\nFile offset: {}",
        method.id.0,
        method.declaring_type.0,
        format_token(method.token),
        method.slot,
        method.parameters.len(),
        method.return_type.0,
        address.map_or("-", |address| address.module.as_str()),
        format_hex(address.map(|address| address.relative_address)),
        format_hex(address.map(|address| address.virtual_address)),
        format_hex(address.map(|address| address.file_offset)),
    );
    code_view::show(ui, &text);
}

pub fn type_csharp(ui: &mut egui::Ui, data: &mut ProjectData, type_id: il2cpp_core::model::TypeId) {
    if !data.csharp_types.contains_key(&type_id) {
        let project = data.project.clone();
        let resolver = resolver(&project);
        let context = ExportContext {
            metadata: project.metadata(),
            types: &resolver,
            native_methods: project.native_methods(),
        };
        let mut output = Vec::new();
        let result = render_type(&context, type_id, &mut output)
            .map(|()| String::from_utf8_lossy(&output).into_owned())
            .unwrap_or_else(|error| format!("// C# rendering failed: {error}"));
        data.csharp_types.insert(type_id, result);
    }
    if let Some(code) = data.csharp_types.get(&type_id) {
        code_view::show(ui, code);
    }
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.strong(label);
    ui.monospace(value).context_menu(|ui| {
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(value.to_owned());
            ui.close_menu();
        }
    });
    ui.end_row();
}
