# RF measurement workspace

This is an integrated extension of the existing HAL → adapter → RFV/1 → Core
architecture, not a replacement driver or a production-validated RF instrument.
The dashboard is embedded in `rfv`; it has no CDN, telemetry or JavaScript build
step. Rust analysis uses only workspace crates and the standard library.

## Start on the RTL8821CE host

Build `./build.sh`, then use the existing three-terminal service procedure:

```sh
mkdir -m 700 "$HOME/.rfv-run" # verify permissions if already present
# Terminal 1
./target/release/rfv-adapter-rtw88 serve "$HOME/.rfv-run/adapter.sock"
# Terminal 2
./target/release/rfvd serve "$HOME/.rfv-run/core.sock" "$HOME/.rfv-run/adapter.sock"
# Terminal 3
export RFV_UI_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(24))')"
# Copy this generated secret into the dashboard's token field. Do not publish it.
printf '%s\n' "$RFV_UI_TOKEN"
./target/release/rfv ui "$HOME/.rfv-run/core.sock" wlan0 8080
```

Open the host's port 8080 (locally `http://localhost:8080`, or through a trusted
HTTPS proxy). The gateway binds `0.0.0.0`; restrict network access with your
firewall. The page is public; **every data/analysis API requires the token**.
No token is embedded in HTML, URLs, logs or browser persistent storage.
Transport on the listening port is HTTP, not TLS. This small single-client
research gateway is not an Internet-facing production server: requests are
serialized and a slow client can temporarily delay others. Do not expose it
without TLS, access controls and request-rate limits.

Connect queries discovery and one sample. Start live polls after each completed
query with a two-second delay; this is not per-packet streaming. Errors preserve
old values with a stale indication, not zeros. Pause does not cancel an already
running read-only query. Data are kept in browser memory (last 300 batches and
approximately 32 MiB JSON), not automatically stored to disk.

### Navigation

- **Live Monitor:** measured scalar context, per-link RSSI history, source/quality.
- **CSI Research:** evidence-scoped support and offline RFVCSI/1 imports only.
- **RF Coverage:** explicit user-position recording; measured-point heatmap and
  location comparison, no filled-in propagation surface or invented coordinates.
- **Link Budget:** Rust free-space calculator, every input tagged with provenance.
- **RF Optimizer:** heuristic diagnostics, constraints and pinned before/after.
- **Range Extenders:** existing BSS cache plus multi-node registration/guidance.
- **Hardware:** runtime identity/firmware information and source-scope limitations.
- **Diagnostics:** query failures, missing fields and parser failures.
- **Recordings:** browser session export/replay and CLI recording instructions.

## Coverage and recording

Measurements are never assumed to have positions. An unchecked position form
creates an **unlocated signal history**. To record a location, provide reference
frame, label, X/Y metres and position source; enable it only while at that point.
No geographic coordinate system, floor-plan dimensions, antenna height or third
spatial dimension is inferred. The dashboard groups by device/interface/peer,
channel and coordinate frame; squares indicate **measured points only**. One
reading gives one point, not a spatial RF propagation estimate. There is no
interpolated/predicted coverage heatmap, image floor-plan importer or 3D map in
this release. Unmeasured space is blank.

Core statistics report arithmetic mean dBm, population standard deviation in dB,
count, age and `coverage_confidence=not_quantified`. Mean dBm is a descriptive
log-domain statistic, not mean linear RF power. RSSI is uncalibrated. Repeated
samples of the same last-received driver value may not be independent.
Future observation times generate a clock warning rather than negative age.

```sh
./target/release/rfv record "$HOME/.rfv-run/core.sock" wlan0 desk.rfv 30 1000 \
  floor-ground desk 2.0 3.0 tape-measure
# Move manually; do not keep recording the old position during movement.
./target/release/rfv record "$HOME/.rfv-run/core.sock" wlan0 hall.rfv 30 1000 \
  floor-ground hall 7.0 3.0 tape-measure
./target/release/rfv replay desk.rfv
./target/release/rfv coverage desk.rfv hall.rfv
```

CLI recordings are new mode-0600 files; overwrite is refused. Limits: 10,000
batches, 64 MiB/file (64 MiB combined for comparison), 1 second minimum delay.
Use a private directory. Failure may leave a partial recording: completed frames
are retained; a truncated final frame is rejected, not silently accepted.
Replay preserves original observation times and sources and revalidates records.
Coverage rejects explicit simulation and nonmonotonic per-link observations.
Pass recordings in chronological order. No interpolation across missing samples
or unobserved intervals is performed. Large exports/captures belong outside Git.

