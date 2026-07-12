//! ledger.rs — the host-local admission ledger: announced capacity minus the
//! demands of currently RUNNING jobs. deliberately process-local (consensus
//! never sees load); a crashed node's over-commitments die with its leases.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub struct ResourceLedger {
    capacity: BTreeMap<String, u64>,
    running: Arc<Mutex<BTreeMap<String, BTreeMap<String, u64>>>>,
}

impl ResourceLedger {
    pub fn new(capacity: BTreeMap<String, u64>) -> Self {
        Self {
            capacity,
            running: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// free = capacity − Σ running, per dimension; a demanded dimension the
    /// capacity never named is a mismatch (absent ≠ infinite). empty demands
    /// trivially fit — the demandless legacy path costs nothing here.
    pub fn fits(&self, demands: &BTreeMap<String, u64>) -> bool {
        let running = self.running.lock().expect("ledger lock");
        demands.iter().all(|(dim, want)| {
            let Some(cap) = self.capacity.get(dim) else {
                return false;
            };
            let used: u64 = running.values().filter_map(|d| d.get(dim)).sum();
            cap.saturating_sub(used) >= *want
        })
    }

    /// record a run's demands under its attempt key; the guard releases on
    /// drop, so every exit path (ok, error, panic-unwind) frees the slot.
    pub fn reserve(&self, key: &str, demands: &BTreeMap<String, u64>) -> ReservationGuard {
        if !demands.is_empty() {
            self.running
                .lock()
                .expect("ledger lock")
                .insert(key.to_string(), demands.clone());
        }
        ReservationGuard {
            running: Arc::clone(&self.running),
            key: key.to_string(),
        }
    }
}

pub struct ReservationGuard {
    running: Arc<Mutex<BTreeMap<String, BTreeMap<String, u64>>>>,
    key: String,
}

impl Drop for ReservationGuard {
    fn drop(&mut self) {
        self.running.lock().expect("ledger lock").remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn reservations_subtract_and_release_on_drop() {
        let l = ResourceLedger::new(res(&[("cores", 8)]));
        let guard = l.reserve("s1:0", &res(&[("cores", 6)]));
        assert!(!l.fits(&res(&[("cores", 4)])), "6 of 8 reserved");
        assert!(l.fits(&res(&[("cores", 2)])));
        drop(guard);
        assert!(l.fits(&res(&[("cores", 8)])), "released on drop");
    }
}
