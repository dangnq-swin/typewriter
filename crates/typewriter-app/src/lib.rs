//! The typewriter app. [`run`] opens it on a [`Stage`]: [`Plain`] for
//! `typewriter`; the desk edition draws its own through the hooks, with
//! [`draw`]. `typewriter-import` shares the app's folders and settings, the
//! machines, and importing an .odt.

mod app;
mod audio;
pub mod draw;
mod filing;
pub mod import;
mod input;
mod instance;
pub mod machines;
pub mod odt;
mod picker;
mod printing;
mod render;
pub mod settings;
#[cfg(test)]
mod simulate;
mod stage;
pub mod storage;
#[cfg(not(windows))]
mod terminal;

use std::ffi::OsStr;
use std::path::PathBuf;

#[cfg(feature = "snapshot")]
#[doc(hidden)]
pub use app::snapshot;
pub use stage::{Plain, Stage};

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

/// A command from the command line, else the window on `stage`. Every
/// edition shares projects, settings and the one open desk.
pub fn run(stage: impl Stage + 'static) -> anyhow::Result<()> {
    #[cfg(not(windows))]
    {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if terminal::run(&stage, &args)? {
            return Ok(());
        }
    }
    // A project while the app is open goes to it: one desk.
    let project = std::env::args_os().nth(1).filter(|arg| !is_option(arg));
    if project.is_some_and(|path| instance::hand_over(&PathBuf::from(path))) {
        return Ok(());
    }
    // Load before the window opens: it must open fullscreen at once.
    let settings = settings::SettingsFile::load();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(stage.title())
            .with_app_id(stage.command())
            .with_inner_size([900.0, 1000.0])
            .with_fullscreen(settings.0.look.fullscreen),
        ..Default::default()
    };
    eframe::run_native(
        stage.command(),
        options,
        Box::new(|cc| {
            let app = app::TypewriterApp::new(cc, settings, Box::new(stage))?;
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
        assert!(!is_option(OsStr::new("novel.typr")));
    }
}
