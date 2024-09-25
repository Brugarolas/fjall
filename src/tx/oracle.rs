use crate::snapshot_tracker::SnapshotTracker;
use crate::Instant;

use super::conflict_manager::ConflictChecker;
use lsm_tree::SequenceNumberCounter;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

pub enum CommitOutcome<E> {
    Ok,
    Aborted(E),
    Conflicted,
}

pub struct Oracle {
    pub(super) write_serialize_lock: Mutex<BTreeMap<u64, ConflictChecker>>,
    pub(super) seqno: SequenceNumberCounter,
    pub(super) snapshot_tracker: SnapshotTracker,
}

impl Oracle {
    #[allow(clippy::nursery)]
    pub(super) fn with_commit<E, F: FnOnce() -> Result<(), E>>(
        &self,
        instant: Instant,
        conflict_checker: ConflictChecker,
        f: F,
    ) -> Result<CommitOutcome<E>, Error> {
        let mut committed_txns = self
            .write_serialize_lock
            .lock()
            .map_err(|_| Error::Poisoned)?;

        // If the committed_txn.ts is less than Instant that implies that the
        // committed_txn finished before the current transaction started.
        // We don't need to check for conflict in that case.
        // This change assumes linearizability. Lack of linearizability could
        // cause the read ts of a new txn to be lower than the commit ts of
        // a txn before it.
        let conflicted =
            committed_txns
                .range((instant + 1)..)
                .any(|(_ts, other_conflict_checker)| {
                    conflict_checker.has_conflict(other_conflict_checker)
                });

        self.snapshot_tracker.close(instant);
        let safe_to_gc = self.snapshot_tracker.get_seqno_safe_to_gc();
        committed_txns.retain(|ts, _| *ts > safe_to_gc);

        if conflicted {
            return Ok(CommitOutcome::Conflicted);
        }

        if let Err(e) = f() {
            return Ok(CommitOutcome::Aborted(e));
        }

        committed_txns.insert(self.seqno.get(), conflict_checker);

        Ok(CommitOutcome::Ok)
    }

    pub(super) fn write_serialize_lock(
        &self,
    ) -> Result<MutexGuard<BTreeMap<u64, ConflictChecker>>, Error> {
        self.write_serialize_lock.lock().map_err(Error::from)
    }
}

#[derive(Debug)]
pub enum Error {
    /// Poisoned write serialization lock
    Poisoned,
}

impl std::error::Error for Error {}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_value: PoisonError<T>) -> Self {
        Self::Poisoned
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, PartitionCreateOptions, TxKeyspace, TxPartitionHandle};

    #[allow(clippy::unwrap_used)]
    #[test]
    fn oracle_committed_txns_does_not_leak() {
        let tmpdir = tempfile::tempdir().unwrap();
        let ks = Config::new(tmpdir.path()).open_transactional().unwrap();

        let part = ks
            .open_partition("foo", PartitionCreateOptions::default())
            .unwrap();

        for _ in 0..250 {
            run_tx(&ks, &part).unwrap();
        }

        assert!(dbg!(ks.orc.write_serialize_lock.lock().unwrap().len()) < 200);

        for _ in 0..200 {
            run_tx(&ks, &part).unwrap();
        }

        assert!(dbg!(ks.orc.write_serialize_lock.lock().unwrap().len()) < 200);
    }

    fn run_tx(ks: &TxKeyspace, part: &TxPartitionHandle) -> Result<(), Box<dyn std::error::Error>> {
        let mut tx1 = ks.write_tx()?;
        let mut tx2 = ks.write_tx()?;
        tx1.insert(part, "hello", "world");

        tx1.commit()??;
        assert!(part.contains_key("hello")?);

        _ = tx2.get(part, "hello")?;

        tx2.insert(part, "hello", "world2");
        assert!(tx2.commit()?.is_err()); // intended to conflict

        Ok(())
    }
}
