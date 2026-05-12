#![no_std]
#![cfg_attr(feature = "alloc", feature(allocator_api))]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod node_id;
pub mod node;
pub mod edge;
pub mod capability;
pub mod graph_pool;
pub mod hash;

pub use node_id::{NodeId, NodeType};
pub use node::NodeHeader;
pub use edge::{Edge, EdgeKind, EdgeFlags};
pub use capability::{CapabilityToken, CapRights};
pub use graph_pool::{GraphPool, GraphPoolConfig, GraphError};
pub use hash::fnv1a_64;
