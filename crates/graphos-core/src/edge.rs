use crate::node_id::NodeId;
use crate::capability::CapabilityToken;

/// What happens when this edge is traversed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EdgeKind {
    Reference = 0,
    StaticFn = 1,
    JitCompiled = 2,
    WasmCall = 3,
    InferenceCall = 4,
    Message = 5,
    CapDelegate = 6,
    Alias       = 7,
    Stdin       = 8,
    Stdout      = 9,
    CwdEdge     = 10,
    PipeSegment = 11,
    HistoryLink = 12,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct EdgeFlags: u8 {
        const BIDIRECTIONAL = 0b0000_0001;
        const LAZY          = 0b0000_0010;
        const MEMOIZED      = 0b0000_0100;
        const ASYNC         = 0b0000_1000;
        const VOLATILE      = 0b0001_0000;
        const GC_REACHABLE  = 0b1000_0000;
    }
}

/// Edge weight for priority scheduling
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct EdgeWeight(pub f32);

impl EdgeWeight {
    pub const DEFAULT: Self = Self(1.0);
    pub const HIGH: Self = Self(10.0);
    pub const LOW: Self = Self(0.1);
}

/// Complete edge structure: 64 bytes (one cache line)
#[repr(C, align(64))]
pub struct Edge {
    /// Source node (16 bytes)
    pub source: NodeId,
    /// Target node (16 bytes)
    pub target: NodeId,
    /// Edge kind discriminant (1 byte)
    pub kind: EdgeKind,
    /// Priority weight (4 bytes)
    pub weight: EdgeWeight,
    /// Capability required to traverse (8 bytes)
    pub required_cap: CapabilityToken,
    /// Payload pointer/offset — meaning depends on EdgeKind (8 bytes)
    /// StaticFn: function pointer
    /// JitCompiled: offset into JIT code cache
    /// WasmCall: (module_id << 32) | function_index
    /// InferenceCall: model node counter
    pub payload: u64,
    /// Edge behavior flags (1 byte)
    pub flags: EdgeFlags,
    /// Padding to 64 bytes
    pub _pad: [u8; 7],
}

const _: () = assert!(core::mem::size_of::<Edge>() == 64);
const _: () = assert!(core::mem::align_of::<Edge>() == 64);

impl Edge {
    pub fn new_named(source: NodeId, target: NodeId, name: &str) -> Self {
        Self {
            source,
            target,
            kind: EdgeKind::Reference,
            weight: EdgeWeight::DEFAULT,
            required_cap: CapabilityToken::OPEN,
            payload: crate::hash::fnv1a_64(name.as_bytes()),
            flags: EdgeFlags::empty(),
            _pad: [0; 7],
        }
    }

    pub fn name_hash(&self) -> u64 {
        self.payload
    }

    pub fn new_reference(source: NodeId, target: NodeId) -> Self {
        Self {
            source,
            target,
            kind: EdgeKind::Reference,
            weight: EdgeWeight::DEFAULT,
            required_cap: CapabilityToken::OPEN,
            payload: 0,
            flags: EdgeFlags::empty(),
            _pad: [0; 7],
        }
    }

    pub fn new_static_fn(
        source: NodeId,
        target: NodeId,
        fn_ptr: u64,
        weight: EdgeWeight,
    ) -> Self {
        Self {
            source,
            target,
            kind: EdgeKind::StaticFn,
            weight,
            required_cap: CapabilityToken::OPEN,
            payload: fn_ptr,
            flags: EdgeFlags::empty(),
            _pad: [0; 7],
        }
    }
}
