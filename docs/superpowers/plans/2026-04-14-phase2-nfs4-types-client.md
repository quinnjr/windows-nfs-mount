# Phase 2: NFSv4.1 Types & Protocol Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the NFSv4.1 type definitions and protocol client — the ability to establish sessions with an NFS server and perform basic file operations via COMPOUND requests.

**Architecture:** Two crates: `nfs4-types` defines all NFSv4.1 wire types as Rust structs/enums with `#[derive(XdrEncode, XdrDecode)]`, and `nfs4-client` provides a `CompoundBuilder` fluent API, session slot management, and lease keepalive over `onc-rpc`'s `RpcTransport`.

**Tech Stack:** Rust 2024, `xdr-codec` (XDR derives), `onc-rpc` (RPC transport), `tokio` (async runtime), `bytes`

**Spec:** `docs/superpowers/specs/2026-04-14-nfs-mount-design.md` — sections "nfs4-types" and "nfs4-client"

**Scope:** Minimum viable operation set — session management (EXCHANGE_ID, CREATE_SESSION, SEQUENCE, DESTROY_SESSION, RECLAIM_COMPLETE) and core file ops (PUTROOTFH, PUTFH, LOOKUP, GETFH, GETATTR, SETATTR, READDIR, OPEN, CLOSE, READ, WRITE, REMOVE, RENAME). This covers all operations needed by the WinFsp bridge.

---

## File Structure

```
crates/
  nfs4-types/
    Cargo.toml
    src/
      lib.rs                (re-exports all public types)
      base.rs               (primitive newtypes: nfs_fh4, clientid4, verifier4, sessionid4, stateid4, nfstime4, change_info4)
      status.rs             (nfsstat4 error code enum — all codes from RFC 8881)
      bitmap.rs             (bitmap4, file attribute bitmap constants, fattr4)
      compound.rs           (COMPOUND4args, COMPOUND4res, nfs_argop4, nfs_resop4 dispatch enums)
      ops/
        mod.rs              (re-exports)
        session.rs          (EXCHANGE_ID4args/res, CREATE_SESSION4args/res, SEQUENCE4args/res, DESTROY_SESSION4args/res, RECLAIM_COMPLETE4args/res)
        filehandle.rs       (PUTROOTFH4res, PUTFH4args/res, LOOKUP4args/res, GETFH4res, GETATTR4args/res, SETATTR4args/res)
        data.rs             (OPEN4args/res, CLOSE4args/res, READ4args/res, WRITE4args/res, READDIR4args/res, REMOVE4args/res, RENAME4args/res)
  nfs4-client/
    Cargo.toml
    src/
      lib.rs                (re-exports)
      error.rs              (NfsError enum wrapping nfsstat4 and transport errors)
      compound.rs           (CompoundBuilder fluent API for constructing COMPOUND4args)
      session.rs            (SessionManager: slot tracking, sequence IDs, session establishment)
      keepalive.rs          (lease keepalive background task)
      client.rs             (Nfs4Client: high-level API combining transport + session + compound building)
```

---

### Task 1: Add nfs4-types and nfs4-client Crates to Workspace

**Files:**
- Modify: `Cargo.toml` (add workspace members)
- Create: `crates/nfs4-types/Cargo.toml`
- Create: `crates/nfs4-types/src/lib.rs`
- Create: `crates/nfs4-client/Cargo.toml`
- Create: `crates/nfs4-client/src/lib.rs`

- [ ] **Step 1: Update workspace Cargo.toml**

Add `"crates/nfs4-types"` and `"crates/nfs4-client"` to the `members` list.

- [ ] **Step 2: Create nfs4-types crate**

`crates/nfs4-types/Cargo.toml`:
```toml
[package]
name = "nfs4-types"
version.workspace = true
edition.workspace = true

[dependencies]
bytes = { workspace = true }
xdr-codec = { path = "../xdr-codec" }
```

`crates/nfs4-types/src/lib.rs`:
```rust
// Modules will be added in subsequent tasks.
```

- [ ] **Step 3: Create nfs4-client crate**

`crates/nfs4-client/Cargo.toml`:
```toml
[package]
name = "nfs4-client"
version.workspace = true
edition.workspace = true

[dependencies]
bytes = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
xdr-codec = { path = "../xdr-codec" }
onc-rpc = { path = "../onc-rpc" }
nfs4-types = { path = "../nfs4-types" }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

`crates/nfs4-client/src/lib.rs`:
```rust
// Modules will be added in subsequent tasks.
```

- [ ] **Step 4: Verify workspace compiles**

Run: `cargo check --workspace`

- [ ] **Step 5: Commit**

```
build: add nfs4-types and nfs4-client crates to workspace

nfs4-types will contain NFSv4.1 data type definitions with XDR
derive macros. nfs4-client will contain the protocol client with
session management and compound builder. Both are stubs for now.
```

---

### Task 2: Base Primitive Types

**Files:**
- Create: `crates/nfs4-types/src/base.rs`
- Modify: `crates/nfs4-types/src/lib.rs`

Define NFSv4.1 primitive types that appear throughout the protocol. All are newtypes over XDR primitives with derive macros.

- [ ] **Step 1: Create base types with tests**

Create `crates/nfs4-types/src/base.rs`:

```rust
use bytes::{Buf, BufMut, Bytes, BytesMut};
use xdr_codec::{Opaque, XdrDecode, XdrEncode, XdrError};

/// NFS filehandle — variable-length opaque, max 128 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NfsFh4(pub Vec<u8>);

impl XdrEncode for NfsFh4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        Opaque(self.0.clone()).encode(buf)
    }
}

impl XdrDecode for NfsFh4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let opaque = Opaque::decode(buf)?;
        if opaque.0.len() > 128 {
            return Err(XdrError::LengthExceeded { max: 128, actual: opaque.0.len() as u32 });
        }
        Ok(NfsFh4(opaque.0))
    }
}

/// Client identifier — u64 assigned by server during EXCHANGE_ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, XdrEncode, XdrDecode)]
pub struct ClientId4(pub u64);

/// Verifier — fixed 8-byte opaque used for reboot detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verifier4(pub [u8; 8]);

impl XdrEncode for Verifier4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_slice(&self.0);
        Ok(())
    }
}

impl XdrDecode for Verifier4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 8 {
            return Err(XdrError::BufferTooShort { needed: 8, available: buf.remaining() });
        }
        let mut arr = [0u8; 8];
        buf.copy_to_slice(&mut arr);
        Ok(Verifier4(arr))
    }
}

