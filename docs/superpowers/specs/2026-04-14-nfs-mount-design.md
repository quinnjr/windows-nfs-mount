# Windows NFS Mount — Design Specification

**Date:** 2026-04-14
**Status:** Draft

## Overview

A general-purpose NFSv4.1 client for Windows, written in Rust. Exposes NFS
mounts as native drive letters via WinFsp. Primary test target is AWS S3 Files
(NFS access to S3 directory buckets), but the client implements the full
NFSv4.1 protocol and works against any compliant server.

### Motivation

AWS S3 Files exposes S3 buckets as NFSv4.1 endpoints, but the tooling
(`amazon-efs-utils`, `efs-proxy`) is Linux-only. Windows' built-in NFS client
lacks NFSv4.1 session support, has no concept of IAM authentication, and
cannot perform the mandatory TLS upgrade that AWS requires. No existing Windows
NFS client fills this gap.

### Goals

- Full NFSv4.1 protocol implementation per RFC 8881
- WinFsp-based filesystem integration (drive letter mounts, full Win32 API
  compatibility)
- AWS IAM credential resolution and automatic refresh
- Mandatory TLS support (RFC 9289 RPC-over-TLS) for AWS; optional for general
  NFS servers
- CLI for interactive use, Windows service for persistent auto-mounts
- Enterprise-grade resilience: reconnection, state reclaim, credential rotation

### Non-Goals

