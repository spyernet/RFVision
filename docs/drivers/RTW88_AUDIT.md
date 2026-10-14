# Supplied rtw88 source audit

Research date: 2026-10-14. Static inspection, **not hardware validation**.

## Provenance

The original session checkout contained only README. The supplied folder was
then available in repository commit
`f7c9b55132c6387b357f8bca18c54938ff33bc8f`; that commit was fast-forwarded onto the
existing session branch. Input: `drv/rtw88/`, 98 files, Git tree
`b0ffbf222400f28c13ba9bd30e3c6399daaf28f1`.

All 98 file Git blob IDs matched the corresponding directory at upstream Linux
commit [`4056b3c9d3c92ea45955641dbafeb9b960e146ef`](https://github.com/torvalds/linux/tree/4056b3c9d3c92ea45955641dbafeb9b960e146ef/drivers/net/wireless/realtek/rtw88).
This is an exact reproducible source reference. It does not identify which
release/package supplied the user's installed driver, nor prove an original
archive download URL. No standalone version or archive provenance was supplied.
Do not substitute the running sandbox kernel version for this source revision.

A preliminary `lwfinger/rtw88` reference was fetched while the folder was absent.
It is ignored development material, not the audited input or a build dependency.
The source manifest records per-file SHA-256 and declared licences; run
`python3 tools/audit_source.py --check` to detect input changes. No supplied file
was modified, compiled, installed or copied into proprietary implementation.

## Licensing findings

97 files declare `GPL-2.0 OR BSD-3-Clause`. `Kconfig` declares `GPL-2.0-only`.
Several module declarations say `Dual BSD/GPL`; these do not replace the need
for per-file, linked-kernel, firmware and distribution analysis. No blanket
proprietary relicensing is permitted. No complete kernel licence/notice package
or firmware binary accompanies this extracted subtree. See LICENSING.md for
release blockers. The dual-licence declarations are not used as permission to
copy driver implementation into Core.

## Receive path evidence

All references below refer to the supplied, manifest-pinned files:

| Area | Evidence and conclusion |
|---|---|
| PCI identity | `rtw8821ce.c:11–21`: Realtek IDs B821 and C821; `rtw8821c_hw_spec` |
| Physical/source expectations | `rtw8821c.c:1980+`: 2G/5G, HT/VHT, descriptor sizes, firmware filename. Channel code supports 20/40/80 MHz cases; actual capabilities can be firmware/efuse constrained |
| Runtime RF paths | `main.c:1891+`: RF type/paths read from hardware; `main.c:1980+`: firmware feature report / NSS limits. Do not turn source constants into measured antenna counts |
| PCI receive | `pci.c:1071+`: calls generic RX descriptor parser, removes descriptor, driver-info and shift bytes before normal stack processing |
| Descriptor | `rx.h`: six 32-bit little-endian words / 24 bytes; length, CRC/ICV, PHY-present, driver-info length, rate, bandwidth and low TSF |
| Layout | `rx.c:275–331`: bounds/descriptor checks; PHY begins after descriptor + shift; frame begins after PHY/driver-info; driver-info size is multiplied by eight |
| PHY pages | `rtw8821c.c:615–684`: page 0 CCK power conversion; page 1 PWDB_A, width and scalar signal. No subcarrier-indexed complex matrix |
| Stack exposure | `rx.c:205–258`: mac80211 frequency, rate/MCS/NSS, signal, chain signal, width, CRC flag and low TSF are populated; signal absence flagged |
| Extra fields | `rtw8821c.h:202–214`: EVM, CFO-tail, SNR macros exist; the inspected RTL8821C parser does not consume them |
| Debug information | `debug.c:863+`: generic EVM/CFO/SNR output; existence does not prove RTL8821C values were populated or valid |
| Firmware | `fw.h:51+`, `fw.c` C2H processing: no raw channel-matrix export identified in inspected paths; firmware internals not supplied |
| Beamforming | `bf.c`: feedback configuration/rate controls; not a userspace CSI export |

## Accessible Linux interfaces

- sysfs netdev metadata/statistics: least-privilege baseline, implemented.
- nl80211 through existing `iw`: station and interface state, optional survey and
  PHY advertisement; parser-based v0.1 implementation. Actual field population
  varies by driver, firmware, association, kernel and privilege.
- ethtool driver info: optional version reporting, implemented through an
  independently installed executable.
- mac80211 monitor/radiotap: potential per-packet metadata acquisition; **not
  implemented** and never enabled automatically because interface combinations
  and monitor operation can affect normal networking.
- debugfs: private, unstable and can have side effects; no automatic traversal,
  register reads/writes, or generic debug export is implemented.

## Outstanding evidence

No physical adapter is exposed in this sandbox (`/sys/class/net`: eth0, lo).
No installed device firmware version, antenna count, real station measurement,
RF accuracy, connectivity preservation under load, or CSI samples were verified.
The provided driver source is not proof of these runtime properties. An exact
hardware/firmware/kernel baseline and hardware test plan remain required.
