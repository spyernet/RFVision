//! Linux Unix-socket transport. Filesystem permissions are the v1 trust boundary.
use crate::*;
use std::fs;
use std::os::unix::{
    fs::{FileTypeExt, MetadataExt, PermissionsExt},
    net::{UnixListener, UnixStream},
};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

pub fn private_directory(path: &Path) -> io::Result<()> {
    let m = fs::symlink_metadata(path)?;
    let uid = fs::metadata("/proc/self")?.uid();
    if !m.is_dir() || m.uid() != uid || m.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket/install directory must be owned by this user, mode 0700, and not a symlink",
        ));
    }
    Ok(())
}
fn socket_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || path.as_os_str().len() > 100 {
        return Err(invalid("use an absolute socket path of at most 100 bytes"));
    }
    private_directory(path.parent().ok_or_else(|| invalid("socket parent"))?)
}
struct Deadline<'a> {
    socket: &'a mut UnixStream,
    end: Instant,
}
impl Read for Deadline<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self
            .end
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "frame deadline"))?;
        self.socket.set_read_timeout(Some(left))?;
        self.socket.read(buf)
    }
}
fn read_frame(socket: &mut UnixStream, seconds: u64) -> io::Result<Frame> {
    Frame::read(Deadline {
        socket,
        end: Instant::now() + Duration::from_secs(seconds),
    })
}
fn write_frame(socket: &mut UnixStream, frame: &Frame) -> io::Result<()> {
    // An encoded frame is bounded; write timeout limits a blocked peer.
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    frame.write(socket)
}
pub fn call(path: &Path, kind: u16, records: Vec<Record>) -> io::Result<Vec<Record>> {
    socket_path(path)?;
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_socket()
        || meta.uid() != fs::metadata("/proc/self")?.uid()
        || meta.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "untrusted socket",
        ));
    }
    let mut s = UnixStream::connect(path)?;
    let hello = Frame::request(HELLO, 1, vec![record(&[("protocol", "RFV/1.0")])]);
    write_frame(&mut s, &hello)?;
    let h = read_frame(&mut s, 3)?;
    if h.kind != HELLO
        || h.id != 1
        || h.flags != RESPONSE
        || h.records
            .first()
            .and_then(|r| r.get("protocol"))
            .map(String::as_str)
            != Some("RFV/1.0")
    {
        return Err(invalid("HELLO negotiation failed"));
    }
    let request = Frame::request(kind, 2, records);
    write_frame(&mut s, &request)?;
    let response = read_frame(&mut s, 20)?;
    if response.id != 2 || response.flags != RESPONSE {
        return Err(invalid("response correlation"));
    }
    if response.kind == ERROR {
        return Err(io::Error::other(json(&response.records)));
    }
    if response.kind != kind {
        return Err(invalid("response opcode"));
    }
    Ok(response.records)
}
fn connection(
    mut s: UnixStream,
    role: &str,
    handler: &impl Fn(u16, &[Record]) -> Result<Vec<Record>, String>,
) -> io::Result<()> {
    let hello = read_frame(&mut s, 3)?;
    if hello.flags != 0
        || hello.kind != HELLO
        || hello.records.len() != 1
        || hello.records[0].get("protocol").map(String::as_str) != Some("RFV/1.0")
    {
        write_frame(
            &mut s,
            &hello.failure("NEGOTIATION_REQUIRED", "send RFV/1.0 HELLO first"),
        )?;
        return Ok(());
    }
    write_frame(
        &mut s,
        &hello.reply(vec![record(&[
            ("protocol", "RFV/1.0"),
            ("role", role),
            ("operations", "discover,sample"),
            ("transport", "bounded-request-response"),
            ("max_frame", "1048576"),
        ])]),
    )?;
    let req = read_frame(&mut s, 3)?;
    let reply = if req.flags != 0 || req.id == hello.id || !matches!(req.kind, DISCOVER | SAMPLE) {
        req.failure(
            "BAD_REQUEST",
            "unknown opcode, flags, or repeated request ID",
        )
    } else if (req.kind == DISCOVER && !req.records.is_empty())
        || (req.kind == SAMPLE
            && (req.records.len() != 1
                || req.records[0].len() != 1
                || !req.records[0].contains_key("interface")))
    {
        req.failure("BAD_REQUEST", "invalid operation fields")
    } else {
        match handler(req.kind, &req.records) {
            Ok(records) => req.reply(records),
            Err(message) => req.failure("ACQUISITION_FAILED", &message),
        }
    };
    write_frame(&mut s, &reply)
}
struct SocketGuard {
    path: PathBuf,
    inode: u64,
    device: u64,
}
impl Drop for SocketGuard {
    fn drop(&mut self) {
        if let Ok(m) = fs::symlink_metadata(&self.path) {
            if m.ino() == self.inode && m.dev() == self.device {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}
pub fn serve<F>(path: &Path, role: &'static str, handler: F) -> io::Result<()>
where
    F: Fn(u16, &[Record]) -> Result<Vec<Record>, String> + Send + Sync + 'static,
{
    socket_path(path)?;
    // Never unlink a preexisting path, including a stale socket.
    let listener = UnixListener::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let meta = fs::symlink_metadata(path)?;
    let _guard = SocketGuard {
        path: path.to_owned(),
        inode: meta.ino(),
        device: meta.dev(),
    };
    let handler = Arc::new(handler);
    let active = Arc::new(AtomicUsize::new(0));
    eprintln!("{role}: listening on {} (RFV/1.0, passive)", path.display());
    for incoming in listener.incoming() {
        let stream = incoming?;
        if active.fetch_add(1, Ordering::AcqRel) >= 8 {
            active.fetch_sub(1, Ordering::AcqRel);
            drop(stream);
            continue;
        }
        let count = active.clone();
        let handler = handler.clone();
        std::thread::spawn(move || {
            struct Slot(Arc<AtomicUsize>);
            impl Drop for Slot {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::AcqRel);
                }
            }
            let _slot = Slot(count);
            if let Err(e) = connection(stream, role, &*handler) {
                eprintln!("IPC session ended: {e}");
            }
        });
    }
    Ok(())
}
