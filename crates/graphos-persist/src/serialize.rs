use core::mem;
use graphos_core::{GraphPool, NodeHeader, Edge};
use crate::block::BlockDevice;
use crate::header::PersistHeader;
use crate::error::PersistError;

/// Size of the header slot on disk (padded to 64 bytes for alignment).
const HEADER_SLOT: usize = 64;
const NODE_SIZE: usize = mem::size_of::<NodeHeader>(); // 64
const EDGE_SIZE: usize = mem::size_of::<Edge>();       // 64

/// Serialize the graph pool to the block device.
///
/// Layout:
///   [0..64)                              — PersistHeader (32 bytes) + padding
///   [64..64 + node_count*64)             — NodeHeaders as raw bytes
///   [64 + node_count*64 .. + edge_count*64) — Edges as raw bytes
pub fn save_graph(pool: &GraphPool, disk: &mut BlockDevice, epoch: u64) -> Result<(), PersistError> {
    let node_count = pool.node_count();
    let edge_count = pool.edge_count();

    let total_size = HEADER_SLOT + node_count * NODE_SIZE + edge_count * EDGE_SIZE;
    if total_size > disk.capacity() {
        return Err(PersistError::BufferTooSmall);
    }

    // Serialize nodes + edges as raw bytes to compute checksum
    let nodes_bytes = unsafe {
        core::slice::from_raw_parts(
            pool.nodes_slice().as_ptr() as *const u8,
            node_count * NODE_SIZE,
        )
    };
    let edges_bytes = unsafe {
        core::slice::from_raw_parts(
            pool.edges_slice().as_ptr() as *const u8,
            edge_count * EDGE_SIZE,
        )
    };

    // CRC32 over all payload bytes
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in nodes_bytes.iter().chain(edges_bytes.iter()) {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    let checksum = !crc;

    // Write header
    let header = PersistHeader::new(node_count as u32, edge_count as u32, checksum, epoch);
    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const PersistHeader as *const u8,
            mem::size_of::<PersistHeader>(),
        )
    };
    disk.write(0, header_bytes)?;

    // Write nodes
    disk.write(HEADER_SLOT, nodes_bytes)?;

    // Write edges
    disk.write(HEADER_SLOT + node_count * NODE_SIZE, edges_bytes)?;

    Ok(())
}

/// Deserialize the graph from the block device into `pool`.
///
/// Returns the epoch stored in the header on success.
pub fn load_graph(pool: &mut GraphPool, disk: &BlockDevice) -> Result<u64, PersistError> {
    // Read header
    let mut header_buf = [0u8; mem::size_of::<PersistHeader>()];
    disk.read(0, &mut header_buf)?;

    let header = unsafe { *(header_buf.as_ptr() as *const PersistHeader) };
    header.validate()?;

    let node_count = header.node_count as usize;
    let edge_count = header.edge_count as usize;

    let payload_size = node_count * NODE_SIZE + edge_count * EDGE_SIZE;
    let total_size = HEADER_SLOT + payload_size;
    if total_size > disk.capacity() {
        return Err(PersistError::BufferTooSmall);
    }

    // Read nodes directly into pool backing array
    let nodes_dst = unsafe {
        core::slice::from_raw_parts_mut(
            pool.nodes_mut_raw().as_mut_ptr() as *mut u8,
            node_count * NODE_SIZE,
        )
    };
    disk.read(HEADER_SLOT, nodes_dst)?;

    // Read edges directly into pool backing array
    let edges_dst = unsafe {
        core::slice::from_raw_parts_mut(
            pool.edges_mut_raw().as_mut_ptr() as *mut u8,
            edge_count * EDGE_SIZE,
        )
    };
    disk.read(HEADER_SLOT + node_count * NODE_SIZE, edges_dst)?;

    // Verify checksum
    let computed = crc32_over(nodes_dst, edges_dst);
    if computed != header.checksum {
        return Err(PersistError::ChecksumMismatch);
    }

    // Restore counts
    unsafe {
        pool.restore_raw(node_count, edge_count);
    }

    Ok(header.epoch)
}

/// Inline CRC32 over two contiguous slices.
fn crc32_over(a: &[u8], b: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in a.iter().chain(b.iter()) {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}
