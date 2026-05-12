//! Physical and virtual address types for x86_64 memory management.

/// Size of a standard page (4 KiB).
pub const PAGE_SIZE: usize = 4096;

/// A physical memory address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(pub u64);

/// A virtual memory address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(pub u64);

/// A physical frame aligned to PAGE_SIZE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysFrame(pub PhysAddr);

impl PhysAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn is_aligned(self, align: u64) -> bool {
        self.0 % align == 0
    }

    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }

    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
}

impl VirtAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Offset within the 4 KiB page (bits 0..11).
    pub fn page_offset(self) -> u16 {
        (self.0 & 0xFFF) as u16
    }

    /// Page table level 1 index (bits 12..20).
    pub fn p1_index(self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }

    /// Page table level 2 index (bits 21..29).
    pub fn p2_index(self) -> usize {
        ((self.0 >> 21) & 0x1FF) as usize
    }

    /// Page table level 3 index (bits 30..38).
    pub fn p3_index(self) -> usize {
        ((self.0 >> 30) & 0x1FF) as usize
    }

    /// Page table level 4 index (bits 39..47).
    pub fn p4_index(self) -> usize {
        ((self.0 >> 39) & 0x1FF) as usize
    }
}

impl PhysFrame {
    /// Return the frame containing the given physical address.
    pub fn containing(addr: PhysAddr) -> Self {
        Self(addr.align_down(PAGE_SIZE as u64))
    }

    /// The start address of this frame.
    pub fn start_address(self) -> PhysAddr {
        self.0
    }

    /// Frame number (address / PAGE_SIZE).
    pub fn number(self) -> usize {
        (self.0.as_u64() / PAGE_SIZE as u64) as usize
    }
}
