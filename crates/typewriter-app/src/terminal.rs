//! The command line, Linux only: flags and `typewriter import`. On Windows
//! the app has no console; `typewriter-import` is the console program there.

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::bail;

use crate::{OPTIONS, VERSION, import, is_help, is_option, is_version, storage};

const USAGE: &str = "\
usage: typewriter [project.folder.ron]
       typewriter import manuscript.odt [project]";

/// Handles a command or flag. False if the app should open.
pub fn run(args: &[OsString]) -> anyhow::Result<bool> {
    let [first, rest @ ..] = args else {
        return Ok(false);
    };
    if first == import::COMMAND {
        import::run("typewriter import", rest)?;
    } else if is_help(first) {
        println!("{}", help());
    } else if is_version(first) {
        println!("typewriter {VERSION}");
    } else if is_option(first) {
        bail!("unknown option {}\n{USAGE}", first.display());
    } else {
        return Ok(false);
    }
    Ok(true)
}

fn help() -> String {
    let shown = |path: Option<PathBuf>| {
        path.map_or_else(|| "(unknown)".to_owned(), |p| storage::home_relative(&p))
    };
    format!(
        "\
typewriter {VERSION}: a typewriter simulator for focused writing

{USAGE}

Opens the project given, else the last one, else a new draft.
`typewriter import` retypes an .odt into a new project: see `typewriter import --help`.

{OPTIONS}

settings  {}
data      {} (drafts, machine profiles)",
        shown(storage::config_path()),
        shown(storage::data_dir()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_file_opens_the_app() {
        let args = |args: &[&str]| args.iter().map(OsString::from).collect::<Vec<_>>();
        assert!(!run(&args(&[])).unwrap());
        assert!(!run(&args(&["novel.folder.ron"])).unwrap());
        assert!(run(&args(&["--version"])).unwrap());
        assert!(run(&args(&["--bogus"])).is_err());
    }
}
