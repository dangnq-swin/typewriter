//! `typewriter-desk`: the desk edition, on the app's library. It opens the
//! same projects and settings as `typewriter`.

// Release builds on Windows: no console window beside the app. Nothing
// printed shows there.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    typewriter_app::run(typewriter_app::Edition::Desk)
}
