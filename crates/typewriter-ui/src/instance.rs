//! One app instance for project files: a launch with a project, while the app is
//! open, hands it over a local socket and exits, and the open app puts it in.
//! Unix domain sockets and Windows named pipes, through `interprocess`.

use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use eframe::egui::Context;
use interprocess::ConnectWaitMode;
use interprocess::local_socket::{
    ConnectOptions, GenericFilePath, GenericNamespaced, Listener, ListenerOptions, Name, Stream,
    prelude::*,
};

/// How long a launch waits for the open app to answer.
const ANSWER_WITHIN: Duration = Duration::from_millis(250);
/// Every mode shares one: the app's name in the desktop's IPC namespace.
#[cfg(unix)]
const PIPE: &str = "";
#[cfg(not(unix))]
const PIPE: &str = "typewriter";

/// Projects handed over by later launches.
pub struct Listening {
    projects: Receiver<PathBuf>,
    /// The Unix socket file, to take away on a clean exit.
    socket: Option<PathBuf>,
}

impl Listening {
    /// The next project handed over, if any.
    pub fn take(&self) -> Option<PathBuf> {
        self.projects.try_recv().ok()
    }

    /// Call on a clean exit: later launches open their own window.
    pub fn stop(&self) {
        if let Some(socket) = &self.socket {
            let _ = std::fs::remove_file(socket);
        }
    }
}

/// Hands `project` to an open app. True if one took it.
pub fn hand_over(project: &Path) -> bool {
    let socket = socket_path();
    match_name(&socket, PIPE, project)
}

/// Listens for projects, waking `ctx` for each. `None` if another app
/// already listens, or there is nowhere to.
pub fn listen(ctx: &Context) -> Option<Listening> {
    let socket = socket_path();
    listen_on(&socket, PIPE, ctx)
}

/// In the user's runtime folder: private to them, gone at logout. Pipes
/// have no path to keep.
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

/// The socket's name: a path where there is one, else `ns` in the desktop's
/// namespace. `ns` must outlive the returned name.
fn name<'s>(socket: Option<&'s Path>, ns: &'s str) -> Option<Name<'s>> {
    match socket {
        Some(socket) => socket.to_fs_name::<GenericFilePath>().ok(),
        None => ns.to_ns_name::<GenericNamespaced>().ok(),
    }
}

fn connect(name: Name<'_>) -> io::Result<Stream> {
    ConnectOptions::new()
        .name(name)
        .wait_mode(ConnectWaitMode::Timeout(ANSWER_WITHIN))
        .connect_sync()
}

/// Writes `project` to an open app's socket. True if an app took it.
fn match_name(socket: &Option<PathBuf>, ns: &str, project: &Path) -> bool {
    let Some(name) = name(socket.as_deref(), ns) else {
        return false;
    };
    let Ok(project) = std::path::absolute(project) else {
        return false;
    };
    let Some(message) = encode(&project) else {
        return false;
    };
    // The connect is the hand-off test: nothing answers where no app
    // listens, and a stale pipe or socket fails at once.
    connect(name).is_ok_and(|mut stream| stream.write_all(&message).is_ok())
}

fn listen_on(socket: &Option<PathBuf>, ns: &str, ctx: &Context) -> Option<Listening> {
    let listener = match ListenerOptions::new()
        .name(name(socket.as_deref(), ns)?)
        .create_sync()
    {
        Ok(listener) => listener,
        // A crashed run's socket file, left where the listener could not
        // take it away: reclaim it, unless an app answers on it. Then that
        // app takes projects, not this one.
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
            if connect(name(socket.as_deref(), ns)?).is_ok() {
                return None;
            }
            let socket_file = socket.as_ref()?;
            std::fs::remove_file(socket_file).ok()?;
            ListenerOptions::new()
                .name(name(Some(socket_file.as_path()), ns)?)
                .create_sync()
                .ok()?
        }
        Err(_) => return None,
    };
    Some(Listening {
        projects: accept(listener, ctx.clone()),
        socket: socket.clone(),
    })
}

