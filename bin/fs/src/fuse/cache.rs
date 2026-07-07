//! a bounded read-through block cache for the read-only mount.
//!
//! content at a pinned snapshot is immutable, so a byte once fetched is valid for
//! the mount's whole lifetime — caching is pure win, the only question is memory.
//! reads are served in fixed [`BLOCK`]-sized, block-aligned windows (one block =
//! one `Read` byte-range against the node, which the module caps at exactly this
//! size). a bounded budget evicts the oldest block (FIFO) once exceeded, so a
//! huge file streams through a fixed footprint instead of pinning every byte.
//!
//! the cache keys on `(ino, block_index)` and stores each block as an `Arc<Vec<u8>>`
//! so a hit hands back a cheap clone without copying the block. `read_range`
//! assembles an arbitrary `(offset, len)` request from these blocks, calling the
//! supplied fetcher only for the blocks it lacks.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

/// the block granularity: 1 MiB, matching the module's chunk size and its
/// `MAX_READ_BYTES` read cap, so one miss is exactly one node round-trip.
pub const BLOCK: u64 = 1024 * 1024;

/// a FIFO-evicting block cache with a byte budget. not an LRU — insertion order is
/// good enough for the sequential/read-ahead access a kernel drives, and it keeps
/// eviction O(1) with no per-hit bookkeeping.
pub struct BlockCache {
    budget: usize,
    used: usize,
    blocks: HashMap<(u64, u64), Arc<Vec<u8>>>,
    /// eviction order (oldest first).
    order: VecDeque<(u64, u64)>,
}

impl BlockCache {
    /// a cache holding at most ~`budget_bytes` of block data.
    pub fn new(budget_bytes: usize) -> Self {
        BlockCache {
            budget: budget_bytes,
            used: 0,
            blocks: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// bytes currently held (for tests/introspection).
    #[allow(dead_code)]
    pub fn used(&self) -> usize {
        self.used
    }

    fn get(&self, key: (u64, u64)) -> Option<Arc<Vec<u8>>> {
        self.blocks.get(&key).cloned()
    }

    fn insert(&mut self, key: (u64, u64), data: Arc<Vec<u8>>) {
        if self.blocks.contains_key(&key) {
            return;
        }
        self.used += data.len();
        self.blocks.insert(key, data);
        self.order.push_back(key);
        // evict oldest until back under budget, but always keep at least the block
        // just inserted (a single block larger than the budget is still served).
        while self.used > self.budget && self.order.len() > 1 {
            if let Some(old) = self.order.pop_front()
                && let Some(v) = self.blocks.remove(&old)
            {
                self.used -= v.len();
            }
        }
    }

    /// serve `[offset, offset+len)` of the file `ino` (whose total size is
    /// `file_size`), fetching any missing blocks via `fetch(block_offset,
    /// block_len)` which must return up to `block_len` bytes starting at
    /// `block_offset`. the returned buffer is clamped to the file size (a read
    /// past EOF yields fewer bytes, per POSIX). `fetch` errors short-circuit.
    pub fn read_range<F, E>(
        &mut self,
        ino: u64,
        offset: u64,
        len: u64,
        file_size: u64,
        mut fetch: F,
    ) -> Result<Vec<u8>, E>
    where
        F: FnMut(u64, u64) -> Result<Vec<u8>, E>,
    {
        let end = offset.saturating_add(len).min(file_size);
        if offset >= end {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity((end - offset) as usize);
        let mut pos = offset;
        while pos < end {
            let block_index = pos / BLOCK;
            let block_start = block_index * BLOCK;
            let key = (ino, block_index);
            let block = match self.get(key) {
                Some(b) => b,
                None => {
                    // fetch exactly one block window, clamped to the file tail.
                    let want = BLOCK.min(file_size - block_start);
                    let bytes = fetch(block_start, want)?;
                    let arc = Arc::new(bytes);
                    self.insert(key, arc.clone());
                    arc
                }
            };
            let within = (pos - block_start) as usize;
            if within >= block.len() {
                // the backend returned a short block (e.g. a truncated tail); stop
                // rather than read out of bounds — the caller sees a short read.
                break;
            }
            let take = ((end - pos) as usize).min(block.len() - within);
            out.extend_from_slice(&block[within..within + take]);
            pos += take as u64;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a fetcher over an in-memory file: serves `[off, off+len)` and counts calls.
    fn file_fetcher<'a>(
        bytes: &'a [u8],
        calls: &'a std::cell::Cell<usize>,
    ) -> impl FnMut(u64, u64) -> Result<Vec<u8>, ()> + 'a {
        move |off: u64, len: u64| {
            calls.set(calls.get() + 1);
            let start = off as usize;
            let stop = (off + len).min(bytes.len() as u64) as usize;
            Ok(bytes[start..stop].to_vec())
        }
    }

    #[test]
    fn assembles_a_range_across_blocks() {
        let data: Vec<u8> = (0..(BLOCK * 2 + 1234)).map(|i| (i % 256) as u8).collect();
        let mut cache = BlockCache::new(64 * 1024 * 1024);
        let calls = std::cell::Cell::new(0);
        let got = cache
            .read_range(
                7,
                BLOCK - 10,
                40,
                data.len() as u64,
                file_fetcher(&data, &calls),
            )
            .unwrap();
        assert_eq!(
            got,
            &data[(BLOCK - 10) as usize..(BLOCK - 10 + 40) as usize]
        );
        // the range straddles block 0 and block 1 → two distinct fetches.
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn a_second_read_is_a_cache_hit() {
        let data: Vec<u8> = (0..BLOCK).map(|i| (i % 256) as u8).collect();
        let mut cache = BlockCache::new(64 * 1024 * 1024);
        let calls = std::cell::Cell::new(0);
        let _ = cache
            .read_range(1, 0, 100, data.len() as u64, file_fetcher(&data, &calls))
            .unwrap();
        let again = cache
            .read_range(1, 50, 100, data.len() as u64, file_fetcher(&data, &calls))
            .unwrap();
        assert_eq!(again, &data[50..150]);
        assert_eq!(calls.get(), 1, "the block was cached after the first read");
    }

    #[test]
    fn read_past_eof_is_clamped() {
        let data = vec![9u8; 500];
        let mut cache = BlockCache::new(1024 * 1024);
        let calls = std::cell::Cell::new(0);
        let got = cache
            .read_range(1, 400, 1000, data.len() as u64, file_fetcher(&data, &calls))
            .unwrap();
        assert_eq!(got.len(), 100, "clamped to the 500-byte file");
    }

    #[test]
    fn eviction_bounds_the_footprint() {
        // budget of 2 blocks; touch 4 distinct blocks → only ~2 blocks retained.
        let data: Vec<u8> = vec![1u8; (BLOCK * 4) as usize];
        let mut cache = BlockCache::new((BLOCK * 2) as usize);
        let calls = std::cell::Cell::new(0);
        for b in 0..4u64 {
            let _ = cache
                .read_range(
                    1,
                    b * BLOCK,
                    16,
                    data.len() as u64,
                    file_fetcher(&data, &calls),
                )
                .unwrap();
        }
        assert!(
            cache.used() <= (BLOCK * 2) as usize,
            "held ≤ budget: {}",
            cache.used()
        );
        assert_eq!(calls.get(), 4, "each distinct block fetched once");
        // block 0 was evicted → re-reading it fetches again (5th fetch).
        let _ = cache
            .read_range(1, 0, 16, data.len() as u64, file_fetcher(&data, &calls))
            .unwrap();
        assert_eq!(calls.get(), 5, "evicted block re-fetched");
    }
}
