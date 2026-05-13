use graphos_core::{GraphPool, NodeId};
use crate::bfs_queue::BfsQueue;
use crate::evictor::Evictor;
use crate::prefetch::PrefetchEngine;
use crate::swap_table::SwapTable;
use crate::working_set::WorkingSet;

pub struct PagerConfig {
    pub prefetch_depth: u8,
    pub eviction_threshold: u16,
    pub high_watermark: u16,
    pub batch_size: u16,
}

impl Default for PagerConfig {
    fn default() -> Self {
        Self {
            prefetch_depth: 3,
            eviction_threshold: 1024,
            high_watermark: 4096,
            batch_size: 64,
        }
    }
}

pub struct PagerStats {
    pub faults: u64,
    pub prefetch_hits: u64,
    pub evictions: u64,
    pub total_resident: u32,
    pub total_swapped: u32,
}

pub struct SemanticPager {
    pub swap_table: SwapTable,
    pub working_set: WorkingSet,
    pub bfs_queue: BfsQueue,
    pub prefetch: PrefetchEngine,
    pub evictor: Evictor,
    pub config: PagerConfig,
    pub stats: PagerStats,
}

impl SemanticPager {
    pub fn new(config: PagerConfig) -> Self {
        let depth = config.prefetch_depth;
        Self {
            swap_table: SwapTable::new(),
            working_set: WorkingSet::new(depth),
            bfs_queue: BfsQueue::new(),
            prefetch: PrefetchEngine::new(),
            evictor: Evictor::new(),
            config,
            stats: PagerStats {
                faults: 0,
                prefetch_hits: 0,
                evictions: 0,
                total_resident: 0,
                total_swapped: 0,
            },
        }
    }

    /// Called when the shell navigates to a new node.
    /// Updates the primary working set root and recalculates the hot zone.
    pub fn on_navigate(&mut self, pool: &GraphPool, target: NodeId) {
        if let Some(target_idx) = pool.node_index(target) {
            self.working_set.set_root(0, target);
            self.working_set.reset();
            self.prefetch.prefetch_neighborhood(
                pool,
                &mut self.working_set,
                &mut self.bfs_queue,
                target_idx,
                self.config.prefetch_depth,
            );
        }
    }

    /// Called when any node is accessed (read/traversal).
    /// Triggers speculative prefetch of its neighborhood.
    pub fn on_access(&mut self, pool: &GraphPool, slab_index: u32) {
        // Only prefetch if this node is at the edge of the hot zone
        let dist = self.working_set.distance(slab_index);
        if dist >= self.config.prefetch_depth.saturating_sub(1) {
            self.prefetch.prefetch_neighborhood(
                pool,
                &mut self.working_set,
                &mut self.bfs_queue,
                slab_index,
                2, // shallow prefetch for edge nodes
            );
        }
    }

    /// Get summary stats for the .pager shell command
    pub fn stats_summary(&self, pool: &GraphPool) -> (u32, u32, u32) {
        let total = pool.node_count() as u32;
        let mut swapped = 0u32;
        for i in 0..pool.node_count() {
            if pool.nodes_slice()[i].residency_state() != 0 {
                swapped += 1;
            }
        }
        let resident = total - swapped;
        let hot = self.hot_count();
        (resident, swapped, hot)
    }

    fn hot_count(&self) -> u32 {
        let mut count = 0u32;
        for &word in self.working_set.hot_bitmap.iter() {
            count += word.count_ones();
        }
        count
    }
}
