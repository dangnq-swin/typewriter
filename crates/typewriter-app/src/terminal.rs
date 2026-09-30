//! The command line, Linux only: flags, `typewriter --import` among them. On Windows
//! the app has no console; `typewriter-import` is the console program there.

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::bail;

use crate::{OPTIONS, Stage, VERSION, import, is_help, is_option, is_version, storage};

fn usage(command: &str) -> String {
    format!(
        "\
usage: {command} [project.typr]
       {command} --import manuscript.odt [project]"
    )
}

/// Handles a command or flag. False if the app should open.
pub fn run(stage: &dyn Stage, args: &[OsString]) -> anyhow::Result<bool> {
    let [first, rest @ ..] = args else {
        return Ok(false);
    };
    let command = stage.command();
    if first == import::FLAG {
        import::run(&format!("{command} --import"), rest)?;
    } else if is_help(first) {
        println!("{}", help(stage));
    } else if is_version(first) {
        println!("{command} {VERSION}");
    } else if is_option(first) {
        bail!("unknown option {}\n{}", first.display(), usage(command));
    } else {
        return Ok(false);
    }
    Ok(true)
}

fn help(stage: &dyn Stage) -> String {
    let shown = |path: Option<PathBuf>| {
        path.map_or_else(|| "(unknown)".to_owned(), |p| storage::home_relative(&p))
    };
    format!(
        "\
{command} {VERSION}: {about}

{usage}

Opens the project given, else the last one, else a new draft.
`--import` retypes an .odt into a new project: see `{command} --import --help`.

{OPTIONS}

settings  {}
data      {} (drafts, machine profiles)",
        shown(storage::config_path()),
        shown(storage::data_dir()),
        command = stage.command(),
        usage = usage(stage.command()),
        about = stage.about(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plain;

    /// Another edition, by its own name.
    struct Other;

    impl Stage for Other {
        fn command(&self) -> &'static str {
            "typewriter-other"
        }

        fn title(&self) -> &'static str {
            "Other"
        }

        fn about(&self) -> &'static str {
            "another edition"
        }
    }

    #[test]
    fn a_project_file_opens_the_app() {
        let args = |args: &[&str]| args.iter().map(OsString::from).collect::<Vec<_>>();
        for stage in [&Plain as &dyn Stage, &Other] {
            let run = |a: &[&str]| run(stage, &args(a));
            assert!(!run(&[]).unwrap());
            assert!(!run(&["novel.typr"]).unwrap());
            assert!(run(&["--version"]).unwrap());
            assert!(run(&["--bogus"]).is_err());
            assert!(run(&["--import", "--help"]).unwrap());
            assert!(!run(&["import"]).unwrap(), "a project named so");
        }
        assert!(help(&Other).starts_with("typewriter-other "));
    }
}
