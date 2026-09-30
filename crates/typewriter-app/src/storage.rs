//! The app's own folders (XDG; `%APPDATA%\typewriter` on Windows) and
//! crash-safe writes.
//!
//! A project lives wherever the user saved it; until then it is a draft in
//! the data folder's `drafts/`, so nothing typed is lost.

use std::fs::{self, File, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use jiff::Timestamp;

/// A project file: a manila folder on screen, RON inside.
pub const EXTENSION: &str = ".folder.ron";
/// A draft's name.
pub const UNTITLED: &str = "Untitled";

/// `$<variable>/typewriter`, or `~/<fallback>/typewriter`. XDG says to
/// ignore relative paths.
#[cfg(not(windows))]
fn xdg_dir(variable: &str, fallback: &str) -> Option<PathBuf> {
    let base = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))?;
    Some(base.join("typewriter"))
}

/// Drafts, machine profiles and the app's markers.
#[cfg(not(windows))]
pub fn data_dir() -> Option<PathBuf> {
    xdg_dir("XDG_DATA_HOME", ".local/share")
}

#[cfg(not(windows))]
fn config_dir() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// `%APPDATA%\typewriter`, for config and data alike.
#[cfg(windows)]
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())?;
    Some(base.join("typewriter"))
}

#[cfg(windows)]
fn config_dir() -> Option<PathBuf> {
    data_dir()
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

/// The user's machine profiles.
pub fn profiles_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("profiles"))
}

fn drafts_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("drafts"))
}

/// A fresh draft path, named by its start time (UTC).
pub fn new_draft_path() -> Option<PathBuf> {
    let name = format!("draft-{}{EXTENSION}", timestamp(Timestamp::now()));
    drafts_dir().map(|dir| dir.join(name))
}

pub fn is_draft(path: &Path) -> bool {
    drafts_dir().is_some_and(|dir| path.starts_with(dir))
}

/// The file name, lossily, `""` if none.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The project's name, as on the folder's tab.
pub fn display_name(path: &Path) -> String {
    if is_draft(path) {
        return UNTITLED.to_owned();
    }
    let name = file_name(path);
    name.strip_suffix(EXTENSION).unwrap_or(&name).to_owned()
}

/// Shortens home paths to `~/…`. Windows users don't know `~`: full path.
pub fn home_relative(path: &Path) -> String {
    if cfg!(windows) {
        return path.display().to_string();
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home
        .as_deref()
        .and_then(|home| path.strip_prefix(home).ok())
    {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// `novel` or `novel.ron` → `novel.folder.ron`.
pub fn with_extension(path: PathBuf) -> PathBuf {
    let name = file_name(&path);
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

/// Writes a temp file beside `path`, then renames it over: a crash never
/// leaves a half-written file.
pub fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temporary = path.with_file_name(format!(".{}.tmp", file_name(path)));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(contents.as_ref())?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

/// Removed on a clean exit: found at start, the last run crashed.
fn running_marker() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("running"))
}

/// This run's marker, locked while the app runs. The OS drops the lock with
/// the process, so a marker found unlocked is a crashed run's.
pub struct Running(Option<File>);

impl Running {
    /// No marker: for tests.
    #[cfg(test)]
    pub fn nowhere() -> Self {
        Self(None)
    }

    /// Marks this run. True if the last run crashed (not merely still running).
    pub fn mark() -> (Self, bool) {
        match running_marker().map(|path| lock_marker(&path)) {
            Some(Ok(marked)) => marked,
            Some(Err(err)) => {
                eprintln!("could not mark the app as running: {err}");
                (Self(None), false)
            }
            None => (Self(None), false),
        }
    }

    /// Call on a clean exit.
    pub fn clear(&mut self) {
        // Only the run holding the lock: another still runs on it.
        if self.0.take().is_some()
            && let Some(marker) = running_marker()
        {
            let _ = fs::remove_file(marker);
        }
    }
}

fn lock_marker(path: &Path) -> io::Result<(Running, bool)> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let found = path.exists();
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok((Running(Some(file)), found)),
        Err(TryLockError::WouldBlock) => Ok((Running(None), false)),
        Err(TryLockError::Error(err)) => Err(err),
    }
}

fn last_project_record() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("last-folder"))
}

/// Last run's project, if it still exists.
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

/// `YYYY-MM-DD-HHMMSS`, UTC.
fn timestamp(at: Timestamp) -> String {
    at.strftime("%Y-%m-%d-%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_calendar_dates() {
        let at = |seconds| timestamp(Timestamp::from_second(seconds).unwrap());
        assert_eq!(at(0), "1970-01-01-000000");
        // 2026-09-29 14:30:05 UTC.
        assert_eq!(at(1_790_692_205), "2026-09-29-143005");
        assert_eq!(at(951_782_400), "2000-02-29-000000");
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

    #[test]
    fn a_marker_left_unlocked_is_a_crash() {
        let dir = std::env::temp_dir().join(format!("typewriter-marker-{}", std::process::id()));
        let path = dir.join("running");
        let (first, crashed) = lock_marker(&path).unwrap();
        assert!(first.0.is_some() && !crashed);
        let (second, crashed) = lock_marker(&path).unwrap();
        assert!(second.0.is_none() && !crashed, "still running, not crashed");
        drop(first);
        let (third, crashed) = lock_marker(&path).unwrap();
        assert!(third.0.is_some() && crashed);
        drop(third);
        fs::remove_dir_all(dir).unwrap();
    }
}
