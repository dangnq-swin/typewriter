//! The typewriter app, shared by both modes. [`run`] opens it on a
//! [`Stage`]: [`Plain`] for `typewriter-plain`; `typewriter`, normal mode,
//! `typewriter`, draws its own through the hooks, with [`draw`].
//! `typewriter-import` shares the app's folders and settings, the
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

use typewriter_render::ui::FrameApp;

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

/// The window `stage` asks for.
fn window_options(stage: &impl Stage) -> eframe::NativeOptions {
    eframe::NativeOptions {
        depth_buffer: stage.depth_buffer(),
        ..Default::default()
    }
}

/// A windowed launch's size, and what the first exit from a fullscreen
/// launch restores.
const DEFAULT_WINDOW: eframe::egui::Vec2 = eframe::egui::Vec2::new(900.0, 1000.0);

/// A command from the command line, else the window on `stage`. Every
/// mode shares projects, settings and the one running instance.
pub fn run(stage: impl Stage + 'static) -> anyhow::Result<()> {
    #[cfg(not(windows))]
    {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if terminal::run(&stage, &args)? {
            return Ok(());
        }
    }
    // A project while the app is open goes to it: one instance.
    let project = std::env::args_os().nth(1).filter(|arg| !is_option(arg));
    if project.is_some_and(|path| instance::hand_over(&PathBuf::from(path))) {
        return Ok(());
    }
    // Load before the window opens: it must open fullscreen at once.
    let settings = settings::SettingsFile::load();
    let fullscreen = settings.0.look.fullscreen;
    // The render crate's own window, for the stages that moved onto it.
    if stage.opens_render_window() {
        return render_window(stage, settings, fullscreen);
    }
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title(stage.title())
        .with_app_id(stage.command())
        .with_fullscreen(fullscreen);
    // A Windows bug (upstream, eframe 0.36): after the desktop sizes a new
    // window for fullscreen, eframe re-applies `inner_size` and shrinks the
    // fullscreen window to it. Ask for a window size only when the window
    // opens windowed; the app restores it on the first exit from a
    // fullscreen launch.
    if !fullscreen {
        viewport = viewport.with_inner_size(DEFAULT_WINDOW);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..window_options(&stage)
    };
    eframe::run_native(
        stage.command(),
        options,
        Box::new(|cc| {
            let app = app::TypewriterApp::new(
                &cc.egui_ctx,
                cc.wgpu_render_state.as_ref(),
                settings,
                Box::new(stage),
            )?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("failed to start the GUI: {e}"))
}

/// The render crate's own window: the winit loop plain mode runs in, the one
/// the desk moves to and the renderer grows in.
fn render_window(
    stage: impl Stage + 'static,
    settings: (settings::Settings, settings::SettingsFile, Option<String>),
    fullscreen: bool,
) -> anyhow::Result<()> {
    // eframe's default clear: a dim grey, kept a little translucent.
    let clear_color =
        eframe::egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).to_normalized_gamma_f32();
    typewriter_render::window::run(
        typewriter_render::window::Options {
            title: stage.title(),
            app_id: stage.command(),
            fullscreen,
            windowed_size: Some(DEFAULT_WINDOW),
            clear_color,
            // No depth attachment yet: plain mode draws no depth pass.
            depth_stencil: None,
        },
        |ctx, render_state| {
            let app = app::TypewriterApp::new(ctx, render_state, settings, Box::new(stage))?;
            Ok(Box::new(app) as Box<dyn FrameApp>)
        },
    )
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
