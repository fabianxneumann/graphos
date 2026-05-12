/// 256-bit hash output
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Hash256(pub [u8; 32]);

impl Hash256 {
    pub const ZERO: Self = Self([0u8; 32]);

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Debug for Hash256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Hash256(")?;
        for b in &self.0[..4] {
            write!(f, "{:02x}", b)?;
        }
        write!(f, "...)")
    }
}

// SHA-256 initial values (first 32 bits of the fractional parts of the square roots of the first 8 primes)
const IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// ARX quarter-round mixing on array indices (avoids mutable borrow issues)
#[inline(always)]
fn quarter_round(v: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    v[a] = v[a].wrapping_add(v[b]);
    v[d] ^= v[a];
    v[d] = v[d].rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] ^= v[c];
    v[b] = v[b].rotate_right(12);
    v[a] = v[a].wrapping_add(v[b]);
    v[d] ^= v[a];
    v[d] = v[d].rotate_right(8);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] ^= v[c];
    v[b] = v[b].rotate_right(7);
}

/// Load a little-endian u32 from a byte slice
#[inline(always)]
fn load_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// Compress a 64-byte block into the state
fn compress(state: &mut [u32; 8], block: &[u8]) {
    // Load 16 message words from block
    let mut m = [0u32; 16];
    for i in 0..16 {
        m[i] = load_u32_le(block, i * 4);
    }

    // Working state: 16 words arranged as a 4x4 matrix
    let mut v = [0u32; 16];
    v[0] = state[0];
    v[1] = state[1];
    v[2] = state[2];
    v[3] = state[3];
    v[4] = state[4];
    v[5] = state[5];
    v[6] = state[6];
    v[7] = state[7];
    // Lower half initialized from IV XOR'd with message schedule
    v[8] = IV[0] ^ m[0];
    v[9] = IV[1] ^ m[1];
    v[10] = IV[2] ^ m[2];
    v[11] = IV[3] ^ m[3];
    v[12] = IV[4] ^ m[4];
    v[13] = IV[5] ^ m[5];
    v[14] = IV[6] ^ m[6];
    v[15] = IV[7] ^ m[7];

    // 10 rounds of mixing
    for round in 0..10 {
        // Inject more message words each round (sigma-like schedule)
        let offset = ((round * 2) % 16) as usize;
        v[0] = v[0].wrapping_add(m[offset % 16]);
        v[5] = v[5].wrapping_add(m[(offset + 1) % 16]);
        v[10] = v[10].wrapping_add(m[(offset + 2) % 16]);
        v[15] = v[15].wrapping_add(m[(offset + 3) % 16]);

        // Column rounds
        quarter_round(&mut v, 0, 4, 8, 12);
        quarter_round(&mut v, 1, 5, 9, 13);
        quarter_round(&mut v, 2, 6, 10, 14);
        quarter_round(&mut v, 3, 7, 11, 15);

        // Diagonal rounds
        quarter_round(&mut v, 0, 5, 10, 15);
        quarter_round(&mut v, 1, 6, 11, 12);
        quarter_round(&mut v, 2, 7, 8, 13);
        quarter_round(&mut v, 3, 4, 9, 14);
    }

    // Finalization: XOR upper and lower halves back into state
    for i in 0..8 {
        state[i] ^= v[i] ^ v[i + 8];
    }
}

/// Hash arbitrary data into 256 bits using a BLAKE3-inspired ARX construction
pub fn hash_256(data: &[u8]) -> Hash256 {
    let mut state = IV;
    let len = data.len();

    // Process full 64-byte blocks
    let full_blocks = len / 64;
    for i in 0..full_blocks {
        let offset = i * 64;
        compress(&mut state, &data[offset..offset + 64]);
    }

    // Handle remaining bytes (pad with zeros)
    let remaining = len % 64;
    if remaining > 0 || len == 0 {
        let mut last_block = [0u8; 64];
        let offset = full_blocks * 64;
        let tail = &data[offset..];
        let copy_len = tail.len();
        let dest = &mut last_block[..copy_len];
        dest.copy_from_slice(tail);

        // Encode total length in last 8 bytes of padding (length strengthening)
        let len_bytes = (len as u64).to_le_bytes();
        last_block[56] = len_bytes[0];
        last_block[57] = len_bytes[1];
        last_block[58] = len_bytes[2];
        last_block[59] = len_bytes[3];
        last_block[60] = len_bytes[4];
        last_block[61] = len_bytes[5];
        last_block[62] = len_bytes[6];
        last_block[63] = len_bytes[7];

        compress(&mut state, &last_block);
    }

    // Convert state words to output bytes (little-endian)
    let mut output = [0u8; 32];
    for i in 0..8 {
        let bytes = state[i].to_le_bytes();
        output[i * 4] = bytes[0];
        output[i * 4 + 1] = bytes[1];
        output[i * 4 + 2] = bytes[2];
        output[i * 4 + 3] = bytes[3];
    }

    Hash256(output)
}
