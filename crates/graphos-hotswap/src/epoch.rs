use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

const MAX_THREADS: usize = 16;
const RETIRE_QUEUE_SIZE: usize = 64;

/// A retired payload waiting for safe reclamation
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct RetiredEntry {
    payload: u64,
    retire_epoch: u64,
    valid: bool,
}

impl RetiredEntry {
    const EMPTY: Self = Self {
        payload: 0,
        retire_epoch: 0,
        valid: false,
    };
}

/// Queue of retired payloads
struct RetireQueue {
    entries: [RetiredEntry; RETIRE_QUEUE_SIZE],
    head: usize,
    count: usize,
}

impl RetireQueue {
    const fn new() -> Self {
        Self {
            entries: [RetiredEntry::EMPTY; RETIRE_QUEUE_SIZE],
            head: 0,
            count: 0,
        }
    }

    fn push(&mut self, payload: u64, epoch: u64) -> bool {
        if self.count >= RETIRE_QUEUE_SIZE {
            return false;
        }
        let idx = (self.head + self.count) % RETIRE_QUEUE_SIZE;
        self.entries[idx] = RetiredEntry {
            payload,
            retire_epoch: epoch,
            valid: true,
        };
        self.count += 1;
        true
    }
}

/// Global epoch tracker for epoch-based reclamation.
///
/// Prevents use-after-free during concurrent edge traversal by tracking
/// which epochs are still observed by active threads/nodes.
pub struct EpochTracker {
    global_epoch: AtomicU64,
    thread_epochs: [AtomicU64; MAX_THREADS],
    active_threads: AtomicU32,
    retire_queue: RetireQueue,
}

impl EpochTracker {
    pub const fn new() -> Self {
        // AtomicU64::new is const — build array manually
        const MAX_EPOCH: AtomicU64 = AtomicU64::new(u64::MAX);
        Self {
            global_epoch: AtomicU64::new(0),
            thread_epochs: [MAX_EPOCH; MAX_THREADS],
            active_threads: AtomicU32::new(0),
            retire_queue: RetireQueue::new(),
        }
    }

    /// Get current global epoch
    pub fn current_epoch(&self) -> u64 {
        self.global_epoch.load(Ordering::Acquire)
    }

    /// Advance global epoch by one, returns new epoch value
    pub fn advance_epoch(&self) -> u64 {
        self.global_epoch.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Pin epoch for a thread/node. While the guard is held,
    /// no payload retired at or after this epoch will be reclaimed.
    pub fn pin(&self) -> EpochGuard<'_> {
        let slot = self.active_threads.fetch_add(1, Ordering::AcqRel) as usize;
        // Wrap around if we exceed MAX_THREADS (best-effort, no panic in no_std kernel)
        let slot = slot % MAX_THREADS;
        let current = self.global_epoch.load(Ordering::Acquire);
        self.thread_epochs[slot].store(current, Ordering::Release);
        EpochGuard { tracker: self, slot }
    }

    /// Retire a payload — it will be reclaimable once all threads advance
    /// past the current epoch.
    pub fn retire(&mut self, payload: u64) {
        let epoch = self.global_epoch.load(Ordering::Acquire);
        // If queue is full, attempt reclaim first
        if !self.retire_queue.push(payload, epoch) {
            self.reclaim();
            // Try again — if still full, silently drop (best-effort in no_std)
            let _ = self.retire_queue.push(payload, epoch);
        }
    }

    /// Reclaim all payloads that are safe to free (all active threads have
    /// advanced past their retire_epoch). Returns number of reclaimed entries.
    pub fn reclaim(&mut self) -> u32 {
        let min_epoch = self.min_active_epoch();
        let mut reclaimed = 0u32;

        for i in 0..RETIRE_QUEUE_SIZE {
            let entry = &mut self.retire_queue.entries[i];
            if entry.valid && entry.retire_epoch < min_epoch {
                // Safe to reclaim — in a real system this would free memory
                // or mark a code region as reusable
                entry.valid = false;
                reclaimed += 1;
                if self.retire_queue.count > 0 {
                    self.retire_queue.count -= 1;
                }
            }
        }
        reclaimed
    }

    /// Get minimum epoch across all active threads.
    /// A payload is safe to reclaim only if its retire_epoch < min_active_epoch.
    fn min_active_epoch(&self) -> u64 {
        let mut min = u64::MAX;
        for i in 0..MAX_THREADS {
            let epoch = self.thread_epochs[i].load(Ordering::Acquire);
            if epoch < min {
                min = epoch;
            }
        }
        min
    }
}

/// RAII guard that pins the current epoch for a "thread" (node).
/// While held, no payload retired at or after the pinned epoch will be reclaimed.
pub struct EpochGuard<'a> {
    tracker: &'a EpochTracker,
    slot: usize,
}

impl<'a> Drop for EpochGuard<'a> {
    fn drop(&mut self) {
        // Unpin: set thread epoch to MAX (= not blocking anything)
        self.tracker.thread_epochs[self.slot].store(u64::MAX, Ordering::Release);
        self.tracker.active_threads.fetch_sub(1, Ordering::AcqRel);
    }
}
