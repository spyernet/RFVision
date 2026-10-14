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

## Dashboard and recordings

The opt-in `rfv ui` listener binds 0.0.0.0 and serves public static HTML, but
requires a 32..256-character generated token for all APIs. Use a private network
or HTTPS reverse proxy and firewall; do not expose raw HTTP to untrusted networks.
The browser keeps the token in memory only, sends it in Authorization headers and
never needs another localhost service. No CORS allowlist exemption is enabled.
No token is logged or shipped in HTML. Inline script/style CSP is used for the
embedded page; data are rendered with textContent, not HTML concatenation.

The research gateway is single-threaded, has per-read/write timeouts rather than
absolute deadlines, and is not DoS hardened. Put rate limits/absolute deadlines
at a trusted proxy for remote use. Same-user processes/root can already access
the private Unix sockets and process environment; this is not a multi-tenant
security boundary. Body/header/import/capture sizes and history counts are bounded.
No arbitrary file reads, shell strings, active scans, txpower/channel setters or
vendor management endpoints are exposed. Imported provenance is not authenticated.

Recordings reveal identifiers, network relationships and positions. CLI new files
use mode 0600 and refuse overwrite; browser downloads follow browser/OS download
permissions and are NOT guaranteed mode 0600. Store both in private locations.
Browser and CSI imports are labelled replay/unverified or simulation; never use
an imported label as cryptographic proof of hardware origin. Raw CSI input must
come from documented authorized captures. Ignore `.rfv` recordings in Git and
keep browser JSON/CSI TSV captures outside the repository.
