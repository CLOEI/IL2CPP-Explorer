pub fn show(ui: &mut egui::Ui, text: &str) {
    let rows = text.lines().count().max(1);
    let mut text = text.to_owned();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY)
            .desired_rows(rows),
    );
}
