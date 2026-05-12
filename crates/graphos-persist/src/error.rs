/// Errors produced by the persistence layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistError {
    /// No suitable disk/partition found.
    DiskNotFound,
    /// Read operation failed.
    ReadFailed,
    /// Write operation failed.
    WriteFailed,
    /// Header magic bytes do not match.
    InvalidMagic,
    /// CRC32 checksum does not match payload.
    ChecksumMismatch,
    /// On-disk format version is incompatible.
    VersionMismatch,
    /// Provided buffer is too small for the operation.
    BufferTooSmall,
}
