# Security and radio safety

## Threat model

RFVision v0.1 is a local single-user diagnostic tool. Protect against malformed
IPC frames, accidental unsafe installation, unexpected/disconnected devices,
unbounded helper output, and misleading measurement provenance. A same-UID
malicious process or root can control files/IPC and is not isolated by filesystem
permissions. No multi-tenant or remote service deployment is supported.

## Implemented controls

- Original Rust workspace forbids unsafe code; no third-party crates, kernel
  linking, firmware execution or arbitrary commands from IPC.
- Private owned directories, mode-0600 sockets, endpoint ownership/type checks
  in Rust clients, refusal to overwrite any existing socket path.
- Version and opcode negotiation, request correlation, allocation/count/length
  limits, duplicate/UTF-8 checks, bounded active sessions and frame deadlines.
- Interface validation and actual-device lookup; no shell interpolation.
- Root-owned fixed-path helper executables, sanitized environment, read-only
  argument vectors, two-second execution limit, bounded stdout and stderr.
- Disconnection/rebind checks discard acquisition batches; no stale-value cache.
- Missing data are not zeros. Diagnostics have bounded text. CSI is not faked
  from RSSI. Synthetic tests never establish hardware capability.
- Explicit private-prefix installation, allowlisted rollback and no driver,
  firmware, privilege, network, firewall or system-service configuration edits.

## Limits requiring deployment review

- Filesystem access is authentication; `SO_PEERCRED`/group-role authorization is
  deferred. Private parent checks do not protect against a malicious same UID.
- Unix `connect` has no independent timeout; writes use per-write timeouts.
  Eight slots bound concurrent sessions but same-UID resource exhaustion remains
  possible. No systemd cgroup or CPU/memory/seccomp policy is installed.
- SIGTERM/Ctrl-C may leave a stale socket. Refusal to automatically remove it is
  deliberate; confirm process termination before manual unlink.
- Helper text output is not a stable API; strict parsing can omit unfamiliar
  formats. Helper availability/permissions and version should be audited on the
  target distro. Normal device firmware behavior is not attested by RFVision.
- No cryptographic adapter identity, signed manifest or authenticated binary
  installer is implemented. Use approved builds and trusted local endpoints.
- The Python example is a protocol illustration; unlike the Rust client it does
  not precheck socket ownership/mode. Use only a known private endpoint.
- Device paths, MAC addresses, channel state, firmware and diagnostic metadata
  are potentially sensitive. Reports are printed locally, not uploaded. Secure
  saved reports and redact identifiers before sharing.
- No clinical/medical use, occupancy/person identification claim or imaging
  capability is supported. Statistical/measurement accuracy is not validated.

## Regulatory boundary

Acquisition performs only passive public status queries. It does not bypass
regulatory restrictions, change channels, raise transmit power, create monitor
interfaces, inject packets or stop normal Wi-Fi. Existing networking may continue
ordinary transmissions independently. Radio behavior, regulatory compliance and
safe future experiments still require hardware/firmware review.

## Vulnerability handling

No public security contact has been configured. Project owners should establish
a private reporting channel and signed release process before distribution.
Do not post credentials, personal packet captures or exploitable environment
secrets in a public issue. Distribution is blocked pending the legal and hardware
release gates, not merely successful software tests.
