use graphos_cas::Hash256;
use crate::error::SnapshotError;

pub const SNAPSHOT_MAGIC: [u8; 4] = *b"SNAP";
pub const SNAPSHOT_VERSION: u16 = 1;

/// Snapshot header -- 64 bytes (one cache line)
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct SnapshotHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub flags: u16,
    pub epoch: u64,
    pub parent_epoch: u64,
    pub node_count: u32,
    pub edge_count: u32,
    pub content_hash: Hash256,
}

const _: () = assert!(core::mem::size_of::<SnapshotHeader>() == 64);

impl SnapshotHeader {
    pub fn new(
        epoch: u64,
        parent_epoch: u64,
        node_count: u32,
        edge_count: u32,
        content_hash: Hash256,
    ) -> Self {
        Self {
            magic: SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION,
            flags: 0,
            epoch,
            parent_epoch,
            node_count,
            edge_count,
            content_hash,
        }
    }

    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.magic != SNAPSHOT_MAGIC {
            return Err(SnapshotError::InvalidSnapshot);
        }
        if self.version != SNAPSHOT_VERSION {
            return Err(SnapshotError::InvalidSnapshot);
        }
        Ok(())
    }
}
