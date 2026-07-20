//! ledger.rs — the host-local admission ledger: announced capacity minus the
//! demands of currently RUNNING jobs. deliberately process-local (consensus
//! never sees load); a crashed node's over-commitments die with its leases.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use capability_host::RunCancellation;
use tokio::sync::Notify;

pub struct ResourceLedger {
    capacity: BTreeMap<String, u64>,
    running: Arc<Mutex<BTreeMap<String, BTreeMap<String, u64>>>>,
    available: Arc<Notify>,
}

impl ResourceLedger {
    pub fn new(capacity: BTreeMap<String, u64>) -> Self {
        Self {
            capacity,
            running: Arc::new(Mutex::new(BTreeMap::new())),
            available: Arc::new(Notify::new()),
        }
    }

    /// Turn omitted dimensions into an honest upper bound. On a sandboxed
    /// node, leaving (say) memory unspecified means the run may use the whole
    /// node, not zero memory: reserve and enforce the full announced capacity
    /// for that dimension. An empty ledger is the Direct legacy path, where a
    /// demandless run remains deliberately unrestricted.
    pub(crate) fn accounted_demands(
        &self,
        demands: &BTreeMap<String, u64>,
    ) -> BTreeMap<String, u64> {
        if self.capacity.is_empty() {
            return demands.clone();
        }
        let mut accounted = demands.clone();
        for (dimension, capacity) in &self.capacity {
            accounted.entry(dimension.clone()).or_insert(*capacity);
        }
        accounted
    }

    /// Whether this node could ever satisfy the demands. Unlike [`Self::fits`],
    /// this ignores current occupancy so an assigned attempt can queue behind
    /// work already running here instead of losing its effect.
    pub(crate) fn within_capacity(&self, demands: &BTreeMap<String, u64>) -> bool {
        demands
            .iter()
            .all(|(dim, want)| self.capacity.get(dim).is_some_and(|cap| cap >= want))
    }

    /// free = capacity − Σ running, per dimension; a demanded dimension the
    /// capacity never named is a mismatch (absent ≠ infinite). Callers pass
    /// [`Self::accounted_demands`] so omitted sandbox dimensions cost their
    /// full capacity; only the empty-capacity Direct legacy path stays free.
    pub fn fits(&self, demands: &BTreeMap<String, u64>) -> bool {
        let running = self.running.lock().expect("ledger lock");
        self.fits_locked(&running, demands)
    }

    fn fits_locked(
        &self,
        running: &BTreeMap<String, BTreeMap<String, u64>>,
        demands: &BTreeMap<String, u64>,
    ) -> bool {
        demands.iter().all(|(dim, want)| {
            let Some(cap) = self.capacity.get(dim) else {
                return false;
            };
            let used: u64 = running.values().filter_map(|d| d.get(dim)).sum();
            cap.saturating_sub(used) >= *want
        })
    }

    /// atomically check and reserve one attempt's demands. `gate()` performs
    /// the cheap optimistic check, but concurrent offers can arrive before a
    /// spawned task starts; this second check is the admission linearizer.
    /// the guard releases on drop.
    pub fn try_reserve(
        &self,
        key: &str,
        demands: &BTreeMap<String, u64>,
    ) -> Option<ReservationGuard> {
        let mut running = self.running.lock().expect("ledger lock");
        if !self.fits_locked(&running, demands) {
            return None;
        }
        if !demands.is_empty() {
            running.insert(key.to_string(), demands.clone());
        }
        Some(ReservationGuard {
            running: Arc::clone(&self.running),
            available: Arc::clone(&self.available),
            key: key.to_string(),
        })
    }

