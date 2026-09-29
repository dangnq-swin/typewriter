//! The typewriter app. [`run`] opens it; `typewriter-import` shares the
//! app's folders and settings, the machines, and importing an .odt.

mod app;
mod audio;
mod filing;
pub mod import;
mod input;
pub mod machines;
pub mod odt;
mod render;
pub mod settings;
#[cfg(test)]
mod simulate;
pub mod storage;
#[cfg(not(windows))]
mod terminal;

use std::ffi::OsStr;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Both commands' options, for their help.
pub const OPTIONS: &str = "\
options:
  -h, --help     print this help
  -V, --version  print the version";

pub fn is_help(arg: &OsStr) -> bool {
    arg == "-h" || arg == "--help"
}

pub fn is_version(arg: &OsStr) -> bool {
    arg == "-V" || arg == "--version"
}

/// Starts with `-`: not a file.
pub fn is_option(arg: &OsStr) -> bool {
    arg.as_encoded_bytes().starts_with(b"-")
}

/// `typewriter`: a command from the command line, else the window.
pub fn run() -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_told_from_files() {
        assert!(is_help(OsStr::new("-h")) && is_help(OsStr::new("--help")));
        assert!(is_version(OsStr::new("-V")) && is_version(OsStr::new("--version")));
        assert!(is_option(OsStr::new("--bogus")));
        assert!(!is_option(OsStr::new("novel.folder.ron")));
    }
}
