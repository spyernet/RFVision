//! Located measurements or explicitly unlocated history; no propagation interpolation.
#[cfg(test)]
use rfv_ipc::record;
use rfv_ipc::Record;
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub reference: String,
    pub label: String,
    pub x_m: f64,
    pub y_m: f64,
    pub source: String,
}
impl Position {
    pub fn validate(&self) -> Result<(), String> {
        if self.reference.trim().is_empty()
            || self.label.trim().is_empty()
            || self.source.trim().is_empty()
            || !self.x_m.is_finite()
            || !self.y_m.is_finite()
        {
            return Err(
                "position requires reference frame, label, finite coordinates and source".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Observation {
    pub measurement: Record,
    pub frequency_mhz: Option<f64>,
    pub position: Option<Position>,
}
impl Observation {
    pub fn validate(&self) -> Result<(), String> {
        rfv_ipc::measurement::validate(&self.measurement)?;
        if self.measurement["name"] != "rssi" {
            return Err("coverage requires measured RSSI, not a prediction".into());
        }
        if self
            .measurement
            .get("quality")
            .is_some_and(|s| s == "simulation")
            || self
                .measurement
                .get("time_basis")
                .is_some_and(|s| s == "synthetic")
        {
            return Err("synthetic inputs are not measured coverage".into());
        }
        if let Some(f) = self.frequency_mhz {
            if !f.is_finite() || !(2000. ..=80000.).contains(&f) {
                return Err("invalid frequency".into());
            }
        }
        if let Some(p) = &self.position {
            p.validate()?;
        }
        Ok(())
    }
    pub fn timestamp(&self) -> u128 {
        self.measurement["observed_unix_ns"].parse().unwrap_or(0)
    }
    pub fn rssi(&self) -> f64 {
        self.measurement["value"].parse().unwrap_or(f64::NAN)
    }
    pub fn same_link(&self, other: &Self) -> bool {
        ["device_id", "interface", "peer"]
            .iter()
            .all(|k| self.measurement.get(*k) == other.measurement.get(*k))
            && self.frequency_mhz == other.frequency_mhz
    }
}
#[derive(Default)]
pub struct History {
    pub observations: std::collections::VecDeque<Observation>,
}
impl History {
    pub fn push(&mut self, o: Observation) -> Result<(), String> {
        o.validate()?;
        if self
            .observations
            .iter()
            .rev()
            .find(|p| p.same_link(&o))
            .is_some_and(|p| p.timestamp() >= o.timestamp())
        {
            return Err("duplicate or nonmonotonic link observation".into());
        }
        if self.observations.len() == 10000 {
            self.observations.pop_front();
        }
        self.observations.push_back(o);
        Ok(())
    }
    pub fn summarize(&self, now_ns: u128) -> Vec<Record> {
        // Deliberately group by location AND link, never mix APs or coordinate frames.
        let mut groups: std::collections::BTreeMap<Vec<String>, Vec<&Observation>> =
            std::collections::BTreeMap::new();
        for o in &self.observations {
            let m = &o.measurement;
            let mut key = vec![
                m["device_id"].clone(),
                m["interface"].clone(),
                m.get("peer").cloned().unwrap_or_default(),
                o.frequency_mhz.map(|f| f.to_string()).unwrap_or_default(),
            ];
            if let Some(p) = &o.position {
                key.extend([
                    p.reference.clone(),
                    p.label.clone(),
                    p.x_m.to_string(),
                    p.y_m.to_string(),
                    p.source.clone(),
                ]);
            }
            groups.entry(key).or_default().push(o);
        }
        groups
            .values()
            .map(|g| {
                let last = g[g.len() - 1];
                let mean = g.iter().map(|o| o.rssi()).sum::<f64>() / g.len() as f64;
                let stddev = (g.iter().map(|o| (o.rssi() - mean).powi(2)).sum::<f64>()
                    / g.len() as f64)
                    .sqrt();
                let mut r = last.measurement.clone();
                r.insert("kind".into(), "coverage_summary".into());
                r.remove("value");
                for (k, v) in [
                    ("mean_rssi_dbm", mean.to_string()),
                    ("stddev_db", stddev.to_string()),
                    ("count", g.len().to_string()),
                    (
                        "coverage_confidence",
                        "not_quantified; point observations only".into(),
                    ),
                    (
                        "representation",
                        if last.position.is_some() {
                            "located_measurements"
                        } else {
                            "unlocated_signal_history"
                        }
                        .into(),
                    ),
                ] {
                    r.insert(k.into(), v);
                }
                if let Some(age) = now_ns.checked_sub(last.timestamp()) {
                    r.insert("age_seconds".into(), (age as f64 / 1e9).to_string());
                } else {
                    r.insert(
                        "clock_warning".into(),
                        "observation is in the future".into(),
                    );
                }
                if let Some(f) = last.frequency_mhz {
                    r.insert("frequency_mhz".into(), f.to_string());
                }
                if let Some(p) = &last.position {
                    for (k, v) in [
                        ("reference", p.reference.clone()),
                        ("location", p.label.clone()),
                        ("x_m", p.x_m.to_string()),
                        ("y_m", p.y_m.to_string()),
                        ("position_source", p.source.clone()),
                    ] {
                        r.insert(k.into(), v);
                    }
                }
                r
            })
            .collect()
    }
}
pub fn from_batch(rows: &[Record], position: Option<Position>) -> Vec<Observation> {
    rows.iter()
        .filter(|r| {
            r.get("kind").map(String::as_str) == Some("measurement")
                && r.get("name").map(String::as_str) == Some("rssi")
        })
        .map(|r| {
            let frequency_mhz = rows
                .iter()
                .find(|f| {
                    f.get("device_id") == r.get("device_id")
                        && f.get("name").map(String::as_str) == Some("channel_frequency")
                })
                .and_then(|f| f.get("value")?.parse().ok());
            Observation {
                measurement: r.clone(),
                frequency_mhz,
                position: position.clone(),
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn obs(t: &str) -> Observation {
        Observation {
            measurement: record(&[
                ("kind", "measurement"),
                ("interface", "test"),
                ("device_id", "fixture"),
                ("name", "rssi"),
                ("value", "-65"),
                ("unit", "dBm"),
                ("source", "parser_fixture_only"),
                ("observed_unix_ns", t),
                ("time_basis", "host_query_observation_not_rx_time"),
                ("quality", "driver_reported_uncalibrated"),
            ]),
            frequency_mhz: Some(2412.),
            position: None,
        }
    }
    #[test]
    fn provenance_and_time() {
        let mut h = History::default();
        h.push(obs("1")).unwrap();
        assert!(h.push(obs("1")).is_err());
        assert_eq!(
            h.summarize(2)[0]["representation"],
            "unlocated_signal_history"
        );
        let mut o = obs("3");
        o.measurement.insert("quality".into(), "simulation".into());
        assert!(h.push(o).is_err());
        assert!(h.summarize(0)[0].contains_key("clock_warning"));
    }
    #[test]
    fn no_invented_locations() {
        let mut o = obs("1");
        o.position = Some(Position {
            reference: "floor A".into(),
            label: "desk".into(),
            x_m: f64::NAN,
            y_m: 0.,
            source: "user".into(),
        });
        assert!(o.validate().is_err());
    }
}

#[cfg(test)]
mod grouping_tests {
    use super::*;
    #[test]
    fn locations_and_peers_do_not_merge() {
        // Handcrafted parser-shaped fixtures, not recorded RF data.
        let base = record(&[
            ("kind", "measurement"),
            ("interface", "test0"),
            ("device_id", "algorithm_fixture"),
            ("name", "rssi"),
            ("value", "-60"),
            ("unit", "dBm"),
            ("source", "algorithm_test_only"),
            ("observed_unix_ns", "1"),
            ("time_basis", "test_clock"),
            ("quality", "test_parser_shape"),
            ("peer", "AP1"),
        ]);
        let mut h = History::default();
        for (time, label, x, peer) in [
            (1, "desk", 0., "AP1"),
            (2, "hall", 5., "AP1"),
            (3, "desk", 0., "AP2"),
        ] {
            let mut m = base.clone();
            m.insert("observed_unix_ns".into(), time.to_string());
            m.insert("peer".into(), peer.into());
            h.push(Observation {
                measurement: m,
                frequency_mhz: Some(2412.),
                position: Some(Position {
                    reference: "test-floor".into(),
                    label: label.into(),
                    x_m: x,
                    y_m: 0.,
                    source: "algorithm fixture coordinates".into(),
                }),
            })
            .unwrap();
        }
        let rows = h.summarize(4);
        assert_eq!(rows.len(), 3);
        assert!(rows
            .iter()
            .all(|r| r["count"] == "1" && r["representation"] == "located_measurements"));
    }
}
