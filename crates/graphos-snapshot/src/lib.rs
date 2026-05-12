#![no_std]

pub mod error;
pub mod header;
pub mod create;
pub mod restore;
pub mod diff;

pub use error::SnapshotError;
pub use header::SnapshotHeader;
pub use create::{SnapshotRegistry, SnapshotEntry};
pub use restore::restore_snapshot;
pub use diff::{SnapshotDiff, diff_snapshots};
