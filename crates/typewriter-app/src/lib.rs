//! What `typewriter` and `typewriter-import` share: the app's folders and
//! settings, the machines, and importing an .odt.

pub mod import;
pub mod machines;
pub mod odt;
pub mod settings;
pub mod storage;

use std::ffi::OsStr;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Both commands' options, for their help.
pub const OPTIONS: &str = "\
options:
  -h, --help     print this help
  -V, --version  print the version";

pub fn is_help(arg: &OsStr) -> bool {
    arg == "-h" || arg == "--help"
}

pub fn is_version(arg: &OsStr) -> bool {
    arg == "-V" || arg == "--version"
}

/// Starts with `-`: not a file.
pub fn is_option(arg: &OsStr) -> bool {
    arg.as_encoded_bytes().starts_with(b"-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_told_from_files() {
        assert!(is_help(OsStr::new("-h")) && is_help(OsStr::new("--help")));
        assert!(is_version(OsStr::new("-V")) && is_version(OsStr::new("--version")));
        assert!(is_option(OsStr::new("--bogus")));
        assert!(!is_option(OsStr::new("novel.folder.ron")));
    }
}
