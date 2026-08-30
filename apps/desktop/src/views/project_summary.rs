use crate::state::ProjectData;

pub fn show(ui: &mut egui::Ui, data: &ProjectData) {
    let project = &data.project;
    let metadata = project.metadata();
    ui.heading("Project Summary");
    ui.add_space(8.0);
    egui::Grid::new("project_summary")
        .num_columns(2)
        .show(ui, |ui| {
            row(
                ui,
                "Binary",
                data.binary_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("-"),
            );
            row(
                ui,
                "Metadata",
                data.metadata_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("-"),
            );
            row(ui, "Format", &project.binary_format().to_string());
            row(ui, "Architecture", &project.architecture().to_string());
            row(
                ui,
                "Metadata version",
                &format!("v{}", project.metadata_version().raw()),
            );
            row(ui, "Assemblies", &metadata.assemblies.len().to_string());
            row(ui, "Types", &metadata.types.len().to_string());
            row(ui, "Methods", &metadata.methods.len().to_string());
            row(
                ui,
                "Native methods",
                &project
                    .native_methods()
                    .map_or(0, |index| index.mapped_method_count())
                    .to_string(),
            );
        });
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.strong(label);
    ui.monospace(value);
    ui.end_row();
}
