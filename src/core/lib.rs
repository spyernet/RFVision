//! Driver-independent validation, capability reconciliation and signal primitives.
#![forbid(unsafe_code)]
use rfv_ipc::{record, Record};
pub mod csi;
pub const STATUSES: &[&str] = &[
    "AVAILABLE",
    "AVAILABLE_WITH_ADAPTER",
    "REQUIRES_DRIVER_MODIFICATION",
    "REQUIRES_FIRMWARE_SUPPORT",
    "REQUIRES_HARDWARE_SUPPORT",
    "UNSUPPORTED",
    "UNVERIFIED",
];

pub fn process(mut rows: Vec<Record>) -> Result<Vec<Record>, String> {
    // An adapter's declarations are not measurements. Only this batch's values can
    // promote a conditional capability to AVAILABLE for this interface and identity.
    for r in &rows {
        match r.get("kind").map(String::as_str) {
            Some("measurement") => {
                for key in [
                    "interface",
                    "device_id",
                    "name",
                    "value",
                    "unit",
                    "source",
                    "observed_unix_ns",
                    "time_basis",
                    "quality",
                ] {
                    if r.get(key).is_none_or(|v| v.is_empty()) {
                        return Err(format!("measurement missing {key}"));
                    }
                }
                if matches!(
                    r["name"].as_str(),
                    "csi"
                        | "csi_amplitude"
                        | "csi_phase"
                        | "complex_channel_estimates"
                        | "per_subcarrier"
                        | "channel_frequency_response"
                        | "channel_impulse_response"
                ) {
                    return Err("CSI requires a validated complex-data transport; scalar CSI is not supported".into());
                }
                let value = r["value"]
                    .parse::<f64>()
                    .map_err(|_| "non-numeric measurement")?;
                if !value.is_finite() {
                    return Err("non-finite measurement".into());
                }
                if r["observed_unix_ns"].parse::<u128>().is_err() {
                    return Err("invalid observation timestamp".into());
                }
                if !rows.iter().any(|d| {
                    d.get("kind").map(String::as_str) == Some("device")
                        && d.get("device_id") == r.get("device_id")
                        && d.get("interface") == r.get("interface")
                }) {
                    return Err("orphan measurement".into());
                }
            }
            Some("capability") => {
                if !STATUSES.contains(&r.get("status").map(String::as_str).unwrap_or("")) {
                    return Err("invalid capability status".into());
                }
                for key in [
                    "interface",
                    "device_id",
                    "feature",
                    "implementation",
                    "evidence",
                ] {
                    if r.get(key).is_none_or(|s| s.is_empty()) {
                        return Err(format!("capability missing {key}"));
                    }
                }
            }
            Some(_) => {}
            None => return Err("record has no kind".into()),
        }
    }
    let measurements: Vec<_> = rows
        .iter()
        .filter(|r| r.get("kind").map(String::as_str) == Some("measurement"))
        .cloned()
        .collect();
    for r in &mut rows {
        if r.get("kind").map(String::as_str) != Some("capability") {
            continue;
        }
        if let Some(m) = measurements.iter().find(|m| {
            m.get("quality").map(String::as_str) != Some("simulation")
                && m.get("time_basis").map(String::as_str) != Some("synthetic")
                && m.get("name") == r.get("feature")
                && m.get("interface") == r.get("interface")
                && m.get("device_id") == r.get("device_id")
        }) {
            r.insert("status".into(), "AVAILABLE".into());
            r.insert(
                "evidence".into(),
                format!(
                    "Observed in this batch via {}; availability may change",
                    m["source"]
                ),
            );
            r.insert("observed_unix_ns".into(), m["observed_unix_ns"].clone());
        } else if r.get("status").map(String::as_str) == Some("AVAILABLE") {
            r.insert("status".into(), "UNVERIFIED".into());
            r.insert(
                "evidence".into(),
                "No corresponding measurement in this batch".into(),
            );
        }
    }
    rows.push(record(&[
        ("kind", "processing"),
        ("version", env!("CARGO_PKG_VERSION")),
        ("measurements", &measurements.len().to_string()),
        ("safety", "read_only_no_driver_changes"),
        ("calibration", "not_applied"),
        ("csi_reconstruction", "not_performed"),
    ]));
    Ok(rows)
}
/// Counter reset/rollover/device replacement is not a negative traffic rate.
pub fn counter_delta(previous: u64, current: u64, same_device: bool) -> Option<u64> {
    if same_device {
        current.checked_sub(previous)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_capability_from_nothing() {
        let rows = vec![record(&[
            ("kind", "capability"),
            ("interface", "wlan0"),
            ("device_id", "x"),
            ("feature", "csi"),
            ("status", "AVAILABLE"),
            ("implementation", "test"),
            ("evidence", "claim"),
        ])];
        assert_eq!(process(rows).unwrap()[0]["status"], "UNVERIFIED");
    }
    #[test]
    fn no_unknown_states() {
        assert!(process(vec![record(&[("kind", "capability"), ("status", "MAGIC")])]).is_err());
    }
    #[test]
    fn reset_is_not_traffic() {
        assert_eq!(counter_delta(9, 1, true), None);
        assert_eq!(counter_delta(1, 9, false), None);
        assert_eq!(counter_delta(1, 9, true), Some(8));
    }
}

#[cfg(test)]
mod measurement_tests {
    use super::*;
    fn batch() -> Vec<Record> {
        vec![
            record(&[
                ("kind", "device"),
                ("interface", "test0"),
                ("device_id", "fixture"),
            ]),
            record(&[
                ("kind", "measurement"),
                ("interface", "test0"),
                ("device_id", "fixture"),
                ("name", "rssi"),
                ("value", "-50"),
                ("unit", "dBm"),
                ("source", "unit_fixture"),
                ("observed_unix_ns", "1"),
                ("time_basis", "fixture"),
                ("quality", "simulation"),
            ]),
            record(&[
                ("kind", "capability"),
                ("interface", "test0"),
                ("device_id", "fixture"),
                ("feature", "rssi"),
                ("status", "AVAILABLE_WITH_ADAPTER"),
                ("implementation", "fixture"),
                ("evidence", "test"),
            ]),
        ]
    }
    #[test]
    fn simulation_is_not_hardware_support() {
        assert_eq!(
            process(batch()).unwrap()[2]["status"],
            "AVAILABLE_WITH_ADAPTER"
        );
    }
    #[test]
    fn observed_status_scoped_to_identity() {
        let mut b = batch();
        b[1].insert("quality".into(), "driver_reported_uncalibrated".into());
        assert_eq!(process(b.clone()).unwrap()[2]["status"], "AVAILABLE");
        b[2].insert("device_id".into(), "another-device".into());
        assert_eq!(process(b).unwrap()[2]["status"], "AVAILABLE_WITH_ADAPTER");
    }
    #[test]
    fn malformed_nan_or_scalar_csi_rejected() {
        let mut b = batch();
        b[1].insert("value".into(), "NaN".into());
        assert!(process(b).is_err());
        let mut b = batch();
        b[1].remove("source");
        assert!(process(b).is_err());
        let mut b = batch();
        b[1].insert("name".into(), "csi".into());
        assert!(process(b).is_err());
        let mut b = batch();
        b[1].insert("device_id".into(), "orphan".into());
        assert!(process(b).is_err());
    }
}
