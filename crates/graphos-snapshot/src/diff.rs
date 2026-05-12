use graphos_persist::BlockDevice;
use crate::header::SnapshotHeader;
use crate::create::SnapshotRegistry;
use crate::error::SnapshotError;

/// Difference between two snapshots (count-based, no per-node diff)
pub struct SnapshotDiff {
    pub added_nodes: u32,
    pub removed_nodes: u32,
    pub added_edges: u32,
    pub removed_edges: u32,
    pub epoch_a: u64,
    pub epoch_b: u64,
}

/// Compare two snapshots by epoch.
/// Returns count-based differences (no per-node granularity without alloc).
pub fn diff_snapshots(
    epoch_a: u64,
    epoch_b: u64,
    registry: &SnapshotRegistry,
    disk: &BlockDevice,
) -> Result<SnapshotDiff, SnapshotError> {
    let entry_a = registry.find_entry(epoch_a)
        .ok_or(SnapshotError::EpochNotFound)?;
    let entry_b = registry.find_entry(epoch_b)
        .ok_or(SnapshotError::EpochNotFound)?;

    // Read headers
    let header_a = read_header(disk, entry_a.disk_offset)?;
    let header_b = read_header(disk, entry_b.disk_offset)?;

    header_a.validate()?;
    header_b.validate()?;

    let nodes_a = header_a.node_count;
    let nodes_b = header_b.node_count;
    let edges_a = header_a.edge_count;
    let edges_b = header_b.edge_count;

    let added_nodes = if nodes_b > nodes_a { nodes_b - nodes_a } else { 0 };
    let removed_nodes = if nodes_a > nodes_b { nodes_a - nodes_b } else { 0 };
    let added_edges = if edges_b > edges_a { edges_b - edges_a } else { 0 };
    let removed_edges = if edges_a > edges_b { edges_a - edges_b } else { 0 };

    Ok(SnapshotDiff {
        added_nodes,
        removed_nodes,
        added_edges,
        removed_edges,
        epoch_a,
        epoch_b,
    })
}

fn read_header(disk: &BlockDevice, offset: usize) -> Result<SnapshotHeader, SnapshotError> {
    let mut buf = [0u8; 64];
    disk.read(offset, &mut buf)
        .map_err(|_| SnapshotError::DiskError)?;
    let header = unsafe {
        core::ptr::read_unaligned(buf.as_ptr() as *const SnapshotHeader)
    };
    Ok(header)
}
