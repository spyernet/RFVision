//! Independent passive rtw88 userspace integration. No driver code is linked.
#![forbid(unsafe_code)]
use rfv_hal::{diagnostic, discover, Device};
use rfv_ipc::{record, Record, DISCOVER, SAMPLE};
use std::path::Path;
fn supported(driver: &str) -> bool {
    let suffix = driver
        .strip_prefix("rtw88_")
        .or_else(|| driver.strip_prefix("rtw_"));
    matches!(
        suffix,
        Some(
            "8821ce"
                | "8821cu"
                | "8821cs"
                | "8822be"
                | "8822bs"
                | "8822bu"
                | "8822ce"
                | "8822cs"
                | "8822cu"
                | "8723de"
                | "8723ds"
                | "8723du"
                | "8723cs"
                | "8812au"
                | "8814ae"
                | "8814au"
                | "8821au"
        )
    )
}
fn capabilities(d: &Device) -> Vec<Record> {
    let mut out = Vec::new();
    for (features,status,implementation,reason) in [
        ("rx_bytes,tx_bytes,rx_packets,tx_packets,rx_errors,tx_errors,rx_dropped,tx_dropped","AVAILABLE_WITH_ADAPTER","implemented","Read-only sysfs counters; availability promoted only after successful read; not PHY-specific error counts"),
        ("rssi,rssi_average,chain_rssi,station_rx_bytes,station_tx_bytes,station_rx_packets,station_tx_packets,tx_retries,tx_failed,rx_drop_misc,rx_bitrate,tx_bitrate,rx_mcs,tx_mcs,rx_nss,tx_nss,channel_frequency,channel_width,channel,configured_tx_power","AVAILABLE_WITH_ADAPTER","implemented","Public iw/nl80211 query; field presence, association, installed tools and permissions determine runtime availability"),
        ("noise,channel_active_time,channel_busy_time,channel_rx_time,channel_tx_time","UNVERIFIED","implemented_parser","Survey parser exists; no assertion that this driver implements survey data"),
        ("rx_timestamp,per_packet_phy_status","AVAILABLE_WITH_ADAPTER","not_implemented","Source RX metadata reaches mac80211; existing monitor/radiotap path is a future explicit opt-in integration; no monitor interface is created"),
        ("evm,frequency_offset,phy_snr","REQUIRES_DRIVER_MODIFICATION","not_implemented","RTL8821C header defines PHY fields but inspected parser does not populate these values; units/validity must be verified before instrumentation"),
        ("csi,csi_amplitude,csi_phase,complex_channel_estimates,per_subcarrier,channel_frequency_response,channel_impulse_response","UNVERIFIED","not_implemented","No raw channel matrix export found in supplied RTL8821C RX/C2H paths; beamforming CSI names are not proof of accessible samples"),
        ("measurement_synchronization,calibrated_received_power","UNVERIFIED","not_implemented","No validated synchronization/calibration source; host observations are not receive timestamps"),
    ] {
        for feature in features.split(',') {
            // Chipset-specific research must not be inherited by other family members.
            let target=d.fields.get("vendor_id").map(String::as_str)==Some("10ec") && matches!(d.fields.get("product_id").map(String::as_str),Some("b821"|"c821"));
            let chipset_specific=matches!(feature,"evm"|"frequency_offset"|"phy_snr");
            out.push(record(&[("kind","capability"),("interface",&d.interface),("device_id",&d.identity),("feature",feature),("status",if chipset_specific && !target {"UNVERIFIED"} else {status}),("implementation",implementation),("evidence",if chipset_specific && !target {"Chipset-specific investigation pending"} else {reason}),("scope","driver_exposed_not_physical_validation")]));
        }
    }
    out
}
fn device_records(d: &Device) -> Vec<Record> {
    let mut info = d.fields.clone();
    let yes = supported(&d.driver);
    info.insert("adapter_supported".into(), yes.to_string());
    info.insert(
        "adapter".into(),
        if yes {
            "rfv-adapter-rtw88"
        } else {
            "unavailable"
        }
        .into(),
    );
    if info.get("vendor_id").map(String::as_str) == Some("10ec") {
        info.insert("manufacturer".into(), "Realtek".into());
        if matches!(
            info.get("product_id").map(String::as_str),
            Some("b821" | "c821")
        ) {
            info.insert("chipset".into(), "RTL8821CE (PCI ID match)".into());
        }
    }
    let mut records = vec![info, rfv_hal::research::csi_capability(d)];
    if yes {
        records.extend(capabilities(d));
    } else {
        records.push(diagnostic(&d.interface,"UNSUPPORTED_DRIVER","No matching RFVision adapter. An independent public-interface integration and evidence-backed capability audit are required; keep the existing driver."));
    }
    records
}
fn handle(kind: u16, requests: &[Record]) -> Result<Vec<Record>, String> {
    let root = Path::new("/sys");
    let devices = discover(root).map_err(|e| e.to_string())?;
    if kind == DISCOVER {
        let mut out = vec![record(&[
            ("kind", "adapter"),
            ("name", "rfv-adapter-rtw88"),
            ("version", env!("CARGO_PKG_VERSION")),
            ("mode", "passive_public_interfaces"),
            ("device_count", &devices.len().to_string()),
        ])];
        if devices.len() > 16 {
            return Err("DEVICE_LIMIT: at most 16 visible interfaces per report".into());
        }
        for d in devices {
            out.extend(device_records(&d));
        }
        return Ok(out);
    }
    if kind == SAMPLE {
        let interface = requests
            .first()
            .and_then(|r| r.get("interface"))
            .ok_or("missing interface")?;
        if !rfv_hal::valid_interface(interface) {
            return Err("invalid interface name".into());
        }
        let d = devices
            .iter()
            .find(|d| &d.interface == interface)
            .ok_or("DEVICE_NOT_FOUND: rescan after hotplug")?;
        if !supported(&d.driver) {
            return Err("UNSUPPORTED_DRIVER: existing driver left unchanged".into());
        }
        let mut out = device_records(d);
        out.extend(rfv_hal::measurement::collect(root, d).map_err(|e| e.to_string())?);
        return Ok(out);
    }
    Err("unknown operation".into())
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 3 && args[1] == "serve" {
        rfv_ipc::transport::serve(Path::new(&args[2]), "rtw88-adapter", handle)
    } else {
        Err(std::io::Error::other(
            "usage: rfv-adapter-rtw88 serve /absolute/private/adapter.sock",
        ))
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn driver_matching_is_exact() {
        assert!(supported("rtw88_8821ce"));
        assert!(supported("rtw_8821ce"));
        for name in [
            "8821ce",
            "rtw88_core",
            "iwlwifi",
            "rtw88_evil",
            "rtw88_8821ce-extra",
        ] {
            assert!(!supported(name));
        }
    }
}
