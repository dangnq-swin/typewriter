//! `typewriter-plain`: the plain edition, the focused writing tool —
//! the app of `typewriter-ui` opened on its `Plain` stage.

// Release builds on Windows: no console window beside the app. Nothing
// printed shows there.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    typewriter_ui::run(typewriter_ui::Plain)
}
