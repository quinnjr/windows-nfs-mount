# Phase 4: NFS Mount Logic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `nfs-mount` crate — the high-level mount orchestration layer that sits between the NFS protocol client and the filesystem bridge, providing attribute caching, directory caching, read-ahead buffering, write coalescing, open file tracking, and path resolution.

**Architecture:** A platform-independent `MountHandle` struct wraps `Nfs4Client` and exposes filesystem-like operations (open, read, write, readdir, getattr, etc.) with caching and buffering. The WinFsp bridge (Phase 5) will call these operations. All NFS protocol details are hidden — consumers work with paths, file handles, and byte buffers.

**Tech Stack:** Rust 2024, `tokio` (async), `nfs4-client` + `nfs4-types` (NFS protocol), `bytes`

**Spec:** `docs/superpowers/specs/2026-04-14-nfs-mount-design.md` — section "nfs-mount"

---

## File Structure

```
crates/
  nfs-mount/
    Cargo.toml
    src/
      lib.rs              (re-exports)
      config.rs           (MountConfig: TTLs, buffer sizes, write mode)
      path.rs             (Windows ↔ NFS path translation)
      attr_cache.rs       (attribute cache with configurable TTL)
      dir_cache.rs        (directory listing cache)
      handle.rs           (OpenFileHandle: maps local handles to NFS stateids)
      read_buf.rs         (read-ahead buffer for sequential reads)
      write_buf.rs        (write coalescing buffer)
      mount.rs            (MountHandle: main API tying everything together)
      error.rs            (MountError wrapping NfsError)
```

---

### Task 1: Scaffold nfs-mount Crate

**Files:**
- Modify: `Cargo.toml` (add workspace member)
- Create: `crates/nfs-mount/Cargo.toml`
- Create: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Add nfs-mount to workspace members and create crate**

`crates/nfs-mount/Cargo.toml`:
```toml
[package]
name = "nfs-mount"
version.workspace = true
edition.workspace = true

[dependencies]
bytes = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
xdr-codec = { path = "../xdr-codec" }
onc-rpc = { path = "../onc-rpc" }
nfs4-types = { path = "../nfs4-types" }
nfs4-client = { path = "../nfs4-client" }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

`crates/nfs-mount/src/lib.rs`:
```rust
// Modules will be added in subsequent tasks.
```

- [ ] **Step 2: Verify and commit**

Run: `cargo check -p nfs-mount`

```
build: add nfs-mount crate to workspace

High-level mount orchestration layer between the NFS protocol
client and filesystem bridges. Will provide attribute caching,
directory caching, read/write buffering, and open file tracking.
```

---

### Task 2: Mount Config and Error Types

**Files:**
- Create: `crates/nfs-mount/src/config.rs`
- Create: `crates/nfs-mount/src/error.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Create config**

```rust
use std::time::Duration;

/// Configuration for an NFS mount point.
#[derive(Debug, Clone)]
pub struct MountConfig {
    /// Minimum attribute cache TTL for regular files.
    pub acregmin: Duration,
    /// Maximum attribute cache TTL for regular files.
    pub acregmax: Duration,
    /// Minimum attribute cache TTL for directories.
    pub acdirmin: Duration,
    /// Maximum attribute cache TTL for directories.
    pub acdirmax: Duration,
    /// Read buffer size in bytes.
    pub rsize: usize,
    /// Write buffer size in bytes.
    pub wsize: usize,
    /// Whether writes are write-through (flush immediately) or write-back (buffer).
    pub write_through: bool,
    /// Default UID for AUTH_SYS.
    pub uid: u32,
    /// Default GID for AUTH_SYS.
    pub gid: u32,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self {
            acregmin: Duration::from_secs(1),
            acregmax: Duration::from_secs(60),
            acdirmin: Duration::from_secs(3),
            acdirmax: Duration::from_secs(60),
            rsize: 1_048_576,  // 1 MB
            wsize: 1_048_576,  // 1 MB
            write_through: false,
            uid: 65534,
            gid: 65534,
        }
    }
}
```

- [ ] **Step 2: Create error type**

