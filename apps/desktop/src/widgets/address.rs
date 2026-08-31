use crate::navigation::AddressTarget;

/// Consistent address text with copy/context actions. Caller may route Go target centrally.
pub fn show(ui: &mut egui::Ui, value: Option<u64>, target: AddressTarget) -> Option<AddressTarget> {
    let text = value.map_or_else(|| "-".to_owned(), |value| format!("0x{value:08X}"));
    let mut go = None;
    ui.monospace(&text).context_menu(|ui| {
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(text.clone());
            ui.close_menu();
        }
        if ui.button("Go to").clicked() {
            go = Some(target);
            ui.close_menu();
        }
    });
    go
}