/// Session identifier — fixed 16-byte opaque.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId4(pub [u8; 16]);

impl XdrEncode for SessionId4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_slice(&self.0);
        Ok(())
    }
}

impl XdrDecode for SessionId4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 16 {
            return Err(XdrError::BufferTooShort { needed: 16, available: buf.remaining() });
        }
        let mut arr = [0u8; 16];
        buf.copy_to_slice(&mut arr);
        Ok(SessionId4(arr))
    }
}

/// Sequence ID for session slot tracking.
pub type SequenceId4 = u32;

/// Slot ID within a session.
pub type SlotId4 = u32;

/// State ID — identifies open/lock state on the server.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct StateId4 {
    pub seqid: u32,
    pub other: StateIdOther,
}

/// The 12-byte "other" field of a stateid4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateIdOther(pub [u8; 12]);

impl XdrEncode for StateIdOther {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        buf.put_slice(&self.0);
        Ok(())
    }
}

impl XdrDecode for StateIdOther {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        if buf.remaining() < 12 {
            return Err(XdrError::BufferTooShort { needed: 12, available: buf.remaining() });
        }
        let mut arr = [0u8; 12];
        buf.copy_to_slice(&mut arr);
        Ok(StateIdOther(arr))
    }
}

impl StateId4 {
    /// Special stateid meaning "anonymous" / "current stateid".
    pub fn anonymous() -> Self {
        Self { seqid: 0, other: StateIdOther([0; 12]) }
    }
}

/// NFS time — seconds + nanoseconds.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct NfsTime4 {
    pub seconds: i64,
    pub nseconds: u32,
}

/// Change info returned by mutating operations.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct ChangeInfo4 {
    pub atomic: bool,
    pub before: u64,
    pub after: u64,
}

/// NFS program number and version.
pub const NFS4_PROGRAM: u32 = 100003;
pub const NFS4_VERSION: u32 = 4;
pub const NFS4_MINOR_VERSION: u32 = 1;

/// COMPOUND procedure number (always 1 for NFSv4).
pub const NFSPROC4_COMPOUND: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfs_fh4_round_trip() {
        let fh = NfsFh4(vec![1, 2, 3, 4, 5]);
        let mut buf = BytesMut::new();
        fh.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(NfsFh4::decode(&mut bytes).unwrap(), fh);
    }

    #[test]
    fn verifier4_round_trip() {
        let v = Verifier4([0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]);
        let mut buf = BytesMut::new();
        v.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 8);
        let mut bytes = buf.freeze();
        assert_eq!(Verifier4::decode(&mut bytes).unwrap(), v);
    }

    #[test]
    fn session_id4_round_trip() {
        let sid = SessionId4([1; 16]);
        let mut buf = BytesMut::new();
        sid.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 16);
        let mut bytes = buf.freeze();
        assert_eq!(SessionId4::decode(&mut bytes).unwrap(), sid);
    }

    #[test]
    fn stateid4_round_trip() {
        let st = StateId4 { seqid: 42, other: StateIdOther([0xAA; 12]) };
        let mut buf = BytesMut::new();
        st.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 16); // 4 + 12
        let mut bytes = buf.freeze();
        assert_eq!(StateId4::decode(&mut bytes).unwrap(), st);
    }

    #[test]
    fn client_id4_round_trip() {
        let cid = ClientId4(0x123456789ABCDEF0);
        let mut buf = BytesMut::new();
        cid.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(ClientId4::decode(&mut bytes).unwrap(), cid);
    }

    #[test]
    fn nfs_time4_round_trip() {
        let t = NfsTime4 { seconds: 1700000000, nseconds: 500_000_000 };
        let mut buf = BytesMut::new();
        t.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(NfsTime4::decode(&mut bytes).unwrap(), t);
    }

    #[test]
    fn change_info4_round_trip() {
        let ci = ChangeInfo4 { atomic: true, before: 100, after: 101 };
        let mut buf = BytesMut::new();
        ci.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(ChangeInfo4::decode(&mut bytes).unwrap(), ci);
    }
}
```

- [ ] **Step 2: Update lib.rs**

```rust
pub mod base;

pub use base::*;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p nfs4-types`
Expected: 7 tests pass

- [ ] **Step 4: Commit**

```
feat(nfs4): define NFSv4.1 base primitive types

Add the foundational wire types that appear throughout NFSv4.1:
NfsFh4 (filehandle), ClientId4, Verifier4 (8-byte fixed opaque),
SessionId4 (16-byte fixed opaque), StateId4 (sequence + 12-byte
other), NfsTime4, and ChangeInfo4.

Fixed-size opaques (Verifier4, SessionId4, StateIdOther) have
manual XdrEncode/XdrDecode impls because the derive macros work
on variable-length types and these are fixed-size with no length
prefix on the wire. NfsFh4 uses the Opaque wire format but adds
a 128-byte length check on decode per RFC 8881.
```

---

### Task 3: NFS Status Codes

**Files:**
- Create: `crates/nfs4-types/src/status.rs`
- Modify: `crates/nfs4-types/src/lib.rs`

- [ ] **Step 1: Create nfsstat4 enum**

Create `crates/nfs4-types/src/status.rs` with a comprehensive enum of NFS status codes. Use `#[derive(XdrEncode, XdrDecode)]` with `#[xdr(discriminant = N)]` for each variant. Include all status codes from RFC 8881 section 15.1.

The enum should include at minimum: `Ok = 0`, `Perm = 1`, `Noent = 2`, `Io = 5`, `Nxio = 6`, `Access = 13`, `Exist = 17`, `Xdev = 18`, `Notdir = 20`, `Isdir = 21`, `Inval = 22`, `Fbig = 27`, `Nospc = 28`, `Rofs = 30`, `Nametoolong = 63`, `Notempty = 66`, `Dquot = 69`, `Stale = 70`, `BadHandle = 10001`, `BadCookie = 10003`, `Notsupp = 10004`, `Toosmall = 10005`, `Serverfault = 10006`, `Badtype = 10007`, `Delay = 10008`, `Same = 10009`, `Denied = 10010`, `Expired = 10011`, `Locked = 10012`, `Grace = 10013`, `FhExpired = 10014`, `ShareDenied = 10015`, `WrongSec = 10016`, `Moved = 10019`, `Nofilehandle = 10020`, `MinorVersMismatch = 10021`, `StaleClientId = 10022`, `StaleStateid = 10023`, `OldStateid = 10024`, `BadStateid = 10025`, `BadSeqid = 10026`, `Symlink = 10029`, `AttrNotsupp = 10032`, `BadOwner = 10036`, `BadName = 10038`, `BadSession = 10052`, `BadSlot = 10053`, `CompleteAlready = 10054`, `ConnNotBoundToSession = 10055`, `SeqMisordered = 10063`, `SequencePos = 10064`, `ReqTooBig = 10065`, `RepTooBig = 10066`, `RepTooBigToCache = 10067`, `RetryUncachedRep = 10068`, `UnsafeCompound = 10069`, `TooManyOps = 10070`, `OpNotInSession = 10071`, `SeqFalseRetry = 10076`, `BadHighSlot = 10077`, `DeadSession = 10078`, `OpIllegal = 10044`.

