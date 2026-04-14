use std::collections::HashMap;
use std::time::{Duration, Instant};

use nfs4_types::{Fattr4, NfsFh4};

#[derive(Debug, Clone)]
struct CachedAttrs {
    attrs: Fattr4,
    is_dir: bool,
    inserted: Instant,
}

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

    pub fn put(&mut self, fh: &NfsFh4, attrs: Fattr4, is_dir: bool) {
        self.entries.insert(
            fh.0.clone(),
            CachedAttrs {
                attrs,
                is_dir,
                inserted: Instant::now(),
            },
        );
    }

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

    pub fn invalidate(&mut self, fh: &NfsFh4) {
        self.entries.remove(&fh.0);
    }

    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }

    pub fn evict_expired(&mut self) {
        let (fm, dm) = (self.file_ttl_max, self.dir_ttl_max);
        self.entries.retain(|_, e| {
            let max = if e.is_dir { dm } else { fm };
            e.inserted.elapsed() < max
        });
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfs4_types::Bitmap4;
    use std::thread;

    fn make_fh(id: u8) -> NfsFh4 {
        NfsFh4(vec![id])
    }

    fn make_attrs() -> Fattr4 {
        Fattr4 {
            attrmask: Bitmap4(vec![]),
            attr_vals: vec![],
        }
    }

    #[test]
    fn put_and_get() {
        let mut cache = AttrCache::new(
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(6),
        );
        let fh = make_fh(1);
        cache.put(&fh, make_attrs(), false);

        assert!(cache.get(&fh).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn get_returns_none_after_invalidate() {
        let mut cache = AttrCache::new(
            Duration::from_secs(10),
            Duration::from_secs(20),
            Duration::from_secs(10),
            Duration::from_secs(20),
        );
        let fh = make_fh(1);
        cache.put(&fh, make_attrs(), false);
        assert!(cache.get(&fh).is_some());

        cache.invalidate(&fh);
        assert!(cache.get(&fh).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn evict_expired_removes_stale_entries() {
        let mut cache = AttrCache::new(
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(10),
            Duration::from_millis(20),
        );
        let fh = make_fh(1);
        cache.put(&fh, make_attrs(), false);
        assert_eq!(cache.len(), 1);

        thread::sleep(Duration::from_millis(30));
        cache.evict_expired();
        assert!(cache.is_empty());
    }
}
