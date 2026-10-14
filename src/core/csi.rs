//! Offline normalization contract only. This module does not acquire CSI.
#[derive(Debug, Clone)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}
#[derive(Debug, Clone)]
pub struct CsiFrame {
    pub device_id: String,
    pub source: String,
    pub simulated: bool,
    pub center_frequency_hz: u64,
    pub bandwidth_hz: u32,
    pub rx_chains: u8,
    pub tx_streams: u8,
    pub subcarrier_indices: Vec<i16>,
    /// Order: subcarrier, RX chain, TX stream. Scaling is not RF power calibration.
    pub iq: Vec<Complex>,
    pub scale: f32,
    /// Unknown/unavailable metadata is absent, not zero-filled.
    pub hardware_timestamp: Option<u64>,
    pub timestamp_unit: Option<String>,
    pub phase_calibration: Option<String>,
}
impl CsiFrame {
    pub fn validate(&self) -> Result<(), String> {
        if self.device_id.trim().is_empty() || self.source.trim().is_empty() {
            return Err("missing provenance".into());
        }
        if self.rx_chains == 0
            || self.rx_chains > 8
            || self.tx_streams == 0
            || self.tx_streams > 8
            || self.subcarrier_indices.is_empty()
            || self.subcarrier_indices.len() > 4096
        {
            return Err("invalid CSI dimensions".into());
        }
        if self.center_frequency_hz == 0
            || self.bandwidth_hz == 0
            || !self.scale.is_finite()
            || self.scale <= 0.0
        {
            return Err("invalid frequency/scale".into());
        }
        if self.subcarrier_indices.windows(2).any(|w| w[0] >= w[1]) {
            return Err("subcarriers must be unique and increasing".into());
        }
        if self.iq.len()
            != self.subcarrier_indices.len() * self.rx_chains as usize * self.tx_streams as usize
        {
            return Err("CSI dimensions do not match IQ length".into());
        }
        if self.iq.iter().any(|c| {
            !c.re.is_finite() || !c.im.is_finite() || !(c.re.hypot(c.im) * self.scale).is_finite()
        }) {
            return Err("non-finite IQ/amplitude".into());
        }
        if self.hardware_timestamp.is_some() != self.timestamp_unit.is_some()
            || self
                .timestamp_unit
                .as_ref()
                .is_some_and(|u| u.trim().is_empty())
        {
            return Err("timestamp requires units".into());
        }
        Ok(())
    }
    /// Mathematical phase, NOT calibrated propagation phase. Zero IQ has no phase.
    pub fn amplitude_phase(&self) -> Result<Vec<(f32, Option<f32>)>, String> {
        self.validate()?;
        Ok(self
            .iq
            .iter()
            .map(|c| {
                (
                    c.re.hypot(c.im) * self.scale,
                    if c.re == 0.0 && c.im == 0.0 {
                        None
                    } else {
                        Some(c.im.atan2(c.re))
                    },
                )
            })
            .collect())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> CsiFrame {
        CsiFrame {
            device_id: "simulation".into(),
            source: "unit_test_only".into(),
            simulated: true,
            center_frequency_hz: 5_180_000_000,
            bandwidth_hz: 20_000_000,
            rx_chains: 1,
            tx_streams: 1,
            subcarrier_indices: vec![-1, 1],
            iq: vec![Complex { re: 3., im: 4. }, Complex { re: 0., im: 0. }],
            scale: 1.,
            hardware_timestamp: None,
            timestamp_unit: None,
            phase_calibration: None,
        }
    }
    #[test]
    fn synthetic_math_not_measurement() {
        let c = fixture();
        let p = c.amplitude_phase().unwrap();
        assert_eq!(p[0].0, 5.);
        assert_eq!(p[1].1, None);
        assert!(c.simulated);
    }
    #[test]
    fn reject_bad_csi() {
        let mut c = fixture();
        c.iq.pop();
        assert!(c.validate().is_err());
        c = fixture();
        c.iq[0].re = f32::NAN;
        assert!(c.validate().is_err());
    }
}

/// Unwrap and remove least-squares constant/linear phase per contiguous valid
/// segment and stream. This removes real channel slope too: NOT propagation
/// calibration. Unknown phase (zero IQ) and missing subcarriers break segments.
pub fn sanitize_phase(indices: &[i16], phases: &[Option<f64>]) -> Result<Vec<Option<f64>>, String> {
    if indices.len() != phases.len()
        || indices.windows(2).any(|w| w[0] >= w[1])
        || phases
            .iter()
            .flatten()
            .any(|p| !p.is_finite() || p.abs() > std::f64::consts::PI)
    {
        return Err("invalid phase inputs".into());
    }
    let mut out = vec![None; phases.len()];
    let mut start = 0;
    while start < phases.len() {
        if phases[start].is_none() {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < phases.len()
            && phases[end].is_some()
            && i32::from(indices[end]) - i32::from(indices[end - 1]) == 1
        {
            end += 1;
        }
        let mut unwrapped = Vec::new();
        let mut previous = phases[start].unwrap();
        unwrapped.push(previous);
        for p in &phases[start + 1..end] {
            let raw = p.unwrap();
            let delta = (raw - previous + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            previous += delta;
            unwrapped.push(previous);
        }
        // Fewer than three bins cannot leave a meaningful detrended residual.
        if end - start >= 3 {
            let n = (end - start) as f64;
            let xm = indices[start..end]
                .iter()
                .map(|x| f64::from(*x))
                .sum::<f64>()
                / n;
            let ym = unwrapped.iter().sum::<f64>() / n;
            let slope = indices[start..end]
                .iter()
                .zip(&unwrapped)
                .map(|(x, y)| (f64::from(*x) - xm) * (y - ym))
                .sum::<f64>()
                / indices[start..end]
                    .iter()
                    .map(|x| (f64::from(*x) - xm).powi(2))
                    .sum::<f64>();
            for (j, y) in unwrapped.iter().enumerate() {
                out[start + j] = Some(y - ym - slope * (f64::from(indices[start + j]) - xm));
            }
        }
        start = end;
    }
    Ok(out)
}
/// Apply externally established per-bin phase offsets without inventing a
/// calibration. Caller must retain reference/method/date in capture metadata.
pub fn correct_phase(
    phases: &[Option<f64>],
    offsets: &[f64],
    calibration_source: &str,
) -> Result<Vec<Option<f64>>, String> {
    if calibration_source.trim().is_empty()
        || phases.len() != offsets.len()
        || offsets.iter().any(|v| !v.is_finite())
        || phases.iter().flatten().any(|v| !v.is_finite())
    {
        return Err("invalid calibration".into());
    }
    Ok(phases
        .iter()
        .zip(offsets)
        .map(|(p, o)| {
            p.map(|p| {
                (p.rem_euclid(std::f64::consts::TAU) - o.rem_euclid(std::f64::consts::TAU)
                    + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI
            })
        })
        .collect())
}
#[cfg(test)]
mod phase_tests {
    use super::*;
    #[test]
    fn unwrap_slope_and_unknown() {
        let p = sanitize_phase(
            &[1, 2, 3, 4],
            &[Some(3.), Some(-3.), Some(-2.716814692820414), None],
        )
        .unwrap();
        assert!(p[..3].iter().all(|p| p.unwrap().abs() < 1e-10));
        assert_eq!(p[3], None);
        assert_eq!(
            sanitize_phase(&[1, 3, 5], &[Some(1.); 3]).unwrap(),
            vec![None; 3]
        );
    }
    #[test]
    fn calibration_requires_provenance() {
        assert!(correct_phase(&[Some(1.)], &[0.2], "").is_err());
        assert!(
            (correct_phase(
                &[Some(1.), None],
                &[0.2, 0.],
                "reference instrument 2026-10-14"
            )
            .unwrap()[0]
                .unwrap()
                - 0.8)
                .abs()
                < 1e-10
        );
    }
}
