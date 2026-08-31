use il2cpp_core::model::StringLiteralId;

use crate::state::ProjectData;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StringFilter {
    All,
    NonEmpty,
    Urls,
}

pub fn show(
    ui: &mut egui::Ui,
    data: &ProjectData,
    query: &mut String,
    filter: &mut StringFilter,
    selected: &mut Option<StringLiteralId>,
) {
    ui.horizontal(|ui| {
        ui.heading("String Literals");
        ui.text_edit_singleline(query);
        ui.selectable_value(filter, StringFilter::All, "All");
        ui.selectable_value(filter, StringFilter::NonEmpty, "Non-empty");
        ui.selectable_value(filter, StringFilter::Urls, "URLs");
    });
    let ids = data
        .string_search
        .find(query)
        .into_iter()
        .filter(|id| {
            let item = &data.project.metadata().string_literals()[id.0];
            match filter {
                StringFilter::All => true,
                StringFilter::NonEmpty => !item.value.is_empty(),
                StringFilter::Urls => item.is_url(),
            }
        })
        .collect::<Vec<_>>();
    ui.label(format!("{} matches", ids.len()));
    ui.columns(2, |columns| {
        let row_height = columns[0].text_style_height(&egui::TextStyle::Monospace);
        egui::ScrollArea::vertical()
            .id_salt("string_literal_list")
            .show_rows(&mut columns[0], row_height, ids.len(), |ui, rows| {
                for id in &ids[rows] {
                    let item = &data.project.metadata().string_literals()[id.0];
                    let value = if item.value.is_empty() {
                        "<empty>".to_owned()
                    } else {
                        truncate(&item.escaped(), 80)
                    };
                    if ui
                        .selectable_label(
                            *selected == Some(*id),
                            format!("{:>6} {:>6}  {}", id.0, item.byte_length, value),
                        )
                        .clicked()
                    {
                        *selected = Some(*id);
                    }
                }
            });
        columns[1].heading("Detail");
        if let Some(literal) = selected.and_then(|id| data.project.metadata().string_literal(id)) {
            columns[1].label(format!("String Literal #{}", literal.id.0));
            columns[1].label(format!("Length: {} bytes", literal.byte_length));
            columns[1].label(format!("Data index: {:#010X}", literal.data_index));
            columns[1].horizontal(|ui| {
                if ui.button("Copy Value").clicked() {
                    ui.ctx().copy_text(literal.value.clone());
                }
                if ui.button("Copy Escaped").clicked() {
                    ui.ctx().copy_text(literal.escaped());
                }
                if ui.button("Copy Index").clicked() {
                    ui.ctx().copy_text(literal.id.0.to_string());
                }
            });
            let mut value = literal.value.clone();
            egui::ScrollArea::vertical()
                .id_salt("string_literal_detail")
                .show(&mut columns[1], |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut value)
                            .desired_rows(16)
                            .interactive(false),
                    );
                });
        } else {
            columns[1].weak("Select a string literal.");
        }
    });
}
fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        format!("{}...", value.chars().take(max).collect::<String>())
    }
}