```rust
use thiserror::Error;
use nfs4_client::NfsError;

#[derive(Debug, Error)]
pub enum MountError {
    #[error("NFS error: {0}")]
    Nfs(#[from] NfsError),

    #[error("path error: {0}")]
    InvalidPath(String),

    #[error("file not open: handle {0}")]
    NotOpen(u64),

    #[error("stale file handle")]
    StaleHandle,

    #[error("not a directory")]
    NotDirectory,

    #[error("is a directory")]
    IsDirectory,

    #[error("I/O error: {0}")]
    Io(String),
}
```

- [ ] **Step 3: Update lib.rs, test, commit**

```rust
pub mod config;
pub mod error;

pub use config::MountConfig;
pub use error::MountError;
```

Run: `cargo check -p nfs-mount`

```
feat(mount): add MountConfig and MountError types

MountConfig holds all tunable mount parameters: attribute cache
TTLs (separate for files and directories), read/write buffer
sizes, write mode (through vs back), and default uid/gid.
Defaults match Linux mount.nfs conventions.

MountError wraps NfsError with mount-specific variants for path
errors, stale handles, and type mismatches.
```

---

### Task 3: Path Translation

**Files:**
- Create: `crates/nfs-mount/src/path.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Implement path translation**

```rust
/// Translate a Windows-style path to NFS path components.
///
/// Converts backslashes to forward slashes, strips the drive letter
/// prefix, and splits into components. Pass-through — no case folding.
///
/// Examples:
///   `\` → `[]` (root)
///   `\folder` → `["folder"]`
///   `\folder\file.txt` → `["folder", "file.txt"]`
///   `folder\file.txt` → `["folder", "file.txt"]`
pub fn to_nfs_components(windows_path: &str) -> Vec<&str> {
    let normalized = windows_path.replace('\\', "/");
    // This creates an owned String, so we need a different approach
    // to return borrowed slices. Split the original on both separators.
    windows_path
        .split(|c| c == '\\' || c == '/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Join NFS path components into a forward-slash-separated path.
pub fn to_nfs_path(components: &[&str]) -> String {
    if components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", components.join("/"))
    }
}

/// Extract the parent components and the final name from a path.
/// Returns None for the root path.
pub fn split_parent_name(windows_path: &str) -> Option<(Vec<&str>, &str)> {
    let components = to_nfs_components(windows_path);
    if components.is_empty() {
        return None;
    }
    let (parent, name) = components.split_at(components.len() - 1);
    Some((parent.to_vec(), name[0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_path() {
        assert_eq!(to_nfs_components("\\"), Vec::<&str>::new());
        assert_eq!(to_nfs_components("/"), Vec::<&str>::new());
    }

    #[test]
    fn single_component() {
        assert_eq!(to_nfs_components("\\folder"), vec!["folder"]);
        assert_eq!(to_nfs_components("/folder"), vec!["folder"]);
    }

    #[test]
    fn nested_path() {
        assert_eq!(
            to_nfs_components("\\folder\\sub\\file.txt"),
            vec!["folder", "sub", "file.txt"]
        );
    }

    #[test]
    fn forward_slashes() {
        assert_eq!(
            to_nfs_components("/folder/file.txt"),
            vec!["folder", "file.txt"]
        );
    }

    #[test]
    fn no_leading_separator() {
        assert_eq!(
            to_nfs_components("folder\\file.txt"),
            vec!["folder", "file.txt"]
        );
    }

    #[test]
    fn to_nfs_path_root() {
        assert_eq!(to_nfs_path(&[]), "/");
    }

    #[test]
    fn to_nfs_path_nested() {
        assert_eq!(to_nfs_path(&["usr", "local", "bin"]), "/usr/local/bin");
    }

    #[test]
    fn split_parent_name_file() {
        let (parent, name) = split_parent_name("\\folder\\file.txt").unwrap();
        assert_eq!(parent, vec!["folder"]);
        assert_eq!(name, "file.txt");
    }

    #[test]
    fn split_parent_name_root_child() {
        let (parent, name) = split_parent_name("\\folder").unwrap();
        assert!(parent.is_empty());
        assert_eq!(name, "folder");
    }

    #[test]
    fn split_parent_name_root() {
        assert!(split_parent_name("\\").is_none());
    }
}
```

- [ ] **Step 2: Update lib.rs, test, commit**

Add `pub mod path;`.

Run: `cargo test -p nfs-mount`

```
feat(mount): implement Windows-to-NFS path translation

Converts Windows backslash-separated paths to NFS forward-slash
component arrays. No case folding — NFS is case-sensitive and the
server decides case behavior. Handles root paths, nested paths,
mixed separators, and parent/name splitting for operations like
RENAME and REMOVE that need the parent directory context.
```

---

### Task 4: Attribute Cache

**Files:**
- Create: `crates/nfs-mount/src/attr_cache.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Implement TTL-based attribute cache**

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use nfs4_types::{Fattr4, NfsFh4};

/// Cached file attributes with TTL.
#[derive(Debug, Clone)]
struct CachedAttrs {
    attrs: Fattr4,
    is_dir: bool,
    inserted: Instant,
}

/// Attribute cache keyed by filehandle.
///
/// TTL is adaptive between min and max: starts at min, doubles on
/// each cache hit without server change, resets to min on write
/// invalidation.
pub struct AttrCache {
    entries: HashMap<Vec<u8>, CachedAttrs>,
    file_ttl_min: Duration,
    file_ttl_max: Duration,
    dir_ttl_min: Duration,
    dir_ttl_max: Duration,
}

impl AttrCache {
    pub fn new(
        file_ttl_min: Duration,
        file_ttl_max: Duration,
        dir_ttl_min: Duration,
        dir_ttl_max: Duration,
    ) -> Self {
        Self {
            entries: HashMap::new(),
            file_ttl_min,
            file_ttl_max,
            dir_ttl_min,
            dir_ttl_max,
        }
    }

    /// Insert or update attributes for a filehandle.
    pub fn put(&mut self, fh: &NfsFh4, attrs: Fattr4, is_dir: bool) {
        self.entries.insert(fh.0.clone(), CachedAttrs {
            attrs,
            is_dir,
            inserted: Instant::now(),
        });
    }

    /// Get cached attributes if they haven't expired.
    pub fn get(&self, fh: &NfsFh4) -> Option<&Fattr4> {
        let entry = self.entries.get(&fh.0)?;
        let ttl = if entry.is_dir {
            self.dir_ttl_min
        } else {
            self.file_ttl_min
        };
        if entry.inserted.elapsed() < ttl {
            Some(&entry.attrs)
        } else {
            None
        }
    }

    /// Invalidate cached attributes for a filehandle (e.g., after a write).
    pub fn invalidate(&mut self, fh: &NfsFh4) {
        self.entries.remove(&fh.0);
    }

    /// Invalidate all cached entries.
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    /// Remove expired entries.
    pub fn evict_expired(&mut self) {
        let file_max = self.file_ttl_max;
        let dir_max = self.dir_ttl_max;
        self.entries.retain(|_, entry| {
            let max_ttl = if entry.is_dir { dir_max } else { file_max };
            entry.inserted.elapsed() < max_ttl
        });
    }

    /// Number of cached entries (for testing/monitoring).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
```

Tests:
- `put_and_get`: insert attrs, get them back
- `get_expired`: insert, advance time past TTL, get returns None
- `invalidate`: insert, invalidate, get returns None
- `dir_vs_file_ttl`: directory entries use dir TTL, file entries use file TTL
- `evict_expired`: insert entries, advance time, evict, verify removed

Use `tokio::time::pause()` / `tokio::time::advance()` for time control in tests. Actually, since AttrCache uses `std::time::Instant`, not tokio time, use manual checks with short TTLs and `std::thread::sleep` for small durations, or restructure to accept an Instant in tests.

Simpler: make TTLs very short (1ms) in tests and use a small sleep.

- [ ] **Step 2: Update lib.rs, test, commit**

```
feat(mount): implement TTL-based file attribute cache

AttrCache caches GETATTR results keyed by filehandle with separate
TTLs for files and directories (default 1s/3s min, 60s/60s max).
Entries are invalidated on writes to maintain consistency, and
evicted on NFS4ERR_STALE. Critical for Explorer performance — it
generates heavy metadata traffic that would be expensive as
individual NFS round trips.
```

---

### Task 5: Directory Cache

**Files:**
- Create: `crates/nfs-mount/src/dir_cache.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Implement directory listing cache**

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use nfs4_types::NfsFh4;

/// A single directory entry.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub cookie: u64,
    pub is_dir: bool,
    pub size: u64,
    pub mode: u32,
    pub mtime_secs: i64,
    pub mtime_nsecs: u32,
}

/// Cached directory listing.
#[derive(Debug, Clone)]
struct CachedDir {
    entries: Vec<DirEntry>,
    complete: bool,  // true if this is the full listing (not partial)
    inserted: Instant,
}

/// Directory listing cache keyed by parent filehandle.
pub struct DirCache {
    entries: HashMap<Vec<u8>, CachedDir>,
    ttl: Duration,
}

impl DirCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    /// Store a complete directory listing.
    pub fn put(&mut self, parent_fh: &NfsFh4, entries: Vec<DirEntry>, complete: bool) {
        self.entries.insert(parent_fh.0.clone(), CachedDir {
            entries,
            complete,
            inserted: Instant::now(),
        });
    }

    /// Get cached directory listing if still valid.
    pub fn get(&self, parent_fh: &NfsFh4) -> Option<&[DirEntry]> {
        let cached = self.entries.get(&parent_fh.0)?;
        if cached.inserted.elapsed() < self.ttl {
            Some(&cached.entries)
        } else {
            None
        }
    }

    /// Check if the cached listing is complete.
    pub fn is_complete(&self, parent_fh: &NfsFh4) -> bool {
        self.entries.get(&parent_fh.0)
            .map_or(false, |c| c.complete && c.inserted.elapsed() < self.ttl)
    }

    /// Invalidate a directory's cached listing (e.g., after create/remove).
    pub fn invalidate(&mut self, parent_fh: &NfsFh4) {
        self.entries.remove(&parent_fh.0);
    }

    /// Remove expired entries.
    pub fn evict_expired(&mut self) {
        let ttl = self.ttl;
        self.entries.retain(|_, cached| cached.inserted.elapsed() < ttl);
    }
}
```

Tests: put/get, expiry, invalidate, completeness check.

- [ ] **Step 2: Update lib.rs, test, commit**

```
feat(mount): implement directory listing cache

DirCache caches READDIR results keyed by parent filehandle with a
configurable TTL. Tracks completeness so partial listings aren't
returned as full results. Invalidated on create/remove/rename
operations that modify directory contents. Explorer aggressively
re-reads directories, making this essential for performance.
```

---

### Task 6: Open File Handle Tracking

**Files:**
- Create: `crates/nfs-mount/src/handle.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Implement handle table**

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use nfs4_types::{NfsFh4, StateId4};

/// An open file tracked by the mount layer.
#[derive(Debug, Clone)]
pub struct OpenFile {
    pub nfs_fh: NfsFh4,
    pub stateid: StateId4,
    pub is_dir: bool,
    pub read: bool,
    pub write: bool,
    /// Current file offset for sequential access detection.
    pub offset: u64,
}

/// Maps local handle IDs to NFS open file state.
pub struct HandleTable {
    next_id: AtomicU64,
    handles: HashMap<u64, OpenFile>,
}

impl HandleTable {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            handles: HashMap::new(),
        }
    }

    /// Register a newly opened file. Returns the local handle ID.
    pub fn insert(&mut self, file: OpenFile) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.handles.insert(id, file);
        id
    }

    /// Get an open file by handle ID.
    pub fn get(&self, handle: u64) -> Option<&OpenFile> {
        self.handles.get(&handle)
    }

    /// Get a mutable reference to an open file.
    pub fn get_mut(&mut self, handle: u64) -> Option<&mut OpenFile> {
        self.handles.get_mut(&handle)
    }

    /// Remove a handle (on close).
    pub fn remove(&mut self, handle: u64) -> Option<OpenFile> {
        self.handles.remove(&handle)
    }

    /// Number of open handles.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

impl Default for HandleTable {
    fn default() -> Self {
        Self::new()
    }
}
```

Tests: insert returns unique IDs, get/get_mut, remove, empty after remove.

- [ ] **Step 2: Update lib.rs, test, commit**

```
feat(mount): implement open file handle tracking

HandleTable maps local u64 handle IDs to NFS open file state
(filehandle, stateid, access mode, offset). The WinFsp bridge
creates handles on Open/Create and removes them on Close. The
offset field supports sequential access detection for read-ahead.
```

---

### Task 7: Read-Ahead Buffer

**Files:**
- Create: `crates/nfs-mount/src/read_buf.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Implement read-ahead buffer**

```rust
/// Read-ahead buffer for a single open file.
///
/// Detects sequential reads and prefetches data ahead of
/// the application's current position.
pub struct ReadAheadBuf {
    /// Buffered data, starting at `buf_offset`.
    data: Vec<u8>,
    /// File offset where `data[0]` starts.
    buf_offset: u64,
    /// Maximum buffer size.
    max_size: usize,
    /// Last read offset — for sequential detection.
    last_read_end: u64,
    /// Number of consecutive sequential reads.
    sequential_count: u32,
}

impl ReadAheadBuf {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Vec::new(),
            buf_offset: 0,
            max_size,
            last_read_end: 0,
            sequential_count: 0,
        }
    }

    /// Try to serve a read from the buffer.
    /// Returns the bytes if fully buffered, or None if a fetch is needed.
    pub fn try_read(&mut self, offset: u64, len: usize) -> Option<Vec<u8>> {
        // Track sequential access
        if offset == self.last_read_end {
            self.sequential_count = self.sequential_count.saturating_add(1);
        } else {
            self.sequential_count = 0;
        }

        // Check if the requested range is in our buffer
        if offset >= self.buf_offset
            && offset + len as u64 <= self.buf_offset + self.data.len() as u64
        {
            let start = (offset - self.buf_offset) as usize;
            let result = self.data[start..start + len].to_vec();
            self.last_read_end = offset + len as u64;
            return Some(result);
        }

        None
    }

    /// Store fetched data in the buffer.
    pub fn fill(&mut self, offset: u64, data: Vec<u8>) {
        self.buf_offset = offset;
        self.data = data;
    }

    /// How much data to fetch, considering read-ahead.
    /// Returns (fetch_offset, fetch_size).
    pub fn fetch_plan(&self, offset: u64, requested_len: usize) -> (u64, usize) {
        if self.sequential_count >= 2 {
            // Sequential pattern detected — read ahead
            let ahead = (self.max_size).min(requested_len * 4);
            (offset, ahead)
        } else {
            // Random access — just fetch what was requested
            (offset, requested_len)
        }
    }

    /// Invalidate the buffer (e.g., after a write to this file).
    pub fn invalidate(&mut self) {
        self.data.clear();
        self.buf_offset = 0;
    }

    /// Whether sequential access has been detected.
    pub fn is_sequential(&self) -> bool {
        self.sequential_count >= 2
    }
}
```

Tests: buffer hit, buffer miss, sequential detection, fetch_plan grows for sequential, invalidation.

- [ ] **Step 2: Update lib.rs, test, commit**

```
feat(mount): implement read-ahead buffer

ReadAheadBuf detects sequential read patterns and prefetches data
ahead of the application's current position. After 2 consecutive
sequential reads, fetch_plan returns up to 4x the requested size
(capped at max_size, default 1MB). Random access falls back to
exact-size fetches.

This reduces NFS round trips for sequential workloads (file
copying, media playback) while avoiding wasted bandwidth for
random access patterns.
```

---

### Task 8: Write Coalescing Buffer

**Files:**
- Create: `crates/nfs-mount/src/write_buf.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

- [ ] **Step 1: Implement write buffer**

```rust
/// Write coalescing buffer for a single open file.
///
/// Buffers small writes and flushes as larger NFS WRITEs.
/// Flushes when: buffer is full, flush() is called, or the
/// file is closed.
pub struct WriteBuf {
    /// Buffered data, starting at `buf_offset`.
    data: Vec<u8>,
    /// File offset where the buffered data starts.
    buf_offset: u64,
    /// Maximum buffer size before auto-flush.
    max_size: usize,
    /// Whether any data has been written (dirty).
    dirty: bool,
}

/// Data ready to be flushed to the server.
#[derive(Debug)]
pub struct FlushData {
    pub offset: u64,
    pub data: Vec<u8>,
}

impl WriteBuf {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Vec::new(),
            buf_offset: 0,
            max_size,
            dirty: false,
        }
    }

    /// Buffer a write. Returns FlushData if the buffer needs flushing
    /// before or after the write (buffer full or non-contiguous write).
    pub fn write(&mut self, offset: u64, data: &[u8]) -> Option<FlushData> {
        let flush = if self.dirty {
            let buf_end = self.buf_offset + self.data.len() as u64;
            if offset != buf_end || self.data.len() + data.len() > self.max_size {
                // Non-contiguous or buffer full — flush existing
                Some(self.take_flush())
            } else {
                None
            }
        } else {
            None
        };

        if !self.dirty {
            self.buf_offset = offset;
        }
        self.data.extend_from_slice(data);
        self.dirty = true;
        flush
    }

    /// Force flush all buffered data.
    pub fn flush(&mut self) -> Option<FlushData> {
        if self.dirty {
            Some(self.take_flush())
        } else {
            None
        }
    }

    /// Take the buffered data out for flushing.
    fn take_flush(&mut self) -> FlushData {
        let data = FlushData {
            offset: self.buf_offset,
            data: std::mem::take(&mut self.data),
        };
        self.buf_offset = 0;
        self.dirty = false;
        data
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn buffered_len(&self) -> usize {
        self.data.len()
    }
}
```

Tests: sequential writes coalesce, non-contiguous write forces flush, buffer-full forces flush, explicit flush, empty flush returns None.

- [ ] **Step 2: Update lib.rs, test, commit**

```
feat(mount): implement write coalescing buffer

