use crate::hash::Hash256;
use graphos_core::NodeId;

/// Content identifier -- the hash of the stored data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ContentId {
    pub hash: Hash256,
}

impl ContentId {
    /// Create from raw data by hashing it
    pub fn from_data(data: &[u8]) -> Self {
        Self {
            hash: crate::hash::hash_256(data),
        }
    }

    /// Convert to a deterministic NodeId (for graph integration).
    /// Uses first 8 bytes as counter, node_type = ContentAddr (0x0019).
    pub fn to_node_id(&self) -> NodeId {
        let bytes = self.hash.as_bytes();
        let counter = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        NodeId::new(graphos_core::NodeType::ContentAddr, counter, 0)
    }

    /// Check if this is a zero/null ID
    pub fn is_null(&self) -> bool {
        self.hash == Hash256::ZERO
    }
}
