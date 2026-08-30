pub fn install(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(24, 28, 33);
    visuals.window_fill = egui::Color32::from_rgb(30, 34, 40);
    visuals.extreme_bg_color = egui::Color32::from_rgb(15, 18, 22);
    visuals.faint_bg_color = egui::Color32::from_rgb(38, 44, 51);
    visuals.selection.bg_fill = egui::Color32::from_rgb(45, 92, 128);
    visuals.hyperlink_color = egui::Color32::from_rgb(100, 180, 230);
    context.set_visuals(visuals);

    let mut style = (*context.style()).clone();
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    context.set_style(style);
}