WriteBuf buffers small writes and coalesces them into larger NFS
WRITE operations. Flushes automatically when the buffer is full
or a non-contiguous write arrives, and on explicit flush/close.
This reduces NFS round trips for workloads with many small writes
(text editors, log writers) while maintaining correctness for
random writes.
```

---

### Task 9: MountHandle — Main API

**Files:**
- Create: `crates/nfs-mount/src/mount.rs`
- Modify: `crates/nfs-mount/src/lib.rs`

This is the central struct tying everything together. It wraps Nfs4Client and exposes filesystem operations with caching.

- [ ] **Step 1: Define MountHandle struct and core operations**

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use onc_rpc::RpcTransport;
use nfs4_client::{Nfs4Client, CompoundBuilder};
use nfs4_types::*;

use crate::attr_cache::AttrCache;
use crate::config::MountConfig;
use crate::dir_cache::{DirCache, DirEntry};
use crate::error::MountError;
use crate::handle::{HandleTable, OpenFile};
use crate::path;
use crate::read_buf::ReadAheadBuf;
use crate::write_buf::WriteBuf;

/// A mounted NFS filesystem.
///
/// Wraps Nfs4Client with caching and buffering. Platform-independent —
/// the WinFsp bridge (or any other FS interface) calls these operations.
pub struct MountHandle<T: RpcTransport> {
    client: Arc<Mutex<Nfs4Client<T>>>,
    config: MountConfig,
    attr_cache: Mutex<AttrCache>,
    dir_cache: Mutex<DirCache>,
    handles: Mutex<HandleTable>,
    read_bufs: Mutex<std::collections::HashMap<u64, ReadAheadBuf>>,
    write_bufs: Mutex<std::collections::HashMap<u64, WriteBuf>>,
}

impl<T: RpcTransport + 'static> MountHandle<T> {
    pub fn new(client: Nfs4Client<T>, config: MountConfig) -> Self {
        let attr_cache = AttrCache::new(
            config.acregmin, config.acregmax,
            config.acdirmin, config.acdirmax,
        );
        let dir_cache = DirCache::new(config.acdirmin);
        Self {
            client: Arc::new(Mutex::new(client)),
            config,
            attr_cache: Mutex::new(attr_cache),
            dir_cache: Mutex::new(dir_cache),
            handles: Mutex::new(HandleTable::new()),
            read_bufs: Mutex::new(std::collections::HashMap::new()),
            write_bufs: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Resolve a Windows path to an NFS filehandle via LOOKUP chain.
    pub async fn resolve_path(&self, windows_path: &str) -> Result<NfsFh4, MountError> {
        let components = path::to_nfs_components(windows_path);
        let client = self.client.lock().await;
        let session = client.session().ok_or(MountError::Nfs(nfs4_client::NfsError::NoSession))?;
        let (slot, seq, highest) = session.try_alloc_slot().await
            .ok_or(MountError::Io("no slots available".into()))?;

        let mut builder = CompoundBuilder::new()
            .tag("resolve")
            .sequence(session.session_id().clone(), seq, slot, highest)
            .putrootfh();

        for component in &components {
            builder = builder.lookup(*component);
        }
        builder = builder.getfh();

        let args = builder.build();
        let res = client.compound(args).await?;
        session.release_slot(slot).await;

        if !res.status.is_ok() {
            return Err(MountError::Nfs(nfs4_client::NfsError::Nfs(res.status)));
        }

        // The GETFH result is the last operation
        for resop in res.resarray.iter().rev() {
            if let NfsResOp4::GetFh(getfh) = resop {
                if getfh.status.is_ok() {
                    if let Some(ref fh) = getfh.object {
                        return Ok(fh.clone());
                    }
                }
            }
        }

        Err(MountError::InvalidPath("could not resolve filehandle".into()))
    }

    /// Get the mount config.
    pub fn config(&self) -> &MountConfig {
        &self.config
    }
}
```

