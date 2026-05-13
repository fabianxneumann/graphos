const BFS_CAPACITY: usize = 4096;

#[derive(Clone, Copy)]
pub struct BfsEntry {
    pub slab_index: u32,
    pub depth: u8,
    pub _pad: [u8; 3],
}

pub struct BfsQueue {
    buffer: [BfsEntry; BFS_CAPACITY],
    head: usize,
    tail: usize,
}

impl BfsQueue {
    pub fn new() -> Self {
        Self {
            buffer: [BfsEntry { slab_index: 0, depth: 0, _pad: [0; 3] }; BFS_CAPACITY],
            head: 0,
            tail: 0,
        }
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head) % BFS_CAPACITY
    }

    pub fn push(&mut self, entry: BfsEntry) -> bool {
        let next_tail = (self.tail + 1) % BFS_CAPACITY;
        if next_tail == self.head {
            return false; // full
        }
        self.buffer[self.tail] = entry;
        self.tail = next_tail;
        true
    }

    pub fn pop(&mut self) -> Option<BfsEntry> {
        if self.is_empty() {
            return None;
        }
        let entry = self.buffer[self.head];
        self.head = (self.head + 1) % BFS_CAPACITY;
        Some(entry)
    }
}
