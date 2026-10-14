# RFVision · RFV

**Passive Linux RF measurement, coverage and CSI research workspace — v0.1.**

RFVision keeps normal Wi-Fi operation separate from RF measurement processing.
It uses an independent `rtw88` adapter process and documented IPC, not linked
Linux driver implementation code. The supplied driver is an **unchanged research
input**, not something the installer builds or replaces.

> **Implemented:** passive diagnostics, provenance-aware link budgets, measured-point
> coverage/history, RF Link Optimizer, cached-network discovery, guided multi-node
> planning, private recordings/replay, offline complex-CSI analysis and a nine-page
> read-only dashboard. **RTL8821CE live CSI remains unavailable.** No firmware
> patching, power-limit bypass, monitor creation or driver replacement is included.
> Algorithm/parser tests are not RF validation; the sandbox has no Wi-Fi device.

See the [RF workspace guide](docs/rf/WORKSPACE.md),
[CSI interchange/analysis contract](docs/rf/CSI_FORMAT.md) and
[hardware validation report](docs/rf/HARDWARE_VALIDATION.md) for exact scope,
limitations and a procedure for your RTL8821CE/Arch Linux host.

## Build

Linux, Rust/Cargo 1.88+ (tested with 1.88.0), Python 3 for integration tests.
There are **no external Cargo dependencies**; builds work offline after installing
Rust. For live RF queries, install your distribution's `iw` and `ethtool`
separately. They are optional at runtime: missing tools produce diagnostics,
not invented values. Some kernels restrict station/survey queries; no privilege
escalation or file capabilities are added automatically.
The root `./build.sh` builds the release wizard, core service and bundled
userspace `rtw88` IPC adapter into `target/release/`.

```sh
./build.sh
cargo build --workspace --release --offline
cargo test --workspace --offline
cargo build --workspace --offline
python3 tests/integration/test_platform.py
python3 tests/integration/test_rf_workspace.py
python3 tools/audit_source.py --check
```

## Run (no installation needed)

Use a short, absolute, private runtime path owned by your user. These commands
run in separate terminals; no root is needed for sysfs discovery or IPC.

```sh
mkdir -m 700 "$HOME/.rfv-run"    # if it exists, verify owner and mode 0700
./target/release/rfv discover

# Terminal 1
./target/release/rfv-adapter-rtw88 serve "$HOME/.rfv-run/adapter.sock"
# Terminal 2
./target/release/rfvd serve "$HOME/.rfv-run/core.sock" "$HOME/.rfv-run/adapter.sock"
# Terminal 3
./target/release/rfv report "$HOME/.rfv-run/core.sock"
./target/release/rfv sample "$HOME/.rfv-run/core.sock" wlp1s0
./target/release/rfv watch "$HOME/.rfv-run/core.sock" wlp1s0 10 1000
./target/release/rfv wizard validate "$HOME/.rfv-run/core.sock"
```

Replace `wlp1s0` with the interface from discovery. Output is a JSON array of
records (all scalar values are strings to preserve integer precision). Watch is
newline-delimited arrays, with a minimum one-second **delay after** each query.
This is low-rate diagnostic sampling, not a high-rate capture engine.

`report` is discovery and conditional capability assessment; `sample` performs
acquisition and marks fields actually observed in that batch as `AVAILABLE`.
An absent adapter, unknown driver, unplugged device or unsupported field is
reported explicitly. A discovered interface is not proof of RF accuracy.

Stop servers with Ctrl-C. Signals currently leave socket files behind. After
confirming both servers have stopped, remove **only** their stale socket files
before restarting. The servers intentionally never unlink an existing path.

## RF workspace commands

With adapter/core running, open the embedded dashboard:

```sh
export RFV_UI_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(24))')"
printf '%s\n' "$RFV_UI_TOKEN"  # paste into the dashboard token field; keep private
./target/release/rfv ui "$HOME/.rfv-run/core.sock" wlan0 8080
```

Open port 8080 locally or through a trusted HTTPS proxy. The server binds all
interfaces; firewall it appropriately. APIs require the token, which is never
embedded in the public page. Live Monitor · CSI Research · RF Coverage · Link
Budget · RF Optimizer · Range Extenders · Hardware · Diagnostics · Recordings.
All calculations/queries are read-only with respect to networking equipment.

