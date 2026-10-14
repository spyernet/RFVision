# RFVision architecture

## Implemented boundary

```text
rfv CLI / wizard ── RFV/1 Unix socket ── rfvd (core + IPC only)
    │                                      │
    └─ local sysfs discovery               └─ RFV/1 Unix socket
                                              │
                                    rfv-adapter-rtw88
                                      │ HAL public interfaces
                                      ├─ read-only sysfs
                                      ├─ independent iw process → nl80211
                                      └─ independent ethtool process
                                              │
                                      existing Linux driver
```

- `src/core/`: driver-independent record validation, capability reconciliation,
  counter-reset-safe delta primitive, and validated offline CSI math contract.
- `src/hal/`: device enumeration and measurements through public interfaces;
  no vendor constants, register layouts, or kernel implementation imports.
- `src/ipc/`: independent binary framing, JSON presentation, Unix transport.
- `src/service/`: `rfvd`, forwards to a configured adapter and processes records.
  It does not depend on HAL or a driver adapter crate.
- `src/drv/rtw88/`: independently written userspace adapter. Owns driver-name
  matching, PCI model labeling, source-backed research capability declarations.
- `src/wizard/`: `rfv`, local discovery, client, assessment, validation and
  explicit private-prefix install/rollback.
- `drv/rtw88/`: supplied third-party source, unchanged, not a Cargo member and
  not a build input. Its original location is preserved intentionally.

Workspace crates expose only the boundaries that exist. Empty reconstruction,
calibration, RF imaging and firmware directories are not presented as subsystems.
No Rust `include!`, FFI, linker dependency or build script references driver code.
The service can communicate with an adapter written in another language; the
integration suite exercises that using a Python fixture.

## Data and capability model

Records carry explicit kind, identity, provenance, units and host observation
clock. Absence is not zero. Capability status and implementation maturity are
separate: `AVAILABLE_WITH_ADAPTER` may describe a future monitor reader while
`implementation=not_implemented` prevents a false claim of implemented capture.
`AVAILABLE` means a non-simulation scalar was observed in the current batch,
not that the hardware is physically validated, calibrated, or available forever.

Firmware version is reported only if the installed driver exposes it. PHY
advertisements are retained as raw `iw` output rather than guessing physical
antenna count, exact chain mapping or usable regulatory bands. Host timestamps
cannot stand in for receive timestamps or cross-device synchronization.

## Lifecycle and performance

A client session consists of HELLO and one request. No measurement work occurs
while idle. Every discovery/sample re-enumerates the relevant network namespace;
a final identity check discards batches if interface/driver identity changed.
The service reconnects to the adapter on each request, so a restarted adapter can
be used without restarting the core (after explicit stale-socket cleanup).

Limits: eight concurrent sessions per server, 1 MiB wire payload, 4,096 records,
64 fields/record, two-second helper timeout, 256 KiB/helper stream. Request
reads have an absolute three-second deadline. Sampling invokes up to five
helpers and is intended for diagnostics. One service currently targets one
adapter endpoint; multi-adapter routing is a future service-manager extension,
not a reason to add vendor logic to Core.

This is **not** zero-copy or high-rate streaming. There is one framed payload
allocation and ordinary serialization/copying; Core currently copies measurement
records when reconciling capabilities. Caller-driven bounded samples provide
natural backpressure; there is no unbounded queue or capture drop claim. Replacing
`iw` subprocesses with an independently audited native nl80211 implementation is
the first performance milestone. Shared-memory rings require a negotiated new
transport, lifecycle/ownership design, overflow counters and benchmarks.

## Deferred, not implemented

- Native nl80211/event-driven hotplug; devices without a visible netdev.
- Persistent sessions, multiplexed subscriptions, multi-adapter routing.
- Radiotap reading and hardware timestamp handling; no monitor creation now.
- Shared-memory binary CSI streaming, calibrated signal processing, CIR.
- Kernel instrumentation and firmware changes (evidence + legal/safety gates).
- Automatic service supervision, upgrades, crash-atomic installer recovery.

Future `iwlwifi`, `ath9k`, `ath10k`, `ath11k`, `mt76` adapters are research targets,
not compatibility promises. New adapters may implement the current IPC contract
without modifying the Core, provided they use its measurement record semantics.

## RF research workspace integration

HAL adds standard read-only BSS-cache/power parsers and an evidence-scoped CSI
provider contract. The existing rtw88 userspace adapter emits generic records;
the GPL/dual-licensed source input is unchanged and not linked or copied.
`rfv-ipc::measurement` owns shared scalar validation. Core adds driver-independent
link-budget, coverage, optimizer, range-assessment, session and offline CSI
modules. `rfvd` serializes bounded session processing; it still has no HAL/driver
implementation dependency. The `rfv` presentation/wizard binary now depends on
Core for offline tools, recording/replay and the embedded authenticated dashboard.
The dashboard queries its own origin; the gateway calls existing Unix IPC.
No new service privilege, driver install or automatic radio configuration path
was introduced. Offline RFVCSI imports are not a live adapter data path.
