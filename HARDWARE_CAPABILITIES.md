# Hardware capabilities — evidence, not marketing

## RTL8821CE source assessment

Input: exact supplied/upstream source identified in the audit. Board, installed
firmware and live RF behavior have **not** been measured here.

| Property | Source evidence | Runtime verification |
|---|---|---|
| PCI identity | Realtek 10ec:b821 / 10ec:c821 | IDs matched only when read from sysfs |
| Bands | 2.4 and 5 GHz in chipset specification | Retain driver PHY advertisement; actual permitted channels depend on device/regulatory state |
| PHY | HT/VHT, source channel paths for 20/40/80 MHz | Current width only if explicitly returned by iw |
| RF paths/NSS | RTL8821C parser handles path A; driver reads hardware type and firmware feature limits | Actual board antenna/RX/TX chain counts unknown; not hardcoded from product name |
| Firmware | `rtw88/rtw8821c_fw.bin` is required by source | Version only if accessible through ethtool; binary not supplied |

## Feature acquisition matrix

Status is static research assessment unless explicitly described as observed.
A report's `implementation` field distinguishes planned integration from working
code. Measurement availability is per device, session and source.

| Feature | Research classification | v0.1 acquisition / limitation |
|---|---|---|
| Netdev bytes/packets/errors/drops | AVAILABLE_WITH_ADAPTER → AVAILABLE on read | sysfs; cumulative network counters, not all PHY errors |
| RSSI / averaged signal | AVAILABLE_WITH_ADAPTER → AVAILABLE on read | iw station; association and field presence required |
| Per-chain signal | AVAILABLE_WITH_ADAPTER → AVAILABLE on read | iw-reported list indices, physical chain mapping unverified |
| Calibrated received power | UNVERIFIED | RSSI in dBm is not an instrument calibration |
| Station retry/failure/drop counts | AVAILABLE_WITH_ADAPTER → AVAILABLE on read | fields only if returned |
| Channel frequency/width | AVAILABLE_WITH_ADAPTER → AVAILABLE on read | current interface context, not per-packet context |
| MCS / spatial-stream information | AVAILABLE_WITH_ADAPTER → AVAILABLE on read | explicitly labeled RX/TX rate fields; no inferred NSS |
| Antenna/RX/TX chain capabilities | UNVERIFIED physical validation | bounded raw iw PHY advertisement only; no normalized physical registry yet |
| Noise/channel timing | UNVERIFIED → AVAILABLE on read | optional survey parser; no guarantee rtw88 supplies survey |
| SNR | REQUIRES_DRIVER_MODIFICATION for inspected RTL8821C PHY fields | no scalar extraction or mismatched RSSI-minus-noise approximation |
| EVM / CFO | REQUIRES_DRIVER_MODIFICATION for inspected RTL8821C fields | header definitions are unused in inspected parser; units/validity pending |
| Per-packet PHY / receive timestamps | AVAILABLE_WITH_ADAPTER, **not implemented** | mac80211 metadata suggests radiotap route; monitor integration not enabled |
| Hardware synchronization | UNVERIFIED | low TSF is not global synchronization; host observation times are distinct |
| CSI, amplitude, phase, complex bins | UNVERIFIED | no verified matrix export; live implementation absent |
| Frequency/impulse response | UNVERIFIED | no valid source measurements; no reconstructed output |

`UNSUPPORTED` is reserved for demonstrated lack of support in a defined scope.
The current adapter rejects non-matching drivers as unsupported **by this RFVision
release**, not incapable silicon. `REQUIRES_FIRMWARE_SUPPORT` and
`REQUIRES_HARDWARE_SUPPORT` exist in the registry but are not speculative labels
for RTL8821CE CSI.

## Other adapters and family members

The rtw88 process recognizes an explicit set of in-tree/out-of-tree module-name
aliases and uses generic public-interface queries. Only RTL8821CE has the
chipset-specific research assessment above; scalar EVM/CFO/SNR requirements
are not propagated as verified facts to other Realtek chips. Driver name alone
never establishes a model, chain count or CSI capability.

Intel iwlwifi, Qualcomm ath9k/ath10k/ath11k, MediaTek mt76 and other consumer
adapters are future research targets. They can be inventoried through sysfs,
but are not supported by the current rtw88 adapter. No automatic driver swap
is proposed for them.

## Required hardware evidence before release claims

Record PCI/USB IDs, board revision if accessible, kernel/module/firmware
versions, regulatory context, operating mode, association, bands/channel width
advertised by the device, and baseline connectivity. Run the non-invasive
hardware procedure under `tests/hardware/README.md`. Preserve raw diagnostic
outputs with privacy controls. Compare to independent instruments for RF
accuracy; software test success cannot substitute for this.