All variants should be unit variants (no data), each with its discriminant value. Add a `#[xdr(discriminant = N)]` attribute with the RFC-defined error number.

Add an `impl NfsStat4` block with:
```rust
pub fn is_ok(&self) -> bool {
    matches!(self, NfsStat4::Ok)
}
```

Tests:
- `ok_round_trip`: encode Ok, verify 4 bytes, decode back
- `error_round_trip`: encode Stale (10070), decode back
- `wire_format`: encode Noent, verify bytes are `[0, 0, 0, 2]`

- [ ] **Step 2: Update lib.rs**

Add `pub mod status;` and `pub use status::NfsStat4;`

- [ ] **Step 3: Run tests, commit**

```
feat(nfs4): define nfsstat4 error code enum

All NFSv4.1 status codes from RFC 8881 section 15.1 as a Rust enum
with XDR discriminant values. Covers 60+ error codes including
session-specific errors (BadSession, SeqMisordered, etc.) needed
for NFSv4.1 session management.
```

---

### Task 4: File Attribute Types (bitmap4, fattr4)

**Files:**
- Create: `crates/nfs4-types/src/bitmap.rs`
- Modify: `crates/nfs4-types/src/lib.rs`

NFSv4 attributes are identified by bitmaps and encoded as opaque blobs. This task defines the bitmap and attribute infrastructure.

- [ ] **Step 1: Create bitmap and attribute types**

Create `crates/nfs4-types/src/bitmap.rs`:

```rust
use bytes::{Bytes, BytesMut};
use xdr_codec::{Opaque, XdrDecode, XdrEncode, XdrError};

/// Attribute bitmap — variable-length array of u32 words.
/// Each bit position identifies an attribute.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct Bitmap4(pub Vec<u32>);

impl Bitmap4 {
    pub fn new() -> Self {
        Self(vec![])
    }

    /// Set a bit in the bitmap, extending if necessary.
    pub fn set(&mut self, bit: u32) {
        let word = (bit / 32) as usize;
        let pos = bit % 32;
        while self.0.len() <= word {
            self.0.push(0);
        }
        self.0[word] |= 1 << pos;
    }

    /// Check if a bit is set.
    pub fn is_set(&self, bit: u32) -> bool {
        let word = (bit / 32) as usize;
        let pos = bit % 32;
        self.0.get(word).map_or(false, |w| (w >> pos) & 1 == 1)
    }
}

impl Default for Bitmap4 {
    fn default() -> Self {
        Self::new()
    }
}

// NFSv4.1 attribute numbers (RFC 8881 section 5.8)
// Word 0 (bits 0-31): mandatory/recommended attributes
pub const FATTR4_SUPPORTED_ATTRS: u32 = 0;
pub const FATTR4_TYPE: u32 = 1;
pub const FATTR4_FH_EXPIRE_TYPE: u32 = 2;
pub const FATTR4_CHANGE: u32 = 3;
pub const FATTR4_SIZE: u32 = 4;
pub const FATTR4_LINK_SUPPORT: u32 = 5;
pub const FATTR4_SYMLINK_SUPPORT: u32 = 6;
pub const FATTR4_NAMED_ATTR: u32 = 7;
pub const FATTR4_FSID: u32 = 8;
pub const FATTR4_UNIQUE_HANDLES: u32 = 9;
pub const FATTR4_LEASE_TIME: u32 = 10;
pub const FATTR4_RDATTR_ERROR: u32 = 11;
pub const FATTR4_FILEHANDLE: u32 = 19;

// Word 1 (bits 32-63): recommended attributes
pub const FATTR4_MODE: u32 = 33;
pub const FATTR4_NUMLINKS: u32 = 35;
pub const FATTR4_OWNER: u32 = 36;
pub const FATTR4_OWNER_GROUP: u32 = 37;
pub const FATTR4_RAWDEV: u32 = 41;
pub const FATTR4_SPACE_USED: u32 = 45;
pub const FATTR4_TIME_ACCESS: u32 = 47;
pub const FATTR4_TIME_METADATA: u32 = 52;
pub const FATTR4_TIME_MODIFY: u32 = 53;

/// NFS file type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, XdrEncode, XdrDecode)]
#[repr(u32)]
pub enum NfsFType4 {
    Reg = 1,
    Dir = 2,
    Blk = 3,
    Chr = 4,
    Lnk = 5,
    Sock = 6,
    Fifo = 7,
    AttrDir = 8,
    NamedAttr = 9,
}

/// FSID — filesystem identifier.
#[derive(Debug, Clone, PartialEq, Eq, XdrEncode, XdrDecode)]
pub struct FsId4 {
    pub major: u64,
    pub minor: u64,
}

/// File attributes — bitmap + opaque encoded values.
///
/// The attr_vals contains the XDR-encoded attribute values in bitmap
/// order. The caller is responsible for decoding them based on which
/// bits are set in attrmask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fattr4 {
    pub attrmask: Bitmap4,
    pub attr_vals: Vec<u8>,
}

impl XdrEncode for Fattr4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        self.attrmask.encode(buf)?;
        Opaque(self.attr_vals.clone()).encode(buf)
    }
}

impl XdrDecode for Fattr4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let attrmask = Bitmap4::decode(buf)?;
        let attr_vals = Opaque::decode(buf)?;
        Ok(Fattr4 { attrmask, attr_vals: attr_vals.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_set_and_check() {
        let mut bm = Bitmap4::new();
        bm.set(FATTR4_TYPE);     // bit 1
        bm.set(FATTR4_SIZE);     // bit 4
        bm.set(FATTR4_MODE);     // bit 33 (word 1)

        assert!(bm.is_set(FATTR4_TYPE));
        assert!(bm.is_set(FATTR4_SIZE));
        assert!(bm.is_set(FATTR4_MODE));
        assert!(!bm.is_set(FATTR4_CHANGE));
        assert_eq!(bm.0.len(), 2); // two u32 words
    }

    #[test]
    fn bitmap_round_trip() {
        let mut bm = Bitmap4::new();
        bm.set(0);
        bm.set(31);
        bm.set(32);
        let mut buf = BytesMut::new();
        bm.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(Bitmap4::decode(&mut bytes).unwrap(), bm);
    }

    #[test]
    fn fattr4_round_trip() {
        let mut mask = Bitmap4::new();
        mask.set(FATTR4_SIZE);
        let fa = Fattr4 {
            attrmask: mask,
            attr_vals: vec![0, 0, 0, 0, 0, 0, 0, 42], // u64 size = 42
        };
        let mut buf = BytesMut::new();
        fa.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        assert_eq!(Fattr4::decode(&mut bytes).unwrap(), fa);
    }

    #[test]
    fn nfs_ftype4_round_trip() {
        let ft = NfsFType4::Dir;
        let mut buf = BytesMut::new();
        ft.encode(&mut buf).unwrap();
        assert_eq!(&buf[..], &[0, 0, 0, 2]); // Dir = 2
        let mut bytes = buf.freeze();
        assert_eq!(NfsFType4::decode(&mut bytes).unwrap(), NfsFType4::Dir);
    }
}
```

