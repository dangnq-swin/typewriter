//! One app instance for project files: a launch with a project, while the app is
//! open, hands it over a socket and exits, and the open app puts it in.
//! Unix only; elsewhere every launch opens its own window.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use eframe::egui::Context;

/// Projects handed over by later launches.
pub struct Listening {
    projects: Receiver<PathBuf>,
    socket: PathBuf,
}

impl Listening {
    /// The next project handed over, if any.
    pub fn take(&self) -> Option<PathBuf> {
        self.projects.try_recv().ok()
    }

    /// On a clean exit: later launches open their own window.
    pub fn stop(&self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Hands `project` to an open app. True if one took it.
pub fn hand_over(project: &Path) -> bool {
    socket_path().is_some_and(|socket| platform::hand_over(&socket, project))
}

/// Listens for projects, waking `ctx` for each. `None` if another app
/// already listens, or there is nowhere to.
pub fn listen(ctx: &Context) -> Option<Listening> {
    let socket = socket_path()?;
    platform::listen(socket, ctx.clone())
}

/// In the user's runtime folder: private to them, gone at logout.
#[cfg(unix)]
fn socket_path() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("typewriter.socket"))
}

#[cfg(not(unix))]
fn socket_path() -> Option<PathBuf> {
    None
}

#[cfg(unix)]
mod platform {
    use std::ffi::OsStr;
    use std::io::{self, BufRead, BufReader, Write};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;

    use eframe::egui::Context;

    use super::Listening;

    /// One path a connection, ended by a newline.
    pub fn hand_over(socket: &Path, project: &Path) -> bool {
        let Ok(project) = std::path::absolute(project) else {
            return false;
        };
        let bytes = project.as_os_str().as_bytes();
        // A newline would end the path early: open it here instead.
        if bytes.contains(&b'\n') {
            return false;
        }
        let Ok(mut stream) = UnixStream::connect(socket) else {
            return false;
        };
        stream
            .write_all(bytes)
            .and_then(|()| stream.write_all(b"\n"))
            .is_ok()
    }

    pub fn listen(socket: PathBuf, ctx: Context) -> Option<Listening> {
        let listener = match UnixListener::bind(&socket) {
            Ok(listener) => listener,
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
                // Left by a run that crashed, unless an app answers on it.
                if UnixStream::connect(&socket).is_ok() {
                    return None;
                }
                std::fs::remove_file(&socket).ok()?;
                UnixListener::bind(&socket).ok()?
            }
            Err(err) => {
                eprintln!("not taking projects from other launches: {err}");
                return None;
            }
        };
        let (sender, projects) = mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut line = Vec::new();
                if BufReader::new(stream).read_until(b'\n', &mut line).is_err() {
                    continue;
                }
                let path = line.strip_suffix(b"\n").unwrap_or(&line);
                if path.is_empty() {
                    continue;
                }
                if sender.send(PathBuf::from(OsStr::from_bytes(path))).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
        Some(Listening { projects, socket })
    }
}

#[cfg(not(unix))]
mod platform {
    use std::path::{Path, PathBuf};

    use eframe::egui::Context;

    use super::Listening;

    pub fn hand_over(_: &Path, _: &Path) -> bool {
        false
    }

    pub fn listen(_: PathBuf, _: Context) -> Option<Listening> {
        None
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn socket(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("typewriter-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("typewriter.socket")
    }

    fn received(listening: &Listening) -> Option<PathBuf> {
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            if let Some(path) = listening.take() {
                return Some(path);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    #[test]
    fn a_later_launch_hands_its_project_to_the_open_app() {
        let socket = socket("hand-over");
        let open = platform::listen(socket.clone(), Context::default()).unwrap();
        assert!(
            platform::listen(socket.clone(), Context::default()).is_none(),
            "one listens"
        );
        assert!(platform::hand_over(&socket, Path::new("/a/novel one.typr")));
        assert_eq!(received(&open).unwrap(), Path::new("/a/novel one.typr"));
        // Relative to where the launch ran.
        assert!(platform::hand_over(&socket, Path::new("novel.typr")));
        let path = received(&open).unwrap();
        assert!(path.is_absolute() && path.ends_with("novel.typr"));
        open.stop();
        std::fs::remove_dir_all(socket.parent().unwrap()).unwrap();
    }

    #[test]
    fn with_no_app_open_the_launch_opens_it_itself() {
        let socket = socket("none-open");
        assert!(!platform::hand_over(&socket, Path::new("/a/novel.typr")));
        // A crashed run's socket: nobody answers, so the next app takes it.
        drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
        assert!(!platform::hand_over(&socket, Path::new("/a/novel.typr")));
        let open = platform::listen(socket.clone(), Context::default()).unwrap();
        assert!(platform::hand_over(&socket, Path::new("/a/novel.typr")));
        assert!(received(&open).is_some());
        open.stop();
        std::fs::remove_dir_all(socket.parent().unwrap()).unwrap();
    }
}
