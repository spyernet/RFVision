# RFV/1.0 IPC specification

**Status: implemented.** Independent of Linux driver layouts and Rust ABI.
Local `AF_UNIX/SOCK_STREAM`; no TCP listener, kernel pointer, ioctl structure or
language-native serialized object. An independent example is in
`examples/ipc_client.py` and is used for interoperability tests.

## Header (24 bytes)

All integers are unsigned, big-endian. Exactly these offsets:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | ASCII `RFV1` |
| 4 | 2 | major = 1 |
| 6 | 2 | minor = 0 |
| 8 | 2 | opcode: HELLO=1, DISCOVER=2, SAMPLE=3, ERROR=255 |
| 10 | 2 | flags: request=0, response=1; other bits rejected |
| 12 | 8 | caller request ID, echoed in response |
| 20 | 4 | payload byte length, inclusive of record count, 4..1,048,576 |

Payload: `u32 record_count`; for each record: `u16 field_count`; for each
field: `u16 key_byte_length`, `u32 value_byte_length`, UTF-8 key bytes, UTF-8
value bytes. Record count ≤4,096; fields ≤64; keys 1..64 bytes; values 0..65,536
bytes. No NUL terminator. Duplicate keys, invalid UTF-8, truncated data, surplus
payload bytes, unsupported versions and limits are rejected. Unknown record
fields should be retained/ignored by clients; they are not operation arguments.
Key order has no semantic meaning (the Rust encoder sorts keys).

**All values are strings.** Booleans use `true`/`false`; numeric measurement
strings are finite decimal numbers. Counters are unsigned decimal integers;
keep them as integers, not IEEE-754 doubles. `observed_unix_ns` is unsigned
integer decimal. JSON CLI output retains these representations. This modest
TLV mapping favors an independently testable, dependency-free control plane;
it is not intended as an efficient complex-matrix encoding.

## Negotiation/session

1. Client sends HELLO with exactly one record containing `protocol=RFV/1.0`.
2. Server replies with protocol, role, supported operations, transport and
   maximum frame size. Version mismatch closes the connection; a malformed
   handshake gets ERROR if it is otherwise a valid frame.
3. Client sends exactly one DISCOVER or SAMPLE with a different request ID.
4. Server returns a correlated response or ERROR and closes the connection.

Only version 1.0 is accepted. No silent downgrade or compatible-minor assumption.
Clients must verify flags, opcode and ID. Eight active sessions per server;
excess connections close, requiring explicit caller retry. No durable session
state, auto-retry storm or hidden measurement polling.

### DISCOVER

No records allowed. Returns `adapter`, `device`, `capability`, `diagnostic`
records; Core adds a `processing` record. No RF samples are implied. The adapter
limits each discovery report to 16 interfaces to bound capability expansion.

### SAMPLE

Exactly one record, exactly one field: `interface=<visible netdev name>`.
Adapter validates name and current driver eligibility, then returns device and
capability records, measurement records, diagnostics, optional driver/PHY info
and sample metadata. Empty/unavailable RF values are not errors in themselves:
partial counter-only batches may still be useful. Disappearance during the
query is an acquisition error and the batch is discarded.

### ERROR

One `kind=error`, `code`, `message` record. Codes include `BAD_REQUEST`,
`NEGOTIATION_REQUIRED`, `ACQUISITION_FAILED`. Acquisition messages identify
`DEVICE_NOT_FOUND`, `DEVICE_CHANGED`, `UNSUPPORTED_DRIVER` or
`ADAPTER_UNAVAILABLE`; these subcodes currently live in human-readable messages,
not a stable nested machine-code field. CLI failures produce stderr JSON and
exit 1. Wizard validation exit 2 means incomplete/failed passive checks.

See [HAL](HAL_SPECIFICATION.md) for record fields, units and limitations.

## Permissions and resource limits

Private immediate parent directory, owned by the current process user with
no group/other permission bits (normally 0700), not a symlink. Socket mode 0600,
absolute path ≤100 bytes. Rust clients check the endpoint's type/owner/mode.
All clients with that Unix user identity are trusted; root is outside this
boundary. No `SO_PEERCRED` policy or multi-user group sharing is implemented.
Do not use setuid binaries or run a root service for untrusted users.

Three-second absolute frame read deadlines on the server; three-second HELLO
response deadline and twenty-second operation-response deadline on the client.
Writes have a three-second per-write socket timeout (not a total transaction
deadline). Local `connect` uses the OS's blocking Unix-socket semantics; it is
not independently deadline-controlled. No TLS; privacy is local permissions.
Signals may leave stale sockets; operators clean them only after stopping the
owning process. Servers never remove an existing path to make binding succeed.

## Future high-rate extension (not implemented)

Control plane would negotiate an additional version/feature for a sealed,
size-bounded shared-memory ring passed via `SCM_RIGHTS`, with read-only consumers,
release/acquire sequence counters, explicit discontinuity/overflow records,
producer epoch and disconnect cleanup. CSI blocks would specify byte order,
complex encoding, dimensions, subcarrier map, timestamps and calibration.
No reader should block the Wi-Fi RX path. Authenticate descriptor ownership,
reject untrusted lengths, benchmark against batched socket I/O, and document
loss accounting before claiming zero-copy/high-rate support. Current v1 rejects
unknown opcodes; it never attempts to interpret proposed ring descriptors.
