#![no_std]
extern crate alloc;

pub mod residency;
pub mod node_page;
pub mod swap_table;
pub mod bfs_queue;
pub mod working_set;
pub mod prefetch;
pub mod evictor;
pub mod pager;

pub use pager::SemanticPager;
pub use residency::ResidencyState;
pub use working_set::WorkingSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerError {
    SwapTableFull,
    DiskReadFailed,
    DiskWriteFailed,
    NodeNotFound,
    AlreadyResident,
    EvictionFailed,
}
