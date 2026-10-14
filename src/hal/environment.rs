//! Read-only cached BSS and advertised power parsing. Never triggers a scan.
use crate::Device;
use rfv_ipc::{record, Record};
fn mac(s: &str) -> bool {
    s.len() == 17
        && s.split(':').count() == 6
        && s.split(':')
            .all(|x| x.len() == 2 && x.bytes().all(|b| b.is_ascii_hexdigit()))
}
pub fn cached_networks(d: &Device, text: &str, time: &str) -> Vec<Record> {
    let mut rows = Vec::new();
    let mut current: Option<Record> = None;
    for line in text.lines().map(str::trim) {
        if let Some(head) = line.strip_prefix("BSS ") {
            if let Some(r) = current.take() {
                rows.push(r);
            }
            let bssid = head.split(['(', ' ']).next().unwrap_or("");
            if mac(bssid) {
                current = Some(record(&[
                    ("kind", "network"),
                    ("interface", &d.interface),
                    ("device_id", &d.identity),
                    ("bssid", bssid),
                    ("source", "iw.scan_cache"),
                    ("observed_unix_ns", time),
                    ("time_basis", "cache_query_not_beacon_time"),
                    ("freshness", "unknown_unless_last_seen_present"),
                    (
                        "compatibility",
                        "unknown; BSS visibility does not prove repeater or mesh support",
                    ),
                ]));
            }
        } else if let Some(r) = &mut current {
            if let Some(v) = line.strip_prefix("SSID: ") {
                r.insert("ssid".into(), v.chars().take(128).collect());
            }
            for (prefix, key, unit, low, high) in [
                ("signal:", "rssi_dbm", "dBm", -127., 0.),
                ("freq:", "frequency_mhz", "", 2000., 80000.),
                ("last seen:", "last_seen_ms", "ms", 0., f64::MAX),
            ] {
                if let Some(v) = line.strip_prefix(prefix) {
                    let words: Vec<_> = v.split_whitespace().collect();
                    if let Some(n) = words
                        .first()
                        .and_then(|s| s.parse::<f64>().ok())
                        .filter(|n| n.is_finite() && *n >= low && *n <= high)
                    {
                        if unit.is_empty() || words.get(1) == Some(&unit) {
                            r.insert(key.into(), n.to_string());
                        }
                    }
                }
            }
        }
    }
    if let Some(r) = current {
        rows.push(r);
    }
    rows.truncate(256);
    rows
}
/// iw phy's effective per-channel advertised ceiling is NOT a full adjustable
/// range and not necessarily the permitted EIRP for an arbitrary antenna.
pub fn power_limits(d: &Device, text: &str, time: &str) -> Vec<Record> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| l.starts_with("* ")) {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.get(2) != Some(&"MHz") {
            continue;
        }
        let Some(freq) = words
            .get(1)
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|f| (2000..=80000).contains(f))
        else {
            continue;
        };
        let mut r=record(&[("kind","power_limit"),("interface",&d.interface),("device_id",&d.identity),("frequency_mhz",&freq.to_string()),("source","iw.phy"),("basis","driver_advertised_effective_limit"),("observed_unix_ns",time),("constraints",line),("minimum_dbm","unknown"),("interpretation","not a settable range or authorization; hardware, firmware, regulatory, SAR and antenna constraints apply")]);
        if line.contains("disabled") {
            r.insert("status".into(), "disabled".into());
        } else if let Some((head, _)) = line.split_once(" dBm)") {
            if let Some(n) = head
                .rsplit('(')
                .next()
                .and_then(|n| n.parse::<f64>().ok())
                .filter(|n| n.is_finite() && (-100. ..=100.).contains(n))
            {
                r.insert("maximum_dbm".into(), n.to_string());
                r.insert("status".into(), "advertised".into());
            }
        }
        if r.contains_key("status") {
            out.push(r);
        }
    }
    out.truncate(256);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    fn d() -> Device {
        Device {
            interface: "test".into(),
            identity: "fixture".into(),
            driver: "fixture".into(),
            fields: Record::new(),
        }
    }
    #[test]
    fn cached_discovery() {
        let r=cached_networks(&d(),"BSS 01:23:45:67:89:ab(on test)\n freq: 2412\n signal: -65.25 dBm\n SSID: office\n last seen: 34 ms ago\nBSS bad\n signal: -30 dBm","1");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["rssi_dbm"], "-65.25");
        assert_eq!(r[0]["last_seen_ms"], "34");
        assert!(cached_networks(&d(), "", "1").is_empty());
    }
    #[test]
    fn no_fabricated_power_range() {
        let r = power_limits(
            &d(),
            "* 2412 MHz [1] (20.0 dBm)\n* 5260 MHz [52] (disabled)\n* 5180 MHz [36] (NaN dBm)",
            "1",
        );
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["minimum_dbm"], "unknown");
        assert!(!r[1].contains_key("maximum_dbm"));
    }
}
