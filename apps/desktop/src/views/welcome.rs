use std::path::Path;

pub enum WelcomeAction {
    SelectBinary,
    SelectMetadata,
    Analyze,
    OpenLocal,
}

pub fn show(
    context: &egui::Context,
    binary: Option<&Path>,
    metadata: Option<&Path>,
    local_target: bool,
) -> Option<WelcomeAction> {
    let mut action = None;
    egui::CentralPanel::default().show(context, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.heading("IL2CPP Explorer");
            ui.weak("Native IL2CPP metadata and executable explorer");
            ui.add_space(30.0);
            ui.horizontal(|ui| {
                ui.label("libil2cpp.so");
                ui.monospace(binary.map_or_else(
                    || "Not selected".to_owned(),
                    |path| path.to_string_lossy().into_owned(),
                ));
                if ui.button("Select Binary").clicked() {
                    action = Some(WelcomeAction::SelectBinary);
                }
            });
            ui.horizontal(|ui| {
                ui.label("global-metadata.dat");
                ui.monospace(metadata.map_or_else(
                    || "Not selected".to_owned(),
                    |path| path.to_string_lossy().into_owned(),
                ));
                if ui.button("Select Metadata").clicked() {
                    action = Some(WelcomeAction::SelectMetadata);
                }
            });
            ui.add_space(16.0);
            if ui
                .add_enabled(
                    binary.is_some() && metadata.is_some(),
                    egui::Button::new("Analyze"),
                )
                .clicked()
            {
                action = Some(WelcomeAction::Analyze);
            }
            if local_target && ui.button("Open Local Target").clicked() {
                action = Some(WelcomeAction::OpenLocal);
            }
        });
    });
    action
}

pub fn loading(context: &egui::Context) {
    egui::CentralPanel::default().show(context, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(120.0);
            ui.heading("Analyzing IL2CPP project...");
            ui.add_space(12.0);
            ui.spinner();
            ui.label("Parsing metadata and native image...");
        });
    });
}

pub fn failed(context: &egui::Context, message: &str) -> Option<bool> {
    let mut retry = None;
    egui::CentralPanel::default().show(context, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(120.0);
            ui.heading("Failed to load project");
            ui.add_space(10.0);
            ui.colored_label(egui::Color32::LIGHT_RED, message);
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Back").clicked() {
                    retry = Some(false);
                }
                if ui.button("Try Again").clicked() {
                    retry = Some(true);
                }
            });
        });
    });
    retry
}
