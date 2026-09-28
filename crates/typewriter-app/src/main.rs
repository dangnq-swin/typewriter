mod app;

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Typewriter")
            .with_app_id("typewriter")
            .with_inner_size([900.0, 1000.0]),
        ..Default::default()
    };
    eframe::run_native(
        "typewriter",
        options,
        Box::new(|_cc| Ok(Box::new(app::TypewriterApp))),
    )
    .map_err(|e| anyhow::anyhow!("failed to start the GUI: {e}"))
}
