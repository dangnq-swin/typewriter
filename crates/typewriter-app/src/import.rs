//! `typewriter import manuscript.odt [project]`: an .odt retyped into a new
//! project on the machine new projects use (`typewriter_core::retype`).

use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use typewriter_core::retype::retype;
use typewriter_core::{Constraints, Typewriter};

use crate::machines::Machines;
use crate::{odt, settings, storage};

pub const COMMAND: &str = "import";
const USAGE: &str = "usage: typewriter import manuscript.odt [project]";

/// Runs the command on the arguments after `import`.
pub fn run(args: &[OsString]) -> anyhow::Result<()> {
    let (source, target) = match args {
        [source] => (Path::new(source), beside(Path::new(source))),
        [source, target] => (Path::new(source), storage::with_extension(target.into())),
        _ => bail!(USAGE),
    };
    if target.exists() {
        bail!("{} already exists", target.display());
    }
    let file =
        File::open(source).with_context(|| format!("could not open {}", source.display()))?;
    let paragraphs = odt::paragraphs(BufReader::new(file))
        .with_context(|| format!("could not read {}", source.display()))?;
    let (settings, _, trouble) = settings::SettingsFile::load();
    if let Some(trouble) = trouble {
        eprintln!("{trouble}");
    }
    let machines = Machines::load()?;
    let profile = machines.for_new(&settings.machine.profile);
    let constraints = settings
        .machine
        .rules
        .constraints(Constraints::default().erase);
    let mut machine = Typewriter::new(profile, constraints)?;
    retype(&mut machine, &paragraphs);
    storage::write_atomic(&target, machine.to_folder_ron()?)
        .with_context(|| format!("could not write {}", target.display()))?;
    let sheets = machine.document().finished().len() + 1;
    let plural = if sheets == 1 { "" } else { "s" };
    println!(
        "Typed {sheets} sheet{plural} on the {} into {}",
        machine.profile().name,
        target.display()
    );
    Ok(())
}

/// `notes/draft.odt` → `notes/draft.folder.ron`.
fn beside(source: &Path) -> PathBuf {
    storage::with_extension(source.with_extension(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_lands_beside_the_manuscript() {
        assert_eq!(
            beside(Path::new("notes/draft.odt")),
            Path::new("notes/draft.folder.ron")
        );
    }

    #[test]
    fn wrong_arguments_show_the_usage() {
        let error = run(&[]).err().map(|e| e.to_string());
        assert_eq!(error.as_deref(), Some(USAGE));
    }
}