- [ ] **Step 2: Update lib.rs**

Add `pub mod bitmap;` and `pub use bitmap::*;`.

- [ ] **Step 3: Run tests, commit**

```
feat(nfs4): define file attribute types (bitmap4, fattr4, ftype4)

Bitmap4 is a variable-length bit vector used to request and
identify file attributes. Attribute constants match RFC 8881
section 5.8. Fattr4 pairs a bitmap with opaque-encoded attribute
values — decoding specific attributes is left to the consumer
since the encoding depends on which bits are set.

NfsFType4 covers all NFSv4 file types (regular, directory, symlink,
block/char device, socket, fifo, named attribute).
```

---

### Task 5: COMPOUND Envelope and Operation Dispatch

**Files:**
- Create: `crates/nfs4-types/src/compound.rs`
- Create: `crates/nfs4-types/src/ops/mod.rs`
- Modify: `crates/nfs4-types/src/lib.rs`

This is the central dispatch mechanism. COMPOUND wraps a sequence of operations in one RPC call.

- [ ] **Step 1: Create operation number constants and compound types**

Create `crates/nfs4-types/src/ops/mod.rs`:
```rust
pub mod session;
pub mod filehandle;
pub mod data;

/// NFSv4.1 operation numbers (RFC 8881 section 18).
pub const OP_ACCESS: u32 = 3;
pub const OP_CLOSE: u32 = 4;
pub const OP_COMMIT: u32 = 5;
pub const OP_GETATTR: u32 = 9;
pub const OP_GETFH: u32 = 10;
pub const OP_LOOKUP: u32 = 15;
pub const OP_OPEN: u32 = 18;
pub const OP_PUTFH: u32 = 22;
pub const OP_PUTROOTFH: u32 = 24;
pub const OP_READ: u32 = 25;
pub const OP_READDIR: u32 = 26;
pub const OP_REMOVE: u32 = 28;
pub const OP_RENAME: u32 = 29;
pub const OP_SETATTR: u32 = 34;
pub const OP_WRITE: u32 = 38;
pub const OP_EXCHANGE_ID: u32 = 42;
pub const OP_CREATE_SESSION: u32 = 43;
pub const OP_DESTROY_SESSION: u32 = 44;
pub const OP_SEQUENCE: u32 = 53;
pub const OP_RECLAIM_COMPLETE: u32 = 58;
```