Browser JSON (`RFV-BROWSER-1`) and binary CLI files (`RFVREC1`) are different
formats; each has its own replay entry point. Browser import is size/schema
checked and marked REPLAY, not a cryptographically verified hardware capture.
Neither format authenticates the producer. Stop live sampling before changing
between real measurements and imported analysis datasets.

## Link budget

```sh
./target/release/rfv budget inputs.tsv
```

TSV rows: `name<TAB>value<TAB>basis<TAB>source`. Blank lines and `#` comments are
allowed. Units are fixed:

| Name | Unit | Required |
|---|---|---|
| tx_power | dBm | yes, conducted configured/measured power, not assumed EIRP |
| tx_gain, rx_gain | dBi | yes |
| tx_loss, rx_loss | dB | yes, cable/connector losses, nonnegative |
| distance | m | yes, positive |
| frequency | MHz | yes, positive |
| fade_reserve | dB | yes, nonnegative planning allowance |
| sensitivity | dBm | optional, applicable MCS/width/PER specification |

Basis: `measured`, `configured`, `manufacturer`, `estimated`. Sources must be
nonempty. Unknown gain/loss is not silently replaced with zero. The UI starts
blank intentionally; entered zeros must still have provenance.

`FSPL = 20 log10(4π d f / c)`, `c = 299792458 m/s`.
`EIRP = Ptx + Gtx − Ltx`.
`Predicted Prx = EIRP + Grx − Lrx − FSPL`.
`Link margin = Prx − sensitivity`.
`Margin after fade reserve = link margin − chosen fade_reserve`.
The unreserved link margin is the theoretical tolerable fade before reaching
that sensitivity under this model, not a measured fading distribution.

No wall/terrain/polarization/mismatch losses are estimated. Far-field applicability
requires physical antenna knowledge the calculator does not have; negative
free-space loss is rejected but that alone is not a complete far-field check.
Outputs remain `kind=prediction`, never `measurement` or a transmit authorization.
No numeric statistical uncertainty is claimed without supplied uncertainty data.

## RF Link Optimizer

Implemented rules (heuristics, not universal thresholds):

- RSSI below −70 dBm: compare placement/orientation; conditional band suggestion.
- ≥10 dB change between matched observations: investigate variation; not proof of
  long-term instability or interference.
- Retry delta / TX packet delta >0.3 with at least 10 TX packets: excessive retry
  attempts; **not** a packet-loss probability and may exceed 1.
- Positive TX failure delta: report driver failures; not end-to-end packet loss.
- Matched in-use survey busy/active deltas >70%: possible congestion; includes
  own traffic, not proof of external interference.

Counter reset, identity/peer/channel change, non-forward clock, >5 minute gap,
missing denominators or impossible busy>active suppress interval inference.
The service serializes sample processing and retains at most 16 previous
interface batches. Placement, channel/band/width changes are manual trials.
No best-channel ranking from cached BSS RSSI, roaming command, packet-loss probe,
Internet speed test or automatic AP change is implemented.

`configured_tx_power` is the setting returned by `iw dev … info`, **not measured
radiated power**. `power_limit` retains effective per-channel `iw phy` advertised
ceilings, disabled flags and raw constraints (including restrictions such as
no-IR/radar). Minimum/settable range remains unknown. Actual permitted operation
also depends on current regulatory, board, firmware, SAR and antenna constraints.
The code never calls a power/channel setter or bypasses those constraints.

## Range infrastructure

BSS records come from `iw dev IFACE scan dump`, which reads the kernel cache and
**does not trigger a scan**. Signal/frequency/SSID/BSSID and optional last-seen age
are parsed. Missing cache, stale entries and insufficient permissions remain
explicit limitations. This is not automatic repeater/mesh role detection.

Register up to 128 devices with `rfv range nodes.tsv` or the dashboard. TSV:
`id<TAB>role<TAB>backhaul<TAB>compatibility<TAB>source`.
Roles: `repeater`, `bridge`, `mesh`, `ethernet_ap`, `external_radio`,
`measurement_node`. Backhaul: `unknown`, `ethernet`, `wireless_shared`,
`wireless_dedicated`. Compatibility: `unknown`, `confirmed`, `incompatible`.
Confirmed is **user-supplied evidence**, not RFVision certification. IDs must be
unique. The UI can export/import the registry; nothing is persisted implicitly.

Guidance covers airtime on shared-channel repeaters, both-hop placement,
4-address/WDS bridge requirements, vendor-specific versus standardized mesh,
wired-backhaul/channel/address planning, and driver/regulatory/security checks
for external radios. No vendor administration protocol or equipment reconfiguration
is implemented. A single radio is never represented as new physical infrastructure.
