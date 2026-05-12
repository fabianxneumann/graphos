/// Approximate square root using fast inverse sqrt + Newton-Raphson iterations.
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let i = f32::to_bits(x);
    let i = 0x5f3759df - (i >> 1);
    let mut guess = 1.0 / f32::from_bits(i);
    // 3 Newton-Raphson iterations
    guess = 0.5 * (guess + x / guess);
    guess = 0.5 * (guess + x / guess);
    guess = 0.5 * (guess + x / guess);
    guess
}

/// 64-dimensional embedding vector (256 bytes)
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct EmbeddingVector {
    pub dims: [f32; 64],
}

impl EmbeddingVector {
    pub const DIMS: usize = 64;

    pub const fn zero() -> Self {
        Self { dims: [0.0; 64] }
    }

    /// Deterministic "random" from seed using LCG, then normalize.
    pub fn from_seed(seed: u64) -> Self {
        let mut state = seed;
        if state == 0 {
            state = 0xDEAD_BEEF_CAFE_BABE;
        }
        let mut dims = [0.0f32; 64];
        for i in 0..64 {
            // LCG: state = state * a + c
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            // Map to [-1.0, 1.0]
            let bits = (state >> 33) as u32; // top 31 bits
            dims[i] = (bits as f32 / (0x7FFF_FFFF_u32 as f32)) * 2.0 - 1.0;
        }
        let mut v = Self { dims };
        v.normalize();
        v
    }

    /// Cosine similarity [-1.0, 1.0]
    pub fn cosine_similarity(&self, other: &Self) -> f32 {
        let d = self.dot(other);
        let na = self.norm();
        let nb = other.norm();
        let denom = na * nb;
        if denom < 1e-10 {
            return 0.0;
        }
        d / denom
    }

    /// L2 (Euclidean) distance
    pub fn l2_distance(&self, other: &Self) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..64 {
            let diff = self.dims[i] - other.dims[i];
            sum += diff * diff;
        }
        sqrt_f32(sum)
    }

    /// Dot product
    pub fn dot(&self, other: &Self) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..64 {
            sum += self.dims[i] * other.dims[i];
        }
        sum
    }

    /// L2 norm (magnitude)
    pub fn norm(&self) -> f32 {
        sqrt_f32(self.dot(self))
    }

    /// Normalize to unit length
    pub fn normalize(&mut self) {
        let n = self.norm();
        if n < 1e-10 {
            return;
        }
        let inv = 1.0 / n;
        for i in 0..64 {
            self.dims[i] *= inv;
        }
    }

    /// Add another vector scaled by factor: self += other * scale
    pub fn add_scaled(&mut self, other: &Self, scale: f32) {
        for i in 0..64 {
            self.dims[i] += other.dims[i] * scale;
        }
    }

    /// Scale all dimensions
    pub fn scale(&mut self, factor: f32) {
        for i in 0..64 {
            self.dims[i] *= factor;
        }
    }
}
