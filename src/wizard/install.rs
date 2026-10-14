//! Explicit, user-local installation only. No kernel/network/package-manager actions.
use rfv_ipc::{record, Record};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
};
const BINS: [&str; 3] = ["rfv", "rfvd", "rfv-adapter-rtw88"];
const RTW88_BINS: [&str; 1] = ["rfv-adapter-rtw88"];
fn private_parent(prefix: &Path) -> io::Result<()> {
    if !prefix.is_absolute() || prefix.file_name().is_none() {
        return Err(io::Error::other("installation prefix must be absolute"));
    }
    rfv_ipc::transport::private_directory(
        prefix
            .parent()
            .ok_or_else(|| io::Error::other("no parent"))?,
    )
}
fn read_binary(path: &Path) -> io::Result<Vec<u8>> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.mode() & 0o111 == 0
        || m.len() > 64 * 1024 * 1024
    {
        return Err(io::Error::other(
            "source must be a regular executable <=64 MiB, not a symlink",
        ));
    }
    let mut b = Vec::new();
    fs::File::open(path)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut b)?;
    if b.len() > 64 * 1024 * 1024 {
        return Err(io::Error::other("binary grew past limit"));
    }
    Ok(b)
}
// Accidental-change detection only; NOT a cryptographic signature or trust check.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}
pub fn plan(prefix: &Path, source: &Path) -> Vec<Record> {
    plan_bins(prefix, source, &BINS)
}
pub fn plan_rtw88(prefix: &Path, source: &Path) -> Vec<Record> {
    plan_bins(prefix, source, &RTW88_BINS)
}
fn plan_bins(prefix: &Path, source: &Path, bins: &[&str]) -> Vec<Record> {
    let actions = if bins == &RTW88_BINS[..] {
        "create new private prefix; copy only rfv-adapter-rtw88 userspace IPC adapter; write ownership manifest"
    } else {
        "create new private prefix; copy rfv, rfvd, rfv-adapter-rtw88; write ownership manifest"
    };
    vec![record(&[
        ("kind", "installation_plan"),
        ("prefix", &prefix.display().to_string()),
        ("source", &source.display().to_string()),
        ("actions", actions),
        ("driver_changes", "none"),
        ("network_changes", "none"),
        ("privilege_escalation", "none"),
        ("autostart", "none"),
        (
            "requires",
            "explicit --apply; existing private parent owned by current user; destination absent; trusted prebuilt executables",
        ),
        (
            "rollback",
            "explicit rollback PREFIX --apply; refuse modified or unexpected files",
        ),
    ])]
}
pub fn install(prefix: &Path, source: &Path) -> io::Result<Vec<Record>> {
    install_bins(prefix, source, &BINS)
}
pub fn install_rtw88(prefix: &Path, source: &Path) -> io::Result<Vec<Record>> {
    install_bins(prefix, source, &RTW88_BINS)
}
fn install_bins(prefix: &Path, source: &Path, bins: &[&str]) -> io::Result<Vec<Record>> {
    private_parent(prefix)?;
    // Read all inputs before changing anything. No binaries are executed here.
    let mut inputs = Vec::new();
    for &name in bins {
        inputs.push((name, read_binary(&source.join(name))?));
    }
    fs::DirBuilder::new().mode(0o700).create(prefix)?; // create_new directory: no replacement
    let mut created = Vec::new();
    let result = (|| {
        let mut manifest = String::from("RFV-INSTALL-1\n");
        for (name, bytes) in &inputs {
            let name = *name;
            let path = prefix.join(name);
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o700)
                .open(&path)?;
            created.push(path);
            f.write_all(bytes)?;
            f.sync_all()?;
            manifest.push_str(&format!(
                "{name}\t{}\t{:016x}\n",
                bytes.len(),
                checksum(bytes)
            ));
        }
        let path = prefix.join("manifest.tsv");
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        created.push(path);
        f.write_all(manifest.as_bytes())?;
        f.sync_all()?;
        let note = if bins == &RTW88_BINS[..] {
            "start the userspace rtw88 IPC adapter explicitly; no system Wi-Fi driver changed"
        } else {
            "start adapter and core explicitly using separate private runtime directory"
        };
        Ok(vec![record(&[
            ("kind", "installation"),
            ("status", "installed"),
            ("prefix", &prefix.display().to_string()),
            ("running", "false"),
            ("driver_changes", "none"),
            ("note", note),
        ])])
    })();
    if result.is_err() {
        for path in created.iter().rev() {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_dir(prefix);
    }
    result
}
pub fn rollback(prefix: &Path) -> io::Result<Vec<Record>> {
    private_parent(prefix)?;
    rfv_ipc::transport::private_directory(prefix)?;
    let mut names: Vec<_> = fs::read_dir(prefix)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<_>>()?;
    names.sort();
    let bins = if names == install_names(&BINS) {
        &BINS[..]
    } else if names == install_names(&RTW88_BINS) {
        &RTW88_BINS[..]
    } else {
        return Err(io::Error::other(
            "rollback refused: unexpected/missing files; no changes made",
        ));
    };
    let manifest_path = prefix.join("manifest.tsv");
    let m = fs::symlink_metadata(&manifest_path)?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > 4096 {
        return Err(io::Error::other("untrusted manifest"));
    }
    let manifest = fs::read_to_string(&manifest_path)?;
    let mut recomputed = String::from("RFV-INSTALL-1\n");
    for &name in bins {
        let data = read_binary(&prefix.join(name))?;
        recomputed.push_str(&format!(
            "{name}\t{}\t{:016x}\n",
            data.len(),
            checksum(&data)
        ));
    }
    if manifest != recomputed {
        return Err(io::Error::other(
            "rollback refused: installed files changed; no changes made",
        ));
    }
    // Only fixed allowlisted names are removed. Never recursively delete a prefix.
    for &name in bins {
        fs::remove_file(prefix.join(name))?;
    }
    fs::remove_file(manifest_path)?;
    fs::remove_dir(prefix)?;
    Ok(vec![record(&[
        ("kind", "rollback"),
        ("status", "removed_owned_installation"),
        ("driver_changes", "none"),
        ("note", "running processes must be stopped separately"),
    ])])
}
fn install_names(bins: &[&str]) -> Vec<String> {
    let mut names = vec!["manifest.tsv".to_string()];
    names.extend(bins.iter().map(|n| n.to_string()));
    names.sort();
    names
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn checksum_detects_edits() {
        assert_ne!(checksum(b"one"), checksum(b"two"));
    }
    #[test]
    fn refuse_public_parent() {
        let path = std::env::temp_dir().join(format!("rfv-public-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(private_parent(&path.join("install")).is_err());
        fs::remove_dir(path).unwrap();
    }
}
