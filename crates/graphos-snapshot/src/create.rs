use graphos_core::GraphPool;
use graphos_persist::BlockDevice;
use graphos_cas::hash::hash_256;
use crate::header::SnapshotHeader;
use crate::error::SnapshotError;

/// Entry tracking a single snapshot on disk
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SnapshotEntry {
    pub epoch: u64,
    pub disk_offset: usize,
    pub total_size: usize,
}

/// Snapshot registry -- tracks where snapshots are stored on disk
pub struct SnapshotRegistry {
    entries: *mut SnapshotEntry,
    capacity: u32,
    count: u32,
    next_epoch: u64,
}

impl SnapshotRegistry {
    /// Create a new registry backed by pre-allocated storage.
    ///
    /// # Safety
    /// `storage` must point to valid, writable memory for at least `capacity` entries.
    pub unsafe fn new(storage: *mut SnapshotEntry, capacity: u32) -> Self {
        Self {
            entries: storage,
            capacity,
            count: 0,
            next_epoch: 1,
        }
    }

    /// Create a snapshot of the current graph state and write it to disk.
    /// Returns the epoch number of the created snapshot.
    pub fn create_snapshot(
        &mut self,
        pool: &GraphPool,
        disk: &mut BlockDevice,
    ) -> Result<u64, SnapshotError> {
        if self.count >= self.capacity {
            return Err(SnapshotError::TooManySnapshots);
        }

        let node_count = pool.node_count() as u32;
        let edge_count = pool.edge_count() as u32;
        let header_size = 64usize;
        let nodes_size = (node_count as usize) * 64;
        let edges_size = (edge_count as usize) * 64;
        let total_size = header_size + nodes_size + edges_size;

        // Find disk offset: after the last snapshot
        let disk_offset = if self.count == 0 {
            0
        } else {
            let last = unsafe { &*self.entries.add((self.count - 1) as usize) };
            last.disk_offset + last.total_size
        };

        // Check if there's enough space on disk
        if disk_offset + total_size > disk.capacity() {
            return Err(SnapshotError::DiskFull);
        }

        // Get raw bytes for nodes and edges
        let nodes_bytes = unsafe {
            core::slice::from_raw_parts(
                pool.nodes_slice().as_ptr() as *const u8,
                nodes_size,
            )
        };
        let edges_bytes = unsafe {
            core::slice::from_raw_parts(
                pool.edges_slice().as_ptr() as *const u8,
                edges_size,
            )
        };

        // Write nodes and edges to disk (after header space)
        disk.write(disk_offset + header_size, nodes_bytes)
            .map_err(|_| SnapshotError::DiskError)?;
        disk.write(disk_offset + header_size + nodes_size, edges_bytes)
            .map_err(|_| SnapshotError::DiskError)?;

        // Compute content hash over nodes + edges data
        let content_hash = if nodes_size + edges_size > 0 {
            hash_nodes_and_edges(nodes_bytes, edges_bytes)
        } else {
            hash_256(&[])
        };

        // Determine parent epoch
        let parent_epoch = if self.next_epoch > 1 {
            self.next_epoch - 1
        } else {
            0
        };

        // Build and write header
        let epoch = self.next_epoch;
        let header = SnapshotHeader::new(epoch, parent_epoch, node_count, edge_count, content_hash);
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const SnapshotHeader as *const u8,
                header_size,
            )
        };
        disk.write(disk_offset, header_bytes)
            .map_err(|_| SnapshotError::DiskError)?;

        // Register entry
        unsafe {
            let entry_ptr = self.entries.add(self.count as usize);
            entry_ptr.write(SnapshotEntry {
                epoch,
                disk_offset,
                total_size,
            });
        }

        self.count += 1;
        self.next_epoch += 1;

        Ok(epoch)
    }

    /// Get the latest snapshot epoch (0 if none exist)
    pub fn latest_epoch(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            unsafe { (*self.entries.add((self.count - 1) as usize)).epoch }
        }
    }

    /// Number of snapshots stored
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Find a snapshot entry by epoch
    pub fn find_entry(&self, epoch: u64) -> Option<&SnapshotEntry> {
        for i in 0..self.count as usize {
            let entry = unsafe { &*self.entries.add(i) };
            if entry.epoch == epoch {
                return Some(entry);
            }
        }
        None
    }
}

/// Hash nodes and edges data without allocation.
/// Hashes nodes and edges separately, then combines both hashes.
fn hash_nodes_and_edges(nodes: &[u8], edges: &[u8]) -> graphos_cas::Hash256 {
    let nodes_hash = hash_256(nodes);
    let edges_hash = hash_256(edges);

    // Combine both hashes into a 64-byte block and hash that
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(nodes_hash.as_bytes());
    combined[32..64].copy_from_slice(edges_hash.as_bytes());
    hash_256(&combined)
}
