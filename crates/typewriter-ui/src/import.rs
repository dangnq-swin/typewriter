//! `typewriter --import manuscript.odt [project]`: an .odt retyped into a new
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

pub const FLAG: &str = "--import";

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

    /// A one-entry-per-paragraph .odt, good enough for `odt::paragraphs`.
    fn manuscript(paragraphs: &[&str]) -> Vec<u8> {
        use std::io::Write;
        let body = paragraphs
            .iter()
            .map(|p| format!("<text:p>{p}</text:p>"))
            .collect::<String>();
        let content = format!(
            "<?xml version=\"1.0\"?><office:document-content \
             xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
             xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
             office:mimetype-version=\"1.2\"><office:body><office:text>\
             {body}</office:text></office:body></office:document-content>"
        );
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("mimetype", options).unwrap();
        zip.write_all(b"application/vnd.oasis.opendocument.text")
            .unwrap();
        zip.start_file("content.xml", options).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn the_manuscript_is_typed_into_a_new_project_beside_it() {
        let dir = crate::storage::test_desktop().join("import-run");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("draft.odt");
        std::fs::write(&source, manuscript(&["Chapter One", "It was fine."])).unwrap();
        let os = |p: &std::path::Path| OsString::from(p);
        run("typewriter-import", &[os(&source)]).unwrap();
        let target = dir.join("draft.typr");
        assert!(target.exists(), "the project lands beside the manuscript");
        let machine =
            crate::filing::open(&crate::machines::Machines::built_in().unwrap(), &target).unwrap();
        let text = typewriter_core::export::plain_text(machine.document());
        assert!(text.contains("Chapter One"), "{text}");
        assert!(text.contains("It was fine."), "{text}");
        // Never overwrite: a second import to the same name is refused.
        let again = run("typewriter-import", &[os(&source)])
            .err()
            .map(|e| e.to_string());
        assert_eq!(again, Some(format!("{} already exists", target.display())));
        // A source that is not there says so.
        let missing = dir.join("gone.odt");
        let err = run("typewriter-import", &[os(&missing)])
            .err()
            .unwrap()
            .to_string();
        assert!(err.starts_with("could not open "), "{err}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_help_and_version_exit_cleanly() {
        run("typewriter-import", &[OsString::from("--help")]).unwrap();
        run("typewriter-import", &[OsString::from("--version")]).unwrap();
    }
}
