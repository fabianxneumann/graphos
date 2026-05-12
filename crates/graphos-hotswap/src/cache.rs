use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

const MAX_REGIONS: usize = 64;

/// A contiguous region of executable code
#[derive(Clone, Copy)]
pub struct CodeRegion {
    pub base: *mut u8,
    pub size: usize,
    pub generation: u64,
    pub active: bool,
}

impl CodeRegion {
    const EMPTY: Self = Self {
        base: core::ptr::null_mut(),
        size: 0,
        generation: 0,
        active: false,
    };
}

/// Cache of code regions (LRU-managed)
pub struct CodeCache {
    regions: [CodeRegion; MAX_REGIONS],
    active_count: AtomicU32,
    clock: AtomicU64,
}

unsafe impl Send for CodeCache {}
unsafe impl Sync for CodeCache {}

impl CodeCache {
    pub const fn new() -> Self {
        Self {
            regions: [CodeRegion::EMPTY; MAX_REGIONS],
            active_count: AtomicU32::new(0),
            clock: AtomicU64::new(0),
        }
    }

    /// Register a new code region. Returns region_id on success.
    pub fn register(&mut self, base: *mut u8, size: usize) -> Option<u32> {
        let count = self.active_count.load(Ordering::Acquire);
        if count as usize >= MAX_REGIONS {
            return None;
        }

        // Find first inactive slot
        for i in 0..MAX_REGIONS {
            if !self.regions[i].active {
                let gen = self.clock.fetch_add(1, Ordering::AcqRel);
                self.regions[i] = CodeRegion {
                    base,
                    size,
                    generation: gen,
                    active: true,
                };
                self.active_count.fetch_add(1, Ordering::AcqRel);
                return Some(i as u32);
            }
        }
        None
    }

    /// Mark a region as retired (will be freed after epoch advances).
    /// Returns the retired region data.
    pub fn retire(&mut self, region_id: u32) -> Option<CodeRegion> {
        let idx = region_id as usize;
        if idx >= MAX_REGIONS {
            return None;
        }
        let region = &mut self.regions[idx];
        if !region.active {
            return None;
        }
        region.active = false;
        self.active_count.fetch_sub(1, Ordering::AcqRel);
        Some(*region)
    }

    /// Get region by id
    pub fn get(&self, region_id: u32) -> Option<&CodeRegion> {
        let idx = region_id as usize;
        if idx >= MAX_REGIONS {
            return None;
        }
        let region = &self.regions[idx];
        if region.active {
            Some(region)
        } else {
            None
        }
    }

    /// Number of active regions
    pub fn active_count(&self) -> u32 {
        self.active_count.load(Ordering::Acquire)
    }
}
