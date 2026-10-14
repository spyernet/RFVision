//! Scalar schema shared by acquisition and consumers, independent of any driver.
use crate::Record;
pub fn validate(r: &Record) -> Result<(), String> {
    for k in [
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
        if r.get(k).is_none_or(|v| v.trim().is_empty()) {
            return Err(format!("measurement missing {k}"));
        }
    }
    if r.get("validity").is_some_and(|v| v != "valid") {
        return Err("invalid/unknown scalar is not an observed measurement".into());
    }
    let v: f64 = r["value"]
        .parse()
        .map_err(|_| "invalid measurement number")?;
    if !v.is_finite() {
        return Err("non-finite measurement".into());
    }
    let t: u128 = r["observed_unix_ns"]
        .parse()
        .map_err(|_| "invalid timestamp")?;
    if t == 0 {
        return Err("zero timestamp is not an observation".into());
    }
    let name = r["name"].as_str();
    let unit = r["unit"].as_str();
    if matches!(name, "rssi" | "rssi_average" | "chain_rssi" | "noise")
        && (unit != "dBm" || !(-127. ..=0.).contains(&v))
    {
        return Err("invalid signal unit/range".into());
    }
    if name == "channel_frequency" && (unit != "MHz" || !(2000. ..=80000.).contains(&v)) {
        return Err("invalid frequency".into());
    }
    if name == "channel_width"
        && (unit != "MHz" || ![5., 10., 20., 40., 80., 160., 320.].contains(&v))
    {
        return Err("invalid bandwidth".into());
    }
    if matches!(name, "rx_bitrate" | "tx_bitrate") && (unit != "Mbit/s" || v < 0.) {
        return Err("invalid link rate".into());
    }
    if name == "configured_tx_power" && (unit != "dBm" || !(-100. ..=100.).contains(&v)) {
        return Err("invalid configured power".into());
    }
    if matches!(
        unit,
        "count" | "bytes" | "packets" | "ms" | "index_or_count"
    ) && r["value"].parse::<u64>().is_err()
    {
        return Err("counter/index must be unsigned integer".into());
    }
    if let Some(u) = r.get("uncertainty") {
        let n: f64 = u.parse().map_err(|_| "invalid uncertainty")?;
        if !n.is_finite() || n < 0. {
            return Err("invalid uncertainty".into());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed() {
        let mut r = crate::record(&[
            ("interface", "wlan0"),
            ("device_id", "test"),
            ("name", "rssi"),
            ("value", "-60"),
            ("unit", "dBm"),
            ("source", "algorithm_test"),
            ("observed_unix_ns", "1"),
            ("time_basis", "synthetic"),
            ("quality", "simulation"),
        ]);
        assert!(validate(&r).is_ok());
        for (k, v) in [
            ("observed_unix_ns", "0"),
            ("observed_unix_ns", "tomorrow"),
            ("value", "NaN"),
            ("value", "20"),
            ("unit", "mW"),
        ] {
            let mut a = r.clone();
            a.insert(k.into(), v.into());
            assert!(validate(&a).is_err());
        }
        r.insert("name".into(), "tx_retries".into());
        r.insert("unit".into(), "count".into());
        r.insert("value".into(), "1.5".into());
        assert!(validate(&r).is_err());
    }
}