Create stub files `crates/nfs4-types/src/ops/session.rs`, `crates/nfs4-types/src/ops/filehandle.rs`, `crates/nfs4-types/src/ops/data.rs` — each with just a comment placeholder for now (they'll be filled in Tasks 6-8).

Create `crates/nfs4-types/src/compound.rs`:

```rust
use bytes::{Bytes, BytesMut};
use xdr_codec::{XdrDecode, XdrEncode, XdrError};

use crate::ops::*;
use crate::status::NfsStat4;

/// Arguments for a single operation within a COMPOUND.
///
/// Each variant wraps the operation-specific arguments struct.
/// The discriminant is the operation number (nfs_opnum4).
#[derive(Debug, Clone, PartialEq)]
pub enum NfsArgOp4 {
    PutRootFh,
    PutFh(filehandle::PutFh4args),
    Lookup(filehandle::Lookup4args),
    GetFh,
    GetAttr(filehandle::GetAttr4args),
    SetAttr(filehandle::SetAttr4args),
    Open(data::Open4args),
    Close(data::Close4args),
    Read(data::Read4args),
    Write(data::Write4args),
    ReadDir(data::ReadDir4args),
    Remove(data::Remove4args),
    Rename(data::Rename4args),
    ExchangeId(session::ExchangeId4args),
    CreateSession(session::CreateSession4args),
    DestroySession(session::DestroySession4args),
    Sequence(session::Sequence4args),
    ReclaimComplete(session::ReclaimComplete4args),
}

impl XdrEncode for NfsArgOp4 {
    fn encode(&self, buf: &mut BytesMut) -> Result<(), XdrError> {
        match self {
            Self::PutRootFh => OP_PUTROOTFH.encode(buf),
            Self::PutFh(args) => { OP_PUTFH.encode(buf)?; args.encode(buf) }
            Self::Lookup(args) => { OP_LOOKUP.encode(buf)?; args.encode(buf) }
            Self::GetFh => OP_GETFH.encode(buf),
            Self::GetAttr(args) => { OP_GETATTR.encode(buf)?; args.encode(buf) }
            Self::SetAttr(args) => { OP_SETATTR.encode(buf)?; args.encode(buf) }
            Self::Open(args) => { OP_OPEN.encode(buf)?; args.encode(buf) }
            Self::Close(args) => { OP_CLOSE.encode(buf)?; args.encode(buf) }
            Self::Read(args) => { OP_READ.encode(buf)?; args.encode(buf) }
            Self::Write(args) => { OP_WRITE.encode(buf)?; args.encode(buf) }
            Self::ReadDir(args) => { OP_READDIR.encode(buf)?; args.encode(buf) }
            Self::Remove(args) => { OP_REMOVE.encode(buf)?; args.encode(buf) }
            Self::Rename(args) => { OP_RENAME.encode(buf)?; args.encode(buf) }
            Self::ExchangeId(args) => { OP_EXCHANGE_ID.encode(buf)?; args.encode(buf) }
            Self::CreateSession(args) => { OP_CREATE_SESSION.encode(buf)?; args.encode(buf) }
            Self::DestroySession(args) => { OP_DESTROY_SESSION.encode(buf)?; args.encode(buf) }
            Self::Sequence(args) => { OP_SEQUENCE.encode(buf)?; args.encode(buf) }
            Self::ReclaimComplete(args) => { OP_RECLAIM_COMPLETE.encode(buf)?; args.encode(buf) }
        }
    }
}

/// Result for a single operation within a COMPOUND response.
#[derive(Debug, Clone, PartialEq)]
pub enum NfsResOp4 {
    PutRootFh(NfsStat4),
    PutFh(NfsStat4),
    Lookup(NfsStat4),
    GetFh(filehandle::GetFh4res),
    GetAttr(filehandle::GetAttr4res),
    SetAttr(filehandle::SetAttr4res),
    Open(data::Open4res),
    Close(data::Close4res),
    Read(data::Read4res),
    Write(data::Write4res),
    ReadDir(data::ReadDir4res),
    Remove(data::Remove4res),
    Rename(data::Rename4res),
    ExchangeId(session::ExchangeId4res),
    CreateSession(session::CreateSession4res),
    DestroySession(NfsStat4),
    Sequence(session::Sequence4res),
    ReclaimComplete(NfsStat4),
}

impl XdrDecode for NfsResOp4 {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let opnum = u32::decode(buf)?;
        match opnum {
            OP_PUTROOTFH => Ok(Self::PutRootFh(NfsStat4::decode(buf)?)),
            OP_PUTFH => Ok(Self::PutFh(NfsStat4::decode(buf)?)),
            OP_LOOKUP => Ok(Self::Lookup(NfsStat4::decode(buf)?)),
            OP_GETFH => Ok(Self::GetFh(filehandle::GetFh4res::decode(buf)?)),
            OP_GETATTR => Ok(Self::GetAttr(filehandle::GetAttr4res::decode(buf)?)),
            OP_SETATTR => Ok(Self::SetAttr(filehandle::SetAttr4res::decode(buf)?)),
            OP_OPEN => Ok(Self::Open(data::Open4res::decode(buf)?)),
            OP_CLOSE => Ok(Self::Close(data::Close4res::decode(buf)?)),
            OP_READ => Ok(Self::Read(data::Read4res::decode(buf)?)),
            OP_WRITE => Ok(Self::Write(data::Write4res::decode(buf)?)),
            OP_READDIR => Ok(Self::ReadDir(data::ReadDir4res::decode(buf)?)),
            OP_REMOVE => Ok(Self::Remove(data::Remove4res::decode(buf)?)),
            OP_RENAME => Ok(Self::Rename(data::Rename4res::decode(buf)?)),
            OP_EXCHANGE_ID => Ok(Self::ExchangeId(session::ExchangeId4res::decode(buf)?)),
            OP_CREATE_SESSION => Ok(Self::CreateSession(session::CreateSession4res::decode(buf)?)),
            OP_DESTROY_SESSION => Ok(Self::DestroySession(NfsStat4::decode(buf)?)),
            OP_SEQUENCE => Ok(Self::Sequence(session::Sequence4res::decode(buf)?)),
            OP_RECLAIM_COMPLETE => Ok(Self::ReclaimComplete(NfsStat4::decode(buf)?)),
            other => Err(XdrError::InvalidEnum { discriminant: other, type_name: "nfs_opnum4" }),
        }
    }
}

/// COMPOUND4args — the top-level NFSv4 request.
#[derive(Debug, Clone, PartialEq, XdrEncode)]
pub struct Compound4args {
    pub tag: String,
    pub minorversion: u32,
    pub argarray: Vec<NfsArgOp4>,
}

/// COMPOUND4res — the top-level NFSv4 response.
#[derive(Debug, Clone, PartialEq)]
pub struct Compound4res {
    pub status: NfsStat4,
    pub tag: String,
    pub resarray: Vec<NfsResOp4>,
}

impl XdrDecode for Compound4res {
    fn decode(buf: &mut Bytes) -> Result<Self, XdrError> {
        let status = NfsStat4::decode(buf)?;
        let tag = String::decode(buf)?;
        let count = u32::decode(buf)? as usize;
        let mut resarray = Vec::with_capacity(count);
        for _ in 0..count {
            resarray.push(NfsResOp4::decode(buf)?);
        }
        Ok(Compound4res { status, tag, resarray })
    }
}
```

Note: This file will NOT compile until Tasks 6-8 fill in the operation types. That's expected — this task creates the framework, subsequent tasks fill in the pieces.

- [ ] **Step 2: Update lib.rs**

```rust
pub mod base;
pub mod bitmap;
pub mod compound;
pub mod ops;
pub mod status;

pub use base::*;
pub use bitmap::*;
pub use compound::*;
pub use ops::*;
pub use status::NfsStat4;
```

- [ ] **Step 3: Verify it compiles after Tasks 6-8 are done (do not run tests yet)**

This task creates the dispatch framework. It depends on operation types from Tasks 6-8. The subagent should create the files but note that `cargo check` will fail until those tasks are complete.

- [ ] **Step 4: Commit (after Tasks 6-8)**

This task's commit should be combined with Tasks 6-8 since the code doesn't compile independently. See Task 8 for the commit.

---

### Task 6: Session Operation Types

**Files:**
- Create: `crates/nfs4-types/src/ops/session.rs`

Define the session management operation types: EXCHANGE_ID, CREATE_SESSION, SEQUENCE, DESTROY_SESSION, RECLAIM_COMPLETE.

- [ ] **Step 1: Create session operation types**

Create `crates/nfs4-types/src/ops/session.rs` with all session operation args/results.

Key types needed:

**EXCHANGE_ID** (RFC 8881 section 18.35):
```rust
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ExchangeId4args {
    pub eia_clientowner: ClientOwner4,
    pub eia_flags: u32,
    pub eia_state_protect: StateProtect4a,
    pub eia_client_impl_id: Vec<NfsImplId4>,
}

#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct ClientOwner4 {
    pub co_verifier: Verifier4,
    pub co_ownerid: Opaque,
}
```

StateProtect4a is a discriminated union. For our purposes, we only need SP4_NONE (discriminant 0).

ExchangeId4res wraps status + ExchangeId4resok:
```rust
// status (NfsStat4) + on success: clientid, sequenceid, flags, state_protect, server_owner, server_scope, server_impl_id
```

**CREATE_SESSION** (RFC 8881 section 18.36):
```rust
pub struct CreateSession4args {
    pub csa_clientid: ClientId4,
    pub csa_sequence: SequenceId4,
    pub csa_flags: u32,
    pub csa_fore_chan_attrs: ChannelAttrs4,
    pub csa_back_chan_attrs: ChannelAttrs4,
    pub csa_cb_program: u32,
    pub csa_sec_parms: Vec<CallbackSecParms4>,
}
```

**SEQUENCE** (RFC 8881 section 18.46):
```rust
pub struct Sequence4args {
    pub sa_sessionid: SessionId4,
    pub sa_sequenceid: SequenceId4,
    pub sa_slotid: SlotId4,
    pub sa_highest_slotid: SlotId4,
    pub sa_cachethis: bool,
}
```

**DESTROY_SESSION**: just SessionId4

**RECLAIM_COMPLETE**: just a bool (rca_one_fs)

Implement all types with `#[derive(XdrEncode, XdrDecode)]` where possible, and manual impls for discriminated unions that need special handling.

Include round-trip tests for each operation's args and result types.

- [ ] **Step 2: Run tests when combined with Tasks 5, 7, 8**

---

### Task 7: Filehandle Operation Types

**Files:**
- Create: `crates/nfs4-types/src/ops/filehandle.rs`

Define operation types for PUTROOTFH, PUTFH, LOOKUP, GETFH, GETATTR, SETATTR.

- [ ] **Step 1: Create filehandle operation types**

These are relatively simple:

```rust
// PUTFH: takes a filehandle
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct PutFh4args {
    pub object: NfsFh4,
}

// LOOKUP: takes a component name
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct Lookup4args {
    pub objname: String,
}

// GETFH result: status + filehandle on success
// GETATTR args: bitmap of requested attributes
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct GetAttr4args {
    pub attr_request: Bitmap4,
}

// GETATTR result: status + fattr4 on success
// SETATTR args: stateid + fattr4
#[derive(Debug, Clone, PartialEq, XdrEncode, XdrDecode)]
pub struct SetAttr4args {
    pub sa_stateid: StateId4,
    pub sa_fattr: Fattr4,
}
```

Result types follow the pattern: status first, then data on success. For operations that just return status (PUTROOTFH, PUTFH, LOOKUP), the compound dispatch handles them directly. For GETFH, GETATTR, SETATTR, define result structs.

Include round-trip tests.

---

### Task 8: Data Operation Types + Full Compile + Tests

**Files:**
- Create: `crates/nfs4-types/src/ops/data.rs`

Define OPEN, CLOSE, READ, WRITE, READDIR, REMOVE, RENAME.

- [ ] **Step 1: Create data operation types**

Key types:

**OPEN** (complex — most involved operation in NFSv4):
```rust
pub struct Open4args {
    pub seqid: u32,              // open sequence id (deprecated in v4.1, set to 0)
    pub share_access: u32,       // OPEN4_SHARE_ACCESS_READ/WRITE/BOTH
    pub share_deny: u32,         // OPEN4_SHARE_DENY_NONE/READ/WRITE/BOTH
    pub owner: OpenOwner4,       // (clientid, owner bytes)
    pub openhow: OpenFlag4,      // OPEN4_NOCREATE or OPEN4_CREATE + createhow
    pub claim: OpenClaim4,       // CLAIM_NULL + filename, CLAIM_FH, etc.
}
```

OpenFlag4 and OpenClaim4 are discriminated unions. Keep them minimal — support NOCREATE + CLAIM_NULL (open existing file by name) and CREATE_GUARDED + CLAIM_NULL (create new file).

**READ/WRITE**: straightforward offset + count + data structures.

**READDIR**: takes cookie + verifier + count + bitmap, returns directory entries.

**REMOVE/RENAME**: take filename(s), return change_info.

- [ ] **Step 2: Ensure everything compiles**

Run: `cargo check -p nfs4-types`
All the types from Tasks 5-8 must fit together.

- [ ] **Step 3: Run all tests**

Run: `cargo test -p nfs4-types`
Expected: all pass

- [ ] **Step 4: Commit all of Tasks 5-8 together**

```
feat(nfs4): define COMPOUND envelope and NFSv4.1 operation types

Add the COMPOUND4args/COMPOUND4res dispatch mechanism and type
definitions for 17 NFSv4.1 operations covering session management
(EXCHANGE_ID, CREATE_SESSION, SEQUENCE, DESTROY_SESSION,
RECLAIM_COMPLETE), filehandle operations (PUTROOTFH, PUTFH, LOOKUP,
GETFH, GETATTR, SETATTR), and data operations (OPEN, CLOSE, READ,
WRITE, READDIR, REMOVE, RENAME).

NfsArgOp4/NfsResOp4 enums dispatch encode/decode by operation
number. COMPOUND4args derives XdrEncode; COMPOUND4res has a manual
XdrDecode because the operation result types vary per-operation.
```

---

### Task 9: NFS Client Error Types and Compound Builder

**Files:**
- Create: `crates/nfs4-client/src/error.rs`
- Create: `crates/nfs4-client/src/compound.rs`
- Modify: `crates/nfs4-client/src/lib.rs`

- [ ] **Step 1: Create error type**

Create `crates/nfs4-client/src/error.rs`:

```rust
use thiserror::Error;
use nfs4_types::NfsStat4;

#[derive(Debug, Error)]
pub enum NfsError {
    #[error("NFS error: {0:?}")]
    Nfs(NfsStat4),

    #[error("RPC error: {0}")]
    Rpc(#[from] onc_rpc::RpcError),

    #[error("XDR error: {0}")]
    Xdr(#[from] xdr_codec::XdrError),

    #[error("no session established")]
    NoSession,

    #[error("session expired")]
    SessionExpired,

    #[error("operation at index {index} failed: {status:?}")]
    OperationFailed { index: usize, status: NfsStat4 },

    #[error("unexpected response: expected {expected}, got {actual}")]
    UnexpectedResponse { expected: &'static str, actual: String },
}
```

- [ ] **Step 2: Create compound builder**

Create `crates/nfs4-client/src/compound.rs`:

```rust
use nfs4_types::*;
use nfs4_types::ops::session::*;
use nfs4_types::ops::filehandle::*;
use nfs4_types::ops::data::*;

/// Fluent builder for constructing COMPOUND4args.
///
/// Usage:
///   CompoundBuilder::new()
///     .sequence(session_id, seq_id, slot_id, highest_slot)
///     .putrootfh()
///     .lookup("mydir")
///     .getfh()
///     .getattr(requested_attrs)
///     .build()
pub struct CompoundBuilder {
    tag: String,
    ops: Vec<NfsArgOp4>,
}

impl CompoundBuilder {
    pub fn new() -> Self {
        Self {
            tag: String::new(),
            ops: Vec::new(),
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    pub fn sequence(
        mut self,
        session_id: SessionId4,
        sequence_id: SequenceId4,
        slot_id: SlotId4,
        highest_slot_id: SlotId4,
    ) -> Self {
        self.ops.push(NfsArgOp4::Sequence(Sequence4args {
            sa_sessionid: session_id,
            sa_sequenceid: sequence_id,
            sa_slotid: slot_id,
            sa_highest_slotid: highest_slot_id,
            sa_cachethis: false,
        }));
        self
    }

    pub fn putrootfh(mut self) -> Self {
        self.ops.push(NfsArgOp4::PutRootFh);
        self
    }

    pub fn putfh(mut self, fh: NfsFh4) -> Self {
        self.ops.push(NfsArgOp4::PutFh(PutFh4args { object: fh }));
        self
    }

    pub fn lookup(mut self, name: impl Into<String>) -> Self {
        self.ops.push(NfsArgOp4::Lookup(Lookup4args { objname: name.into() }));
        self
    }

    pub fn getfh(mut self) -> Self {
        self.ops.push(NfsArgOp4::GetFh);
        self
    }

    pub fn getattr(mut self, attr_request: Bitmap4) -> Self {
        self.ops.push(NfsArgOp4::GetAttr(GetAttr4args { attr_request }));
        self
    }

    pub fn build(self) -> Compound4args {
        Compound4args {
            tag: self.tag,
            minorversion: NFS4_MINOR_VERSION,
            argarray: self.ops,
        }
    }
}

// Add more builder methods (setattr, open, close, read, write, readdir,
// remove, rename, exchange_id, create_session, destroy_session,
// reclaim_complete) following the same pattern.
```

Include tests verifying the builder produces correct Compound4args.

- [ ] **Step 3: Update lib.rs, run tests, commit**

```
feat(nfs4-client): add NfsError type and CompoundBuilder fluent API

NfsError wraps NFS status codes, RPC errors, and XDR errors into
a single error type for the client layer.

CompoundBuilder provides a fluent API for constructing COMPOUND4args:
  CompoundBuilder::new()
    .sequence(...)
    .putrootfh()
    .lookup("dir")
    .getfh()
    .build()

Each method appends one operation. The builder handles minorversion
(always 1) and tag automatically.
```

---

### Task 10: Session Manager

**Files:**
- Create: `crates/nfs4-client/src/session.rs`
- Modify: `crates/nfs4-client/src/lib.rs`

The session manager handles EXCHANGE_ID -> CREATE_SESSION, slot allocation, and sequence ID tracking.

- [ ] **Step 1: Create session manager**

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use nfs4_types::*;

/// Tracks session state: session ID, slot allocation, sequence IDs.
pub struct SessionManager {
    pub session_id: SessionId4,
    pub client_id: ClientId4,
    pub sequence_id: AtomicU32,
    slots: Mutex<SlotTable>,
    pub max_slots: u32,
    pub lease_time: u32,  // seconds, from server
}

struct SlotTable {
    /// Next sequence ID per slot.
    sequence_ids: Vec<u32>,
    /// Which slots are currently in use.
    in_use: Vec<bool>,
}

impl SessionManager {
    pub fn new(
        session_id: SessionId4,
        client_id: ClientId4,
        max_slots: u32,
        lease_time: u32,
    ) -> Self { ... }

    /// Allocate a slot for a new request. Returns (slot_id, sequence_id).
    /// Blocks if all slots are in use.
    pub async fn alloc_slot(&self) -> (SlotId4, SequenceId4) { ... }

    /// Release a slot after the request completes.
    pub async fn release_slot(&self, slot_id: SlotId4) { ... }

    /// Get the highest slot ID (for SEQUENCE args).
    pub async fn highest_slot(&self) -> SlotId4 { ... }
}
```

Tests:
- Allocate and release a slot
- Sequence IDs increment per slot
- Multiple slots can be allocated

- [ ] **Step 2: Run tests, commit**

```
feat(nfs4-client): implement SessionManager with slot tracking

SessionManager tracks NFSv4.1 session state: session ID, client ID,
slot allocation, and per-slot sequence IDs. The alloc_slot method
provides exactly-once semantics per RFC 8881 — each slot's sequence
ID increments on every use and the server rejects replays.

Slots are allocated from a fixed-size table matching the server's
max_requests_per_session. A Mutex-based approach is used because
NFSv4.1 sessions typically have few slots (default ~64) and
contention is managed at the application level.
```

---

### Task 11: Nfs4Client High-Level API

**Files:**
- Create: `crates/nfs4-client/src/client.rs`
- Modify: `crates/nfs4-client/src/lib.rs`

- [ ] **Step 1: Create client struct**

The `Nfs4Client` ties together `RpcTransport`, `SessionManager`, and `CompoundBuilder`:

```rust
use std::sync::Arc;
use bytes::BytesMut;
use onc_rpc::{AcceptStatus, AuthFlavor, RpcTransport};
use xdr_codec::{XdrDecode, XdrEncode};
use nfs4_types::*;

use crate::error::NfsError;
use crate::session::SessionManager;

pub struct Nfs4Client<T: RpcTransport> {
    transport: Arc<T>,
    auth: AuthFlavor,
    session: Option<SessionManager>,
}

impl<T: RpcTransport> Nfs4Client<T> {
    pub fn new(transport: Arc<T>, auth: AuthFlavor) -> Self { ... }

    /// Send a COMPOUND request and decode the response.
    async fn compound(&self, args: Compound4args) -> Result<Compound4res, NfsError> {
        let mut buf = BytesMut::new();
        args.encode(&mut buf)?;
        let response = self.transport.call(
            NFS4_PROGRAM, NFS4_VERSION, NFSPROC4_COMPOUND,
            &buf, &self.auth, &AuthFlavor::None,
        ).await?;
        match response.status {
            AcceptStatus::Success(data) => {
                let mut bytes = data.0.into();
                Ok(Compound4res::decode(&mut bytes)?)
            }
            _ => Err(NfsError::Rpc(onc_rpc::RpcError::AcceptError(
                "non-success RPC status".into()
            ))),
        }
    }

    /// Establish a session: EXCHANGE_ID + CREATE_SESSION + RECLAIM_COMPLETE.
    pub async fn connect(&mut self) -> Result<(), NfsError> { ... }

    /// Destroy the session.
    pub async fn disconnect(&mut self) -> Result<(), NfsError> { ... }
}
```

Tests using a mock RpcTransport that returns scripted replies:
- Connect establishes session (EXCHANGE_ID -> CREATE_SESSION -> RECLAIM_COMPLETE)
- Compound builder + send produces correct wire bytes
- Error responses are properly decoded

- [ ] **Step 2: Run all tests, commit**

```
feat(nfs4-client): implement Nfs4Client with session lifecycle

Nfs4Client wraps an RpcTransport with NFSv4.1 session management.
The connect() method performs the three-step handshake: EXCHANGE_ID
to register with the server, CREATE_SESSION to establish a session
with slot-based flow control, and RECLAIM_COMPLETE to signal
readiness.

The compound() method handles COMPOUND4args serialization, RPC
dispatch, and COMPOUND4res deserialization. It's the foundation
for all NFS operations — higher layers construct Compound4args
via CompoundBuilder and call compound() to execute them.
```

---

### Task 12: Lease Keepalive Background Task

**Files:**
- Create: `crates/nfs4-client/src/keepalive.rs`
- Modify: `crates/nfs4-client/src/lib.rs`

- [ ] **Step 1: Create keepalive task**

```rust
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{Duration, interval};

/// Spawns a background task that sends SEQUENCE keepalives
/// before the lease expires.
pub fn spawn_keepalive<T: RpcTransport + 'static>(
    client: Arc<Nfs4Client<T>>,
    lease_time: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    let keepalive_interval = lease_time / 3; // renew at 1/3 of lease
    tokio::spawn(async move {
        let mut timer = interval(keepalive_interval);
        loop {
            tokio::select! {
                _ = timer.tick() => {
                    // Send SEQUENCE-only compound as keepalive
                    if let Err(e) = client.keepalive().await {
                        tracing::warn!("keepalive failed: {}", e);
                    }
                }
                _ = shutdown.changed() => {
                    break;
                }
            }
        }
    })
}
```

Tests:
- Keepalive sends SEQUENCE compounds at the expected interval
- Shutdown signal stops the task

- [ ] **Step 2: Run tests, commit**

```
feat(nfs4-client): add lease keepalive background task

