//! Evidence-scoped CSI discovery. No firmware/register probing or debugfs reads.
use crate::Device;
use rfv_ipc::{record, Record};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsiStatus {
    Supported,
    PartiallySupported,
    DriverLimited,
    FirmwareLimited,
    Unverified,
    Unsupported,
}
impl CsiStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "CSI_SUPPORTED",
            Self::PartiallySupported => "CSI_PARTIALLY_SUPPORTED",
            Self::DriverLimited => "CSI_DRIVER_LIMITED",
            Self::FirmwareLimited => "CSI_FIRMWARE_LIMITED",
            Self::Unverified => "CSI_UNVERIFIED",
            Self::Unsupported => "CSI_UNSUPPORTED",
        }
    }
}
pub fn csi_capability(d: &Device) -> Record {
    let audited = d.fields.get("vendor_id").map(String::as_str) == Some("10ec")
        && matches!(
            d.fields.get("product_id").map(String::as_str),
            Some("c821" | "b821")
        )
        && matches!(d.driver.as_str(), "rtw88_8821ce" | "rtw_8821ce");
    record(&[("kind","csi_assessment"),("interface",&d.interface),("device_id",&d.identity),("driver",&d.driver),
        ("status",if audited {CsiStatus::DriverLimited.as_str()}else{CsiStatus::Unverified.as_str()}),
        ("scope","RFVision standard userspace acquisition path; not a silicon verdict"),
        ("availability","unavailable"),("hardware_support","unverified"),("firmware_support","unverified"),("driver_support",if audited {"no verified complex-matrix export in audited source"}else{"not audited"}),
        ("evidence",if audited {"docs/drivers/RTW88_AUDIT.md: rx.h descriptor; rtw8821c.c PHY pages; fw.c C2H; debug.c scalar diagnostics. Installed kernel source equivalence not verified."}else{"Identity alone does not establish CSI support"}),
        ("interfaces_checked","sysfs identity and audited public-interface implementation; no verified CSI provider registered"),
        ("register_access","not attempted; no documented safe matrix interface"),
        ("debug_interfaces","generic scalar EVM/CFO/SNR are not CSI; not probed"),
        ("next_step","Use independently verified CSI-capable research hardware/firmware and a reviewed provider; keep the existing system driver")])
}
/// Future providers return versioned independent records, never kernel structures.
/// Unsupported providers must return an error, never fabricated frames.
pub trait CsiAcquisition {
    fn capability(&self, device: &Device) -> Record;
    fn acquire(&self, device: &Device) -> Result<Vec<Record>, String>;
}
pub struct StandardCsi;
impl CsiAcquisition for StandardCsi {
    fn capability(&self, d: &Device) -> Record {
        csi_capability(d)
    }
    fn acquire(&self, _: &Device) -> Result<Vec<Record>, String> {
        Err("CSI_UNAVAILABLE: no validated complex-data acquisition provider".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn not_ofdm_inference() {
        let mut d = Device {
            interface: "wlan0".into(),
            identity: "test".into(),
            driver: "rtw88_8821ce".into(),
            fields: record(&[("vendor_id", "10ec"), ("product_id", "c821")]),
        };
        assert_eq!(csi_capability(&d)["status"], "CSI_DRIVER_LIMITED");
        assert!(StandardCsi.acquire(&d).is_err());
        d.driver = "unknown".into();
        assert_eq!(csi_capability(&d)["status"], "CSI_UNVERIFIED");
    }
}
