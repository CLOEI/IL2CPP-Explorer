use crate::state::{SearchMatch, SearchResult};

pub fn show(ui: &mut egui::Ui, results: &[SearchMatch], limited: bool) -> Option<SearchResult> {
    let mut selected = None;
    egui::ScrollArea::vertical()
        .max_height(260.0)
        .show(ui, |ui| {
            for result in results {
                if ui
                    .selectable_label(false, format!("{}  {}", result.kind, result.label))
                    .clicked()
                {
                    selected = Some(result.result);
                }
            }
            if limited {
                ui.weak("Showing first 100 matches.");
            }
        });
    selected
}
