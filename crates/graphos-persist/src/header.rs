use crate::error::PersistError;

/// Magic bytes identifying a GraphOS persistence image.
pub const MAGIC: [u8; 8] = *b"GRAPHOS\0";

/// Current on-disk format version.
pub const VERSION: u32 = 1;

/// On-disk header (32 bytes logical, padded to 64 for alignment).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PersistHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub node_count: u32,
    pub edge_count: u32,
    pub checksum: u32,
    pub epoch: u64,
}

impl PersistHeader {
    /// Create a new header with the given parameters.
    pub fn new(node_count: u32, edge_count: u32, checksum: u32, epoch: u64) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            node_count,
            edge_count,
            checksum,
            epoch,
        }
    }

    /// Validate magic and version fields.
    pub fn validate(&self) -> Result<(), PersistError> {
        if self.magic != MAGIC {
            return Err(PersistError::InvalidMagic);
        }
        if self.version != VERSION {
            return Err(PersistError::VersionMismatch);
        }
        Ok(())
    }
}

/// Compute CRC32 (IEEE polynomial 0xEDB88320) over `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}
