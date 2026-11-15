//! The listening side of `giop:unix` endpoints: where the socket goes,
//! taking over a stale one, and removing it again (docs/unix-transport.md).
//!
//! What omniORB does was observed from outside (the files it makes and the
//! IORs it publishes), and is followed where it is safe: a socket of mode
//! 0777 in `/tmp/omni-USER/` named `PID-SECONDS` by default. Unlike
//! omniORB, a file at the path that is not a socket, or a socket another
//! process still listens on, is left alone and the endpoint fails.

use std::ffi::CStr;
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::trace;

/// How many default socket names this process has made, so that two in the
/// same second differ.
static DEFAULT_NAMES: AtomicU32 = AtomicU32::new(0);

/// The name of the user the process runs as, from the password database
/// (not `$USER`, like omniORB); the uid if it has no entry.
fn user_name() -> String {
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    let mut pwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut buf = vec![0 as libc::c_char; 1024];
    let mut result = std::ptr::null_mut();
    loop {
        // SAFETY: the buffers are valid for the lengths given.
        let rc = unsafe {
            libc::getpwuid_r(
                uid,
                pwd.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut result,
            )
        };
        if rc == libc::ERANGE && buf.len() < 1 << 20 {
            buf.resize(buf.len() * 2, 0);
            continue;
        }
        break;
    }
    if result.is_null() {
        return uid.to_string();
    }
    // SAFETY: getpwuid_r succeeded, so pw_name points into `buf`.
    let name = unsafe { CStr::from_ptr((*result).pw_name) };
    name.to_string_lossy().into_owned()
}

/// A new socket name: `PID-SECONDS` (the pid in nine digits), with `-N`
/// after it from the second one of the process on.
fn default_name() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let n = DEFAULT_NAMES.fetch_add(1, Ordering::Relaxed);
    let base = format!("{:09}-{secs}", std::process::id());
    if n == 0 { base } else { format!("{base}-{n}") }
}

/// The path of the socket for `-ORBendPoint giop:unix:SPEC`: SPEC itself,
/// a new name in SPEC when it ends in `/`, a new name in `/tmp/omni-USER/`
/// (made if missing) when SPEC is empty.
pub(crate) fn socket_path(spec: &str) -> io::Result<PathBuf> {
    if spec.is_empty() {
        let dir = PathBuf::from(format!("/tmp/omni-{}", user_name()));
        if !dir.is_dir() {
            std::fs::DirBuilder::new().mode(0o755).create(&dir)?;
        }
        Ok(dir.join(default_name()))
    } else if spec.ends_with('/') {
        Ok(Path::new(spec).join(default_name()))
    } else {
        Ok(PathBuf::from(spec))
    }
}

/// A socket file this ORB made, removed when dropped (if it is still the
/// one made: another process may have replaced it since).
#[derive(Debug)]
pub(crate) struct SocketFile {
    path: PathBuf,
    dev: u64,
    ino: u64,
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        if let Ok(meta) = std::fs::symlink_metadata(&self.path) {
            if meta.dev() == self.dev && meta.ino() == self.ino {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

/// Listens on a Unix socket at `path`. A socket already there that nobody
/// listens on (left by a process that ended) is removed first; anything
/// else there makes it fail. The socket is made readable and writable by
/// everyone, like omniORB's: the directory decides who can reach it.
pub(crate) fn bind(path: &Path) -> io::Result<(UnixListener, SocketFile)> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if !meta.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists and is not a socket", path.display()),
            ));
        }
        if UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("{} is in use", path.display()),
            ));
        }
        trace::info!("removing the stale socket {}", path.display());
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o777))?;
    let meta = std::fs::symlink_metadata(path)?;
    Ok((
        listener,
        SocketFile {
            path: path.to_owned(),
            dev: meta.dev(),
            ino: meta.ino(),
        },
    ))
}

/// The host name of the machine (`gethostname`), which `giop:unix`
/// components carry: a client uses the socket only when it names the
/// client's own host.
pub(crate) fn host() -> &'static str {
    static HOST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HOST.get_or_init(|| {
        let mut buf = [0u8; 256];
        // SAFETY: the buffer is valid for its length.
        let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
        if rc != 0 {
            return "localhost".into();
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..end]).into_owned()
    })
}
