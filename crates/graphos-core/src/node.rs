use core::sync::atomic::{AtomicU32, AtomicU64};
use crate::node_id::NodeId;
use crate::capability::CapabilityToken;

/// 64-byte node header — exactly one cache line on ARM64/x86_64.
/// All hot fields are atomic for lock-free read access.
#[repr(C, align(64))]
pub struct NodeHeader {
    /// Unique identifier (16 bytes)
    pub id: NodeId,
    /// Node type (bits 0..15) + flags (bits 16..31) (4 bytes)
    pub type_and_flags: AtomicU32,
    /// Reference count for shared ownership (4 bytes)
    pub refcount: AtomicU32,
    /// Offset/pointer to edge list (8 bytes)
    pub edges_ptr: AtomicU64,
    /// Offset/pointer to type-specific payload (8 bytes)
    pub payload_ptr: AtomicU64,
    /// Capability required to access this node (8 bytes)
    pub access_cap: CapabilityToken,
    /// Size of the associated memory region in bytes (4 bytes)
    pub region_size: u32,
    /// Number of outgoing edges (4 bytes)
    pub edge_count: AtomicU32,
    /// Index into the slab allocator (4 bytes)
    pub slab_index: u32,
    /// Padding to 64 bytes (2 bytes)
    _pad: [u8; 2],
}

const _: () = assert!(core::mem::size_of::<NodeHeader>() == 64);
const _: () = assert!(core::mem::align_of::<NodeHeader>() == 64);

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct NodeFlags: u16 {
        const PINNED    = 0b0000_0001;
        const DIRTY     = 0b0000_0010;
        const LOCKED    = 0b0000_0100;
        const GC_MARK   = 0b0000_1000;
        const PERSISTED = 0b0001_0000;
        const SWAPPED   = 0b0010_0000;  // Bit 5 - node data on disk
        const LOADING   = 0b0100_0000;  // Bit 6 - currently being loaded
    }
}

impl NodeHeader {
    pub fn new(id: NodeId, node_type: u16, access_cap: CapabilityToken) -> Self {
        Self {
            id,
            type_and_flags: AtomicU32::new(node_type as u32),
            refcount: AtomicU32::new(1),
            edges_ptr: AtomicU64::new(0),
            payload_ptr: AtomicU64::new(0),
            access_cap,
            region_size: 0,
            edge_count: AtomicU32::new(0),
            slab_index: 0,
            _pad: [0; 2],
        }
    }

    #[inline]
    pub fn node_type(&self) -> u16 {
        (self.type_and_flags.load(core::sync::atomic::Ordering::Relaxed) & 0xFFFF) as u16
    }

    #[inline]
    pub fn flags(&self) -> NodeFlags {
        let raw = (self.type_and_flags.load(core::sync::atomic::Ordering::Relaxed) >> 16) as u16;
        NodeFlags::from_bits_truncate(raw)
    }

    pub fn set_flags(&self, flags: NodeFlags) {
        let current = self.type_and_flags.load(core::sync::atomic::Ordering::Relaxed);
        let new_val = (current & 0x0000_FFFF) | ((flags.bits() as u32) << 16);
        self.type_and_flags.store(new_val, core::sync::atomic::Ordering::Relaxed);
    }

    /// Get the residency state (bits 5-6 of flags): 0=resident, 1=swapped, 2=loading
    #[inline]
    pub fn residency_state(&self) -> u8 {
        ((self.flags().bits() >> 5) & 0b11) as u8
    }

    /// Set the residency state (bits 5-6 of flags)
    pub fn set_residency(&self, state: u8) {
        let current = self.type_and_flags.load(core::sync::atomic::Ordering::Relaxed);
        let flags_raw = ((current >> 16) as u16) & !(0b11 << 5);
        let new_flags = flags_raw | (((state & 0b11) as u16) << 5);
        let new_val = (current & 0x0000_FFFF) | ((new_flags as u32) << 16);
        self.type_and_flags.store(new_val, core::sync::atomic::Ordering::Relaxed);
    }
}
