use crate::Instant;
use ahash::AHasher;
use dashmap::DashMap;
use std::{
    hash::BuildHasherDefault,
    sync::{Arc, RwLock},
};

/// Keeps track of open snapshots
pub struct SnapshotTrackerInner {
    data: DashMap<Instant, usize, BuildHasherDefault<AHasher>>,
    safety_gap: u64,
    lowest_freed_instant: RwLock<Instant>,
}

#[derive(Clone, Default)]
pub struct SnapshotTracker(Arc<SnapshotTrackerInner>);

impl std::ops::Deref for SnapshotTracker {
    type Target = SnapshotTrackerInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for SnapshotTrackerInner {
    fn default() -> Self {
        Self {
            data: DashMap::default(),
            safety_gap: 100,
            lowest_freed_instant: RwLock::default(),
        }
    }
}

impl SnapshotTrackerInner {
    pub fn open(&self, seqno: Instant) {
        log::trace!("open snapshot {seqno}");

        self.data
            .entry(seqno)
            .and_modify(|x| {
                *x += 1;
            })
            .or_insert(1);
    }

    pub fn close(&self, seqno: Instant) {
        log::trace!("close snapshot {seqno}");

        self.data.alter(&seqno, |_, v| v - 1);

        if seqno % self.safety_gap == 0 {
            self.gc(seqno);
        }
    }

    pub fn get_seqno_safe_to_gc(&self) -> Instant {
        *self.lowest_freed_instant.read().expect("lock is poisoned")
    }

    fn gc(&self, watermark: Instant) {
        log::trace!("snapshot gc, watermark={watermark}");

        let mut lock = self.lowest_freed_instant.write().expect("lock is poisoned");

        let seqno_threshold = watermark - self.safety_gap;

        let mut lowest_retained = 0;

        self.data.retain(|&k, v| {
            let should_be_retained = *v > 0 || k > seqno_threshold;

            if should_be_retained {
                lowest_retained = match lowest_retained {
                    0 => k,
                    lo => lo.min(k),
                };
            }

            should_be_retained
        });

        log::trace!("lowest retained snapshot={lowest_retained}");

        *lock = match *lock {
            0 => lowest_retained.saturating_sub(1),
            lo => lo.max(lowest_retained.saturating_sub(1)),
        };

        log::trace!("gc threshold now {}", *lock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn seqno_tracker_reverse_order() {
        let mut map = SnapshotTrackerInner::default();
        map.safety_gap = 5;

        map.open(1);
        map.open(2);
        map.open(3);
        map.open(4);
        map.open(5);
        map.open(6);
        map.open(7);
        map.open(8);
        map.open(9);
        map.open(10);

        map.close(10);
        map.close(9);
        map.close(8);
        map.close(7);
        map.close(6);
        map.close(5);
        map.close(4);
        map.close(3);
        map.close(2);
        map.close(1);

        map.open(11);
        map.close(11);
        map.gc(11);

        assert_eq!(map.get_seqno_safe_to_gc(), 6);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn seqno_tracker_simple() {
        let mut map = SnapshotTrackerInner::default();
        map.safety_gap = 5;

        map.open(1);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.open(2);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.open(3);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.open(4);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.open(5);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.open(6);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.close(1);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.close(2);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.close(3);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.close(4);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.close(5);
        assert_eq!(map.get_seqno_safe_to_gc(), 0);

        map.close(6);
        map.gc(6);
        assert_eq!(map.get_seqno_safe_to_gc(), 1);

        map.open(7);
        map.close(7);
        map.gc(7);
        assert_eq!(map.get_seqno_safe_to_gc(), 2);

        map.open(8);
        map.open(9);
        map.close(9);
        map.gc(9);
        assert_eq!(map.get_seqno_safe_to_gc(), 4);

        map.close(8);
        map.gc(8);
        assert_eq!(map.get_seqno_safe_to_gc(), 4);
    }
}