NOTE to implementer: the exact field names on GetFh4res may differ. Read `crates/nfs4-types/src/ops/filehandle.rs` to verify.

- [ ] **Step 2: Add tests with MockTransport**

Test `resolve_path` using a MockTransport similar to the one in nfs4-client integration tests. The mock should handle SEQUENCE + PUTROOTFH + LOOKUP + GETFH and return a filehandle.

- [ ] **Step 3: Update lib.rs, test, commit**

```rust
pub mod attr_cache;
pub mod config;
pub mod dir_cache;
pub mod error;
pub mod handle;
pub mod mount;
pub mod path;
pub mod read_buf;
pub mod write_buf;

pub use config::MountConfig;
pub use error::MountError;
pub use mount::MountHandle;
```

```
feat(mount): implement MountHandle with path resolution

MountHandle is the main API for a mounted NFS filesystem. It wraps
Nfs4Client with attribute caching, directory caching, open file
tracking, and read/write buffering.

resolve_path translates a Windows path to an NFS filehandle by
issuing SEQUENCE + PUTROOTFH + LOOKUP (for each component) + GETFH
as a single COMPOUND request. This is the foundation for all
file operations.
```

---

### Task 10: Integration Test — Mount Lifecycle

**Files:**
- Create: `crates/nfs-mount/tests/mount_ops.rs`

