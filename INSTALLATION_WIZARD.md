# Safe assessment, installation, validation and rollback

## Absolute safety policy

No default or opt-in driver replacement exists in v0.1. No command runs DKMS,
modprobe/rmmod, package installation, kernel build, firmware upload, network
reconfiguration, regdomain change, active scan or transmit-power adjustment.
The wizard does not stop NetworkManager/wpa_supplicant or create monitor devices.

## 1. Discovery and assessment

`rfv discover` reports wireless interfaces visible in sysfs plus host kernel.
`rfv wizard assess [CORE_SOCKET]` combines local inventory with adapter reporting
when a running service is supplied. Supported/unsupported driver decisions are
owned by the adapter, not embedded in the wizard. Without a service, assessment
explicitly does not assert adapter availability.

Device records include accessible PCI/USB identity, active driver/module,
optional module version and state. A `sample` adds accessible firmware/driver
versions, current channel, metrics and raw driver-advertised PHY capabilities.
Not every required field is universally exposed; missing values stay absent.
Physical RF chains/antenna counts/bands are not declared verified from driver
names. The source expectation and runtime evidence remain distinct.

## 2. Plan

`rfv wizard plan PREFIX BUILT_BINARY_DIRECTORY` has no side effects. It describes
only a new private userspace installation, with explicit limitations. Obtain
approved binaries first; the wizard is not a downloader or a signature verifier.

## 3. Install

`rfv wizard install PREFIX BUILT_BINARY_DIRECTORY --apply`

- Explicit `--apply` is mandatory. It is never inferred from assessment.
- Prefix must be absolute and absent. Its immediate parent must already be a
  private non-symlink directory owned by the current user (mode 0700).
- Sources: regular executable files `rfv`, `rfvd`, `rfv-adapter-rtw88`, each ≤64
  MiB, not symlinks. All are read before any destination is created.
- Creates prefix mode 0700, copies files with exclusive creation/mode 0700,
  syncs files, then writes a mode-0600 ownership manifest.
- No execution, auto-start, privilege escalation, autostart registration,
  modification of existing drivers or overwrite of existing installations.
- Normal I/O failure triggers best-effort cleanup of only files just created.
  An abrupt crash can leave a partial prefix; automatic crash recovery and
  directory durability guarantees are not implemented. Inspect manually rather
  than recursively deleting an unknown tree.

Use a separate private runtime directory to start the two server processes;
see README. Installed files are not an assurance of IPC or hardware readiness.
Installer records explicitly say `running=false`.

## 4. Validate

`rfv wizard validate CORE_SOCKET`

- Snapshots wireless identity/driver/carrier/operstate before and after.
- Verifies service/adapter IPC via DISCOVER.
- Requests a sample for each adapter-supported visible interface.
- Reports measurements and unsupported-feature reasons.
- Checks that observed interface/driver/state did not change.

Exit 0 / `PASS_PASSIVE_CHECKS` requires visible hardware, a measurement on at
least one supported interface, no failed sample, and unchanged observed state.
Exit 2 / `INCOMPLETE_OR_FAILED` means no hardware, no useful acquisition, a failed
sample or changed state. Exit 1 means operational/IPC failure.

**This does not prove Internet connectivity, RF accuracy, RF measurement
availability beyond the returned values, or lack of every transient disruption.**
A counter-only batch can pass passive checks; that is not an RSSI/CSI validation.
Connectivity and throughput regression tests on actual hardware remain a manual
release gate. No ping or synthetic transmission is sent automatically.

## 5. Rollback

Stop running RFVision processes first. Then:
`rfv wizard rollback PREFIX --apply`

Validates private directory ownership, an exact allowlist of installed names,
regular non-symlink binaries and matching manifest checksums before deletion.
It refuses additional/missing/modified files, does not follow manifest paths and
never recursively deletes the tree. FNV-1a detects accidental edits; it is **not
cryptographic authentication**. A same-UID attacker can modify the manifest and
is outside this local trust boundary. No existing driver/firmware ever needs to
be restored because none was changed.

Rollback is not transactionally atomic against a crash halfway through removal.
Running processes are not killed automatically, and stale runtime sockets are
not part of the installation allowlist. Documented explicit cleanup prevents
unintentionally deleting a live endpoint.
