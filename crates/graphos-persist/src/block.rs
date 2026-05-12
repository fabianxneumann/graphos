use crate::error::PersistError;

/// In-memory block device abstraction.
///
/// In Phase 1 (UEFI boot-services environment) we use a pre-allocated memory
/// buffer as a "virtual disk". Real UEFI Block I/O protocol support will be
/// added later.
pub struct BlockDevice {
    buffer: &'static mut [u8],
    capacity: usize,
    block_size: u32,
}

impl BlockDevice {
    /// Create a `BlockDevice` from a pre-allocated memory buffer.
    ///
    /// `block_size` is informational for future alignment; reads/writes are
    /// byte-granular on the memory buffer.
    pub fn from_buffer(buffer: &'static mut [u8], block_size: u32) -> Self {
        let capacity = buffer.len();
        Self {
            buffer,
            capacity,
            block_size,
        }
    }

    /// Read `buf.len()` bytes starting at `offset`.
    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<(), PersistError> {
        let end = offset.checked_add(buf.len()).ok_or(PersistError::ReadFailed)?;
        if end > self.capacity {
            return Err(PersistError::ReadFailed);
        }
        buf.copy_from_slice(&self.buffer[offset..end]);
        Ok(())
    }

    /// Write `data` starting at `offset`.
    pub fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), PersistError> {
        let end = offset.checked_add(data.len()).ok_or(PersistError::WriteFailed)?;
        if end > self.capacity {
            return Err(PersistError::WriteFailed);
        }
        self.buffer[offset..end].copy_from_slice(data);
        Ok(())
    }

    /// Total capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Configured block size.
    pub fn block_size(&self) -> u32 {
        self.block_size
    }
}