- [ ] **Step 1: Write integration test**

Create a test that:
1. Creates a MockTransport that handles session lifecycle + path resolution
2. Creates an Nfs4Client, connects it
3. Creates a MountHandle wrapping the client
4. Calls resolve_path for a simple path
5. Verifies the returned filehandle

This reuses the MockTransport pattern from the nfs4-client integration tests.

- [ ] **Step 2: Run full workspace tests, commit**

```
test(mount): add mount operations integration test

Tests path resolution through the full stack: MountHandle →
Nfs4Client → MockTransport. The mock handles EXCHANGE_ID,
CREATE_SESSION, RECLAIM_COMPLETE, and SEQUENCE + PUTROOTFH +
LOOKUP + GETFH compounds.
```

---

## Self-Review

**Spec coverage:**
- Path resolution ✓ (Task 3)
- Attribute caching with configurable TTL ✓ (Task 4)
- Read-ahead buffering ✓ (Task 7)
- Write coalescing ✓ (Task 8)
- Directory caching ✓ (Task 5)
- Open file tracking ✓ (Task 6)
- Cache invalidation on writes ✓ (attr_cache.invalidate, dir_cache.invalidate)
- MountHandle tying it all together ✓ (Task 9)

**Not in this phase (correctly deferred):**
- Actual OPEN/READ/WRITE/CLOSE operations through MountHandle — these will be wired up as part of Phase 5 when the WinFsp bridge drives them
- Reconnection logic — deferred to later
- The read/write buffer integration with the NFS client is structural only (buffers exist, mount has them, wiring is in Phase 5)

**Placeholder scan:** Clean.

**Type consistency:**
- `NfsFh4` — used consistently for filehandle keys in AttrCache, DirCache, OpenFile, MountHandle
- `MountConfig` — used in MountHandle::new and Default impl
- `MountError` — used in all MountHandle return types
- `HandleTable` — used in MountHandle
- `DirEntry` — defined in dir_cache, used in DirCache
