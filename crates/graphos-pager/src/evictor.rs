use graphos_core::GraphPool;
use graphos_core::node::NodeFlags;
use crate::working_set::WorkingSet;

const MAX_CANDIDATES: usize = 256;

#[derive(Clone, Copy)]
pub struct Candidate {
    pub slab_index: u32,
    pub score: u32,
}

pub struct Evictor {
    candidates: [Candidate; MAX_CANDIDATES],
    candidate_count: usize,
}

impl Evictor {
    pub fn new() -> Self {
        Self {
            candidates: [Candidate { slab_index: 0, score: 0 }; MAX_CANDIDATES],
            candidate_count: 0,
        }
    }

    /// Find the coldest `count` nodes that can be evicted.
    /// Returns slab indices of eviction candidates, sorted coldest-first.
    pub fn find_victims(
        &mut self,
        pool: &GraphPool,
        working_set: &WorkingSet,
        count: usize,
    ) -> &[Candidate] {
        self.candidate_count = 0;
        let target_count = count.min(MAX_CANDIDATES);

        for idx in 0..pool.node_count() {
            let node = &pool.nodes_slice()[idx];
            let flags = node.flags();

            // Skip pinned nodes
            if flags.contains(NodeFlags::PINNED) {
                continue;
            }
            // Skip already swapped
            if node.residency_state() != 0 {
                continue;
            }
            // Skip hot nodes (within N hops of working set)
            if working_set.is_hot(idx as u32) {
                continue;
            }

            let distance = working_set.distance(idx as u32) as u32;
            let score = distance * 256;

            // Insert into candidates (maintain top-K by score descending)
            if self.candidate_count < target_count {
                self.candidates[self.candidate_count] = Candidate {
                    slab_index: idx as u32,
                    score,
                };
                self.candidate_count += 1;
            } else {
                // Find minimum score in candidates and replace if new score is higher
                let mut min_idx = 0;
                let mut min_score = self.candidates[0].score;
                for i in 1..self.candidate_count {
                    if self.candidates[i].score < min_score {
                        min_score = self.candidates[i].score;
                        min_idx = i;
                    }
                }
                if score > min_score {
                    self.candidates[min_idx] = Candidate {
                        slab_index: idx as u32,
                        score,
                    };
                }
            }
        }

        &self.candidates[..self.candidate_count]
    }

    pub fn victim_count(&self) -> usize {
        self.candidate_count
    }

    pub fn victim_index(&self, i: usize) -> u32 {
        self.candidates[i].slab_index
    }
}
