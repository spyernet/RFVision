//! Public Linux interfaces only. All acquisition is read-only and request-driven.
#![forbid(unsafe_code)]
use rfv_ipc::{record, Record};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub mod command;
pub mod environment;
pub mod measurement;
pub mod research;

#[derive(Debug, Clone)]
pub struct Device {
    pub interface: String,
    pub identity: String,
    pub driver: String,
    pub fields: Record,
}
pub fn valid_interface(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 15
        && !s.starts_with('-')
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
pub fn read_text(path: impl AsRef<Path>) -> Option<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(65537)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 65536 {
        return None;
    }
    Some(String::from_utf8(bytes).ok()?.trim().to_owned())
}
pub fn now_ns() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}
fn link_name(p: PathBuf) -> Option<String> {
    fs::canonicalize(p)
        .ok()?
        .file_name()?
        .to_str()
        .map(str::to_owned)
}
pub fn discover(root: &Path) -> io::Result<Vec<Device>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root.join("class/net"))? {
        let entry = entry?;
        let path = entry.path();
        if !path.join("wireless").exists() && !path.join("phy80211").exists() {
            continue;
        }
        let interface = entry.file_name().to_string_lossy().into_owned();
        if !valid_interface(&interface) {
            continue;
        }
        let Some(index) = read_text(path.join("ifindex")) else {
            continue;
        };
        let devpath = fs::canonicalize(path.join("device")).ok();
        let identity = format!(
            "{}#{}",
            devpath
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("net:{interface}")),
            index
        );
        let driver = link_name(path.join("device/driver")).unwrap_or_else(|| "unknown".into());
        let mut fields = record(&[
            ("kind", "device"),
            ("interface", &interface),
            ("device_id", &identity),
            ("ifindex", &index),
            ("driver", &driver),
            ("source", "sysfs"),
            ("hardware_validated", "false"),
        ]);
        for (key, file) in [
            ("operstate", "operstate"),
            ("carrier", "carrier"),
            ("modalias", "device/modalias"),
            ("driver_version", "device/driver/module/version"),
        ] {
            if let Some(value) = read_text(path.join(file)) {
                fields.insert(key.into(), value);
            }
        }
        if let Some(phy) = link_name(path.join("phy80211")) {
            fields.insert("phy".into(), phy);
        }
        if let Some(module) = link_name(path.join("device/driver/module")) {
            fields.insert("module".into(), module);
        }
        // USB netdev's device is usually an interface; IDs live on a parent.
        if let Some(mut parent) = devpath {
            for _ in 0..6 {
                let pci = read_text(parent.join("vendor")).zip(read_text(parent.join("device")));
                let usb =
                    read_text(parent.join("idVendor")).zip(read_text(parent.join("idProduct")));
                if let Some((bus, (vendor, product))) =
                    pci.map(|v| ("pci", v)).or_else(|| usb.map(|v| ("usb", v)))
                {
                    fields.insert("bus".into(), bus.into());
                    fields.insert(
                        "vendor_id".into(),
                        vendor.trim_start_matches("0x").to_lowercase(),
                    );
                    fields.insert(
                        "product_id".into(),
                        product.trim_start_matches("0x").to_lowercase(),
                    );
                    break;
                }
                if !parent.pop() {
                    break;
                }
            }
        }
        out.push(Device {
            interface,
            identity,
            driver,
            fields,
        });
    }
    out.sort_by(|a, b| a.interface.cmp(&b.interface));
    Ok(out)
}
pub fn diagnostic(interface: &str, code: &str, message: &str) -> Record {
    record(&[
        ("kind", "diagnostic"),
        ("interface", interface),
        ("code", code),
        ("message", &message.chars().take(4096).collect::<String>()),
    ])
}
pub fn inventory(root: &Path) -> io::Result<Vec<Record>> {
    let devices = discover(root)?;
    let mut out = vec![record(&[
        ("kind", "host"),
        (
            "kernel",
            &read_text("/proc/sys/kernel/osrelease").unwrap_or_else(|| "unknown".into()),
        ),
        ("device_count", &devices.len().to_string()),
        (
            "scope",
            "wireless network interfaces visible in this namespace",
        ),
    ])];
    out.extend(devices.into_iter().map(|d| d.fields));
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interface_names() {
        for s in ["wlan0", "wlp2s0", "wlx112233445566"] {
            assert!(valid_interface(s));
        }
        for s in ["", "../wlan0", "-x", "x;y", "a b", ".", "abcdefghijklmnop"] {
            assert!(!valid_interface(s));
        }
    }
}

#[cfg(test)]
mod discovery_tests;