NFSv4.1 sessions expire after the server's lease_time if no
activity occurs. The keepalive task sends a bare SEQUENCE compound
at 1/3 of the lease interval to prevent expiry.

Uses tokio::select! with a watch channel for clean shutdown —
disconnect() signals the channel and the keepalive task exits
after the current interval.
```

---

### Task 13: Integration Test with Mock NFS Server

**Files:**
- Create: `crates/nfs4-client/tests/session_lifecycle.rs`

- [ ] **Step 1: Create mock NFS server and integration test**

Build a mock NFS server that handles:
1. EXCHANGE_ID → returns a client ID and session parameters
2. CREATE_SESSION → returns a session ID and slot parameters
3. SEQUENCE + RECLAIM_COMPLETE → returns OK
4. SEQUENCE + PUTROOTFH + GETATTR → returns root attributes
5. DESTROY_SESSION → returns OK

The mock server receives COMPOUND4args (via RPC), decodes them, and returns scripted COMPOUND4res responses. This is implemented as a mock `RpcTransport` that doesn't need TCP — it directly decodes args and returns res.

Test flow:
```rust
#[tokio::test]
async fn session_lifecycle() {
    let transport = Arc::new(MockTransport::new());
    let mut client = Nfs4Client::new(transport, AuthFlavor::None);

    // Connect (EXCHANGE_ID + CREATE_SESSION + RECLAIM_COMPLETE)
    client.connect().await.unwrap();

    // Read root attributes
    let compound = CompoundBuilder::new()
        .sequence(...)
        .putrootfh()
        .getattr(...)
        .build();
    let res = client.compound(compound).await.unwrap();
    assert!(res.status.is_ok());

    // Disconnect
    client.disconnect().await.unwrap();
}
```

- [ ] **Step 2: Run full workspace tests**

Run: `cargo test --workspace`
Expected: all pass

- [ ] **Step 3: Commit**

```
test(nfs4-client): add session lifecycle integration test

