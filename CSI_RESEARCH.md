# RTL8821C / RTL8821CE CSI research

## Conclusion

**CSI: UNVERIFIED. Live acquisition: NOT IMPLEMENTED.** Source inspection found
no documented route exporting raw per-subcarrier complex channel estimates
through the supplied RTL8821C RX path or examined C2H handlers. This is not proof
that the silicon cannot form channel estimates internally. It is also not proof
that a firmware patch is necessary or sufficient.

It is presently **not established** that CSI can be exposed without firmware
modification. The absence of a matrix format, length/index mapping, capture
samples and independently validated transport prevents an honest CSI claim.

## Why the apparent CSI evidence is insufficient

1. The 24-byte receive descriptor describes packet status and location, not
   per-subcarrier IQ. The chipset PHY parser consumes scalar receive power and
   bandwidth from pages 0/1. Header macros for EVM/CFO/SNR are not CSI arrays.
2. `rtw8821c_hw_spec.csi_buf_pg_num = 0` is a software configuration clue about
   reserved buffering; it is **not** a physical impossibility proof.
3. `cfg_csi_rate` and beamforming feedback registers configure feedback behavior.
   A CSI-named feedback parameter does not give RFVision raw uncompressed channel
   estimates or a calibrated phase reference. Compressed over-the-air feedback
   must not be mislabeled as raw receiver CSI.
4. Generic EVM/CFO/SNR debug output can be unpopulated for this chip. The examined
   RTL8821C parser never fills those quantities. A printed zero is not a valid
   measurement merely because a generic debug endpoint exists.
5. The firmware blob `rtw88/rtw8821c_fw.bin` is referenced but absent from the
   supplied source. Firmware capabilities, licensing and redistribution terms
   require separate investigation. No firmware extraction/patching was attempted.

Exact file/line evidence and source identity are in
[the source audit](docs/drivers/RTW88_AUDIT.md).

## Community/upstream comparison

- The full supplied directory matched the pinned
  [upstream rtw88 reference](https://github.com/torvalds/linux/tree/4056b3c9d3c92ea45955641dbafeb9b960e146ef/drivers/net/wireless/realtek/rtw88),
  rather than a supplied CSI-specific fork.
- [Nexmon CSI](https://github.com/seemoo-lab/nexmon_csi/tree/1232edf2cf1e7ae0561676c49e4dd4eb22c8a27e)
  documents specific Broadcom chips/firmwares and explicit complex-data formats.
  Those firmware-specific capabilities are not transferable evidence for
  RTL8821CE. No Nexmon code or configuration steps are included in RFVision.
- [Linux 802.11n CSI Tool](https://github.com/dhalperi/linux-80211n-csitool/tree/ae0db3aadf1b007e4de91c1528a5595bfb46fb5c)
  is a separate driver-modification research project. Its existence is not
  evidence that an unmodified Realtek firmware exports equivalent information.

This was a bounded source/repository review, not an exhaustive survey or a
vendor NDA/register-document audit. No definitive RTL8821C CSI capture register
specification or matching live capture was obtained. Register names alone do
not justify experimental reads/writes, and no such experiments were performed.

## What would need to change?

### Scalar PHY diagnostics (plausible research path, not implemented)

After legal review, a separately licensed driver-side instrument could validate
PHY page type/size and chipset applicability, decode the already-declared EVM,
CFO-tail and SNR fields, establish signedness/scaling/validity against reference
captures, and publish versioned passive records. Instrumentation must never
sleep/block in RX, must use bounded queues with loss accounting, and must preserve
normal packet delivery. This could expose scalar metrics, **not CSI**. Existing
private debug reads alone cannot fix the missing population of those fields.

### Actual CSI (requirements, not a known patch recipe)

1. Obtain legitimately accessible chipset/firmware documentation or validated
   captures showing where a genuine channel matrix exists and how it is read.
2. Establish dimensions, subcarrier/pilot/DC/null indexing, RX/TX stream mapping,
   IQ encoding/scaling, bandwidth, packet correlation, clock and truncation.
3. Determine whether existing firmware exports the data in RX driver-info,
   C2H, or another documented buffer. If yes, add a bounded extraction hook
   **before that data is discarded** in an independently licensed component.
   Do not invent an RX offset or an opcode without evidence.
4. If no such export exists, ask the vendor about legitimate firmware support;
   neither firmware feasibility nor permission has been established. Do not
   label this `REQUIRES_FIRMWARE_SUPPORT` as a proven conclusion yet.
5. Review kernel/firmware licensing and security boundaries before any patch.
   Prove connectivity, suspend/resume and throughput remain acceptable.
6. Validate captures against known channels/reference equipment; publish honest
   limitations, phase offsets, clock drift, AGC and packet-loss behavior.

## Normalization implemented now

`src/core/csi.rs` supplies an offline finite-IQ data contract, dimension and
subcarrier validation, and amplitude/mathematical-phase computation. It is not
wired to a device or IPC capture opcode. Tests use explicitly synthetic complex
numbers and are not RF measurements. RSSI is never input to this conversion.
CIR requires valid frequency-bin mapping and calibration/windowing assumptions;
no IFFT-based CIR, phase reconstruction, imaging or synchronization is claimed.
