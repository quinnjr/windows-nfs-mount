use tokio::sync::Mutex;

use nfs4_types::{ClientId4, SequenceId4, SessionId4, SlotId4};

/// Manages NFSv4.1 session state: session ID, slot allocation,
/// and per-slot sequence IDs.
pub struct SessionManager {
    session_id: SessionId4,
    client_id: ClientId4,
    lease_time: u32,
    slots: Mutex<SlotTable>,
}

struct SlotTable {
    /// Next sequence ID for each slot.
    sequence_ids: Vec<u32>,
    /// Whether each slot is currently in use.
    in_use: Vec<bool>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(
        session_id: SessionId4,
        client_id: ClientId4,
        max_slots: u32,
        lease_time: u32,
    ) -> Self {
        let num_slots = max_slots as usize;
        Self {
            session_id,
            client_id,
            lease_time,
            slots: Mutex::new(SlotTable {
                // Sequence IDs start at 1 per RFC 8881
                sequence_ids: vec![1; num_slots],
                in_use: vec![false; num_slots],
            }),
        }
    }

    pub fn session_id(&self) -> &SessionId4 {
        &self.session_id
    }

    pub fn client_id(&self) -> ClientId4 {
        self.client_id
    }

    pub fn lease_time(&self) -> u32 {
        self.lease_time
    }

    /// Allocate a slot for a new request.
    /// Returns (slot_id, sequence_id, highest_slot_id).
    /// Returns None if all slots are in use.
    pub async fn try_alloc_slot(&self) -> Option<(SlotId4, SequenceId4, SlotId4)> {
        let mut slots = self.slots.lock().await;
        let highest = (slots.in_use.len() - 1) as u32;
        for (i, in_use) in slots.in_use.iter_mut().enumerate() {
            if !*in_use {
                *in_use = true;
                let seq_id = slots.sequence_ids[i];
                return Some((i as u32, seq_id, highest));
            }
        }
        None
    }

    /// Release a slot after the request completes, incrementing its
    /// sequence ID.
    pub async fn release_slot(&self, slot_id: SlotId4) {
        let mut slots = self.slots.lock().await;
        let idx = slot_id as usize;
        if idx < slots.in_use.len() {
            slots.in_use[idx] = false;
            slots.sequence_ids[idx] += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session() -> SessionManager {
        SessionManager::new(
            SessionId4([0xAA; 16]),
            ClientId4(12345),
            4,  // 4 slots
            90, // 90 second lease
        )
    }

    #[tokio::test]
    async fn alloc_and_release_slot() {
        let sm = make_session();
        let (slot, seq, highest) = sm.try_alloc_slot().await.unwrap();
        assert_eq!(slot, 0);
        assert_eq!(seq, 1); // starts at 1
        assert_eq!(highest, 3); // 4 slots, highest = 3
        sm.release_slot(slot).await;
    }

    #[tokio::test]
    async fn sequence_id_increments() {
        let sm = make_session();

        // First allocation: seq=1
        let (slot, seq1, _) = sm.try_alloc_slot().await.unwrap();
        assert_eq!(seq1, 1);
        sm.release_slot(slot).await;

        // Second allocation of same slot: seq=2
        let (slot2, seq2, _) = sm.try_alloc_slot().await.unwrap();
        assert_eq!(slot2, 0); // same slot (first free)
        assert_eq!(seq2, 2);
        sm.release_slot(slot2).await;
    }

    #[tokio::test]
    async fn multiple_slots() {
        let sm = make_session();

        let (s0, _, _) = sm.try_alloc_slot().await.unwrap();
        let (s1, _, _) = sm.try_alloc_slot().await.unwrap();
        let (s2, _, _) = sm.try_alloc_slot().await.unwrap();

        assert_eq!(s0, 0);
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);

        sm.release_slot(s0).await;
        sm.release_slot(s1).await;
        sm.release_slot(s2).await;
    }

    #[tokio::test]
    async fn all_slots_exhausted() {
        let sm = make_session();

        // Allocate all 4 slots
        for _ in 0..4 {
            assert!(sm.try_alloc_slot().await.is_some());
        }

        // Fifth allocation should fail
        assert!(sm.try_alloc_slot().await.is_none());
    }

    #[tokio::test]
    async fn accessors() {
        let sm = make_session();
        assert_eq!(sm.session_id(), &SessionId4([0xAA; 16]));
        assert_eq!(sm.client_id(), ClientId4(12345));
        assert_eq!(sm.lease_time(), 90);
    }
}