```sh
./target/release/rfv budget inputs.tsv
./target/release/rfv range nodes.tsv
./target/release/rfv csi analyze capture.tsv
./target/release/rfv record "$HOME/.rfv-run/core.sock" wlan0 desk.rfv 30 1000 \
  ground-floor desk 2.0 3.0 tape-measure
./target/release/rfv replay desk.rfv
./target/release/rfv coverage desk.rfv
```

These input files are supplied by you, not generated example measurements.
The [workspace guide](docs/rf/WORKSPACE.md) specifies TSV fields, input provenance,
regulatory limitations and recording formats. Positions are explicit user records;
without them, displays are unlocated histories. Link-budget output is always a
prediction, never measured RSSI. CSI imports do not certify hardware support.

## Safe installation wizard

```sh
./target/release/rfv wizard assess
mkdir -m 700 "$HOME/.rfv-local"
./target/release/rfv wizard plan "$HOME/.rfv-local/v0.1" "$PWD/target/release"
./target/release/rfv wizard install "$HOME/.rfv-local/v0.1" "$PWD/target/release" --apply
# Stop any running installed processes before rollback:
./target/release/rfv wizard rollback "$HOME/.rfv-local/v0.1" --apply
```

Installation copies only the three userspace executables into a **new** private
prefix. It does not start services, invoke a package manager, modify firmware,
load modules, create monitor interfaces, change channels, or touch NetworkManager.
Rollback refuses changed files or unexpected content. See
[the wizard specification](INSTALLATION_WIZARD.md) for limitations.

To install only the bundled `rtw88` IPC adapter, use a different new prefix:

```sh
./target/release/rfv wizard plan-rtw88 "$HOME/.rfv-local/rtw88-adapter" "$PWD/target/release"
./target/release/rfv wizard install-rtw88 "$HOME/.rfv-local/rtw88-adapter" "$PWD/target/release" --apply
```

This copies only the `rfv-adapter-rtw88` userspace executable. It is not the
Linux `rtw88` kernel driver and does not install, load, replace, or modify any
system Wi-Fi driver. The install requires explicit `--apply`, creates a new
user-owned private prefix, and is reversible with the rollback command above.

## What can be measured?

| Path | v0.1 behavior |
|---|---|
| sysfs | Netdev packet/byte/error/drop counters; identity, PCI/USB IDs, driver/module, state |
| `iw dev … info` | Current channel/frequency/contiguous width and configured TX power (not radiated power) |
| `iw dev … station dump` | RSSI, average/chain RSSI, counters, retry/failure counts, bitrates, explicit MCS/NSS **when present** |
| `iw dev … survey dump` | Noise and channel timing **if the driver exposes them** |
| `iw phy … info` | Bounded PHY advertisement and per-channel advertised power ceiling/constraints; minimum unknown |
| `iw dev … scan dump` | Existing BSS cache only, measured cached signal/frequency/age when present; no triggered scan |
| `ethtool -i …` | Driver and firmware version, when provided |
| Live CSI / EVM / CFO / calibrated phase | Not acquired; research status and limitations reported |
| Offline CSI import | Validated complex data, amplitude/phase, detrending, temporal features; explicit simulation/unverified-import labels |

No register/debugfs writes, active scans, monitor capture or radio configuration
commands exist in the acquisition path. No SNR is invented by combining unrelated
RSSI and survey samples.

## Documentation

- [Architecture](ARCHITECTURE.md) · [IPC](IPC_SPECIFICATION.md) · [HAL](HAL_SPECIFICATION.md)
- [Driver adapter guide](DRIVER_ADAPTER_GUIDE.md) · [Hardware capabilities](HARDWARE_CAPABILITIES.md)
- [Source audit](docs/drivers/RTW88_AUDIT.md) · [CSI research](CSI_RESEARCH.md)
- [Licensing and release gates](LICENSING.md) · [Security](SECURITY.md)
- [Development](DEVELOPMENT.md) · [Validation report](docs/development/VALIDATION.md)

**Licensing:** original RFVision work is proprietary/reserved rights, with final
commercial terms pending owner/counsel review. That notice does **not** relicense
`drv/rtw88/` or any third-party material. Process separation is a design boundary,
not a legal guarantee. No production distribution is legally cleared by this work.
