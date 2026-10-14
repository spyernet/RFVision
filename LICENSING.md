# Licensing architecture and release gates

**Engineering risk assessment, not legal advice. No legal clearance is implied.**
Professional counsel must review the actual code, interfaces, licences, firmware,
build outputs and proposed distribution in the relevant jurisdictions.

## Original RFVision work

The original Rust crates, tools and documentation are reserved-rights proprietary
work under the scoped root `LICENSE`; final owner attribution, employment/
contributor rights and commercial terms require confirmation. `publish=false`
prevents accidental Cargo registry publication. That notice explicitly excludes
third-party material and cannot override anyone else's licence.

No external Cargo dependency is present. Core/service depend only on original
IPC/Core code; they neither link to nor import the supplied driver. HAL uses
independently implemented public-interface reads. The rtw88 adapter is a separate
executable under `src/drv/`. No driver implementation was copied into it.

## Supplied driver: not proprietary

`drv/rtw88/` remains unchanged and outside the build/package input list.
The audit identifies **97 files declaring GPL-2.0 OR BSD-3-Clause, plus one
GPL-2.0-only Kconfig**. Original notices remain intact. Full per-file hashes and
licence declarations are in `docs/drivers/rtw88-source-manifest.json`.

The repository therefore already contains mixed-licence material. A public
repository/archive of this whole tree is **not a proprietary-only distribution**.
Even dual-licensed files require appropriate notices/terms; kernel linking,
other included files and any future changes require separate analysis.
The subtree lacks a complete standalone licence/notice/source distribution
package. Do not ship it as a cleared driver package or under the root proprietary
notice. Restoring the input to the checkout does not resolve any existing
repository-distribution obligations.

## Interface and runtime evaluation

| Component / interface | Current decision | Review obligations |
|---|---|---|
| sysfs + normal Linux process/socket APIs | Prefer existing public OS interfaces; no driver linking | Confirm target OS/runtime notices and distribution terms |
| nl80211 | Accessed through separately installed iw; native parsing deferred | Inspected pinned `nl80211.h` includes permissive ISC-style text; audit all UAPI headers/notices if later used, do not assume every kernel header has the same terms |
| iw executable | External read-only invocation, not bundled or linked | Verify exact distro version/package licence; independently installed does not by itself settle all combined-work questions |
| ethtool executable | External read-only `-i`, not bundled or linked | GPL family utility; verify package-specific obligations and distribution model with counsel |
| Kernel/rtw88 | Already operating independently; unchanged, unbundled in RFV binary artifact | Kernel/driver licences, any derivative integration, source/notices if separately redistributed |
| Firmware | Use only existing installed firmware via driver | RTL8821C firmware blob not supplied; redistribution/modification/reverse-engineering terms unresolved |
| Rust std/toolchain/runtime | No third-party crate downloads needed | Rust std MIT/Apache and included third-party runtime notices; platform libc/libgcc dynamic dependencies; complete target-binary SBOM before distribution |
| debugfs/register APIs | Not used | Stability, side effects, privileges and derivative-interface issues before integration |
| Future kernel hook/shared ring | Not implemented | Review originality/derivation, kernel licences, required source/build materials and API coupling before coding/distribution |

**IPC, process separation, static/dynamic linking choices, or a dual-licence
header are not automatic exemptions from GPL obligations.** The boundary reduces
implementation coupling and enables independent distribution where appropriate;
legal characterization depends on facts not resolved by this design document.
No claim that all future adapters can be proprietary is made.

## Inventory and packaging

- `docs/licensing/inventory.tsv`: dependency/tool/runtime inventory and unresolved
  reviews. Source manifest: per-file driver inventory. `Cargo.lock`: all six
  local packages, no external crate resolution.
- `tools/package.py`: allowlists only three userspace binaries and selected
  RFVision documents into a separate **review artifact**; excludes `drv/`,
  research references, firmware, toolchains, test fixtures and development files.
  This separation is not an assertion that the artifact is distribution-cleared.
- Building an adapter executable separately is possible with
  `cargo build -p rfv-adapter-rtw88 --release --offline`; no kernel build runs.
- A future driver artifact must preserve its own licence, notices, applicable
  source and build obligations. No proprietary relabeling of GPL changes.

## Required professional review before release

1. Confirm RFVision ownership, proprietary terms and contributor assignments.
2. Review the mixed repository's existing source-distribution obligations;
   obtain complete upstream licence texts/notices and provenance as needed.
3. Review whether each concrete adapter/IPC/kernel integration constitutes a
   derivative/combined work; do not rely on architecture alone.
4. Audit exact platform tool packages, firmware terms, reverse-engineering
   permissions, Rust/runtime notice aggregation and binary link dependencies.
5. Define independent proprietary and third-party distributions and their
   complete source/notice offerings where required.
6. Review any future driver modification **before implementation**, and prohibit
   undocumented hardware writes or firmware experiments pending safety approval.

The development sandbox's Rust tools were npm `@rustbin` repackages of Rust
1.88.0 because no compiler was installed and network hosts were constrained.
They are ignored development artifacts, not runtime dependencies. Rebuild with
an organization-approved, integrity-verified official toolchain before releasing
binaries; npm package labels do not constitute upstream signature verification.

## RF workspace extension review (2026-10-14)

New Rust analysis, HAL parsers and embedded HTML/JavaScript are original RFVision
implementation under the repository's existing reserved-rights terms. No driver
code or firmware was modified, copied into Core, linked, or redistributed as a
new binary. All 98 supplied source files retain their original licenses and hashes.
No third-party runtime Cargo/npm dependency, UI font, CDN asset or CSI capture
was added. Ignored development toolchains/browser test tools are not deliverables
or distribution inputs. Package/release licensing gates still apply.

A future hardware CSI adapter, firmware modification, driver derivative or
redistributed research-tool capture converter needs its own legal review and
complete relevant notices/source/license compliance. Process/IPC separation alone
does **not** guarantee GPL independence. Do not treat this implementation as
clearance to distribute modified drivers, proprietary firmware or derived code.
