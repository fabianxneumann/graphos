#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod error;
pub mod header;
pub mod block;
pub mod serialize;

pub use error::PersistError;
pub use header::PersistHeader;
pub use block::BlockDevice;
pub use serialize::{save_graph, load_graph};
