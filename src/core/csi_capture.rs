//! RFVCSI/1 offline interchange. Imported provenance is an assertion, not certification.
use crate::csi::{Complex, CsiFrame};
use rfv_ipc::{record, Record};
#[derive(Debug)]
pub struct Capture {
    pub driver: String,
    pub frames: Vec<(u128, CsiFrame)>,
}
impl Capture {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 4 * 1024 * 1024 {
            return Err("CSI import exceeds 4 MiB".into());
        }
        let mut lines = text.lines();
        if lines.next() != Some("RFVCSI\t1") {
            return Err("expected RFVCSI<TAB>1".into());
        }
        let mut meta = std::collections::BTreeMap::<String, String>::new();
        let mut total_iq = 0usize;
        let mut blocks: Vec<(u128, Vec<(i16, u8, u8, Complex)>)> = Vec::new();
        for line in lines.filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let p: Vec<_> = line.split('\t').collect();
            match p.first().copied() {
                Some("frame") if p.len() == 2 => {
                    let t = p[1]
                        .parse::<u128>()
                        .map_err(|_| "bad observation timestamp")?;
                    if t == 0 || blocks.last().is_some_and(|b| b.0 >= t) || blocks.len() >= 128 {
                        return Err("nonmonotonic/zero timestamp or >128 frames".into());
                    }
                    blocks.push((t, Vec::new()));
                }
                Some("iq") if p.len() == 6 => {
                    let b = blocks.last_mut().ok_or("IQ before frame")?;
                    total_iq += 1;
                    if total_iq > 16384 || b.1.len() >= 4096 {
                        return Err("4096 IQ values/frame limit".into());
                    }
                    b.1.push((
                        p[1].parse().map_err(|_| "invalid subcarrier")?,
                        p[2].parse().map_err(|_| "invalid RX")?,
                        p[3].parse().map_err(|_| "invalid TX")?,
                        Complex {
                            re: p[4].parse().map_err(|_| "invalid real")?,
                            im: p[5].parse().map_err(|_| "invalid imaginary")?,
                        },
                    ));
                }
                Some(k) if p.len() == 2 && blocks.is_empty() => {
                    if ![
                        "device_id",
                        "driver",
                        "source",
                        "environment",
                        "center_frequency_hz",
                        "bandwidth_hz",
                        "rx_chains",
                        "tx_streams",
                        "scale",
                        "phase_calibration",
                    ]
                    .contains(&k)
                        || meta.insert(k.into(), p[1].into()).is_some()
                    {
                        return Err("unknown/duplicate CSI metadata".into());
                    }
                }
                _ => return Err("invalid CSI line".into()),
            }
        }
        let get = |k: &str| {
            meta.get(k)
                .filter(|v| !v.trim().is_empty())
                .cloned()
                .ok_or_else(|| format!("missing {k}"))
        };
        let driver = get("driver")?;
        let simulated=match get("environment")?.as_str(){"simulation"=>true,"hardware_import"=>false,_=>return Err("environment must be simulation or hardware_import (unverified provenance assertion)".into())};
        let mut frames = Vec::new();
        for (t, values) in blocks {
            let rx: u8 = get("rx_chains")?.parse().map_err(|_| "invalid RX count")?;
            let tx: u8 = get("tx_streams")?.parse().map_err(|_| "invalid TX count")?;
            if rx == 0 || tx == 0 || rx > 8 || tx > 8 {
                return Err("CSI dimensions must be 1..8".into());
            }
            let indices: Vec<i16> = values
                .iter()
                .map(|v| v.0)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            if indices.len() * rx as usize * tx as usize > 4096 {
                return Err("CSI dimensions exceed frame limit".into());
            }
            let expected: Vec<_> = indices
                .iter()
                .flat_map(|k| (0..rx).flat_map(move |r| (0..tx).map(move |s| (*k, r, s))))
                .collect();
            if expected.len() != values.len()
                || values
                    .iter()
                    .zip(expected)
                    .any(|(v, e)| (v.0, v.1, v.2) != e)
            {
                return Err("IQ must cover each bin/RX/TX exactly once in increasing order".into());
            }
            let frame = CsiFrame {
                device_id: get("device_id")?,
                source: get("source")?,
                simulated,
                center_frequency_hz: get("center_frequency_hz")?
                    .parse()
                    .map_err(|_| "invalid frequency")?,
                bandwidth_hz: get("bandwidth_hz")?
                    .parse()
                    .map_err(|_| "invalid bandwidth")?,
                rx_chains: rx,
                tx_streams: tx,
                subcarrier_indices: indices,
                iq: values.into_iter().map(|v| v.3).collect(),
                scale: get("scale")?.parse().map_err(|_| "invalid scale")?,
                hardware_timestamp: None,
                timestamp_unit: None,
                phase_calibration: meta.get("phase_calibration").cloned(),
            };
            frame.validate()?;
            if let Some((_, first)) = frames.first() {
                let first: &CsiFrame = first;
                if first.subcarrier_indices != frame.subcarrier_indices {
                    return Err("temporal capture requires fixed subcarrier mapping".into());
                }
            }
            frames.push((t, frame));
        }
        if frames.is_empty() {
            return Err("no CSI frames".into());
        }
        let c = Self { driver, frames };
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<(), String> {
        let first = &self.frames.first().ok_or("no frames")?.1;
        if self.driver.trim().is_empty()
            || self.frames.len() > 128
            || self.frames.iter().map(|(_, f)| f.iq.len()).sum::<usize>() > 16384
        {
            return Err("CSI capture provenance/size limit".into());
        }
        let mut previous = 0;
        for (t, f) in &self.frames {
            f.validate()?;
            if *t <= previous {
                return Err("nonmonotonic/zero observation timestamp".into());
            }
            previous = *t;
            if f.subcarrier_indices != first.subcarrier_indices
                || f.rx_chains != first.rx_chains
                || f.tx_streams != first.tx_streams
                || f.center_frequency_hz != first.center_frequency_hz
                || f.bandwidth_hz != first.bandwidth_hz
                || f.scale != first.scale
                || f.device_id != first.device_id
                || f.source != first.source
                || f.simulated != first.simulated
            {
                return Err("temporal capture context changed".into());
            }
        }
        Ok(())
    }
    pub fn analyze(&self) -> Result<Vec<Record>, String> {
        self.validate()?;
        let first = &self.frames.first().ok_or("no frames")?.1;
        let mut rows = vec![record(&[
            ("kind", "csi_capture"),
            ("schema", "RFVCSI/1"),
            ("device_id", &first.device_id),
            ("source", &first.source),
            ("driver", &self.driver),
            (
                "environment",
                if first.simulated {
                    "simulation"
                } else {
                    "hardware_import_unverified"
                },
            ),
            ("provenance", "importer assertion; not hardware validation"),
            ("frames", &self.frames.len().to_string()),
            ("subcarriers", &first.subcarrier_indices.len().to_string()),
            (
                "phase",
                "mathematical; detrending is not hardware calibration",
            ),
            (
                "calibration",
                first.phase_calibration.as_deref().unwrap_or("none"),
            ),
            (
                "multipath",
                "spectral ripple only; no path count, CIR, delay spread or localization inferred",
            ),
        ])];
        let mut amplitudes = Vec::new();
        for (t, frame) in &self.frames {
            let ap = frame.amplitude_phase()?;
            let stride = frame.rx_chains as usize * frame.tx_streams as usize;
            for stream in 0..stride {
                let phases: Vec<_> = (stream..ap.len())
                    .step_by(stride)
                    .map(|i| ap[i].1.map(f64::from))
                    .collect();
                let sanitized = crate::csi::sanitize_phase(&frame.subcarrier_indices, &phases)?;
                for (k, index) in frame.subcarrier_indices.iter().enumerate() {
                    let (amplitude, phase) = ap[k * stride + stream];
                    let mut r = record(&[
                        ("kind", "csi_bin"),
                        ("observed_unix_ns", &t.to_string()),
                        ("time_basis", "imported_host_observation_not_rx_time"),
                        ("subcarrier", &index.to_string()),
                        ("rx", &(stream / frame.tx_streams as usize).to_string()),
                        ("tx", &(stream % frame.tx_streams as usize).to_string()),
                        ("amplitude", &amplitude.to_string()),
                        ("amplitude_unit", "scaled_IQ_not_dBm"),
                    ]);
                    if let Some(p) = phase {
                        r.insert("phase_rad".into(), p.to_string());
                    }
                    if let Some(p) = sanitized[k] {
                        r.insert("detrended_phase_rad".into(), p.to_string());
                    }
                    rows.push(r);
                }
            }
            for stream in 0..stride {
                let a: Vec<f64> = (stream..ap.len())
                    .step_by(stride)
                    .map(|i| f64::from(ap[i].0))
                    .collect();
                let mean = a.iter().sum::<f64>() / a.len() as f64;
                if mean > 0. {
                    let ripple =
                        (a.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / a.len() as f64).sqrt()
                            / mean;
                    rows.push(record(&[("kind","csi_spectral_feature"),("observed_unix_ns",&t.to_string()),("stream",&stream.to_string()),("amplitude_coefficient_of_variation",&ripple.to_string()),("interpretation","frequency selectivity proxy; multipath, filtering and measurement artifacts cannot be separated; not a path count")]));
                }
            }
            amplitudes.push(ap.iter().map(|a| a.0 as f64).collect::<Vec<_>>());
        }
        for i in 0..first.iq.len() {
            let mean = amplitudes.iter().map(|a| a[i]).sum::<f64>() / amplitudes.len() as f64;
            let variance = amplitudes
                .iter()
                .map(|a| (a[i] - mean).powi(2))
                .sum::<f64>()
                / amplitudes.len() as f64;
            let ema = amplitudes
                .iter()
                .skip(1)
                .fold(amplitudes[0][i], |a, b| 0.2 * b[i] + 0.8 * a);
            let mut temporal=record(&[("kind","csi_temporal"),("iq_index",&i.to_string()),("amplitude_mean",&mean.to_string()),("amplitude_variance",&variance.to_string()),("amplitude_ema",&ema.to_string()),("ema_alpha","0.2"),("interpretation","amplitude static/dynamic proxy; variance includes noise, AGC and motion, not isolated receiver noise")]);
            if amplitudes.len() > 1 {
                let proxy = amplitudes
                    .windows(2)
                    .map(|w| (w[1][i] - w[0][i]).powi(2))
                    .sum::<f64>()
                    / (2. * (amplitudes.len() - 1) as f64);
                temporal.insert(
                    "successive_difference_noise_proxy".into(),
                    proxy.to_string(),
                );
                temporal.insert("noise_assumptions".into(),"Stationary channel and independent noise required; motion/AGC contaminate this proxy".into());
            }
            rows.push(temporal);
        }
        if self.frames.len() > 1 {
            let duration = (self.frames.last().unwrap().0 - self.frames[0].0) as f64 / 1e9;
            rows[0].insert(
                "observed_frame_rate_hz".into(),
                ((self.frames.len() - 1) as f64 / duration).to_string(),
            );
        }
        let zeros = first.iq.iter().filter(|c| c.re == 0. && c.im == 0.).count();
        rows[0].insert("first_frame_zero_iq".into(), zeros.to_string());
        Ok(rows)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const SYNTHETIC:&str="RFVCSI\t1\ndevice_id\talgorithm_fixture\ndriver\tnone\nsource\tsynthetic_test_only\nenvironment\tsimulation\ncenter_frequency_hz\t5180000000\nbandwidth_hz\t20000000\nrx_chains\t1\ntx_streams\t1\nscale\t1\nframe\t1\niq\t-1\t0\t0\t3\t4\niq\t1\t0\t0\t0\t0\n";
    #[test]
    fn parse_and_quality() {
        let c = Capture::parse(SYNTHETIC).unwrap();
        assert!(c.frames[0].1.simulated);
        let r = c.analyze().unwrap();
        assert_eq!(r[1]["amplitude"], "5");
        assert!(!r[2].contains_key("phase_rad"));
        assert_eq!(r[0]["environment"], "simulation");
    }
    #[test]
    fn reject_invalid() {
        for s in [
            SYNTHETIC.replace("\t3\t4", "\tNaN\t4"),
            SYNTHETIC.replace("frame\t1", "frame\t0"),
            SYNTHETIC.replace("iq\t1\t0\t0", "iq\t-1\t0\t0"),
            SYNTHETIC.replace("environment\tsimulation", "environment\treal"),
        ] {
            assert!(Capture::parse(&s).is_err());
        }
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    #[test]
    fn malformed_capture_cannot_panic_analyzer() {
        let c = Capture {
            driver: "test".into(),
            frames: vec![],
        };
        assert!(c.analyze().is_err());
    }
    #[test]
    fn reject_excessive_dimensions_before_expanding() {
        let text="RFVCSI\t1\ndriver\ttest\nenvironment\tsimulation\nrx_chains\t255\ntx_streams\t255\nframe\t1\niq\t1\t0\t0\t1\t0";
        assert!(Capture::parse(text).unwrap_err().contains("1..8"));
    }
}
