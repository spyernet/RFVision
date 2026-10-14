# Hardware abstraction and measurement contract

## Implemented discovery

`rfv-hal` reads wireless interfaces in `/sys/class/net` identified by `wireless`
or `phy80211`. It discovers active driver/module symlinks, ifindex, operstate,
carrier if readable, modalias, optional module version and PHY name. Device
ancestors are inspected for PCI vendor/device or USB idVendor/idProduct.
Only devices visible in the current network namespace and with a netdev are
covered. Disabled/unbound hardware, rfkill enumeration and wiphys without a
netdev are not exhaustively enumerated in v0.1.

Identity is canonical sysfs device path + ifindex; interface is separately
reported. It is scoped to the live host/network namespace, not a globally
unique persistent hardware serial. Each sample revalidates identity and driver;
there is no event stream or guarantee against an extremely fast disconnect and
reconnect reusing exactly the same identity during a query.

Model identification is an adapter concern. The HAL never assumes physical
bands/antennas from a driver name. `iw phy` information is driver-advertised,
not board-level or firmware-independent verification. Firmware version may be
absent. USB parent lookup is bounded to six ancestors.

## Measurement record

Required: `kind=measurement`, `interface`, `device_id`, `name`, `value`, `unit`,
`source`, `observed_unix_ns`, `time_basis`, `quality`.

Optional: `peer` (station MAC), `chain_index` (position in iw's reported list,
not a verified physical RF-chain mapping), `encoding`, `frequency_mhz`,
`channel_in_use`. A sample record adds `sample_id`, `atomic=false`,
`environment=live`, `csi=not_acquired`. Multiple helper results are **not**
simultaneous. Values do not share a physical receive timestamp.

- Host clock: UNIX nanoseconds at observation, subject to clock adjustments.
- Quality: `driver_reported_uncalibrated`; tests use `simulation`.
- Missing values are absent records, not `0`, empty IQ, or a filled array.
- Sources: `sysfs.netdev`, `iw.station`, `iw.interface`, `iw.survey`.
- Interface counters are cumulative netdev statistics, not RF/CRC-specific
  counters. Zero, if explicitly returned, is a valid counter value.
- RSSI and chain RSSI: signed dBm; not calibrated absolute power. Station
  fields can represent the driver's last received frame/average, not new traffic
  triggered by the query. Current parser accepts -127..0 dBm only.
- Rates: Mbit/s; MCS/NSS only when explicitly labeled by iw; no inferred streams
  from bitrate. Encoding preserves the iw field label. HT MCS is not converted
  into an invented NSS. Channel width accepts explicit 5/10/20/40/80/160/320 MHz,
  not noncontiguous 80+80 widths. This generic parser does not assert RTL8821CE
  supports every width it can parse.
- Survey noise and cumulative timing are bound to their channel. They are not
  paired with station RSSI to manufacture SNR.

The CLI presents numbers as JSON strings to retain large counter/timestamp
precision. Public Rust Core validates required metadata, numeric finiteness,
identity attachment and status vocabulary, not the honesty of a trusted adapter.

## Public query strategy

Fixed root-owned, non-group/world-writable executable locations for `iw` and
`ethtool`; environment cleared, C locale, no shell. Commands:

```
iw dev IFACE info
iw dev IFACE station dump
iw dev IFACE survey dump
iw phy PHY info
ethtool -i IFACE
```

These are read queries; no scan, channel setter, txpower setter, register
access, monitor interface or privileged helper is invoked. Privilege failures,
missing tools, timeout, absent fields and parser mismatches become diagnostics.
The subprocess parser is deliberately conservative and **not a stable iw text
ABI**; unknown formats are not guesses. Long-term native nl80211 replaces this
implementation behind the same HAL contract after UAPI/licence audit.

## Capabilities

Every capability includes interface/device identity, feature, status,
implementation state, evidence and scope.

| Status | Meaning |
|---|---|
| AVAILABLE | Corresponding non-simulation value observed in the current batch; not calibrated or universally available |
| AVAILABLE_WITH_ADAPTER | Known acquisition/integration route; consult implementation and runtime field availability |
| REQUIRES_DRIVER_MODIFICATION | Source inspection points to missing driver exposure; not an authorization to patch |
| REQUIRES_FIRMWARE_SUPPORT | Evidence establishes firmware cooperation as necessary; not currently asserted for RTL8821C CSI |
| REQUIRES_HARDWARE_SUPPORT | Evidence establishes a physical prerequisite; not a software workaround |
| UNSUPPORTED | Demonstrably unavailable in specified scope; do not use for an uninvestigated chipset |
| UNVERIFIED | Insufficient evidence, unsupported current acquisition, or research pending |

Core downgrades an unsubstantiated AVAILABLE claim and excludes explicit
simulation values from promotion. It never conflates a mathematical CSI helper
with device CSI support. Future adapters must retain scope/evidence and label
synthetic fixtures as such.

## Offline CSI type (not an acquisition interface)

`rfv_core::csi::CsiFrame`: provenance, simulation flag, center frequency Hz,
bandwidth Hz, RX/TX dimensions, ordered explicit subcarrier indices, scaled
finite complex floats, optional hardware timestamp + unit and phase calibration
metadata. Layout is `[subcarrier][rx_chain][tx_stream]`.

Validation rejects zero/oversized dimensions, unordered/duplicate bins,
mismatched matrix length, invalid scaling and nonfinite IQ/amplitude. Amplitude
and mathematical phase can be computed for **provided complex inputs**; zero
IQ has no phase. No RSSI conversion, automatic calibration, unwrapping, time
synchronization or CIR reconstruction exists. This Rust data contract is not
currently carried over IPC; a future binary stream schema is required.
