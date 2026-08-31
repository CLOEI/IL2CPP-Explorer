use crate::navigation::AddressTarget;
use crate::state::{ProjectData, format_hex, type_name};

pub fn show(ui: &mut egui::Ui, data: &ProjectData, target: AddressTarget) {
    let project = &data.project;
    let (rva, va, file) = match target {
        AddressTarget::Rva(rva) => (
            Some(rva),
            project.binary().image_base().checked_add(rva),
            project
                .binary()
                .image_base()
                .checked_add(rva)
                .and_then(|value| project.binary().virtual_to_offset(value)),
        ),
        AddressTarget::Va(va) => (
            va.checked_sub(project.binary().image_base()),
            Some(va),
            project.binary().virtual_to_offset(va),
        ),
        AddressTarget::FileOffset(file) => {
            let va = project.binary().offset_to_virtual(file);
            (
                va.and_then(|value| value.checked_sub(project.binary().image_base())),
                va,
                Some(file),
            )
        }
    };
    ui.heading("Address");
    egui::Grid::new("address_details")
        .num_columns(2)
        .show(ui, |ui| {
            row(ui, "RVA", format_hex(rva));
            row(ui, "VA", format_hex(va));
            row(ui, "File offset", format_hex(file));
            if let Some(va) = va {
                if let Some(segment) = project.binary().segments().iter().find(|segment| {
                    va >= segment.virtual_address
                        && va < segment.virtual_address.saturating_add(segment.virtual_size)
                }) {
                    row(ui, "Segment", segment.kind.clone());
                    row(ui, "Permissions", segment.permissions.to_string());
                }
            }
        });
    let Some(va) = va else {
        ui.weak("Address could not be translated for this image.");
        return;
    };
    let exact = project
        .native_methods()
        .and_then(|index| index.method_at_address(va));
    let nearest = project.native_methods().and_then(|index| {
        project
            .metadata()
            .methods
            .iter()
            .filter_map(|method| {
                index
                    .address_of(method.id)
                    .filter(|address| address.virtual_address <= va)
                    .map(|address| (method.id, address.virtual_address))
            })
            .max_by_key(|(_, address)| *address)
    });
    if let Some(method) = exact {
        let item = &project.metadata().methods[method.0];
        ui.separator();
        ui.strong(format!(
            "Method start: {}::{}",
            type_name(project, item.declaring_type),
            item.name
        ));
    } else if let Some((method, start)) = nearest {
        let item = &project.metadata().methods[method.0];
        ui.separator();
        ui.label(format!(
            "Nearest resolved method before this address: {}::{}",
            type_name(project, item.declaring_type),
            item.name
        ));
        ui.monospace(format!("+0x{:X} (function boundary unknown)", va - start));
    }
}
fn row(ui: &mut egui::Ui, label: &str, value: String) {
    ui.strong(label);
    ui.monospace(value);
    ui.end_row();
}
