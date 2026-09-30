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

/// Runs the command on its arguments. `command`: as typed, for the help.
pub fn run(command: &str, args: &[OsString]) -> anyhow::Result<()> {
    let usage = format!("usage: {command} manuscript.odt [project]");
    if let [first, ..] = args {
        if crate::is_help(first) {
            println!("{}", help(command, &usage));
            return Ok(());
        }
        if crate::is_version(first) {
            println!("{command} {}", crate::VERSION);
            return Ok(());
        }
        if crate::is_option(first) {
            bail!("unknown option {}\n{usage}", first.display());
        }
    }
    let (source, target) = match args {
        [source] => (Path::new(source), beside(Path::new(source))),
        [source, target] => (Path::new(source), storage::with_extension(target.into())),
        _ => bail!(usage),
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

fn help(command: &str, usage: &str) -> String {
    format!(
        "\
{command} {}: an .odt manuscript retyped into a new Typewriter project

{usage}

Types each paragraph on the machine new projects use, sheet after sheet, into
`project`, or else beside the manuscript under its name (.typr). Never
overwrites a file.

{}",
        crate::VERSION,
        crate::OPTIONS,
    )
}

/// `notes/draft.odt` → `notes/draft.typr`.
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
            Path::new("notes/draft.typr")
        );
    }

    #[test]
    fn wrong_arguments_show_the_usage() {
        let usage = "usage: typewriter-import manuscript.odt [project]";
        let error = |args: &[&str]| {
            let args: Vec<OsString> = args.iter().map(OsString::from).collect();
            run("typewriter-import", &args).err().map(|e| e.to_string())
        };
        assert_eq!(error(&[]).as_deref(), Some(usage));
        assert_eq!(
            error(&["--bogus"]),
            Some(format!("unknown option --bogus\n{usage}"))
        );
    }
}
