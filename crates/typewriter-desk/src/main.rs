//! `typewriter-desk`: the desk edition, on the app's library. It opens the
//! same projects and settings as `typewriter`, and draws its room and
//! machine through the app's [`Stage`](typewriter_app::Stage) hooks.

// Release builds on Windows: no console window beside the app. Nothing
// printed shows there.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod depth;
mod machine;
mod room;
mod stage;

fn main() -> anyhow::Result<()> {
    typewriter_app::run(stage::Desk)
}
