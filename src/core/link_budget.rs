//! Free-space predictions, not measurements or regulatory authorizations.
use rfv_ipc::{record, Record};
#[derive(Clone, Debug)]
pub struct Input {
    pub value: f64,
    pub source: String,
    pub basis: String,
}
impl Input {
    pub fn validate(&self) -> Result<(), String> {
        if !self.value.is_finite()
            || self.source.trim().is_empty()
            || !["measured", "configured", "manufacturer", "estimated"]
                .contains(&self.basis.as_str())
        {
            return Err("input needs finite value, source and measured/configured/manufacturer/estimated basis".into());
        }
        Ok(())
    }
}
pub fn dbm_to_mw(dbm: f64) -> Result<f64, String> {
    let v = 10_f64.powf(dbm / 10.);
    if !dbm.is_finite() || !v.is_finite() || v <= 0. {
        return Err("power out of range".into());
    }
    Ok(v)
}
pub fn mw_to_dbm(mw: f64) -> Result<f64, String> {
    if !mw.is_finite() || mw <= 0. {
        return Err("mW must be positive and finite".into());
    }
    Ok(10. * mw.log10())
}
pub fn fspl_db(distance_m: f64, frequency_mhz: f64) -> Result<f64, String> {
    if !distance_m.is_finite()
        || !frequency_mhz.is_finite()
        || distance_m <= 0.
        || frequency_mhz <= 0.
    {
        return Err("distance/frequency must be positive finite values".into());
    }
    let loss = 20. * distance_m.log10()
        + 20. * frequency_mhz.log10()
        + 20. * (4. * std::f64::consts::PI / 299.792458).log10();
    if loss < 0. {
        return Err("distance outside free-space far-field approximation".into());
    }
    Ok(loss)
}
/// Required losses must be supplied explicitly, including assumed zeros. Sensitivity
/// is optional and must apply to the intended bandwidth/MCS/PER target.
pub fn calculate(
    inputs: &std::collections::BTreeMap<String, Input>,
) -> Result<Vec<Record>, String> {
    let units = [
        ("tx_power", "dBm"),
        ("tx_gain", "dBi"),
        ("rx_gain", "dBi"),
        ("tx_loss", "dB"),
        ("rx_loss", "dB"),
        ("distance", "m"),
        ("frequency", "MHz"),
        ("fade_reserve", "dB"),
        ("sensitivity", "dBm"),
    ];
    for (key, input) in inputs {
        if !units.iter().any(|(k, _)| k == key) {
            return Err(format!("unknown input {key}"));
        }
        input.validate()?;
    }
    let get = |k: &str| {
        inputs
            .get(k)
            .map(|v| v.value)
            .ok_or_else(|| format!("missing {k}; unknown is not zero"))
    };
    for k in ["tx_loss", "rx_loss", "fade_reserve"] {
        if get(k)? < 0. {
            return Err(format!("negative {k}"));
        }
    }
    let fspl = fspl_db(get("distance")?, get("frequency")?)?;
    let eirp = get("tx_power")? + get("tx_gain")? - get("tx_loss")?;
    let received = eirp + get("rx_gain")? - get("rx_loss")? - fspl;
    let mut rows = Vec::new();
    for (key, unit) in units {
        if let Some(v) = inputs.get(key) {
            rows.push(record(&[
                ("kind", "budget_input"),
                ("name", key),
                ("value", &v.value.to_string()),
                ("unit", unit),
                ("basis", &v.basis),
                ("source", &v.source),
            ]));
        }
    }
    let mut add = |name: &str, value: f64, unit: &str| -> Result<(), String> {
        if !value.is_finite() {
            return Err("budget overflow".into());
        }
        rows.push(record(&[("kind", "prediction"), ("name", name), ("value", &value.to_string()), ("unit", unit), ("model", "free_space_far_field"), ("confidence", "not_quantified"), ("limitations", "No walls, terrain, interference or fading modeled; not measured RSSI; not permission to transmit")]));
        Ok(())
    };
    add("fspl", fspl, "dB")?;
    add("eirp", eirp, "dBm")?;
    add("predicted_received_power", received, "dBm")?;
    if let Some(sensitivity) = inputs.get("sensitivity") {
        let margin = received - sensitivity.value;
        add("link_margin", margin, "dB")?;
        add(
            "margin_after_fade_reserve",
            margin - get("fade_reserve")?,
            "dB",
        )?;
    }
    Ok(rows)
}
/// TSV: name, value, basis, source. Units are defined by the schema above.
pub fn parse(text: &str) -> Result<std::collections::BTreeMap<String, Input>, String> {
    let mut out = std::collections::BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let p: Vec<_> = line.splitn(4, '\t').collect();
        if p.len() != 4 {
            return Err("expected name<TAB>value<TAB>basis<TAB>source".into());
        }
        let v = Input {
            value: p[1].parse().map_err(|_| "invalid number")?,
            basis: p[2].into(),
            source: p[3].into(),
        };
        v.validate()?;
        if out.insert(p[0].into(), v).is_some() {
            return Err("duplicate budget input".into());
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversions() {
        assert!((dbm_to_mw(20.).unwrap() - 100.).abs() < 1e-9);
        assert!((mw_to_dbm(100.).unwrap() - 20.).abs() < 1e-9);
        assert!(mw_to_dbm(0.).is_err());
        assert!(dbm_to_mw(f64::NAN).is_err());
    }
    #[test]
    fn propagation() {
        assert!((fspl_db(100., 2400.).unwrap() - 80.052).abs() < 0.01);
        assert!(
            (fspl_db(200., 2400.).unwrap() - fspl_db(100., 2400.).unwrap() - 6.0206).abs() < 0.001
        );
        assert!(fspl_db(-1., 2400.).is_err());
    }
    #[test]
    fn provenance_and_margin() {
        let t="tx_power\t20\tconfigured\tAP setting\ntx_gain\t2\testimated\tassumption\nrx_gain\t0\testimated\tassumption\ntx_loss\t1\testimated\tassumption\nrx_loss\t0\testimated\tassumption\ndistance\t100\tmeasured\ttape\nfrequency\t2400\tconfigured\tAP\nfade_reserve\t10\testimated\tplanning\nsensitivity\t-85\tmanufacturer\tMCS target datasheet";
        let mut i = parse(t).unwrap();
        let r = calculate(&i).unwrap();
        let v = r.iter().find(|r| r["name"] == "link_margin").unwrap()["value"]
            .parse::<f64>()
            .unwrap();
        assert!((v - 25.948).abs() < 0.01);
        i.remove("rx_gain");
        assert!(calculate(&i).is_err());
        assert!(parse("x\t1\tmagic\tsource").is_err());
    }
}
