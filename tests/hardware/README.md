# RTL8821CE hardware validation — NOT RUN in the development sandbox

Run only on your own/authorized Linux Wi-Fi device with the existing driver.
Do not install a replacement module, change firmware, stop the connection,
create monitor mode, alter channels or elevate transmit power for this test.

1. Build with an approved Rust toolchain. Confirm current Wi-Fi works using your
   ordinary applications. Record kernel, module/firmware version and PCI IDs.
2. Run `rfv discover`, then start the adapter/core in a private runtime directory
   as described in README. Save `rfv report CORE_SOCKET`.
3. Run `rfv sample CORE_SOCKET INTERFACE` while normally associated. Compare
   reported values to read-only `iw dev INTERFACE station dump`,
   `iw dev INTERFACE info`, optional `survey dump`, `iw phy PHY info`,
   `ethtool -i INTERFACE` and sysfs counters. Data are sampled at different times;
   exact equality is not expected if traffic changes. Counter-only success is
   not RF validation.
4. Run `rfv wizard validate CORE_SOCKET`; retain exit code and JSON. Confirm
   connection still works through ordinary applications. The wizard does not
   establish Internet reachability or measure transient disruptions.
5. If safe with a spare adapter/alternate connection, test unplug/replug or normal
   suspend/resume manually. The original network must not be sacrificed just to
   exercise this step. Samples should error on disappearance and discover the
   returned device on subsequent requests. Record identity changes and errors.
6. Confirm CSI hardware/firmware remains UNVERIFIED/not acquired (the new
   csi_assessment identifies CSI_DRIVER_LIMITED only in the standard userspace path) and missing survey/PHY fields
   remain absent, not zero-filled. Never use generic debug zeros as evidence.
7. Compare idle and explicit-sampling CPU/RSS and normal application throughput
   to baseline. Report tool versions, sample count/delay, conditions and observed
   results. Set acceptance thresholds from product requirements before release.
8. Exercise plan/install/rollback in a new private user prefix. Stop RFVision,
   rollback, and confirm original networking/driver/kernel modules unchanged.

Use `tests/hardware/collect_report.py CORE_SOCKET NEW_OUTPUT_DIRECTORY` to save
read-only CLI outputs with private permissions. No capture made by this script
is automatically declared hardware-verified. It saves identifiers; redact them
before sharing. Do not commit large measurements, firmware or captures to Git.

For power/cache/coverage/optimizer/dashboard checks, use the expanded
[RF hardware validation procedure](../../docs/rf/HARDWARE_VALIDATION.md).
