use graphos_core::{GraphPool, NodeId, CapabilityToken, capability::CapRights};
use crate::epoch::EpochTracker;
use crate::error::HotSwapError;

/// Record of a completed swap
#[derive(Debug, Clone, Copy)]
pub struct SwapRecord {
    pub source: NodeId,
    pub target: NodeId,
    pub old_payload: u64,
    pub new_payload: u64,
    pub epoch: u64,
}

/// Atomically swap an edge's payload.
/// The old payload is retired for later reclamation via EBR.
pub fn hot_swap_edge(
    pool: &GraphPool,
    source: NodeId,
    target: NodeId,
    new_payload: u64,
    cap: &CapabilityToken,
    tracker: &mut EpochTracker,
) -> Result<SwapRecord, HotSwapError> {
    if !cap.rights.contains(CapRights::WRITE) {
        return Err(HotSwapError::InsufficientRights);
    }

    // Find the edge
    let edge = pool.edges_from(source)
        .find(|e| e.target == target)
        .ok_or(HotSwapError::EdgeNotFound)?;

    // Read old payload
    let old_payload = edge.payload;

    // Perform swap via volatile write.
    // Edge lives in a static slice — we obtain a mutable pointer to perform
    // the atomic-width write. On x86_64, an aligned u64 write is atomic.
    let edge_ptr = edge as *const graphos_core::Edge as *mut graphos_core::Edge;
    unsafe {
        let payload_ptr = core::ptr::addr_of_mut!((*edge_ptr).payload);
        core::ptr::write_volatile(payload_ptr, new_payload);
    }

    // Retire old payload for deferred reclamation
    let epoch = tracker.current_epoch();
    tracker.retire(old_payload);
    tracker.advance_epoch();

    Ok(SwapRecord {
        source,
        target,
        old_payload,
        new_payload,
        epoch,
    })
}
