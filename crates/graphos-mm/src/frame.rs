//! Bitmap-based physical frame allocator.
//!
//! Each bit represents one 4 KiB frame: 0 = free, 1 = used.

use crate::addr::{PhysAddr, PhysFrame, PAGE_SIZE};

/// A frame allocator that uses a bitmap to track free/used frames.
pub struct BitmapFrameAllocator {
    bitmap: &'static mut [u64],
    total_frames: usize,
    next_free: usize,
}

impl BitmapFrameAllocator {
    /// Initialize from a static buffer.
    ///
    /// # Safety
    /// `bitmap_buf` must live for `'static` and must not be aliased.
    /// All bits are initialized to 0 (free).
    pub unsafe fn init(bitmap_buf: &'static mut [u64], frame_count: usize) -> Self {
        // Clear the bitmap
        for word in bitmap_buf.iter_mut() {
            *word = 0;
        }
        Self {
            bitmap: bitmap_buf,
            total_frames: frame_count,
            next_free: 0,
        }
    }

    /// Mark a frame as used.
    pub fn mark_used(&mut self, frame: PhysFrame) {
        let idx = frame.number();
        if idx < self.total_frames {
            let word = idx / 64;
            let bit = idx % 64;
            self.bitmap[word] |= 1u64 << bit;
        }
    }

    /// Mark a frame as free.
    pub fn mark_free(&mut self, frame: PhysFrame) {
        let idx = frame.number();
        if idx < self.total_frames {
            let word = idx / 64;
            let bit = idx % 64;
            self.bitmap[word] &= !(1u64 << bit);
            // Update hint if this frame is before current hint
            if idx < self.next_free {
                self.next_free = idx;
            }
        }
    }

    /// Allocate one frame, returning it if available.
    pub fn alloc_frame(&mut self) -> Option<PhysFrame> {
        let start_word = self.next_free / 64;
        let total_words = (self.total_frames + 63) / 64;

        for w in start_word..total_words {
            let word = self.bitmap[w];
            if word == u64::MAX {
                // All bits set — no free frame in this word
                continue;
            }
            // Find first zero bit
            let bit = (!word).trailing_zeros() as usize;
            let frame_idx = w * 64 + bit;
            if frame_idx >= self.total_frames {
                return None;
            }
            // Mark as used
            self.bitmap[w] |= 1u64 << bit;
            // Advance hint
            self.next_free = frame_idx + 1;
            let addr = PhysAddr::new((frame_idx as u64) * (PAGE_SIZE as u64));
            return Some(PhysFrame(addr));
        }
        None
    }

    /// Free one frame.
    pub fn free_frame(&mut self, frame: PhysFrame) {
        self.mark_free(frame);
    }

    /// Count of free frames.
    pub fn free_count(&self) -> usize {
        let total_words = (self.total_frames + 63) / 64;
        let mut used = 0usize;
        for w in 0..total_words {
            used += self.bitmap[w].count_ones() as usize;
        }
        self.total_frames - used
    }

    /// Total managed frames.
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }
}
