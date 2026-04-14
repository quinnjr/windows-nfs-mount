# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Phase 1: XDR Codec & ONC RPC

- `xdr-derive`: Proc-macro crate generating `XdrEncode`/`XdrDecode` derive
  macros for structs (named, tuple, unit) and enums (XDR discriminated unions
  with unit, tuple, and struct variants).
- `xdr-codec`: XDR serialization library per RFC 4506. Implements encode/decode
  for primitives (u32, i32, u64, i64, bool), variable-length types (Opaque,
  String, Vec, Option), and re-exports derive macros from `xdr-derive`.
  Property-based fuzz testing via proptest.
- `onc-rpc`: ONC RPC v2 implementation per RFC 5531. Includes AUTH_NONE and
  AUTH_SYS encoding, RPC call/reply message framing, record marking (fragment
  reassembly with 10MB limit), `RpcTransport` async trait, and `TcpTransport`
  with automatic XID tracking.

#### Phase 2: NFSv4.1 Types & Protocol Client

- `nfs4-types`: NFSv4.1 wire type definitions per RFC 8881. Base types
  (NfsFh4, ClientId4, Verifier4, SessionId4, StateId4, NfsTime4, ChangeInfo4),
  NfsStat4 enum (60+ error codes), Bitmap4/Fattr4 attribute types, and
  COMPOUND4args/COMPOUND4res envelope with 17 operation types: EXCHANGE_ID,
  CREATE_SESSION, SEQUENCE, DESTROY_SESSION, RECLAIM_COMPLETE, PUTROOTFH,
  PUTFH, LOOKUP, GETFH, GETATTR, SETATTR, OPEN, CLOSE, READ, WRITE, READDIR,
  REMOVE, RENAME.
- `nfs4-client`: NFSv4.1 protocol client. `CompoundBuilder` fluent API for
  constructing COMPOUND requests. `SessionManager` with slot tracking and
  per-slot sequence IDs for exactly-once semantics. `Nfs4Client` with session
  lifecycle (EXCHANGE_ID + CREATE_SESSION + RECLAIM_COMPLETE handshake,
  DESTROY_SESSION teardown). Lease keepalive background task sending SEQUENCE
  compounds at 1/3 of lease interval.

#### Phase 3: AWS Credentials

- `aws-creds`: AWS IAM credential resolution wrapping the AWS SDK default
  credential chain (environment variables, EC2 IMDS v2, ECS container
  credentials, shared credential file, SSO cache, process credentials).
  `AwsCredentialProvider` with caching (5-minute expiry buffer) and
  `from_static()` for testing. Background refresh task with exponential
  backoff (1s to 5min) on failures.

#### Phase 4: Mount Logic

- `nfs-mount`: High-level mount orchestration layer. `MountConfig` with
  tunable TTLs, buffer sizes, and write mode. Windows-to-NFS path translation.
  `AttrCache` (TTL-based, separate file/directory TTLs). `DirCache` for
  READDIR results with completeness tracking. `HandleTable` mapping local
  handle IDs to NFS stateids. `ReadAheadBuf` with sequential access detection
  (prefetches 4x after 2 consecutive sequential reads). `WriteBuf` coalescing
  small writes with auto-flush on buffer-full or non-contiguous access.
  `MountHandle` wrapping `Nfs4Client` with path resolution via COMPOUND.
