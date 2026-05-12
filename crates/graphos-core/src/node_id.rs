use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u16)]
pub enum NodeType {
    Root = 0x0001,
    Process = 0x0002,
    Memory = 0x0003,
    File = 0x0004,
    Device = 0x0005,
    Model = 0x0006,
    WasmModule = 0x0007,
    Capability = 0x0008,
    Channel = 0x0009,
    Socket = 0x000A,
    Directory = 0x000B,
    Scheduler = 0x000C,
    JitCode = 0x000D,
    Serial = 0x000E,
    Disk = 0x000F,
    Network = 0x0010,
    Filesystem = 0x0011,
    AiEngine = 0x0012,
    Shell       = 0x0013,
    Command     = 0x0015,
    Binding     = 0x0016,
    History     = 0x0017,
    Snapshot    = 0x0018,
    ContentAddr = 0x0019,
}

/// 128-bit globally unique node identifier.
/// Layout: [48-bit timestamp_ms | 16-bit node_type | 64-bit counter]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct NodeId(pub u128);

impl NodeId {
    pub const NULL: Self = Self(0);

    pub fn new(node_type: NodeType, counter: u64, timestamp_ms: u64) -> Self {
        let ts = (timestamp_ms & 0x0000_FFFF_FFFF_FFFF) as u128;
        let nt = (node_type as u16) as u128;
        let ctr = counter as u128;
        Self((ts << 80) | (nt << 64) | ctr)
    }

    #[inline]
    pub fn node_type(self) -> u16 {
        ((self.0 >> 64) & 0xFFFF) as u16
    }

    #[inline]
    pub fn timestamp_ms(self) -> u64 {
        ((self.0 >> 80) & 0x0000_FFFF_FFFF_FFFF) as u64
    }

    #[inline]
    pub fn counter(self) -> u64 {
        (self.0 & 0xFFFF_FFFF_FFFF_FFFF) as u64
    }

    #[inline]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl core::fmt::Debug for NodeId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NodeId(type={:#06x}, ctr={}, ts={})",
            self.node_type(), self.counter(), self.timestamp_ms())
    }
}

/// Atomic counter for generating unique node IDs per type
pub struct NodeIdGenerator {
    counter: AtomicU64,
}

impl NodeIdGenerator {
    pub const fn new() -> Self {
        Self { counter: AtomicU64::new(1) }
    }

    pub fn next(&self, node_type: NodeType, timestamp_ms: u64) -> NodeId {
        let ctr = self.counter.fetch_add(1, Ordering::Relaxed);
        NodeId::new(node_type, ctr, timestamp_ms)
    }
}
