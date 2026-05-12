//! Buddy allocator for kernel heap memory.
//!
//! Orders: 0 = 4 KiB, 1 = 8 KiB, ..., 15 = 128 MiB.
//! Splits larger blocks when smaller ones are needed,
//! merges buddies on free.

use crate::addr::PAGE_SIZE;

/// Maximum order (inclusive). Order 15 = 2^15 pages = 128 MiB.
const MAX_ORDER: usize = 16;

/// A free block in the buddy allocator's free list.
#[repr(C)]
struct FreeBlock {
    next: Option<*mut FreeBlock>,
    order: u8,
}

/// A buddy allocator managing a contiguous memory region.
pub struct BuddyAllocator {
    free_lists: [Option<*mut FreeBlock>; MAX_ORDER],
    base: usize,
    total_size: usize,
}

// BuddyAllocator only used within spin-locked context
unsafe impl Send for BuddyAllocator {}

impl BuddyAllocator {
    /// Create a new buddy allocator managing memory at `base` with `size` bytes.
    ///
    /// # Safety
    /// - `base` must point to a valid, unused memory region of at least `size` bytes.
    /// - The memory must be aligned to PAGE_SIZE.
    /// - The caller must ensure no other code accesses this region.
    pub unsafe fn new(base: *mut u8, size: usize) -> Self {
        let mut alloc = Self {
            free_lists: [None; MAX_ORDER],
            base: base as usize,
            total_size: size,
        };

        // Add the entire region as free blocks, starting from the largest order
        let mut offset = 0usize;
        let mut remaining = size;

        // Work from largest to smallest order to minimize fragmentation
        let mut order = MAX_ORDER - 1;
        loop {
            let block_size = PAGE_SIZE << order;
            while remaining >= block_size {
                let ptr = (alloc.base + offset) as *mut FreeBlock;
                (*ptr).order = order as u8;
                (*ptr).next = alloc.free_lists[order];
                alloc.free_lists[order] = Some(ptr);
                offset += block_size;
                remaining -= block_size;
            }
            if order == 0 {
                break;
            }
            order -= 1;
        }

        alloc
    }

    /// Allocate at least `size` bytes, returning a pointer to the start.
    pub fn alloc(&mut self, size: usize) -> Option<*mut u8> {
        if size == 0 {
            return None;
        }
        let order = order_for_size(size);
        if order >= MAX_ORDER {
            return None;
        }
        self.alloc_order(order)
    }

    /// Free previously allocated memory of `size` bytes at `ptr`.
    ///
    /// # Safety
    /// - `ptr` must have been returned by a previous call to `alloc` with the same `size`.
    /// - Must not be called twice for the same allocation.
    pub unsafe fn free(&mut self, ptr: *mut u8, size: usize) {
        if size == 0 {
            return;
        }
        let order = order_for_size(size);
        if order >= MAX_ORDER {
            return;
        }
        self.free_order(ptr, order);
    }

    /// Total available (free) memory in bytes.
    pub fn available(&self) -> usize {
        let mut total = 0usize;
        for order in 0..MAX_ORDER {
            let block_size = PAGE_SIZE << order;
            let mut current = self.free_lists[order];
            while let Some(ptr) = current {
                total += block_size;
                current = unsafe { (*ptr).next };
            }
        }
        total
    }

    /// Allocate a block of the given order.
    fn alloc_order(&mut self, order: usize) -> Option<*mut u8> {
        // Try this order first
        if let Some(block) = self.free_lists[order] {
            // Remove from free list
            self.free_lists[order] = unsafe { (*block).next };
            return Some(block as *mut u8);
        }

        // No block at this order — try splitting a larger one
        let mut split_order = order + 1;
        while split_order < MAX_ORDER {
            if self.free_lists[split_order].is_some() {
                break;
            }
            split_order += 1;
        }

        if split_order >= MAX_ORDER {
            return None;
        }

        // Remove the block from the higher order
        let block = self.free_lists[split_order].unwrap();
        self.free_lists[split_order] = unsafe { (*block).next };

        // Split down to the requested order
        let mut current_order = split_order;
        while current_order > order {
            current_order -= 1;
            let buddy_size = PAGE_SIZE << current_order;
            let buddy_ptr = ((block as usize) + buddy_size) as *mut FreeBlock;
            unsafe {
                (*buddy_ptr).order = current_order as u8;
                (*buddy_ptr).next = self.free_lists[current_order];
            }
            self.free_lists[current_order] = Some(buddy_ptr);
        }

        Some(block as *mut u8)
    }

    /// Free a block and attempt to merge with its buddy.
    fn free_order(&mut self, ptr: *mut u8, order: usize) {
        let mut current_ptr = ptr as usize;
        let mut current_order = order;

        while current_order < MAX_ORDER - 1 {
            let block_size = PAGE_SIZE << current_order;
            // XOR trick: buddy address = block address XOR block_size
            // But only relative to base
            let relative = current_ptr - self.base;
            let buddy_relative = relative ^ block_size;
            let buddy_ptr = (self.base + buddy_relative) as *mut FreeBlock;

            // Check if buddy is within managed region
            if buddy_relative + block_size > self.total_size {
                break;
            }

            // Try to find and remove buddy from free list
            if !self.remove_from_free_list(buddy_ptr, current_order) {
                break;
            }

            // Merge: take the lower address as the new block
            if buddy_relative < relative {
                current_ptr = self.base + buddy_relative;
            }
            current_order += 1;
        }

        // Insert the (possibly merged) block into the free list
        let block = current_ptr as *mut FreeBlock;
        unsafe {
            (*block).order = current_order as u8;
            (*block).next = self.free_lists[current_order];
        }
        self.free_lists[current_order] = Some(block);
    }

    /// Remove a specific block from a free list. Returns true if found and removed.
    fn remove_from_free_list(&mut self, target: *mut FreeBlock, order: usize) -> bool {
        let mut prev: Option<*mut FreeBlock> = None;
        let mut current = self.free_lists[order];

        while let Some(block) = current {
            if block == target {
                // Remove from list
                let next = unsafe { (*block).next };
                match prev {
                    Some(p) => unsafe { (*p).next = next },
                    None => self.free_lists[order] = next,
                }
                return true;
            }
            prev = Some(block);
            current = unsafe { (*block).next };
        }
        false
    }
}

/// Determine the order needed for a given allocation size.
/// Order 0 = PAGE_SIZE (4 KiB), Order 1 = 2*PAGE_SIZE (8 KiB), etc.
fn order_for_size(size: usize) -> usize {
    let size = if size < PAGE_SIZE { PAGE_SIZE } else { size };
    let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;
    // Round up to next power of 2
    let mut order = 0;
    while (1usize << order) < pages_needed {
        order += 1;
    }
    order
}
