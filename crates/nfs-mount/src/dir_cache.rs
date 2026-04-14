use std::collections::HashMap;
use std::time::{Duration, Instant};

use nfs4_types::NfsFh4;

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

struct CachedDir {
    entries: Vec<DirEntry>,
    complete: bool,
    inserted: Instant,
}

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

    pub fn put(
        &mut self,
        parent_fh: &NfsFh4,
        entries: Vec<DirEntry>,
        complete: bool,
    ) {
        self.entries.insert(
            parent_fh.0.clone(),
            CachedDir {
                entries,
                complete,
                inserted: Instant::now(),
            },
        );
    }

    pub fn get(&self, parent_fh: &NfsFh4) -> Option<&[DirEntry]> {
        let cached = self.entries.get(&parent_fh.0)?;
        if cached.inserted.elapsed() < self.ttl {
            Some(&cached.entries)
        } else {
            None
        }
    }

    pub fn is_complete(&self, parent_fh: &NfsFh4) -> bool {
        self.entries
            .get(&parent_fh.0)
            .map_or(false, |c| c.complete && c.inserted.elapsed() < self.ttl)
    }

    pub fn invalidate(&mut self, parent_fh: &NfsFh4) {
        self.entries.remove(&parent_fh.0);
    }

    pub fn evict_expired(&mut self) {
        let ttl = self.ttl;
        self.entries.retain(|_, c| c.inserted.elapsed() < ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fh(id: u8) -> NfsFh4 {
        NfsFh4(vec![id])
    }

    fn make_entries() -> Vec<DirEntry> {
        vec![
            DirEntry {
                name: "file.txt".into(),
                cookie: 1,
                is_dir: false,
                size: 100,
                mode: 0o644,
                mtime_secs: 1000,
                mtime_nsecs: 0,
            },
            DirEntry {
                name: "subdir".into(),
                cookie: 2,
                is_dir: true,
                size: 0,
                mode: 0o755,
                mtime_secs: 2000,
                mtime_nsecs: 0,
            },
        ]
    }

    #[test]
    fn put_and_get() {
        let mut cache = DirCache::new(Duration::from_secs(10));
        let fh = make_fh(1);
        cache.put(&fh, make_entries(), true);

        let entries = cache.get(&fh).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "file.txt");
        assert_eq!(entries[1].name, "subdir");
    }

    #[test]
    fn completeness_check() {
        let mut cache = DirCache::new(Duration::from_secs(10));
        let fh = make_fh(1);

        cache.put(&fh, make_entries(), false);
        assert!(!cache.is_complete(&fh));

        cache.put(&fh, make_entries(), true);
        assert!(cache.is_complete(&fh));
    }

    #[test]
    fn invalidate() {
        let mut cache = DirCache::new(Duration::from_secs(10));
        let fh = make_fh(1);
        cache.put(&fh, make_entries(), true);
        assert!(cache.get(&fh).is_some());

        cache.invalidate(&fh);
        assert!(cache.get(&fh).is_none());
    }
}
