//! RF Link Optimizer: conservative heuristics, never automatic configuration.
use rfv_ipc::{record, Record};
fn recommendation(interface: &str, code: &str, evidence: String, action: &str) -> Record {
    record(&[
        ("kind", "recommendation"),
        ("interface", interface),
        ("code", code),
        ("evidence", &evidence),
        ("action", action),
        ("confidence", "heuristic_not_causal_proof"),
        ("automatic_changes", "never"),
    ])
}
pub fn assess(rows: &[Record], previous: Option<&[Record]>) -> Vec<Record> {
    let mut out = Vec::new();
    for m in rows
        .iter()
        .filter(|r| r.get("kind").map(String::as_str) == Some("measurement"))
    {
        if rfv_ipc::measurement::validate(m).is_err()
            || m.get("quality").map(String::as_str) == Some("simulation")
            || m.get("time_basis").map(String::as_str) == Some("synthetic")
        {
            continue;
        }
        let v = m["value"].parse::<f64>().unwrap();
        let interface = &m["interface"];
        if m["name"] == "rssi" && v < -70. {
            out.push(recommendation(interface,"WEAK_SIGNAL",format!("RSSI {v} dBm; source {}",m["source"]),"Compare measured AP/client positions and antenna orientations. Test 2.4 GHz for reach only if supported and less congested; do not increase power beyond permitted limits."));
        }
        let frequency = |batch: &[Record]| {
            batch
                .iter()
                .find(|r| {
                    r.get("name").map(String::as_str) == Some("channel_frequency")
                        && r.get("device_id") == m.get("device_id")
                })
                .and_then(|r| r.get("value"))
                .cloned()
        };
        if previous.is_some_and(|old| frequency(old) != frequency(rows)) {
            continue;
        }
        let Some(old) = previous.and_then(|p| {
            p.iter().find(|r| {
                [
                    "kind",
                    "name",
                    "device_id",
                    "interface",
                    "peer",
                    "frequency_mhz",
                ]
                .iter()
                .all(|k| r.get(*k) == m.get(*k))
            })
        }) else {
            continue;
        };
        if rfv_ipc::measurement::validate(old).is_err()
            || old.get("quality").map(String::as_str) == Some("simulation")
            || old.get("time_basis").map(String::as_str) == Some("synthetic")
        {
            continue;
        }
        let now = m["observed_unix_ns"].parse::<u128>().unwrap();
        let before = old["observed_unix_ns"].parse::<u128>().unwrap();
        if now <= before || now - before > 300_000_000_000 {
            continue;
        }
        if m["name"] == "rssi" && (v - old["value"].parse::<f64>().unwrap()).abs() >= 10. {
            out.push(recommendation(interface,"SIGNAL_VARIATION",format!("RSSI changed {} dB between observations",v-old["value"].parse::<f64>().unwrap()),"Repeat measurements at a fixed location; compare placement and obstructions. Two observations do not establish long-term instability."));
        }
        let delta = || -> Option<u64> {
            m["value"]
                .parse::<u64>()
                .ok()?
                .checked_sub(old["value"].parse().ok()?)
        };
        let paired_delta = |name: &str| -> Option<u64> {
            let find = |rs: &[Record], context: &Record| {
                rs.iter()
                    .find(|r| {
                        r.get("name").map(String::as_str) == Some(name)
                            && [
                                "device_id",
                                "interface",
                                "peer",
                                "frequency_mhz",
                                "observed_unix_ns",
                            ]
                            .iter()
                            .all(|k| r.get(*k) == context.get(*k))
                    })
                    .and_then(|r| r.get("value")?.parse::<u64>().ok())
            };
            find(rows, m)?.checked_sub(find(previous?, old)?)
        };
        if m["name"] == "tx_retries" {
            if let (Some(retries), Some(packets)) = (delta(), paired_delta("station_tx_packets")) {
                if packets >= 10 && retries as f64 / packets as f64 > 0.3 {
                    out.push(recommendation(interface,"EXCESSIVE_RETRIES",format!("{retries} retry attempts / {packets} transmitted packets; not packet loss probability"),"Compare congestion and signal at the same position. Trial a narrower supported AP channel width, then measure again."));
                }
            }
        }
        if m["name"] == "tx_failed" {
            if let Some(n) = delta().filter(|n| *n > 0) {
                out.push(recommendation(interface,"TX_FAILURES",format!("{n} reported failures in interval; end-to-end packet loss not measured"),"Check link placement and contention; avoid treating driver failure counts as ping loss."));
            }
        }
        if m["name"] == "channel_busy_time"
            && m.get("channel_in_use").map(String::as_str) == Some("true")
        {
            if let (Some(busy), Some(active)) = (delta(), paired_delta("channel_active_time")) {
                if active > 0 && busy <= active && busy as f64 / active as f64 > 0.7 {
                    out.push(recommendation(interface,"POSSIBLE_CONGESTION",format!("{}% busy time from matched survey counter deltas",100.*busy as f64/active as f64),"Compare permitted channels over repeated intervals. Busy airtime includes your own traffic and is not proof of external interference."));
                }
            }
        }
    }
    if !rows.iter().any(|r| {
        r.get("kind").map(String::as_str) == Some("measurement")
            && r.get("name").map(String::as_str) == Some("rssi")
            && rfv_ipc::measurement::validate(r).is_ok()
            && r.get("quality").map(String::as_str) != Some("simulation")
            && r.get("time_basis").map(String::as_str) != Some("synthetic")
    }) {
        out.push(recommendation("","SIGNAL_UNAVAILABLE","No valid station RSSI in this batch".into(),"Check association, permissions and driver field support. Unknown signal is not zero dBm."));
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_and_weak() {
        assert_eq!(assess(&[], None)[0]["code"], "SIGNAL_UNAVAILABLE");
        let m = record(&[
            ("kind", "measurement"),
            ("device_id", "fixture"),
            ("interface", "test0"),
            ("name", "rssi"),
            ("value", "-80"),
            ("unit", "dBm"),
            ("source", "parser_test"),
            ("quality", "driver_reported_uncalibrated"),
            ("time_basis", "host"),
            ("observed_unix_ns", "1"),
        ]);
        assert_eq!(assess(&[m], None)[0]["code"], "WEAK_SIGNAL");
    }
}

#[cfg(test)]
mod interval_tests {
    use super::*;
    // Parser-shaped algorithm fixtures only; these are never hardware captures.
    fn m(name: &str, value: &str, t: &str) -> Record {
        record(&[
            ("kind", "measurement"),
            ("interface", "test0"),
            ("device_id", "algorithm_fixture"),
            ("peer", "test_peer"),
            ("name", name),
            ("value", value),
            (
                "unit",
                if name == "rssi" {
                    "dBm"
                } else if name == "channel_frequency" {
                    "MHz"
                } else {
                    "count"
                },
            ),
            ("source", "algorithm_test_only"),
            ("quality", "test_parser_shape"),
            ("time_basis", "test_clock"),
            ("observed_unix_ns", t),
        ])
    }
    fn has(rows: &[Record], code: &str) -> bool {
        rows.iter().any(|r| r["code"] == code)
    }
    #[test]
    fn retries_use_matched_deltas_not_loss_percent() {
        let old = vec![
            m("tx_retries", "100", "1"),
            m("station_tx_packets", "100", "1"),
        ];
        let now = vec![
            m("tx_retries", "150", "2"),
            m("station_tx_packets", "200", "2"),
        ];
        assert!(has(&assess(&now, Some(&old)), "EXCESSIVE_RETRIES"));
        let reset = vec![
            m("tx_retries", "2", "2"),
            m("station_tx_packets", "200", "2"),
        ];
        assert!(!has(&assess(&reset, Some(&old)), "EXCESSIVE_RETRIES"));
        let mut stale = now.clone();
        stale[1].insert("observed_unix_ns".into(), "3".into());
        assert!(!has(&assess(&stale, Some(&old)), "EXCESSIVE_RETRIES"));
    }
    #[test]
    fn congestion_and_impossible_counters() {
        let mut old = vec![
            m("channel_busy_time", "100", "1"),
            m("channel_active_time", "100", "1"),
        ];
        let mut now = vec![
            m("channel_busy_time", "180", "2"),
            m("channel_active_time", "200", "2"),
        ];
        for r in old.iter_mut().chain(now.iter_mut()) {
            r.insert("channel_in_use".into(), "true".into());
            r.insert("frequency_mhz".into(), "2412".into());
        }
        assert!(has(&assess(&now, Some(&old)), "POSSIBLE_CONGESTION"));
        now[0].insert("value".into(), "300".into());
        assert!(!has(&assess(&now, Some(&old)), "POSSIBLE_CONGESTION"));
    }
    #[test]
    fn compare_same_peer_channel_clock_only() {
        let old = vec![m("rssi", "-50", "1"), m("channel_frequency", "2412", "1")];
        let mut now = vec![m("rssi", "-65", "2"), m("channel_frequency", "2412", "2")];
        assert!(has(&assess(&now, Some(&old)), "SIGNAL_VARIATION"));
        now[1].insert("value".into(), "5180".into());
        assert!(!has(&assess(&now, Some(&old)), "SIGNAL_VARIATION"));
        now[1].insert("value".into(), "2412".into());
        now[0].insert("peer".into(), "new_peer".into());
        assert!(!has(&assess(&now, Some(&old)), "SIGNAL_VARIATION"));
    }
}
