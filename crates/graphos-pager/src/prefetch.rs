use graphos_core::GraphPool;
use crate::bfs_queue::{BfsQueue, BfsEntry};
use crate::working_set::WorkingSet;

const VISITED_WORDS: usize = 65536 / 64;

pub struct PrefetchEngine {
    visited: [u64; VISITED_WORDS],
}

impl PrefetchEngine {
    pub fn new() -> Self {
        Self {
            visited: [0u64; VISITED_WORDS],
        }
    }

    fn mark_visited(&mut self, idx: u32) -> bool {
        let i = idx as usize;
        if i >= 65536 { return true; }
        let word = i / 64;
        let bit = i % 64;
        if (self.visited[word] >> bit) & 1 == 1 {
            return true; // already visited
        }
        self.visited[word] |= 1u64 << bit;
        false
    }

    fn clear_visited(&mut self) {
        for w in self.visited.iter_mut() {
            *w = 0;
        }
    }

    /// Run BFS from `start_index` up to `depth` hops.
    /// Marks reachable nodes as hot in the working set.
    /// Returns count of nodes marked hot.
    pub fn prefetch_neighborhood(
        &mut self,
        pool: &GraphPool,
        working_set: &mut WorkingSet,
        queue: &mut BfsQueue,
        start_index: u32,
        depth: u8,
    ) -> u32 {
        self.clear_visited();
        queue.clear();

        self.mark_visited(start_index);
        working_set.set_hot(start_index);
        working_set.set_distance(start_index, 0);

        queue.push(BfsEntry {
            slab_index: start_index,
            depth: 0,
            _pad: [0; 3],
        });

        let mut count = 1u32;

        while let Some(entry) = queue.pop() {
            if entry.depth >= depth {
                continue;
            }

            // Iterate edges from this node
            for edge in pool.edges_from_indexed(entry.slab_index) {
                if let Some(target_idx) = pool.node_index(edge.target) {
                    if self.mark_visited(target_idx) {
                        continue; // already seen
                    }

                    let new_depth = entry.depth + 1;
                    working_set.set_hot(target_idx);
                    working_set.set_distance(target_idx, new_depth);
                    count += 1;

                    if new_depth < depth {
                        queue.push(BfsEntry {
                            slab_index: target_idx,
                            depth: new_depth,
                            _pad: [0; 3],
                        });
                    }
                }
            }
        }

        count
    }
}