    /// Wait until the demands fit, then reserve them atomically. Register the
    /// waiter before each optimistic reserve so a release between the failed
    /// check and the await cannot be lost.
    pub(crate) async fn reserve_when_available(
        &self,
        key: &str,
        demands: &BTreeMap<String, u64>,
        cancellation: &RunCancellation,
    ) -> Option<ReservationGuard> {
        loop {
            if cancellation.is_cancelled() {
                return None;
            }
            let released = self.available.notified();
            tokio::pin!(released);
            released.as_mut().enable();
            if let Some(reservation) = self.try_reserve(key, demands) {
                return Some(reservation);
            }
            tokio::select! {
                _ = &mut released => {}
                _ = cancellation.cancelled() => return None,
            }
        }
    }
}

pub struct ReservationGuard {
    running: Arc<Mutex<BTreeMap<String, BTreeMap<String, u64>>>>,
    available: Arc<Notify>,
    key: String,
}

impl Drop for ReservationGuard {
    fn drop(&mut self) {
        self.running.lock().expect("ledger lock").remove(&self.key);
        self.available.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn res(pairs: &[(&str, u64)]) -> BTreeMap<String, u64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn fits_is_per_dimension_and_absent_is_not_infinite() {
        let l = ResourceLedger::new(res(&[("cores", 8), ("mem_gb", 16)]));
        assert!(l.fits(&res(&[("cores", 8)])));
        assert!(!l.fits(&res(&[("cores", 9)])));
        assert!(!l.fits(&res(&[("gpu", 1)])), "absent dimension never fits");
        assert!(l.fits(&res(&[])), "demandless always fits");
        // empty capacity (direct node): only demandless fits.
        let bare = ResourceLedger::new(Default::default());
        assert!(bare.fits(&res(&[])));
        assert!(!bare.fits(&res(&[("cores", 1)])));
        assert!(l.within_capacity(&res(&[("cores", 8)])));
        assert!(!l.within_capacity(&res(&[("cores", 9)])));
    }

    #[test]
    fn omitted_sandbox_dimensions_account_for_full_capacity() {
        let l = ResourceLedger::new(res(&[("cores", 8), ("mem_gb", 16)]));
        assert_eq!(
            l.accounted_demands(&res(&[])),
            res(&[("cores", 8), ("mem_gb", 16)])
        );
        assert_eq!(
            l.accounted_demands(&res(&[("cores", 2)])),
            res(&[("cores", 2), ("mem_gb", 16)])
        );
        let direct = ResourceLedger::new(BTreeMap::new());
        assert!(direct.accounted_demands(&res(&[])).is_empty());
    }

    #[test]
    fn reservations_subtract_and_release_on_drop() {
        let l = ResourceLedger::new(res(&[("cores", 8)]));
        let guard = l
            .try_reserve("s1:0", &res(&[("cores", 6)]))
            .expect("first reservation fits");
        assert!(!l.fits(&res(&[("cores", 4)])), "6 of 8 reserved");
        assert!(l.fits(&res(&[("cores", 2)])));
        assert!(
            l.try_reserve("s2:0", &res(&[("cores", 4)])).is_none(),
            "check and reserve are one critical section"
        );
        drop(guard);
        assert!(l.fits(&res(&[("cores", 8)])), "released on drop");
    }

    #[test]
    fn concurrent_try_reserve_has_one_winner() {
        let ledger = Arc::new(ResourceLedger::new(res(&[("cores", 1)])));
        let start = Arc::new(Barrier::new(3));
        let checked = Arc::new(Barrier::new(3));
        let winners = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for key in ["a", "b"] {
            let ledger = ledger.clone();
            let start = start.clone();
            let checked = checked.clone();
            let winners = winners.clone();
            threads.push(std::thread::spawn(move || {
                start.wait();
                let reservation = ledger.try_reserve(key, &res(&[("cores", 1)]));
                if reservation.is_some() {
                    winners.fetch_add(1, Ordering::SeqCst);
                }
                checked.wait();
                drop(reservation);
            }));
        }

        start.wait();
        checked.wait();
        assert_eq!(winners.load(Ordering::SeqCst), 1);
        for thread in threads {
            thread.join().unwrap();
        }
        assert!(ledger.fits(&res(&[("cores", 1)])));
    }
}
