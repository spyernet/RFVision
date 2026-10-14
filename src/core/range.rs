//! User-registered infrastructure, not inferred repeater/mesh capabilities.
use rfv_ipc::{record, Record};
/// TSV: id, role, backhaul, compatibility, source. Compatibility is an explicit
/// user assertion of vendor-confirmed interoperation, not discovery evidence.
pub fn assess(text: &str) -> Result<Vec<Record>, String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let p: Vec<_> = line.split('\t').collect();
        if p.len() != 5 || p.iter().any(|s| s.trim().is_empty()) || !seen.insert(p[0]) {
            return Err(
                "node requires unique id, role, backhaul, compatibility, source (TSV)".into(),
            );
        }
        if ![
            "repeater",
            "bridge",
            "mesh",
            "ethernet_ap",
            "external_radio",
            "measurement_node",
        ]
        .contains(&p[1])
            || ![
                "wireless_shared",
                "wireless_dedicated",
                "ethernet",
                "unknown",
            ]
            .contains(&p[2])
            || !["confirmed", "unknown", "incompatible"].contains(&p[3])
        {
            return Err("invalid node enum".into());
        }
        let guidance=match p[1] {
            "repeater"=>"Place where backhaul is still strong; measure both hops. Shared-channel forwarding consumes airtime; throughput is not inferred from RSSI.",
            "mesh"=>"Verify vendor mesh interoperability, security, supported bands and backhaul before configuring. 802.11s/EasyMesh/vendor mesh are not interchangeable.",
            "bridge"=>"Verify bridge/WDS/4-address support on both ends and security compatibility. A client connection alone does not prove transparent bridging.",
            "ethernet_ap"=>"Verify wired backhaul, AP mode, channel plan and LAN addressing; avoid unintended double NAT. Measure coverage after placement.",
            "external_radio"=>"Verify Linux driver, regulatory domain, bands, security and device mode using vendor documentation.",
            _=>"Record node identity, location reference, clock basis and calibration. Unsynchronized RSSI surveys are not coherent multi-radio CSI.",
        };
        out.push(record(&[
            ("kind", "range_node"),
            ("id", p[0]),
            ("role", p[1]),
            ("backhaul", p[2]),
            ("compatibility", p[3]),
            ("source", p[4]),
            ("basis", "user_registered_not_auto_verified"),
            ("guidance", guidance),
            ("configuration", "manual_only"),
        ]));
        if out.len() > 128 {
            return Err("node limit 128".into());
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nodes() {
        let r=assess("AP1\tmesh\twireless_dedicated\tunknown\tuser inventory\nAP2\tethernet_ap\tethernet\tconfirmed\tvendor manual").unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["compatibility"], "unknown");
        assert!(
            assess("a\tmesh\tunknown\tunknown\ttest\na\tbridge\tunknown\tunknown\ttest").is_err()
        );
    }
}
