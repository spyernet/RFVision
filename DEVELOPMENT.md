# Development

## Reproducible checks

Use an approved Rust toolchain (MSRV 1.88, tested compiler 1.88.0); Linux only.
No external Cargo crates or build scripts. The sandbox toolchain provenance is
explicitly documented in LICENSING.md; production builds must use an approved,
integrity-verified toolchain.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
cargo build --workspace --offline
python3 tests/integration/test_platform.py
python3 tools/audit_source.py --check
cargo build --workspace --release --offline
cargo tree --workspace --offline
```

Integration tests spawn bounded child servers, use private temporary sockets,
and stop all children at completion. They perform real sandbox enumeration and
separate explicit simulated wire exchanges; they never change drivers/networks.
Rust tests cover parsers, sysfs fixtures, frame handling, capability honesty,
CSI math on labeled synthetic input, and installer restrictions. Test-only fake
sysfs roots are not exposed as runtime CLI options.

## Layout and engineering rules

See ARCHITECTURE.md. Add no driver-specific import/dependency to Core or service.
Put vendor knowledge only in `src/drv/<driver>/`. Do not edit the supplied driver
for convenience. Before any dependency addition, update the licence inventory,
review transitive licences and verify sources/lockfile. Before kernel or firmware
integration, stop for the explicit legal/safety gates.

No unknown fields become zeros. Preserve units, source, identity, clock domain,
quality and simulation labeling. Any CSI capture extension needs a validated
matrix schema and hardware evidence, not a scalar field called CSI. Preserve
normal Wi-Fi and regulatory behavior; the query runner rejects operations outside
its explicit read-only allowlist.

## Packaging

```sh
python3 tools/package.py target/rfvision-review.tar.gz
# Inspect archive: no kernel source, firmware, research or toolchain content.
tar -tzf target/rfvision-review.tar.gz
```

This generates a **review artifact**, not a distribution authorization. Destination
creation is exclusive; remove only a known earlier generated archive or choose a
new output filename before rebuilding it. Review target dynamic dependencies
(e.g. `ldd target/release/rfvd`) and full standard-library/runtime licence notices.

## Remaining release work

Actual RTL8821CE tests, parser validation against supported distro tool versions,
firmware/physical capability audit, source/runtime notice completion, security and
legal reviews, approved-toolchain rebuild, and measured CPU/memory/connectivity
regression baselines. Do not infer performance guarantees from no-hardware tests.
High-rate transport/native netlink remain future projects with their own tests.
