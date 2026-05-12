use crate::embedding::EmbeddingVector;

/// Approximate square root (duplicated here to avoid cross-module private fn)
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let i = f32::to_bits(x);
    let i = 0x5f3759df - (i >> 1);
    let mut guess = 1.0 / f32::from_bits(i);
    guess = 0.5 * (guess + x / guess);
    guess = 0.5 * (guess + x / guess);
    guess = 0.5 * (guess + x / guess);
    guess
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Position3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Velocity3D {
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
}

impl Position3D {
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub fn distance_to(&self, other: &Self) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        sqrt_f32(dx * dx + dy * dy + dz * dz)
    }

    /// Normalized direction vector from self to other.
    /// Returns (0,0,0) if positions are identical.
    pub fn direction_to(&self, other: &Self) -> (f32, f32, f32) {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        let dist = sqrt_f32(dx * dx + dy * dy + dz * dz);
        if dist < 1e-10 {
            return (0.0, 0.0, 0.0);
        }
        let inv = 1.0 / dist;
        (dx * inv, dy * inv, dz * inv)
    }
}

impl Velocity3D {
    pub const ZERO: Self = Self { dx: 0.0, dy: 0.0, dz: 0.0 };
}

/// Physics simulation configuration
pub struct PhysicsConfig {
    pub coulomb_constant: f32,
    pub spring_constant: f32,
    pub damping: f32,
    pub semantic_gravity: f32,
    pub dt: f32,
    pub similarity_threshold: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            coulomb_constant: 100.0,
            spring_constant: 0.1,
            damping: 0.95,
            semantic_gravity: 5.0,
            dt: 0.016,
            similarity_threshold: 0.5,
        }
    }
}

/// The vector space — parallel arrays indexed by node slot
pub struct VectorSpace {
    embeddings: *mut EmbeddingVector,
    positions: *mut Position3D,
    velocities: *mut Velocity3D,
    capacity: u32,
    count: u32,
}

unsafe impl Send for VectorSpace {}
unsafe impl Sync for VectorSpace {}

impl VectorSpace {
    /// Initialize from pre-allocated buffers.
    ///
    /// # Safety
    /// All buffers must be valid for `capacity` elements and properly aligned.
    pub unsafe fn new(
        emb_buf: *mut EmbeddingVector,
        pos_buf: *mut Position3D,
        vel_buf: *mut Velocity3D,
        capacity: u32,
    ) -> Self {
        Self {
            embeddings: emb_buf,
            positions: pos_buf,
            velocities: vel_buf,
            capacity,
            count: 0,
        }
    }

    /// Set embedding for slot i
    pub fn set_embedding(&mut self, index: u32, emb: EmbeddingVector) {
        if index < self.count {
            unsafe {
                *self.embeddings.add(index as usize) = emb;
            }
        }
    }

    /// Get embedding for slot i
    pub fn get_embedding(&self, index: u32) -> Option<&EmbeddingVector> {
        if index < self.count {
            unsafe { Some(&*self.embeddings.add(index as usize)) }
        } else {
            None
        }
    }

    /// Set position for slot i
    pub fn set_position(&mut self, index: u32, pos: Position3D) {
        if index < self.count {
            unsafe {
                *self.positions.add(index as usize) = pos;
            }
        }
    }

    /// Get position for slot i
    pub fn get_position(&self, index: u32) -> Option<&Position3D> {
        if index < self.count {
            unsafe { Some(&*self.positions.add(index as usize)) }
        } else {
            None
        }
    }

    /// Get mutable position for slot i
    pub fn get_position_mut(&mut self, index: u32) -> Option<&mut Position3D> {
        if index < self.count {
            unsafe { Some(&mut *self.positions.add(index as usize)) }
        } else {
            None
        }
    }

    /// Get velocity for slot i
    pub fn get_velocity(&self, index: u32) -> Option<&Velocity3D> {
        if index < self.count {
            unsafe { Some(&*self.velocities.add(index as usize)) }
        } else {
            None
        }
    }

    /// Get mutable velocity for slot i
    pub fn get_velocity_mut(&mut self, index: u32) -> Option<&mut Velocity3D> {
        if index < self.count {
            unsafe { Some(&mut *self.velocities.add(index as usize)) }
        } else {
            None
        }
    }

    /// Register a new node (assigns next slot).
    /// Initializes embedding from seed, position spread out, velocity zero.
    pub fn register_node(&mut self, seed: u64) -> Option<u32> {
        if self.count >= self.capacity {
            return None;
        }
        let slot = self.count;
        self.count += 1;

        let emb = EmbeddingVector::from_seed(seed);
        unsafe {
            *self.embeddings.add(slot as usize) = emb;
            // Spread nodes in a deterministic pattern based on slot
            let angle1 = (slot as f32) * 2.399; // golden angle
            let angle2 = (slot as f32) * 0.618;
            let radius = 5.0 + (slot as f32) * 0.5;
            let pos = Position3D {
                x: radius * cos_approx(angle1),
                y: radius * sin_approx(angle1),
                z: radius * cos_approx(angle2),
            };
            *self.positions.add(slot as usize) = pos;
            *self.velocities.add(slot as usize) = Velocity3D::ZERO;
        }

        Some(slot)
    }

    /// Number of registered nodes
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Capacity of the space
    pub fn capacity(&self) -> u32 {
        self.capacity
    }
}

/// Sine approximation (Bhaskara I formula, good enough for layout)
fn sin_approx(x: f32) -> f32 {
    // Normalize to [0, 2*PI) range
    const TWO_PI: f32 = 6.283185;
    const PI: f32 = 3.141593;
    let mut a = x;
    // Reduce to [0, 2pi)
    while a < 0.0 {
        a += TWO_PI;
    }
    while a >= TWO_PI {
        a -= TWO_PI;
    }
    // Now a in [0, 2pi)
    let sign = if a > PI { -1.0 } else { 1.0 };
    if a > PI {
        a -= PI;
    }
    // Bhaskara I: sin(x) ≈ 16x(pi-x) / (5*pi^2 - 4x(pi-x))
    let num = 16.0 * a * (PI - a);
    let den = 5.0 * PI * PI - 4.0 * a * (PI - a);
    if den < 1e-10 {
        return 0.0;
    }
    sign * num / den
}

/// Cosine approximation
fn cos_approx(x: f32) -> f32 {
    const HALF_PI: f32 = 1.570796;
    sin_approx(x + HALF_PI)
}
