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
        if self.device_id.is_empty() || self.source.is_empty() {
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
        if self.hardware_timestamp.is_some() != self.timestamp_unit.is_some() {
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
