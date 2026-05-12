//! x86_64 page table structures and entry manipulation.

use crate::addr::{PhysAddr, PhysFrame};
use bitflags::bitflags;

bitflags! {
    /// Page table entry flags for x86_64.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PageFlags: u64 {
        const PRESENT       = 1 << 0;
        const WRITABLE      = 1 << 1;
        const USER          = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const NO_CACHE      = 1 << 4;
        const ACCESSED      = 1 << 5;
        const DIRTY         = 1 << 6;
        const HUGE_PAGE     = 1 << 7;
        const GLOBAL        = 1 << 8;
        const NO_EXECUTE    = 1 << 63;
    }
}

/// A single page table entry (64 bits).
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    /// An empty (not present) entry.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Whether the PRESENT bit is set.
    pub fn is_present(self) -> bool {
        self.0 & 1 != 0
    }

    /// Extract flags from this entry.
    pub fn flags(self) -> PageFlags {
        PageFlags::from_bits_truncate(self.0)
    }

    /// Extract the physical frame from bits 12..51.
    pub fn frame(self) -> Option<PhysFrame> {
        if self.is_present() {
            let addr = self.0 & 0x000F_FFFF_FFFF_F000;
            Some(PhysFrame(PhysAddr::new(addr)))
        } else {
            None
        }
    }

    /// Set this entry to point at `frame` with the given `flags`.
    pub fn set(&mut self, frame: PhysFrame, flags: PageFlags) {
        self.0 = frame.start_address().as_u64() | flags.bits();
    }

    /// Clear this entry (set to not present).
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

/// A 4 KiB aligned page table with 512 entries.
#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn empty() -> Self {
        Self {
            entries: [PageTableEntry::empty(); 512],
        }
    }
}

/// Errors that can occur during page mapping operations.
#[derive(Debug, Clone, Copy)]
pub enum MapError {
    /// The frame allocator could not provide a frame.
    FrameAllocationFailed,
    /// The virtual address is already mapped.
    AlreadyMapped,
    /// The virtual address is not mapped.
    NotMapped,
    /// The address is invalid (e.g., not canonical).
    InvalidAddress,
}
