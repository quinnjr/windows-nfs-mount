# Project Progress

NFSv4.1 client for Windows in Rust. Mounts NFS shares (including AWS S3 Files)
as native drive letters via WinFsp.

## Current State

**48 source files, ~7,700 lines of Rust, 171 tests passing.**

### Completed (Phases 1-4)

#### Phase 1: XDR Codec & ONC RPC

| Crate | Status | Description |
|---|---|---|
| `xdr-derive` | **Done** | Proc-macro crate generating `XdrEncode`/`XdrDecode` for structs and enums (discriminated unions) |
| `xdr-codec` | **Done** | XDR serialization per RFC 4506 — primitives (u32, i32, u64, i64, bool), variable-length types (Opaque, String, Vec, Option), derive macro re-export. Proptest fuzz-tested. |
| `onc-rpc` | **Done** | ONC RPC v2 per RFC 5531 — AUTH_NONE/AUTH_SYS encoding, RPC call/reply messages, record marking (fragment framing), `RpcTransport` trait, async `TcpTransport` with XID tracking |

#### Phase 2: NFSv4.1 Types & Protocol Client

| Crate | Status | Description |
|---|---|---|
| `nfs4-types` | **Done** | NFSv4.1 wire types per RFC 8881 — base types (filehandles, stateids, verifiers, session IDs), `NfsStat4` (60+ error codes), `Bitmap4`/`Fattr4`, COMPOUND4args/res envelope, 17 operation types across session, filehandle, and data categories |
| `nfs4-client` | **Done** | Protocol client — `CompoundBuilder` fluent API, `SessionManager` with slot tracking, `Nfs4Client` with connect/disconnect lifecycle (EXCHANGE_ID + CREATE_SESSION + RECLAIM_COMPLETE), lease keepalive background task |

#### Phase 3: AWS Credentials

| Crate | Status | Description |
|---|---|---|
| `aws-creds` | **Done** | AWS IAM credential resolution wrapping the SDK default chain (env vars, IMDS, ECS, credential files, SSO, process credentials). Caching with 5-min expiry buffer, background refresh task with exponential backoff. |

#### Phase 4: Mount Logic

| Crate | Status | Description |
|---|---|---|
| `nfs-mount` | **Done** | High-level mount orchestration — `MountConfig`, `MountError`, Windows-to-NFS path translation, `AttrCache` (TTL-based), `DirCache`, `HandleTable`, `ReadAheadBuf` (sequential detection), `WriteBuf` (coalescing), `MountHandle` with path resolution via COMPOUND |

---

## Remaining Work

### Phase 5: WinFsp Filesystem Bridge

**Crate:** `winfsp-bridge`
**Requires:** Windows build environment, WinFsp SDK installed
**Estimated scope:** ~8 tasks

The bridge implements WinFsp's `FileSystemInterface` callbacks, translating
Win32 file operations to NFS operations through `MountHandle`.

| Task | Description | Complexity |
|---|---|---|
| 5.1 Scaffold `winfsp-bridge` crate | Add crate with `winfsp-sys` FFI binding dependency | Low |
| 5.2 WinFsp FFI bindings | Bindgen or manual bindings for WinFsp's C API (`FspFileSystemCreate`, `FspFileSystemStartDispatcher`, callback structs) | Medium |
| 5.3 Filesystem provider skeleton | Implement `FileSystemInterface` with stub callbacks that return `STATUS_NOT_IMPLEMENTED` | Medium |
| 5.4 Open / Create / Close | Wire `Open` to LOOKUP+GETFH or OPEN, `Create` to OPEN with CREATE flag, `Close` to CLOSE. Use `HandleTable` for tracking. | High |
| 5.5 Read / Write / Flush | Wire `Read` to READ via `ReadAheadBuf`, `Write` to WRITE via `WriteBuf`, `Flush` to COMMIT. | High |
| 5.6 GetFileInfo / SetFileInfo | Wire to GETATTR/SETATTR via `AttrCache`. Translate NFS attributes to Win32 `FILE_INFO`. | Medium |
| 5.7 ReadDirectory | Wire to READDIR via `DirCache`. Translate NFS directory entries to Win32 `DIR_INFO`. | Medium |
| 5.8 Rename / Delete / Security | Wire RENAME, REMOVE. Synthesize Windows security descriptors from POSIX mode bits. | Medium |
| 5.9 Threading bridge | Bridge WinFsp's synchronous callback thread pool to the Tokio async runtime (`block_on` per callback). | Medium |
| 5.10 Integration tests | Mount a drive letter, exercise Win32 file APIs, verify round-trip. Requires WinFsp installed. | High |

### Phase 6: TLS Transport (RPC-over-TLS)

**Crate:** `onc-rpc` (extend existing)
**Requires:** `rustls`, `rustls-platform-verifier`
**Estimated scope:** ~4 tasks

Required for AWS S3 Files and any NFS server using RPC-over-TLS (RFC 9289).

| Task | Description | Complexity |
|---|---|---|
| 6.1 Add `rustls` dependency | Add rustls + platform verifier to onc-rpc | Low |
| 6.2 TLS upgrade (STARTTLS) | Implement AUTH_TLS NULL probe: send AUTH_TLS RPC, on success upgrade TCP to TLS | High |
| 6.3 `TlsTransport` | Implement `RpcTransport` over TLS connection (same as TcpTransport but over `tokio-rustls` stream) | Medium |
| 6.4 Auto-detection | `tls=auto` mode: probe for TLS, fall back to plain TCP. `tls=require`: fail if no TLS. `tls=disable`: skip probe. | Medium |

