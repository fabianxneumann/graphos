#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    DiskFull,
    InvalidSnapshot,
    EpochNotFound,
    CorruptData,
    HashMismatch,
    TooManySnapshots,
    DiskError,
}
