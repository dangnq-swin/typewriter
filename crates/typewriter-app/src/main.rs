// Release builds on Windows: no console window beside the app. Nothing
// printed shows there.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod audio;
mod filing;
mod input;
mod render;
#[cfg(test)]
mod simulate;
#[cfg(not(windows))]
mod terminal;

// The library's modules, reached as `crate::…` like the app's own.
#[cfg(test)]
use typewriter_app::odt;
use typewriter_app::{machines, settings, storage};

fn main() -> anyhow::Result<()> {
    #[cfg(not(windows))]
    {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if terminal::run(&args)? {
            return Ok(());
        }
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
