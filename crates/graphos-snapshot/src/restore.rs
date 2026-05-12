use graphos_core::GraphPool;
use graphos_persist::BlockDevice;
use graphos_cas::hash::hash_256;
use crate::header::SnapshotHeader;
use crate::create::SnapshotRegistry;
use crate::error::SnapshotError;

/// Restore graph state from a snapshot identified by epoch.
pub fn restore_snapshot(
    epoch: u64,
    registry: &SnapshotRegistry,
    pool: &mut GraphPool,
    disk: &BlockDevice,
) -> Result<(), SnapshotError> {
    // Find entry
    let entry = registry.find_entry(epoch)
        .ok_or(SnapshotError::EpochNotFound)?;

    // Read header from disk
    let mut header_bytes = [0u8; 64];
    disk.read(entry.disk_offset, &mut header_bytes)
        .map_err(|_| SnapshotError::DiskError)?;

    // Interpret as SnapshotHeader
    let header = unsafe {
        core::ptr::read_unaligned(header_bytes.as_ptr() as *const SnapshotHeader)
    };
    header.validate()?;

    let node_count = header.node_count as usize;
    let edge_count = header.edge_count as usize;
    let nodes_size = node_count * 64;
    let edges_size = edge_count * 64;

    // Read nodes into pool
    let nodes_dst = unsafe {
        core::slice::from_raw_parts_mut(
            pool.nodes_mut_raw().as_mut_ptr() as *mut u8,
            nodes_size,
        )
    };
    disk.read(entry.disk_offset + 64, nodes_dst)
        .map_err(|_| SnapshotError::DiskError)?;

    // Read edges into pool
    let edges_dst = unsafe {
        core::slice::from_raw_parts_mut(
            pool.edges_mut_raw().as_mut_ptr() as *mut u8,
            edges_size,
        )
    };
    disk.read(entry.disk_offset + 64 + nodes_size, edges_dst)
        .map_err(|_| SnapshotError::DiskError)?;

    // Restore counts
    unsafe {
        pool.restore_raw(node_count, edge_count);
    }

    // Verify content hash
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

    let computed_hash = hash_nodes_and_edges(nodes_bytes, edges_bytes);
    if computed_hash != header.content_hash {
        return Err(SnapshotError::HashMismatch);
    }

    Ok(())
}

/// Hash nodes and edges data without allocation (same logic as in create.rs)
fn hash_nodes_and_edges(nodes: &[u8], edges: &[u8]) -> graphos_cas::Hash256 {
    let nodes_hash = hash_256(nodes);
    let edges_hash = hash_256(edges);
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(nodes_hash.as_bytes());
    combined[32..64].copy_from_slice(edges_hash.as_bytes());
    hash_256(&combined)
}
