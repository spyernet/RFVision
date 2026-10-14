# Writing a driver adapter

An adapter is a standalone process in `src/drv/<driver>/`, not a plug-in linked
into Core. Do not copy the driver's implementation, structs or firmware parsing
code into proprietary components. No kernel modifications belong in the
proprietary Cargo workspace.

1. **Audit first:** pin source/firmware provenance, licences, public interfaces
   and evidence. Document unanswered physical/firmware/legal questions.
2. **Identify accurately:** active driver plus PCI/USB IDs, optional accessible
   firmware version; separate unrecognized from incapable hardware.
3. **Prefer public interfaces:** sysfs/nl80211/ethtool and read-only behavior.
   Do not globally traverse debugfs or expose arbitrary register access.
4. **Implement RFV/1.0:** HELLO, DISCOVER, SAMPLE; exact framing and bounds in
   IPC_SPECIFICATION.md. A language-independent example is provided. The
   service forwards to any configured compatible endpoint without knowing the
   adapter's vendor. Multiple simultaneous adapter endpoints need future routing.
5. **Report evidence:** device records, capability records, then measurements
   with identity, source, units, quality and timestamp semantics. Unsupported
   and missing features need reasons. Never use a fabricated matrix or zero as
   a missing measurement. Synthetic input must retain simulation labeling.
6. **Isolate lifecycle:** rescan at request boundaries; discard samples when the
   device identity changes. Do not reuse measurements after disconnect/rebind.
7. **Bound resource use:** no unbounded RX queue, process output, record count or
   worker creation. Prefer event-driven acquisition when appropriate.
8. **Test in layers:** parser fixtures, independent-language IPC, absent hardware,
   disconnect/reconnect, permissions, unsupported fields, then actual hardware.
9. **Release gates:** complete dependency/notice inventory, legal review of
   interfaces and distribution, measured performance/connectivity regression
   tests, rollback verification. Dynamic loading/IPC are not legal safe harbors.

## rtw88 implementation

`src/drv/rtw88/main.rs` owns the explicit module aliases and RTL8821CE identity
labels. The independent HAL owns generic public-query parsing. Driver source
under `drv/rtw88/` is neither linked nor executed. `DISCOVER` offers conditional
capabilities; `SAMPLE` performs passive queries. All register accesses, debugfs
metrics and raw CSI extraction remain unimplemented.

For a future kernel hook, create a separately licensed integration artifact only
after reviewing kernel/driver and firmware obligations. Preserve original notices,
provide required source/build materials under the appropriate licence, and do
not place modified GPL code under the RFVision proprietary notice. The present
userspace-only package tool intentionally has no kernel-integration path.
