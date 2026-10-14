# RFVCSI/1 — offline, driver-independent complex data

This format is an **import/record/replay contract**, not a new live CSI provider
or an undocumented RTL8821CE capture recipe. Existing genuine captures require
a reviewed converter from their documented native format. No third-party CSI
driver/firmware is bundled. Do not relabel algorithm fixtures as hardware data.

Run `rfv csi analyze CAPTURE.tsv` or import on the CSI Research page. A saved
source file can be reopened/replayed; there is no live CSI recording on RTL8821CE.
CLI analysis is JSON records. Browser charts select a recorded frame and RX/TX
stream; axes are actual subcarrier indices, not invented physical positions.

The first line is literally `RFVCSI<TAB>1`. Subsequent metadata rows have two
fields separated by a tab, before the first frame:

| Key | Required content |
|---|---|
| device_id | originating hardware identifier, or explicit algorithm fixture ID |
| driver | source driver identifier/version or `none` for tests |
| source | capture/conversion provenance; nonempty |
| environment | exactly `hardware_import` or `simulation` |
| center_frequency_hz | positive integer |
| bandwidth_hz | positive integer |
| rx_chains, tx_streams | integers 1..8 |
| scale | positive finite multiplier applied to IQ amplitude |
| phase_calibration | optional reference/method/date; metadata alone applies no correction |

A frame starts with `frame<TAB>observed_unix_ns` (positive u128). Observation times
must strictly increase. These are imported **host observation times**, not device
receive times or synchronized clocks. Rate is `(N−1)/(last−first)` only for ≥2
frames, a mean observed frame rate, not uniform RF sampling.

IQ rows: `iq<TAB>subcarrier_index<TAB>rx_index<TAB>tx_index<TAB>real<TAB>imaginary`.
Bins are signed i16; RX/TX indices start at zero. Ordering is increasing bin,
then RX, then TX. Every bin must contain each RX/TX combination exactly once.
Duplicate/missing/unsorted combinations, nonfinite numbers, overflowed amplitude,
invalid scales, changing mappings and empty frames are rejected. Missing bins
must not be filled with zeros by a converter. A real zero IQ has no valid phase.
No DC/pilot/null mapping is guessed from array length.

Limits: 4 MiB text, 128 frames, 4096 IQ values/frame, 16,384 IQ values/capture.
They intentionally bound parsing, analysis and browser response size. This is an
offline research interchange, not the future high-rate IPC streaming schema.
Unknown/duplicate metadata is rejected. A hardware-import tag is an unverified
producer assertion; analysis reports `hardware_import_unverified`. No automatic
capability promotion or physical validation follows from importing a file.

## Implemented analysis

- Complex IQ validation; amplitude `hypot(re, im) × scale`; phase `atan2(im,re)`.
  Amplitude unit is scaled IQ, not dBm, voltage or calibrated channel gain.
- Phase unwrap and least-squares constant/linear removal per stream and contiguous
  valid-bin segment. Gaps/zero IQ break segments; ≥3 bins are needed for residuals.
  Output is `detrended_phase_rad`, never calibrated propagation phase. This
  removes legitimate channel slope as well as some nuisance offsets.
- `csi::correct_phase` accepts an external per-bin phase-offset calibration and
  requires a reference identifier. Automatic RF calibration/offset discovery is
  not provided. Merely attaching a calibration string does not apply this API.
- Per-bin temporal amplitude mean, population variance and EMA (α=0.2 per frame,
  not a time-normalized filter). Static/dynamic amplitude proxies are useful but
  not coherent static/dynamic complex-channel separation.
- Successive-difference squared amplitude /2 noise proxy. Interpreting it as
  receiver noise requires a stationary channel and independent noise; motion and
  AGC contaminate it. It is neither measured thermal noise nor SNR.
- Per-frame/stream amplitude coefficient of variation across frequency, a
  frequency-selectivity/multipath **proxy**, also affected by filtering and
  measurement artifacts. No number of paths, path delays or distances is inferred.
- Zero-IQ count in the first frame, provenance/format validation, frame/bin count,
  calibration state, simulation marking and observed mean frame rate.

Not implemented: hardware-specific CSI decoding/acquisition, IQ clipping detection
without native quantization metadata, coherent cross-radio synchronization,
CIR/IFFT with a verified full FFT mapping, delay-spread/multipath reconstruction,
noise isolation, calibrated time of flight, localization, body imaging, or true
uniform-rate high-speed CSI streaming. Never derive these from RSSI.
