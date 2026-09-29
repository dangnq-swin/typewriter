mod app;
mod audio;
mod filing;
mod import;
mod input;
mod machines;
mod odt;
mod render;
mod settings;
#[cfg(test)]
mod simulate;
mod storage;

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let [command, rest @ ..] = args.as_slice()
        && command == import::COMMAND
    {
        return import::run(rest);
    }
    // Load before the window opens: it must open fullscreen at once.
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
