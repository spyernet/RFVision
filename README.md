# RFVision · RFV

**Passive, capability-aware Linux Wi-Fi diagnostics — Rust userspace foundation, v0.1.**

RFVision keeps normal Wi-Fi operation separate from RF measurement processing.
It uses an independent `rtw88` adapter process and documented IPC, not linked
Linux driver implementation code. The supplied driver is an **unchanged research
input**, not something the installer builds or replaces.

> **Honest scope:** working discovery, service/adapter IPC, conditional passive
> measurements, capability reports and reversible user-local installation.
> **No CSI acquisition has been demonstrated on RTL8821CE.** There is no CSI
> capture, RF reconstruction, medical imaging, firmware patch, transmit-power
> enhancement or driver replacement in this release. Actual RTL8821CE validation
> remains pending; the development sandbox has no Wi-Fi device.

## Build

Linux, Rust/Cargo 1.88+ (tested with 1.88.0), Python 3 for integration tests.
There are **no external Cargo dependencies**; builds work offline after installing
Rust. For live RF queries, install your distribution's `iw` and `ethtool`
separately. They are optional at runtime: missing tools produce diagnostics,
not invented values. Some kernels restrict station/survey queries; no privilege
escalation or file capabilities are added automatically.

```sh
cargo build --workspace --release --offline
cargo test --workspace --offline
cargo build --workspace --offline
python3 tests/integration/test_platform.py
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

## What can be measured?

| Path | v0.1 behavior |
|---|---|
| sysfs | Netdev packet/byte/error/drop counters; identity, PCI/USB IDs, driver/module, state |
| `iw dev … info` | Current channel frequency and recognized contiguous channel width |
| `iw dev … station dump` | RSSI, average/chain RSSI, counters, retry/failure counts, bitrates, explicit MCS/NSS **when present** |
| `iw dev … survey dump` | Noise and channel timing **if the driver exposes them** |
| `iw phy … info` | Raw bounded driver-advertised PHY capabilities, not normalized physical verification |
| `ethtool -i …` | Driver and firmware version, when provided |
| CSI / EVM / CFO / calibrated phase | Not acquired; research status and limitations reported |

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
