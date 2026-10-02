//! `typewriter`: normal mode on the app's library, the default.
//! It opens the same projects and settings as `typewriter-plain`, and draws
//! its room and machine through the app's [`Stage`](typewriter_ui::Stage)
//! hooks.

// Release builds on Windows: no console window beside the app. Nothing
// printed shows there.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod depth;
mod machine;
mod room;
mod stage;

fn main() -> anyhow::Result<()> {
    typewriter_ui::run(stage::Desk)
}
