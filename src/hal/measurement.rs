use super::*;

fn measurement(
    d: &Device,
    name: &str,
    value: impl ToString,
    unit: &str,
    source: &str,
    time: &str,
) -> Record {
    record(&[
        ("kind", "measurement"),
        ("source_driver", &d.driver),
        ("measurement_type", "scalar"),
        ("validity", "valid"),
        ("calibration", "not_applied"),
        ("capability_status", "observed"),
        ("interface", &d.interface),
        ("device_id", &d.identity),
        ("name", name),
        ("value", &value.to_string()),
        ("unit", unit),
        ("source", source),
        ("observed_unix_ns", time),
        ("time_basis", "host_query_observation_not_rx_time"),
        ("quality", "driver_reported_uncalibrated"),
    ])
}
fn signed(s: &str) -> Option<i64> {
    s.split_whitespace().next()?.parse().ok()
}
fn unsigned(s: &str) -> Option<u64> {
    s.split_whitespace().next()?.parse().ok()
}
fn add_context(mut r: Record, peer: &str) -> Record {
    if !peer.is_empty() {
        r.insert("peer".into(), peer.into());
    }
    r
}
/// Independent parser of iw's human-readable output, not an nl80211 ABI promise.
/// Unknown/malformed values are omitted, never converted to zero.
pub fn parse_station(d: &Device, text: &str, time: &str) -> Vec<Record> {
    let mut out = Vec::new();
    let mut peer = "";
    for line in text.lines().map(str::trim) {
        if let Some(p) = line.strip_prefix("Station ") {
            peer = p.split_whitespace().next().unwrap_or("");
            continue;
        }
        if peer.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            let metric = match key {
                "signal" => Some("rssi"),
                "signal avg" => Some("rssi_average"),
                _ => None,
            };
            if let Some(name) = metric {
                if value.ends_with("dBm") {
                    if let Some(v) = signed(value).filter(|v| (-127..=0).contains(v)) {
                        out.push(add_context(
                            measurement(d, name, v, "dBm", "iw.station", time),
                            peer,
                        ));
                    }
                    if key == "signal" {
                        if let Some((_, tail)) = value.split_once('[') {
                            if let Some((chains, _)) = tail.split_once(']') {
                                for (i, val) in chains.split(',').take(8).enumerate() {
                                    if let Ok(v) = val.trim().parse::<i64>() {
                                        if (-127..=0).contains(&v) {
                                            let mut r = add_context(
                                                measurement(
                                                    d,
                                                    "chain_rssi",
                                                    v,
                                                    "dBm",
                                                    "iw.station",
                                                    time,
                                                ),
                                                peer,
                                            );
                                            r.insert("chain_index".into(), i.to_string());
                                            out.push(r);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for (label, name, unit) in [
                ("rx bytes", "station_rx_bytes", "bytes"),
                ("tx bytes", "station_tx_bytes", "bytes"),
                ("rx packets", "station_rx_packets", "packets"),
                ("tx packets", "station_tx_packets", "packets"),
                ("tx retries", "tx_retries", "count"),
                ("tx failed", "tx_failed", "count"),
                ("rx drop misc", "rx_drop_misc", "count"),
            ] {
                if key == label {
                    if let Some(v) = unsigned(value) {
                        out.push(add_context(
                            measurement(d, name, v, unit, "iw.station", time),
                            peer,
                        ));
                    }
                }
            }
            if matches!(key, "rx bitrate" | "tx bitrate") {
                let direction = if key == "rx bitrate" { "rx" } else { "tx" };
                let words: Vec<_> = value.split_whitespace().collect();
                if words.get(1) == Some(&"MBit/s") {
                    if let Some(v) = words
                        .first()
                        .and_then(|w| w.parse::<f64>().ok())
                        .filter(|v| v.is_finite() && *v >= 0.0)
                    {
                        out.push(add_context(
                            measurement(
                                d,
                                &format!("{direction}_bitrate"),
                                v,
                                "Mbit/s",
                                "iw.station",
                                time,
                            ),
                            peer,
                        ));
                    }
                }
                for pair in words.windows(2) {
                    if matches!(
                        pair[0],
                        "MCS" | "VHT-MCS" | "HE-MCS" | "EHT-MCS" | "VHT-NSS" | "HE-NSS" | "EHT-NSS"
                    ) {
                        if let Ok(v) = pair[1].parse::<u8>() {
                            let metric = if pair[0].ends_with("NSS") {
                                "nss"
                            } else {
                                "mcs"
                            };
                            let mut r = add_context(
                                measurement(
                                    d,
                                    &format!("{direction}_{metric}"),
                                    v,
                                    "index_or_count",
                                    "iw.station",
                                    time,
                                ),
                                peer,
                            );
                            r.insert("encoding".into(), pair[0].into());
                            out.push(r);
                        }
                    }
                }
            }
        }
    }
    out
}
pub fn parse_info(d: &Device, text: &str, time: &str) -> Vec<Record> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(v) = line
            .strip_prefix("txpower ")
            .and_then(|s| s.strip_suffix(" dBm"))
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite() && (-100. ..=100.).contains(v))
        {
            let mut r = measurement(d, "configured_tx_power", v, "dBm", "iw.interface", time);
            r.insert(
                "basis".into(),
                "configured_not_measured_radiated_power".into(),
            );
            out.push(r);
        }
        if !line.starts_with("channel ") {
            continue;
        }
        if let Some(channel) = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|n| *n > 0 && *n <= 233)
        {
            out.push(measurement(
                d,
                "channel",
                channel,
                "index_or_count",
                "iw.interface",
                time,
            ));
        }
        if let Some((_, tail)) = line.split_once('(') {
            if tail.contains(" MHz)") {
                if let Some(freq) = unsigned(tail).filter(|v| (2000..=80000).contains(v)) {
                    out.push(measurement(
                        d,
                        "channel_frequency",
                        freq,
                        "MHz",
                        "iw.interface",
                        time,
                    ));
                }
            }
        }
        if let Some((_, tail)) = line.split_once("width: ") {
            if tail.split_whitespace().nth(1) == Some("MHz,")
                || tail.split_whitespace().nth(1) == Some("MHz")
            {
                if let Some(width) =
                    unsigned(tail).filter(|v| [5, 10, 20, 40, 80, 160, 320].contains(v))
                {
                    out.push(measurement(
                        d,
                        "channel_width",
                        width,
                        "MHz",
                        "iw.interface",
                        time,
                    ));
                }
            }
        }
    }
    out
}
pub fn parse_survey(d: &Device, text: &str, time: &str) -> Vec<Record> {
    let mut out = Vec::new();
    let mut frequency = None;
    let mut in_use = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with("Survey data from") {
            frequency = None;
            in_use = false;
        }
        if let Some(v) = line.strip_prefix("frequency:") {
            frequency = unsigned(v);
            in_use = line.contains("[in use]");
        }
        if let Some(v) = line.strip_prefix("noise:") {
            if v.trim_end().ends_with("dBm") {
                if let (Some(f), Some(n)) =
                    (frequency, signed(v).filter(|v| (-127..=0).contains(v)))
                {
                    let mut r = measurement(d, "noise", n, "dBm", "iw.survey", time);
                    r.insert("frequency_mhz".into(), f.to_string());
                    r.insert("channel_in_use".into(), in_use.to_string());
                    out.push(r);
                }
            }
        }
        for (key, name) in [
            ("channel active time:", "channel_active_time"),
            ("channel busy time:", "channel_busy_time"),
            ("channel receive time:", "channel_rx_time"),
            ("channel transmit time:", "channel_tx_time"),
        ] {
            if let Some(v) = line.strip_prefix(key) {
                if v.trim_end().ends_with("ms") {
                    if let (Some(f), Some(n)) = (frequency, unsigned(v)) {
                        let mut r = measurement(d, name, n, "ms", "iw.survey", time);
                        r.insert("frequency_mhz".into(), f.to_string());
                        r.insert("channel_in_use".into(), in_use.to_string());
                        out.push(r);
                    }
                }
            }
        }
    }
    out
}
pub fn collect(root: &Path, d: &Device) -> io::Result<Vec<Record>> {
    let mut out = Vec::new();
    let time = now_ns();
    for name in [
        "rx_bytes",
        "tx_bytes",
        "rx_packets",
        "tx_packets",
        "rx_errors",
        "tx_errors",
        "rx_dropped",
        "tx_dropped",
    ] {
        match read_text(
            root.join("class/net")
                .join(&d.interface)
                .join("statistics")
                .join(name),
        )
        .and_then(|v| v.parse::<u64>().ok())
        {
            Some(v) => out.push(measurement(
                d,
                name,
                v,
                if name.ends_with("bytes") {
                    "bytes"
                } else {
                    "count"
                },
                "sysfs.netdev",
                &time,
            )),
            None => out.push(diagnostic(&d.interface, "COUNTER_UNAVAILABLE", name)),
        }
    }
    for (operation, args) in [
        ("info", vec!["dev", d.interface.as_str(), "info"]),
        (
            "station",
            vec!["dev", d.interface.as_str(), "station", "dump"],
        ),
        (
            "survey",
            vec!["dev", d.interface.as_str(), "survey", "dump"],
        ),
    ] {
        match command::run("iw", &args) {
            Ok(text) => {
                let time = now_ns();
                let parsed = match operation {
                    "info" => parse_info(d, &text, &time),
                    "station" => parse_station(d, &text, &time),
                    _ => parse_survey(d, &text, &time),
                };
                if parsed.is_empty() {
                    out.push(diagnostic(&d.interface,"NO_MEASUREMENTS",&format!("iw {operation}: no recognized values; disconnected, unsupported, or parser mismatch")));
                }
                out.extend(parsed);
            }
            Err(e) => out.push(diagnostic(
                &d.interface,
                "QUERY_UNAVAILABLE",
                &format!("iw {operation}: {e}"),
            )),
        }
    }
    if let Some(phy) = d
        .fields
        .get("phy")
        .filter(|s| s.starts_with("phy") && s[3..].bytes().all(|b| b.is_ascii_digit()))
    {
        match command::run("iw", &["phy", phy, "info"]) {
            Ok(text) if text.len() <= 65536 => {
                out.extend(crate::environment::power_limits(d, &text, &now_ns()));
                out.push(record(&[
                    ("kind", "wiphy"),
                    ("interface", &d.interface),
                    ("source", "iw.phy"),
                    ("evidence", "driver_advertised_not_physical_validation"),
                    ("raw", &text),
                ]));
            }
            Ok(_) => out.push(diagnostic(
                &d.interface,
                "OUTPUT_LIMIT",
                "wiphy report too large",
            )),
            Err(e) => out.push(diagnostic(
                &d.interface,
                "QUERY_UNAVAILABLE",
                &format!("iw phy: {e}"),
            )),
        }
    }
    match command::run("iw", &["dev", &d.interface, "scan", "dump"]) {
        Ok(text) => {
            let networks = crate::environment::cached_networks(d, &text, &now_ns());
            if networks.is_empty() {
                out.push(diagnostic(
                    &d.interface,
                    "SCAN_CACHE_EMPTY",
                    "No cached networks; no scan was triggered",
                ));
            }
            out.extend(networks);
        }
        Err(e) => out.push(diagnostic(
            &d.interface,
            "SCAN_CACHE_UNAVAILABLE",
            &e.to_string(),
        )),
    }
    match command::run("ethtool", &["-i", &d.interface]) {
        Ok(text) => {
            let mut r = record(&[
                ("kind", "driver_info"),
                ("interface", &d.interface),
                ("source", "ethtool.driver_info"),
            ]);
            for line in text.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    if matches!(k, "driver" | "version" | "firmware-version" | "bus-info")
                        && !v.trim().is_empty()
                    {
                        r.insert(k.into(), v.trim().into());
                    }
                }
            }
            out.push(r);
        }
        Err(e) => out.push(diagnostic(
            &d.interface,
            "QUERY_UNAVAILABLE",
            &format!("ethtool: {e}"),
        )),
    }
    // A disconnected/replaced device invalidates the entire acquisition batch.
    if !discover(root)?.iter().any(|current| {
        current.interface == d.interface
            && current.identity == d.identity
            && current.driver == d.driver
    }) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "DEVICE_CHANGED: disconnect/rebind during acquisition; batch discarded",
        ));
    }
    out.push(record(&[
        ("kind", "sample"),
        ("interface", &d.interface),
        ("device_id", &d.identity),
        ("sample_id", &time),
        ("atomic", "false"),
        ("environment", "live"),
        ("csi", "not_acquired"),
    ]));
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn device() -> Device {
        Device {
            interface: "wlan0".into(),
            identity: "fixture".into(),
            driver: "fixture".into(),
            fields: Record::new(),
        }
    }
    #[test]
    fn station_values_and_chains() {
        let rs=parse_station(&device(),"Station 11:22:33:44:55:66 (on wlan0)\n signal: -51 [-50, -52] dBm\n rx packets: 12\n rx bitrate: 433.3 MBit/s VHT-MCS 9 80MHz VHT-NSS 1\n", "1");
        assert_eq!(rs.len(), 7);
        assert!(rs
            .iter()
            .any(|r| r["name"] == "rx_nss" && r["value"] == "1"));
        assert!(!rs.iter().any(|r| r["name"].contains("csi")));
    }
    #[test]
    fn invalid_and_missing_are_not_zero() {
        assert!(parse_station(
            &device(),
            "Station aa\nsignal: bogus dBm\nrx packets: -2\nrx bitrate: NaN MBit/s",
            "1"
        )
        .is_empty());
        assert!(parse_survey(&device(), "noise: -90 dBm", "1").is_empty());
        assert!(parse_info(
            &device(),
            "channel 36 (5180 MHz), width: 80+80 MHz, center1: 5210 MHz",
            "1"
        )
        .iter()
        .all(|r| r["name"] != "channel_width"));
    }
    #[test]
    fn channel_and_survey_context() {
        assert_eq!(
            parse_info(
                &device(),
                "channel 36 (5180 MHz), width: 80 MHz, center1: 5210 MHz",
                "1"
            )
            .len(),
            3
        );
        let r = parse_survey(
            &device(),
            "frequency: 5180 MHz [in use]\nnoise: -95 dBm\nchannel busy time: 123 ms",
            "1",
        );
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["frequency_mhz"], "5180");
    }
}

#[cfg(test)]
mod power_tests {
    use super::*;
    #[test]
    fn configuration_is_not_radiated_measurement() {
        let d = Device {
            interface: "test0".into(),
            identity: "algorithm_fixture".into(),
            driver: "fixture".into(),
            fields: Record::new(),
        };
        let rows = parse_info(&d, "txpower 18.00 dBm", "1");
        assert_eq!(rows[0]["name"], "configured_tx_power");
        assert_eq!(rows[0]["basis"], "configured_not_measured_radiated_power");
        assert!(parse_info(&d, "txpower NaN dBm", "1").is_empty());
    }
}
