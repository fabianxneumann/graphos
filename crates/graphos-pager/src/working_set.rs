use graphos_core::NodeId;

const MAX_ROOTS: usize = 8;
const MAX_NODES: usize = 65536;
const BITMAP_WORDS: usize = MAX_NODES / 64; // 1024 u64 words

pub struct WorkingSet {
    roots: [NodeId; MAX_ROOTS],
    root_count: u8,
    pub horizon_depth: u8,
    pub hot_bitmap: [u64; BITMAP_WORDS],
    pub distance_map: [u8; MAX_NODES],
    pub generation: u32,
}

impl WorkingSet {
    pub fn new(depth: u8) -> Self {
        Self {
            roots: [NodeId::NULL; MAX_ROOTS],
            root_count: 0,
            horizon_depth: depth,
            hot_bitmap: [0u64; BITMAP_WORDS],
            distance_map: [255u8; MAX_NODES],
            generation: 0,
        }
    }

    pub fn add_root(&mut self, id: NodeId) {
        if self.root_count < MAX_ROOTS as u8 {
            self.roots[self.root_count as usize] = id;
            self.root_count += 1;
        }
    }

    pub fn set_root(&mut self, index: usize, id: NodeId) {
        if index < MAX_ROOTS {
            self.roots[index] = id;
            if index >= self.root_count as usize {
                self.root_count = (index + 1) as u8;
            }
        }
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots[..self.root_count as usize]
    }

    pub fn is_hot(&self, slab_index: u32) -> bool {
        let idx = slab_index as usize;
        if idx >= MAX_NODES { return false; }
        let word = idx / 64;
        let bit = idx % 64;
        (self.hot_bitmap[word] >> bit) & 1 == 1
    }

    pub fn set_hot(&mut self, slab_index: u32) {
        let idx = slab_index as usize;
        if idx >= MAX_NODES { return; }
        let word = idx / 64;
        let bit = idx % 64;
        self.hot_bitmap[word] |= 1u64 << bit;
    }

    pub fn clear_hot(&mut self, slab_index: u32) {
        let idx = slab_index as usize;
        if idx >= MAX_NODES { return; }
        let word = idx / 64;
        let bit = idx % 64;
        self.hot_bitmap[word] &= !(1u64 << bit);
    }

    pub fn distance(&self, slab_index: u32) -> u8 {
        let idx = slab_index as usize;
        if idx >= MAX_NODES { return 255; }
        self.distance_map[idx]
    }

    pub fn set_distance(&mut self, slab_index: u32, dist: u8) {
        let idx = slab_index as usize;
        if idx >= MAX_NODES { return; }
        if dist < self.distance_map[idx] {
            self.distance_map[idx] = dist;
        }
    }

    pub fn reset(&mut self) {
        self.hot_bitmap = [0u64; BITMAP_WORDS];
        self.distance_map = [255u8; MAX_NODES];
        self.generation += 1;
    }
}
