mod app;
mod audio;
mod filing;
mod input;
mod machines;
mod render;
mod settings;
mod storage;

fn main() -> anyhow::Result<()> {
    // Read before the window opens, so it opens fullscreen straight away.
    let settings = settings::SettingsFile::load();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Typewriter")
            .with_app_id("typewriter")
            .with_inner_size([900.0, 1000.0])
            .with_fullscreen(settings.0.look.fullscreen),
        ..Default::default()
    };
    eframe::run_native(
        "typewriter",
        options,
        Box::new(|cc| {
            let app = app::TypewriterApp::new(cc, settings)?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("failed to start the GUI: {e}"))
}
