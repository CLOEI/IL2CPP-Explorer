mod actions;
mod app;
mod history;
mod navigation;
mod recent;
mod state;
mod theme;
mod views;
mod widgets;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "IL2CPP Explorer",
        eframe::NativeOptions::default(),
        Box::new(|creation_context| {
            theme::install(&creation_context.egui_ctx);
            Ok(Box::new(app::Il2CppExplorerApp::default()))
        }),
    )
}
