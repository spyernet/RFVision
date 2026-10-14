# Validation report — 2026-10-14

## Scope and environment

Actual Linux sandbox, kernel `6.1.158+`, x86_64, Rust/Cargo 1.88.0.
No Wi-Fi interface is exposed (`eth0` and `lo` only); `iw` and `ethtool` are not
installed here. No kernel driver was built, installed, replaced, disabled or
modified. No RF/CSI measurements were obtained. Rust tools used for this run
were development-only npm repackages; production toolchain approval is pending.

## Reproduced software checks

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | PASS, no warnings |
| `cargo test --workspace --offline` | PASS: 21 unit tests, 0 failures |
| `cargo build --workspace --offline` | PASS: all three debug executables |
| `python3 tests/integration/test_platform.py` | PASS: 8 tests, 0 failures |
| `cargo build --workspace --release --offline` | PASS: all three optimized executables |
| `python3 tools/audit_source.py --check` | PASS: 98 supplied source files unchanged |
| `git diff --exit-code origin/main -- drv/rtw88` | PASS: supplied subtree unchanged |
| `cargo tree --workspace --offline` | Six original local crates; no external Cargo dependencies; no driver dependency in Core/service |
| Userspace-only packaging and archive listing | PASS: three binaries and allowlisted documents, no driver/firmware/toolchain |
| Release `ldd rfvd` | Dynamic platform libc/libgcc_s and loader; no driver library; full runtime notice review pending |

### Unit coverage

- Exact driver alias matching (not arbitrary prefix matching).
- PCI/USB sysfs fixture identity, parent lookup, ifindex change and disappearance.
- Interface-name validation and rejection of mutating/injected helper commands.
- Station RSSI/chains/counters/MCS/NSS; channel and survey context; omission of
  malformed/missing/nonfinite values, including noncontiguous width.
- Wire round-trip and every truncation point; duplicate keys, size/version
  rejection and JSON escaping.
- Capability status vocabulary, current-identity scoping, absence of observations,
  simulation exclusion, orphan/invalid measurements and scalar-CSI rejection.
- Explicitly synthetic complex-input normalization/math and invalid CSI rejection.
- Counter-reset-safe delta and private installation-parent/checksum safeguards.

### Integration coverage

1. Real host discovery with independent Python wire client; private socket mode.
2. Nonexistent/injected interface errors, no invented samples.
3. HELLO requirement and unknown-opcode errors.
4. Oversized frame rejection and absolute slow-peer deadline, followed by a
   successful subsequent client request.
5. No clobbering existing socket paths; refusal of public runtime directory.
6. Adapter loss/restart and subsequent rediscovery without restarting Core.
7. No-side-effect plan, mandatory consent, no overwrite, refusal to roll back
   modified/unexpected files, then successful exact rollback.
8. Python fixture adapter interoperability, with **simulation labels preserved**
   and no promotion of simulated RSSI to a hardware AVAILABLE capability.

## Actual release smoke observation

Both release servers were started against the actual sandbox, queried through
the release CLI, then stopped. Raw structured results are recorded in
[`sandbox-observation.json`](sandbox-observation.json):

- `rfv discover`: exit 0, device_count = 0.
- `rfv report CORE_SOCKET`: exit 0, adapter detected, measurements = 0.
- `rfv wizard validate CORE_SOCKET`: exit **2**, IPC passed, `NO_WIFI_DEVICE`,
  `INCOMPLETE_OR_FAILED`, physical accuracy `NOT_TESTED`, CSI `NOT_ACQUIRED`.

The exit-2 result is the correct refusal to claim hardware success. It is **not**
a passed hardware validation. Synthetic fixtures above are software tests only.
No production installation was performed; only temporary test prefixes were
created and rolled back.

## Still unverified / release gates

- Actual RTL8821CE identification, firmware version, operating bands/widths,
  accessible RF fields and chain/antenna mapping.
- Measurement accuracy, runtime helper output across distro versions and
  permission models, physical disconnect/reconnect and suspend/resume.
- Existing Wi-Fi connectivity, throughput, latency and CPU/memory overhead on
  the target hardware under ordinary use and sampling load.
- Real CSI extraction, validated phase/calibration/synchronization and CIR.
- High-rate streaming / zero-copy claims (not implemented or benchmarked).
- Professional licensing/distribution/security review, complete notices/SBOM,
  approved-toolchain rebuild and signed release process.

Follow `tests/hardware/README.md`; report incomplete tests honestly. Software
success cannot substitute for these gates.