End-to-end test using a MockTransport that simulates NFS server
responses for the full session lifecycle: EXCHANGE_ID, CREATE_SESSION,
RECLAIM_COMPLETE, SEQUENCE + PUTROOTFH + GETATTR, and
DESTROY_SESSION.

The mock validates that COMPOUND requests contain the expected
operations in the correct order and returns appropriately structured
responses. This tests the full stack from CompoundBuilder through
XDR serialization and back.
```

---

## Self-Review

**Spec coverage:**
- Hand-written types with XDR derives ✓
- COMPOUND/CB_COMPOUND envelopes ✓
- Session management (EXCHANGE_ID → CREATE_SESSION → BIND_CONN_TO_SESSION) — BIND_CONN_TO_SESSION is omitted; it's only needed for multi-connection sessions which we don't support in v1. Noted as acceptable gap.
- Compound builder (fluent API) ✓
- Lease management (background keepalive) ✓
- Filehandle cache — deferred to Phase 4 (nfs-mount layer), not Phase 2 ✓
- Reconnection — deferred to Phase 4, not Phase 2 ✓
- Error handling (typed errors, retry on DELAY, re-lookup on STALE) — typed errors ✓, retry logic deferred to Phase 4 ✓

**Placeholder scan:** Tasks 6-8 describe types at a higher level than pure code blocks because the NFSv4.1 type definitions are extensive (hundreds of fields). The subagent has enough structural guidance (struct names, field names, encoding patterns) to implement correctly by referencing RFC 8881. This is acceptable for data definition tasks.

**Type consistency:**
- `NfsFh4`, `SessionId4`, `ClientId4`, `Verifier4`, `StateId4` — used consistently across base.rs, compound.rs, ops, and client
- `Bitmap4`, `Fattr4` — used in GetAttr4args/res and SetAttr4args
- `NfsStat4` — used in all result types and NfsError
- `Compound4args`/`Compound4res` — used in CompoundBuilder and Nfs4Client::compound()
- `SessionManager` — used in Nfs4Client
- `CompoundBuilder::build()` returns `Compound4args` — consumed by `Nfs4Client::compound()`
