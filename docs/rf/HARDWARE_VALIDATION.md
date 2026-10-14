# RF workspace hardware and software validation

Date: 2026-10-14. Branch workspace changes; no driver modifications.

## Evidence levels

| Feature | Implemented | Test basis | Real RTL8821CE validation |
|---|---|---|---|
| Identity/driver discovery | yes | sysfs fixture tests and actual empty sandbox | user supplied discovery output only; not reproduced here |
| Passive scalar acquisition | yes, conditional fields | parser/unit and IPC integration | pending |
| Cached network discovery | yes, scan dump only | parser fixtures / missing-device path | pending |
| Configured TX power / advertised ceiling | yes, conditional | parser fixtures; no setters | pending |
| CSI capability report | source-scoped detector | pinned local source + identity tests | installed driver equivalence unverified |
| RTL8821CE live CSI | **unavailable** | no export/provider demonstrated | not acquired |
| Offline CSI math/format/replay | yes, bounded subset | explicitly synthetic algorithm tests | no genuine capture supplied |
| Link budgets/conversions | yes | analytic reference calculations | predictions, not RF validation |
| Coverage/history/comparison | yes, measured points only | schema/provenance/clock tests | pending field survey |
| Optimizer | heuristic rules | matched/reset/missing/counter/channel tests | thresholds not device-calibrated |
| Range-device assessments | manual evidence + cache | registry/parser tests | no automatic interoperability certification |
| Dashboard/recordings | yes | HTTP/CLI tests and headless Chromium | end-to-end live radio still pending |

The provided user output identifies `wlan0`, `phy0`, PCI `10ec:c821`,
`rtw88_8821ce`, carrier 1 and kernel string `7.2.7-zen1-1-zen`. This establishes
what was reported on that host; it does **not** establish CSI, RF accuracy,
firmware version, per-packet export or an exact match to the audited source.
This sandbox exposes only `eth0` and `lo`. No Wi-Fi hardware tests were run here.

The local pinned source audit was rechecked: all 98 supplied driver files remain
unchanged. `rtw8821c.c:615–684` parses scalar power/width; header EVM/CFO/SNR fields
are not consumed by that parser. `csi_buf_pg_num=0` and `cfg_csi_rate` are not
matrix exports or proofs of hardware impossibility. Firmware internals were not
available. No registers or private debugfs endpoints were probed.

## Reproducible software checks

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo build --workspace
python3 tests/integration/test_platform.py
python3 tests/integration/test_rf_workspace.py
python3 tools/audit_source.py --check
```

Results for this change: workspace check passed; all 45 Rust tests passed; the
release build (`./build.sh`) passed; existing
9 platform integration tests and 5 new RF-workspace integration tests passed;
driver audit passed. A headless Chromium run exercised all nine navigation pages,
Rust-backed budget calculation, node registration, API empty-host errors and
390px mobile layout with no JavaScript errors. The optional browser smoke script
is `tests/ui/dashboard.cjs`; no browser/toolchain packages are shipped in RFVision.
Rust 1.88.0 was installed in ignored `.tools/` from npm-mirrored toolchain packages
because the sandbox initially lacked Cargo; use an approved toolchain for release.
These are software checks, not regulatory, calibration or production certification.

## Hardware acceptance procedure for Arch Linux / wlan0

1. Preserve the normal driver and association. Record `rfv discover`, kernel,
   `ethtool -i wlan0`, PCI ID, board/antenna information if known and regulatory
   context. Compare the installed source/package to the pinned source separately.
2. Start existing adapter/core, then `rfv report` and `rfv sample` as in README.
   Compare RSSI/rate/MCS/width/counters against `iw dev wlan0 station dump` and
   `iw dev wlan0 info` near the same time; account for non-atomic observations.
3. Compare power ceilings/disabled channels to `iw phy phy0 info`. Treat missing
   limits as unavailable; do not infer a minimum or issue a txpower command.
4. Compare BSS entries against `iw dev wlan0 scan dump`. Expect old/empty cache
   and permission failures. Verify RFVision has not triggered an active scan.
5. Record repeated stationary observations at two or more documented positions
   with the same AP/channel. Confirm age/count/reference provenance, no fabricated
   coordinates, and blank unmeasured map regions. Compare to external calibrated
   equipment before making accuracy claims.
6. Inspect optimizer rules with normal authorized traffic; survey busy time may
   be absent on this driver. Retry counts are not loss probabilities. Manually
   trial placement/orientation or a supported AP setting, then pin/compare like
   links. No automatic settings change is expected.
7. Verify CSI page remains unavailable, firmware/hardware support unverified,
   subcarrier/rate/phase absent unless an independently sourced capture is imported.
   Do not submit generic PHY status or zero debug fields as CSI evidence.
8. Run `rfv wizard validate`; report exact PASS/INCOMPLETE status. Test normal
   connectivity before/during/after acquisition. With an alternate safe connection,
   test disconnection/reconnection, missing `iw`, permission failures, stale cache,
   clock correction and counter reset. Do not disrupt the primary network just
   to execute a test.
9. Export private recordings, node registry and actual results. Record tool versions,
   durations, CPU/RSS, omissions and unexpected behavior. Never mark fixtures or
   imported unverified captures as hardware-validated. Do not commit personal
   SSIDs, BSSIDs, locations, captures or tokens.

Further hardware is needed for a **demonstrated supported CSI acquisition route**
if RTL8821CE's standard interface remains the only local radio interface. Evaluate
exact supported chip/firmware/OS combinations in the pinned Nexmon CSI or Intel
5300 CSI Tool references in CSI_RESEARCH.md; those are research ecosystems, not
plug-and-play RFVision adapters or an Arch compatibility promise. Procurement
requires checking firmware licensing, driver/kernel compatibility, IQ format,
calibration and legal use. No specific purchase is certified by this report.
