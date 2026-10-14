//! Fixed executable locations, argument vectors, bounded output and execution time.
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const LIMIT: u64 = 262_144;
fn trusted_binary(name: &str) -> io::Result<String> {
    if !matches!(name, "iw" | "ethtool") {
        return Err(io::Error::other("command not allowed"));
    }
    for prefix in ["/usr/sbin", "/usr/bin", "/sbin", "/bin"] {
        let p = format!("{prefix}/{name}");
        if let Ok(m) = fs::metadata(&p) {
            if m.is_file() && m.uid() == 0 && m.mode() & 0o022 == 0 && m.mode() & 0o111 != 0 {
                return Ok(p);
            }
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound,format!("trusted {name} executable not installed; install it using your distribution package manager")))
}
pub fn run(name: &str, args: &[&str]) -> io::Result<String> {
    let allowed = match (name, args) {
        ("iw", ["dev", interface, "info"])
        | ("iw", ["dev", interface, "station", "dump"])
        | ("iw", ["dev", interface, "survey", "dump"])
        | ("iw", ["dev", interface, "scan", "dump"])
        | ("ethtool", ["-i", interface]) => super::valid_interface(interface),
        ("iw", ["phy", phy, "info"]) => phy
            .strip_prefix("phy")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        _ => false,
    };
    if !allowed {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "only allowlisted read-only queries are permitted",
        ));
    }
    let program = trusted_binary(name)?;
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    fn drain(pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<io::Result<Vec<u8>>> {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            pipe.take(LIMIT + 1).read_to_end(&mut b)?;
            Ok(b)
        })
    }
    let stdout = drain(child.stdout.take().unwrap());
    let stderr = drain(child.stderr.take().unwrap());
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            other => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(match other {
                    Err(e) => e,
                    _ => io::Error::new(io::ErrorKind::TimedOut, "query exceeded two seconds"),
                });
            }
        }
    };
    let out = stdout
        .join()
        .map_err(|_| io::Error::other("stdout reader failed"))??;
    let err = stderr
        .join()
        .map_err(|_| io::Error::other("stderr reader failed"))??;
    let status = status?;
    if out.len() > LIMIT as usize || err.len() > LIMIT as usize {
        return Err(io::Error::other("query output limit"));
    }
    if !status.success() {
        return Err(io::Error::other(format!(
            "{name}: {status}: {}",
            String::from_utf8_lossy(&err).trim()
        )));
    }
    String::from_utf8(out).map_err(|_| io::Error::other("non UTF-8 query output"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_mutating_and_injected_commands() {
        for (name, args) in [
            ("iw", vec!["dev", "wlan0", "scan"]),
            (
                "iw",
                vec!["dev", "wlan0", "set", "txpower", "fixed", "3000"],
            ),
            ("ethtool", vec!["-s", "wlan0", "speed", "100"]),
            ("iw", vec!["dev", "../x", "info"]),
            ("sh", vec!["-c", "echo bad"]),
        ] {
            assert_eq!(
                run(name, &args).unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
        }
    }
}