- NFSv4.2 (future work, not in scope for v1)
- pNFS / parallel I/O (AWS S3 Files doesn't support it)
- Client-side delegation (AWS S3 Files doesn't support it)
- GUI (future work)
- macOS or Linux support (use native NFS clients on those platforms)

---

## Architecture

### Crate Structure

```
windows-nfs-mount/
├── Cargo.toml              (workspace root)
├── crates/
│   ├── xdr-codec/          (XDR serialization, zero dependencies)
│   ├── xdr-derive/         (proc-macro crate for XdrEncode/XdrDecode derives)
│   ├── onc-rpc/            (ONC RPC v2, depends on xdr-codec)
│   ├── nfs4-types/         (NFSv4.1 type definitions, depends on xdr-codec)
│   ├── nfs4-client/        (protocol client, depends on onc-rpc + nfs4-types)
│   ├── aws-creds/          (IAM credential resolution)
│   ├── nfs-mount/          (high-level mount logic, caching, lease mgmt)
│   ├── winfsp-bridge/      (WinFsp filesystem provider)
│   ├── cli/                (CLI binary, depends on nfs-mount + winfsp-bridge)
│   └── service/            (Windows service binary, depends on nfs-mount + winfsp-bridge)
```

### Dependency Flow

```
xdr-codec <- xdr-derive (proc-macro, re-exported by xdr-codec)
xdr-codec <- onc-rpc <- nfs4-client <- nfs-mount <- winfsp-bridge <- cli/service
              nfs4-types /                aws-creds /
```

Strictly one-directional. No circular dependencies.

### Key External Dependencies

| Crate | Dependency | Purpose |
|---|---|---|
| `nfs4-client` | `tokio` | Async runtime |
| `nfs4-client` | `rustls` | TLS (mandatory for AWS, optional otherwise) |
| `winfsp-bridge` | `winfsp-sys` | WinFsp FFI bindings |
| `cli` | `clap` | CLI argument parsing |
| `service` | `windows-service` | Windows Service Control Manager |
| `aws-creds` | `aws-config`, `aws-credential-types` | AWS SDK credential chain |
| `cli` | `ratatui`, `crossterm` | TUI configuration interface |
| `cli` | `toml_edit` | Lossless config file editing |

### Why Separate `nfs4-types` from `nfs4-client`

NFSv4.1 defines hundreds of types and operations. Isolating them lets the
codec and type definitions compile independently, enables code generation from
RFC XDR definitions, and keeps client logic focused on behavior rather than
data representation.

### Why `aws-creds` Is Its Own Crate

Keeps the AWS SDK dependency (which is large) isolated. General NFS users who
don't need AWS auth avoid pulling it in. Also enables independent testing of
credential resolution.

---

## Protocol Stack

### `xdr-codec` — XDR Serialization (RFC 4506)

Derive macro approach: `#[derive(XdrEncode, XdrDecode)]` on structs and enums.
The derive macros live in a separate `xdr-derive` proc-macro crate, re-exported
by `xdr-codec` so consumers only depend on one crate. Types map directly to XDR
primitives (int, uint, hyper, opaque, string, arrays, optionals, unions).
Operates on `bytes::Bytes` / `BytesMut` for zero-copy where possible. No async,
no I/O — pure data transformation.

### `onc-rpc` — RPC v2 Framing (RFC 5531)

Handles:
- Record marking (4-byte length-prefixed fragments)
- Call/reply message framing
- XID tracking and matching
- Auth flavor negotiation

Exposes an async `RpcTransport` trait:

```rust
trait RpcTransport {
    async fn call(&self, program: u32, version: u32,
                  procedure: u32, args: &impl XdrEncode,
                  auth: &AuthFlavor) -> Result<RpcReply>;
}
```

Two implementations: `TcpTransport` (plain TCP) and `TlsTransport` (rustls
over TCP). Auth flavors supported: `AUTH_NONE`, `AUTH_SYS`, and a pluggable
`AUTH_TLS` for the RFC 9289 RPC-over-TLS upgrade mechanism.

### `nfs4-types` — NFSv4.1 Data Types (RFC 8881)

All operation arguments, results, attributes, and error codes as hand-written
Rust types with XDR derive macros, closely mirroring the XDR definitions in
RFC 8881. Hand-written rather than auto-generated because Rust type ergonomics
(enums with data, newtypes, builder patterns) don't map 1:1 from raw XDR, and
the types benefit from idiomatic Rust naming and documentation. The `COMPOUND`
and `CB_COMPOUND` procedure envelopes live here. Pure data crate — no logic,
no I/O.

### `nfs4-client` — Protocol Client (RFC 8881)

The NFSv4.1 state machine:

- **Session management:** `EXCHANGE_ID` -> `CREATE_SESSION` ->
  `BIND_CONN_TO_SESSION`. Maintains session slots for exactly-once semantics.
  Tracks sequence IDs per slot.
- **Compound builder:** Fluent API for constructing COMPOUND operations:
  `compound().putrootfh().lookup("dir").getfh().getattr(attrs).build()`
- **Lease management:** Background task sends `SEQUENCE` keepalives before
  lease expiry. Handles `RECLAIM_COMPLETE` on reconnection.
- **Filehandle cache:** Maps path components to filehandles to avoid repeated
  LOOKUPs.
- **Reconnection:** Detects transport failures, re-establishes sessions,
  reclaims state (open files, locks).

**Error handling:** NFSv4.1 errors are rich and meaningful. The client translates
them to typed Rust errors that upper layers act on — retry on `DELAY`, re-lookup
on `STALE`, reclaim on `EXPIRED`. Transport errors are separate from protocol
errors.

---

## Filesystem Layer

### `nfs-mount` — High-Level Mount Logic

Sits between the NFS protocol client and the WinFsp bridge.

**Path resolution:** Translates Windows paths (`N:\folder\file.txt`) to NFS
paths (`/folder/file.txt`). NFS is case-sensitive, Windows apps may not be.
Strategy: pass through as-is, let the server decide. No client-side case
folding.

**Attribute caching:** Caches `GETATTR` results with configurable TTL (default
3s for directories, 1s for files). Cache invalidated on local writes. Entries
evicted on `NFS4ERR_STALE`. Critical for performance — Explorer generates
heavy metadata traffic.

**Read-ahead buffering:** Detects sequential read patterns and issues larger NFS
READs ahead of WinFsp requests. Configurable buffer size (default 4MB, matching
AWS's recommendation for 1MB+ I/O sizes).

**Write coalescing:** Buffers small writes and flushes as larger NFS WRITEs.
Flush on `CLOSE`, `FLUSH`, or buffer-full. Configurable write-back vs
write-through per mount.

**Directory caching:** Caches `READDIR` results for the TTL window. Explorer
aggressively re-reads directories, so this is important.

**Open file tracking:** Maps Windows file handles to NFS stateids. Handles
`OPEN`/`CLOSE` lifecycle and tracks lock state per handle.

### `winfsp-bridge` — WinFsp Filesystem Provider

Implements WinFsp's `FileSystemInterface` callbacks:

| WinFsp Callback | NFS Operations |
|---|---|
| `Open` | `OPEN` / `LOOKUP` + `GETFH` |
| `Close` | `CLOSE` |
| `Read` | `READ` (via read-ahead buffer) |
| `Write` | `WRITE` (via write coalescer) |
| `Flush` | `COMMIT` |
| `GetFileInfo` | `GETATTR` (via cache) |
| `SetFileInfo` | `SETATTR` |
| `ReadDirectory` | `READDIR` (via cache) |
| `Create` | `OPEN` with `OPEN4_CREATE` |
| `Cleanup` / `Delete` | `REMOVE` / `RMDIR` |
| `Rename` | `RENAME` |
| `GetSecurity` | `GETATTR` for ACL/mode -> synthesized Windows SD |
| `SetSecurity` | `SETATTR` for mode (best-effort) |

**Security descriptor synthesis:** NFS returns POSIX uid/gid/mode. WinFsp
expects Windows security descriptors. We synthesize a minimal SD: owner SID
mapped from uid, group SID from gid, DACL derived from mode bits. This is a
lossy translation but matches what other NFS clients (including Microsoft's)
do. Configurable uid/gid-to-SID mapping via mount options.

**Threading model:** WinFsp dispatches callbacks on its own thread pool. Each
callback spawns onto the Tokio runtime for async NFS operations, then blocks
the WinFsp thread on the result. Keeps the async protocol client clean while
satisfying WinFsp's synchronous callback model.

---

## AWS Credentials & TLS

### `aws-creds` — Credential Resolution

Wraps the AWS SDK credential chain with NFS-specific lifecycle management.

**Credential sources** (checked in order, matching AWS SDK convention):

1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
   `AWS_SESSION_TOKEN`)
2. EC2 instance metadata service (IMDS v2, IMDSv1 fallback)
3. ECS container credentials (if `AWS_CONTAINER_CREDENTIALS_RELATIVE_URI` set)
4. Shared credential file (`~/.aws/credentials`, profile-aware)
5. SSO token cache (`~/.aws/sso/cache/`)
6. Process credentials (`credential_process` in AWS config)

**Automatic refresh:** Background task monitors expiry and refreshes before
credentials lapse. Emits events for service logging. Refresh failures trigger
exponential backoff with jitter.

**Interface to NFS client:** The `aws-creds` crate does not plug into the RPC
`AuthFlavor` mechanism directly. AWS S3 Files authenticates via IAM at the
network/TLS layer, not via NFS RPC auth. The flow: `aws-creds` resolves IAM
credentials, passes them to the TLS transport layer where they are used during
connection establishment (the S3 Files endpoint validates the IAM identity of
the connecting EC2 instance or VPC). The RPC auth flavor on the wire is
`AUTH_SYS` with a nominal uid/gid — the actual authorization is IAM-based on
the server side. The NFS protocol client itself is unaware of AWS.

### TLS Integration (RFC 9289)

AWS S3 Files requires RPC-over-TLS. The upgrade flow:

1. Client connects via plain TCP to the NFS port
2. Client sends an `AUTH_TLS` RPC NULL probe
3. Server responds acknowledging TLS support
4. Client upgrades the connection to TLS (STARTTLS pattern)
5. All subsequent RPC traffic is encrypted

The `TlsTransport` in `nfs4-client` handles this transparently. Uses `rustls`
with the platform's certificate store (via `rustls-platform-verifier`) for
server cert validation. No client certificates — AWS authenticates via IAM
credentials passed out-of-band.

**For non-AWS NFS servers:** TLS is optional. Client attempts `AUTH_TLS` probe;
if the server rejects it, falls back to plain TCP. Configurable to require TLS
or disable the probe entirely.

---

## CLI & Windows Service

### CLI

```
windows-nfs-mount mount <server>:<export> <drive-letter> [options]
windows-nfs-mount umount <drive-letter>
windows-nfs-mount list
windows-nfs-mount status <drive-letter>
windows-nfs-mount config              (TUI configuration interface)
windows-nfs-mount service install|uninstall|start|stop
```

### Mount Options

| Option | Default | Description |
|---|---|---|
| `vers` | `4.1` | NFS version |
| `tls` | `auto` | `auto` / `require` / `disable` |
| `auth` | `auto` | `auto` / `sys` / `aws` |
| `aws-profile` | `default` | AWS credential profile |
| `rsize` | `1048576` | Read buffer size |
| `wsize` | `1048576` | Write buffer size |
| `acregmin` | `1` | File attribute cache min TTL (seconds) |
| `acregmax` | `60` | File attribute cache max TTL |
| `acdirmin` | `3` | Dir attribute cache min TTL |
| `acdirmax` | `60` | Dir attribute cache max TTL |
| `retrans` | `2` | RPC retransmissions before hard error |
| `timeo` | `600` | RPC timeout (tenths of second) |
| `uid` | `65534` | Default UID for AUTH_SYS |
| `gid` | `65534` | Default GID for AUTH_SYS |

Option naming mirrors Linux `mount.nfs` conventions.

### Config File

Location: `%PROGRAMDATA%\windows-nfs-mount\config.toml`

```toml
[defaults]
tls = "auto"
rsize = 1048576

[[mount]]
source = "fs-01234567.s3-nfs.us-east-1.amazonaws.com:/"
drive = "N"
auth = "aws"
aws-profile = "production"
auto = true

[[mount]]
source = "nas.internal:/shared"
drive = "S"
auth = "sys"
uid = 1000
gid = 1000
auto = true
```

### TUI Configuration Interface

A terminal UI built with `ratatui` for interactively managing mount
configurations. Launched via `windows-nfs-mount config`.

**Capabilities:**
- Browse and edit `config.toml` mount entries in a structured form (not raw
  text editing)
- Add new mount points with guided field entry (server, export, drive letter,
  auth type, options)
- Edit existing mount points — tab between fields, validate on save
- Remove mount points with confirmation
- Edit global defaults (`[defaults]` section)
- Test connectivity to a configured server (sends NFS NULL probe, reports
  TLS/auth status)
- View active mount status when the service is running (live read from the
  service via named pipe)

**Layout:**
- Left pane: list of configured mounts (highlighted with status indicators
  when service is running: green = mounted, red = error, gray = not auto)
- Right pane: detail/edit form for the selected mount
- Bottom bar: keybinding hints (Enter = edit, n = new, d = delete, t = test,
  q = quit)

**Dependencies:** `ratatui` + `crossterm` backend (cross-platform terminal
support on Windows). Only used in the `cli` crate — the service and protocol
crates have no TUI dependency.

**Config file round-tripping:** Uses `toml_edit` (not `toml`) for
serialization to preserve comments, formatting, and ordering in the user's
config file. Edits are surgical — only modified entries change on disk.

### Windows Service

- Registers as `WindowsNfsMount` via Windows Service Control Manager
- On start: reads config, mounts all entries with `auto = true`
- Graceful stop: flushes dirty buffers, sends `CLOSE` for open files,
  `DESTROY_SESSION`, unmounts WinFsp volumes
- Fast shutdown path: best-effort flush, then teardown
- **Named pipe IPC** (`\\.\pipe\windows-nfs-mount`): when the service is
  running, CLI `mount`/`umount` commands send requests to the service rather
  than mounting directly. Ensures the service tracks all active mounts.
- **Event log integration:** Logs mount/unmount, credential refresh, and errors
  to Windows Event Log under a custom source

---

## Error Handling & Resilience

### Reconnection Strategy

- **Transport layer:** Detects TCP/TLS failures. Exponential backoff (1s, 2s,
  4s... capped at 60s) with jitter. Configurable max retry for soft mounts;
  hard mounts retry indefinitely.
- **Session layer:** After reconnect, performs `EXCHANGE_ID` -> `CREATE_SESSION`
  -> `RECLAIM_COMPLETE`. Reclaims open state within the server's grace period.
  If reclaim fails, propagates I/O errors to applications.
- **Credential layer:** If reconnection fails due to expired AWS credentials,
  triggers refresh before retrying. If refresh itself fails, logs actionable
  error and enters retry loop — the service doesn't give up permanently.

### NFS-to-Windows Error Translation

| NFS Error | NTSTATUS | Meaning |
|---|---|---|
| `NFS4ERR_NOENT` | `STATUS_OBJECT_NAME_NOT_FOUND` | File not found |
| `NFS4ERR_ACCESS` / `NFS4ERR_PERM` | `STATUS_ACCESS_DENIED` | Permission denied |
| `NFS4ERR_EXIST` | `STATUS_OBJECT_NAME_COLLISION` | Already exists |
| `NFS4ERR_NOSPC` | `STATUS_DISK_FULL` | No space |
| `NFS4ERR_STALE` | `STATUS_FILE_INVALID` | Stale filehandle |
| `NFS4ERR_DELAY` | (retry internally) | Server busy |
| `NFS4ERR_GRACE` | (retry internally) | Server in grace period |
| Transport failure | `STATUS_NETWORK_UNREACHABLE` | Connection lost |

### Logging

Uses `tracing` with structured logging throughout.

**Output targets:**
- Windows Event Log (service mode) — `WARN` and above
- File log (`%PROGRAMDATA%\windows-nfs-mount\logs\`) — rolling daily,
  configurable retention, `DEBUG` level
- stderr (CLI mode) — interactive use

**RPC tracing:** At `TRACE` level, logs full compound operations with timing.

**Performance counters:** Tracks read/write throughput, cache hit rates, RPC
latency per mount. Exposed via `status` CLI command.

---

## Testing Strategy

### Unit Tests

- **`xdr-codec`:** Round-trip encode/decode for every type. Property-based
  testing with `proptest` for fuzzing edge cases.
- **`onc-rpc`:** Fragment reassembly, XID matching, auth encoding, malformed
  message rejection. Byte-sequence fixtures, no network.
- **`nfs4-types`:** Encode/decode for every operation. Verified against captured
  NFS traffic (pcap -> byte fixtures).
- **`nfs4-client`:** Session slot allocation, sequence ID tracking, compound
  builder. Mock `RpcTransport` with scripted replies.
- **`aws-creds`:** Credential chain resolution with mock providers. Expiry and
  refresh logic.

### Integration Tests

- **Local NFS server:** Docker container running `nfs-ganesha` (v4.1 support).
  Mount, perform file operations, verify. Runs on Linux CI to validate protocol
  stack without WinFsp.
- **WinFsp integration:** Runs on Windows CI. Mounts via WinFsp, exercises
  Win32 file APIs (`CreateFile`, `ReadFile`, `WriteFile`, `FindFirstFile`).
  Validates full stack.
- **AWS integration:** Separate suite requiring AWS credentials and a live S3
  Files filesystem. Run manually or in dedicated AWS CI. Tests IAM flow, TLS
  upgrade, basic file operations.

### Conformance Tests

- `pynfs` — Linux NFS community's NFSv4.1 conformance suite. Periodic
  validation, not CI.

### Performance Benchmarks

- `criterion` for XDR encode/decode throughput
- `iai` for instruction-count benchmarks on hot paths
- End-to-end throughput: sequential read/write, random I/O, metadata-heavy
  workloads. Compared against Linux `mount.nfs` as baseline.