/// Hands each project to `sender`, waking `ctx` for it.
fn accept(listener: Listener, ctx: Context) -> Receiver<PathBuf> {
    let (sender, projects) = mpsc::channel();
    std::thread::spawn(move || {
        // Windows accepts on demand: a connected-but-unaccepted pipe
        // instance blocks every later client.
        for stream in listener.incoming().flatten() {
            let mut message = Vec::new();
            if BufReader::new(stream)
                .read_until(b'\n', &mut message)
                .is_err()
            {
                continue;
            }
            let line = message.strip_suffix(b"\n").unwrap_or(&message);
            // A message the wire cannot carry as a path never comes from
            // `encode`: a foreign sender, to be ignored.
            if let Some(project) = decode(line) {
                if sender.send(project).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        }
    });
    projects
}

/// One path a connection, ended by a newline. A newline in the path would
/// end it early: the launch opens the file itself instead.
#[cfg(unix)]
fn encode(path: &Path) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    (!bytes.contains(&b'\n')).then(|| [bytes, b"\n"].concat())
}

/// Windows keeps paths as UTF-16 behind an `OsStr` no byte channel can carry
/// whole: send the text, and a path that is not text opens its own window.
#[cfg(not(unix))]
fn encode(path: &Path) -> Option<Vec<u8>> {
    let text = path.to_str()?;
    (!text.contains('\n')).then(|| [text.as_bytes(), b"\n"].concat())
}

#[cfg(unix)]
fn decode(bytes: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    (!bytes.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(not(unix))]
fn decode(bytes: &[u8]) -> Option<PathBuf> {
    let text = std::str::from_utf8(bytes).ok()?;
    (!text.is_empty()).then(|| PathBuf::from(text))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    /// A socket no other test run collides with: a path on Unix, a leaked
    /// namespace name elsewhere.
    fn test_socket(tag: &str) -> (Option<PathBuf>, &'static str) {
        let unique = format!("typewriter-{tag}-{}", std::process::id());
        #[cfg(unix)]
        {
            let dir = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&dir).unwrap();
            (Some(dir.join("typewriter.socket")), "")
        }
        #[cfg(not(unix))]
        {
            (None, Box::leak(unique.into_boxed_str()))
        }
    }

    fn clean_up(socket: &Option<PathBuf>) {
        #[cfg(unix)]
        if let Some(socket) = socket {
            std::fs::remove_dir_all(socket.parent().unwrap()).unwrap();
        }
        #[cfg(not(unix))]
        let _ = socket;
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
        let (socket, ns) = test_socket("hand-over");
        let open = listen_on(&socket, ns, &Context::default()).unwrap();
        assert!(
            listen_on(&socket, ns, &Context::default()).is_none(),
            "one listens"
        );
        assert!(match_name(&socket, ns, Path::new(project_one())));
        assert_eq!(received(&open).unwrap(), Path::new(project_one()));
        // Relative to where the launch ran.
        assert!(match_name(&socket, ns, Path::new("novel.typr")));
        let path = received(&open).unwrap();
        assert!(path.is_absolute() && path.ends_with("novel.typr"));
        open.stop();
        clean_up(&socket);
    }

    #[test]
    fn with_no_app_open_the_launch_opens_it_itself() {
        let (socket, ns) = test_socket("none-open");
        assert!(!match_name(&socket, ns, Path::new(project_one())));
        // A crashed run's socket: nobody answers, so the next app takes it.
        #[cfg(unix)]
        drop(std::os::unix::net::UnixListener::bind(socket.as_ref().unwrap()).unwrap());
        #[cfg(not(unix))]
        drop(
            ListenerOptions::new()
                .name(name(None, ns).unwrap())
                .create_sync()
                .unwrap(),
        );
        assert!(!match_name(&socket, ns, Path::new(project_one())));
        let open = listen_on(&socket, ns, &Context::default()).unwrap();
        assert!(match_name(&socket, ns, Path::new(project_one())));
        assert!(received(&open).is_some());
        open.stop();
        clean_up(&socket);
    }

    #[cfg(unix)]
    fn project_one() -> &'static str {
        "/a/novel one.typr"
    }

    #[cfg(not(unix))]
    fn project_one() -> &'static str {
        "C:\\a\\novel one.typr"
    }
}
