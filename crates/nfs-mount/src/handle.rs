use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use nfs4_types::{NfsFh4, StateId4};

#[derive(Debug, Clone)]
pub struct OpenFile {
    pub nfs_fh: NfsFh4,
    pub stateid: StateId4,
    pub is_dir: bool,
    pub read: bool,
    pub write: bool,
    pub offset: u64,
}

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

    pub fn insert(&mut self, file: OpenFile) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.handles.insert(id, file);
        id
    }

    pub fn get(&self, handle: u64) -> Option<&OpenFile> {
        self.handles.get(&handle)
    }

    pub fn get_mut(&mut self, handle: u64) -> Option<&mut OpenFile> {
        self.handles.get_mut(&handle)
    }

    pub fn remove(&mut self, handle: u64) -> Option<OpenFile> {
        self.handles.remove(&handle)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use nfs4_types::StateIdOther;

    fn make_open_file(id: u8) -> OpenFile {
        OpenFile {
            nfs_fh: NfsFh4(vec![id]),
            stateid: StateId4 {
                seqid: 1,
                other: StateIdOther([0; 12]),
            },
            is_dir: false,
            read: true,
            write: false,
            offset: 0,
        }
    }

    #[test]
    fn insert_returns_unique_ids() {
        let mut table = HandleTable::new();
        let id1 = table.insert(make_open_file(1));
        let id2 = table.insert(make_open_file(2));
        let id3 = table.insert(make_open_file(3));

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn get_and_get_mut() {
        let mut table = HandleTable::new();
        let id = table.insert(make_open_file(1));

        assert!(table.get(id).is_some());
        assert_eq!(table.get(id).unwrap().nfs_fh.0, vec![1]);

        table.get_mut(id).unwrap().offset = 42;
        assert_eq!(table.get(id).unwrap().offset, 42);
    }

    #[test]
    fn remove() {
        let mut table = HandleTable::new();
        let id = table.insert(make_open_file(1));
        assert!(!table.is_empty());

        let removed = table.remove(id);
        assert!(removed.is_some());
        assert!(table.is_empty());
        assert!(table.get(id).is_none());
    }
}
