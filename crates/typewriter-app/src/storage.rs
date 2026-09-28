//! Where project files live and how they are written.
//!
//! A project file is wherever the user saved it. Until then it is a draft in
//! `$XDG_DATA_HOME/typewriter/drafts/`, so nothing typed is ever lost.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A project is shown as a manila folder, and its file is RON inside.
pub const EXTENSION: &str = ".folder.ron";
/// The name of a project not yet saved under one.
pub const UNTITLED: &str = "Untitled";

/// `$XDG_DATA_HOME/typewriter`, or `~/.local/share/typewriter`.
fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })?;
    Some(base.join("typewriter"))
}

fn drafts_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("drafts"))
}

/// A new, unused draft path, named by when it was started (UTC).
pub fn new_draft_path() -> Option<PathBuf> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let name = format!("draft-{}{EXTENSION}", timestamp(seconds));
    drafts_dir().map(|dir| dir.join(name))
}

pub fn is_draft(path: &Path) -> bool {
    drafts_dir().is_some_and(|dir| path.starts_with(dir))
}

/// The project's name, as on the folder's tab.
pub fn display_name(path: &Path) -> String {
    if is_draft(path) {
        return UNTITLED.to_owned();
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.strip_suffix(EXTENSION).unwrap_or(&name).to_owned()
}

/// `~/…` for paths in the home directory, which are shorter to read.
pub fn home_relative(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home
        .as_deref()
        .and_then(|home| path.strip_prefix(home).ok())
    {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// Makes a chosen path a project file: `novel` or `novel.ron` becomes
/// `novel.folder.ron`.
pub fn with_extension(path: PathBuf) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.ends_with(EXTENSION) {
        return path;
    }
    let stem = name.strip_suffix(".ron").unwrap_or(&name);
    path.with_file_name(format!("{stem}{EXTENSION}"))
}

/// An export beside the project file: `novel.folder.ron` → `novel.txt`.
pub fn export_path(folder: &Path, extension: &str) -> PathBuf {
    let name = display_name(folder);
    folder.with_file_name(format!("{name}.{extension}"))
}

/// Writes a temporary file beside `path` and renames it over `path`, so a
/// crash mid-write never leaves a half-written file.
pub fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.tmp"));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(contents.as_ref())?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

fn last_project_record() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("last-folder"))
}

/// The project that was open last time, if it is still there.
pub fn last_project() -> Option<PathBuf> {
    let record = fs::read_to_string(last_project_record()?).ok()?;
    let path = PathBuf::from(record.trim_end_matches('\n'));
    path.is_file().then_some(path)
}

pub fn remember_last(path: &Path) -> io::Result<()> {
    let Some(record) = last_project_record() else {
        return Ok(());
    };
    write_atomic(&record, format!("{}\n", path.display()))
}

/// `YYYY-MM-DD-HHMMSS` for seconds since the Unix epoch, in UTC.
fn timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    // Both fit in u32: day is 1..=31 and month is 1..=12.
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_calendar_dates() {
        assert_eq!(timestamp(0), "1970-01-01-000000");
        // 2026-09-29 14:30:05 UTC.
        assert_eq!(timestamp(1_790_692_205), "2026-09-29-143005");
        assert_eq!(timestamp(951_782_400), "2000-02-29-000000");
    }

    #[test]
    fn chosen_names_become_folder_files() {
        let folder = |p: &str| with_extension(PathBuf::from(p));
        assert_eq!(folder("/a/novel"), PathBuf::from("/a/novel.folder.ron"));
        assert_eq!(folder("/a/novel.ron"), PathBuf::from("/a/novel.folder.ron"));
        assert_eq!(
            folder("/a/novel.folder.ron"),
            PathBuf::from("/a/novel.folder.ron")
        );
        assert_eq!(folder("/a/v1.2"), PathBuf::from("/a/v1.2.folder.ron"));
    }

    #[test]
    fn exports_sit_beside_the_folder_file() {
        let folder = Path::new("/home/me/writing/novel.folder.ron");
        assert_eq!(display_name(folder), "novel");
        assert_eq!(
            export_path(folder, "md"),
            PathBuf::from("/home/me/writing/novel.md")
        );
    }

    #[test]
    fn atomic_writes_replace_the_file_and_leave_no_temporary() {
        let dir = std::env::temp_dir().join(format!("typewriter-test-{}", std::process::id()));
        let path = dir.join("a.folder.ron");
        write_atomic(&path, "one").unwrap();
        write_atomic(&path, "two").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