### Phase 7: CLI Binary

**Crate:** `cli`
**Requires:** `clap`, `ratatui`, `crossterm`, `toml_edit`
**Estimated scope:** ~7 tasks

| Task | Description | Complexity |
|---|---|---|
| 7.1 Scaffold `cli` crate | Binary crate with `clap` for argument parsing | Low |
| 7.2 Config file parsing | Parse `config.toml` with mount entries, defaults, and profiles | Medium |
| 7.3 `mount` command | Connect to NFS server, establish session, start WinFsp filesystem on drive letter | High |
| 7.4 `umount` command | Flush buffers, destroy session, stop WinFsp filesystem | Medium |
| 7.5 `list` / `status` commands | Show active mounts with status, cache hit rates, RPC latency | Low |
| 7.6 `service` subcommand | Install/uninstall/start/stop the Windows service | Medium |
| 7.7 TUI config (`config` command) | Ratatui-based interactive config editor — mount list, edit form, connectivity test | High |

### Phase 8: Windows Service

**Crate:** `service`
**Requires:** `windows-service` crate, Windows APIs
**Estimated scope:** ~5 tasks

| Task | Description | Complexity |
|---|---|---|
| 8.1 Scaffold `service` crate | Binary crate with `windows-service` | Low |
| 8.2 Service registration | Register/unregister with Windows Service Control Manager | Medium |
| 8.3 Auto-mount on start | Read config, mount all entries with `auto = true` | Medium |
| 8.4 Named pipe IPC | CLI communicates with running service via `\\.\pipe\windows-nfs-mount` for mount/umount | High |
| 8.5 Graceful shutdown | Handle `SERVICE_CONTROL_STOP`: flush buffers, close files, destroy sessions, unmount. Handle `SERVICE_CONTROL_SHUTDOWN` with fast path. | Medium |
| 8.6 Event log integration | Log mount/unmount/errors to Windows Event Log | Low |

### Cross-Cutting Concerns (Can Be Done Anytime)

| Task | Description | Priority |
|---|---|---|
| Structured logging | Add `tracing` throughout all crates, stderr + file + Event Log subscribers | Medium |
| Reconnection logic | Detect transport failures, re-establish sessions, reclaim open state | High |
| Error retry policy | Automatic retry on NFS4ERR_DELAY/GRACE, re-lookup on STALE | High |
| Filehandle cache | Cache path-to-filehandle mappings in `nfs-mount` to avoid repeated LOOKUPs | Medium |
| NFS-to-NTSTATUS mapping | Translate NFS error codes to Windows NTSTATUS for WinFsp | Medium |
| Performance counters | Track read/write throughput, cache hit rates, RPC latency per mount | Low |
| CI pipeline | GitHub Actions: Linux (cargo test, nfs-ganesha integration), Windows (WinFsp integration) | Medium |
| nfs-ganesha integration tests | Docker-based tests against a real NFSv4.1 server | High |
| pynfs conformance | Validate against the NFS community's conformance test suite | Low |
| criterion benchmarks | XDR throughput, RPC round-trip latency, end-to-end I/O | Low |

---

## Suggested Build Order

```
Phase 6 (TLS)  ─── needed for AWS S3 Files
     │
Phase 5 (WinFsp Bridge) ─── needed for drive letter mounts
     │
Phase 7 (CLI) ─── first usable binary
     │
Phase 8 (Service) ─── persistent mounts
```

Phase 6 (TLS) can be done in parallel with Phase 5 since they touch different
crates. The CLI and Service depend on the WinFsp bridge being functional.

Reconnection logic and error retry should be added before any production use
— they're essential for reliability but not blocking initial development.

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────┐
│                  CLI / Service                       │
│              (clap, ratatui, windows-service)        │
├─────────────────────────────────────────────────────┤
│                 winfsp-bridge                        │
│           (WinFsp FileSystemInterface)               │
├─────────────────────────────────────────────────────┤
│                  nfs-mount                           │
│    (AttrCache, DirCache, ReadAhead, WriteBuf,        │
│     HandleTable, PathTranslation, MountHandle)       │
├──────────────────────┬──────────────────────────────┤
│     nfs4-client      │        aws-creds             │
│  (CompoundBuilder,   │  (AwsCredentialProvider,     │
│   SessionManager,    │   background refresh)        │
│   Nfs4Client)        │                              │
├──────────────────────┴──────────────────────────────┤
│                  nfs4-types                          │
│          (all NFSv4.1 wire types)                    │
├─────────────────────────────────────────────────────┤
│                   onc-rpc                            │
│     (TcpTransport, TlsTransport, RecordMarking,     │
│      AuthFlavors, RPC Messages)                      │
├─────────────────────────────────────────────────────┤
│              xdr-codec + xdr-derive                  │
│         (XDR serialization, derive macros)           │
└─────────────────────────────────────────────────────┘
```

Boxes above are built. **Bold borders** indicate remaining work:
- `winfsp-bridge`: not started
- `cli` / `service`: not started
- `onc-rpc` TlsTransport: not started
- Reconnection, retry logic, logging: not started
