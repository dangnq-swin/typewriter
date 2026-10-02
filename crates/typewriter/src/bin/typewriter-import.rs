//! `typewriter-import manuscript.odt [project]`: `typewriter --import` as a
//! console program, for Windows, where `typewriter` has no console.

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    typewriter_ui::import::run("typewriter-import", &args)
}

// Cargo can't build a binary for one OS only.
#[cfg(not(windows))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("typewriter-import is for Windows: use `typewriter --import`")
}
