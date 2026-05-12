use graphos_core::GraphPool;
use crate::space::{VectorSpace, PhysicsConfig};

/// Execute one physics simulation step.
///
/// Applies three forces:
/// 1. Coulomb repulsion between all node pairs
/// 2. Spring attraction along edges
/// 3. Semantic gravity between similar embeddings
///
/// Then updates velocities (with damping) and positions.
pub fn physics_step(
    space: &mut VectorSpace,
    pool: &GraphPool,
    config: &PhysicsConfig,
) {
    let count = space.count() as usize;
    if count == 0 {
        return;
    }

    // Fixed-size force accumulator (max 4096 nodes)
    const MAX_PHYSICS_NODES: usize = 4096;
    let n = if count > MAX_PHYSICS_NODES { MAX_PHYSICS_NODES } else { count };

    let mut forces_x = [0.0f32; MAX_PHYSICS_NODES];
    let mut forces_y = [0.0f32; MAX_PHYSICS_NODES];
    let mut forces_z = [0.0f32; MAX_PHYSICS_NODES];

    // 1. Coulomb repulsion between all node pairs
    let mut i = 0;
    while i < n {
        let pos_i = match space.get_position(i as u32) {
            Some(p) => *p,
            None => { i += 1; continue; }
        };
        let mut j = i + 1;
        while j < n {
            let pos_j = match space.get_position(j as u32) {
                Some(p) => *p,
                None => { j += 1; continue; }
            };

            let dx = pos_j.x - pos_i.x;
            let dy = pos_j.y - pos_i.y;
            let dz = pos_j.z - pos_i.z;
            let mut dist_sq = dx * dx + dy * dy + dz * dz;
            // Clamp minimum distance
            if dist_sq < 0.01 {
                dist_sq = 0.01;
            }
            let dist = sqrt_f32_local(dist_sq);
            let inv_dist = 1.0 / dist;

            // Force magnitude: coulomb / dist^2
            let force_mag = config.coulomb_constant / dist_sq;

            // Direction from i to j (repulsion pushes them apart)
            let fx = force_mag * dx * inv_dist;
            let fy = force_mag * dy * inv_dist;
            let fz = force_mag * dz * inv_dist;

            // i gets pushed away from j (negative direction)
            forces_x[i] -= fx;
            forces_y[i] -= fy;
            forces_z[i] -= fz;
            // j gets pushed away from i (positive direction)
            forces_x[j] += fx;
            forces_y[j] += fy;
            forces_z[j] += fz;

            j += 1;
        }
        i += 1;
    }

    // 2. Spring attraction along edges
    let edge_count = pool.edge_count();
    let edges = pool.edges_slice();
    let mut e = 0;
    while e < edge_count {
        let edge = &edges[e];
        // Use slab_index approach: find source/target node slab indices
        let src_id = edge.source;
        let tgt_id = edge.target;

        if src_id.is_null() || tgt_id.is_null() {
            e += 1;
            continue;
        }

        // Find slot indices from the pool's node slab_index
        let src_slot = match pool.find_node(src_id) {
            Some(nh) => nh.slab_index,
            None => { e += 1; continue; }
        };
        let tgt_slot = match pool.find_node(tgt_id) {
            Some(nh) => nh.slab_index,
            None => { e += 1; continue; }
        };

        if src_slot as usize >= n || tgt_slot as usize >= n {
            e += 1;
            continue;
        }

        let pos_s = match space.get_position(src_slot) {
            Some(p) => *p,
            None => { e += 1; continue; }
        };
        let pos_t = match space.get_position(tgt_slot) {
            Some(p) => *p,
            None => { e += 1; continue; }
        };

        let dx = pos_t.x - pos_s.x;
        let dy = pos_t.y - pos_s.y;
        let dz = pos_t.z - pos_s.z;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        let dist = sqrt_f32_local(dist_sq);

        if dist < 1e-10 {
            e += 1;
            continue;
        }

        // Spring force: F = spring_constant * (dist - rest_length)
        let rest_length = 1.0;
        let force_mag = config.spring_constant * (dist - rest_length);
        let inv_dist = 1.0 / dist;

        let fx = force_mag * dx * inv_dist;
        let fy = force_mag * dy * inv_dist;
        let fz = force_mag * dz * inv_dist;

        // Attract source toward target
        forces_x[src_slot as usize] += fx;
        forces_y[src_slot as usize] += fy;
        forces_z[src_slot as usize] += fz;
        // Attract target toward source
        forces_x[tgt_slot as usize] -= fx;
        forces_y[tgt_slot as usize] -= fy;
        forces_z[tgt_slot as usize] -= fz;

        e += 1;
    }

    // 3. Semantic gravity between similar embeddings
    i = 0;
    while i < n {
        let emb_i = match space.get_embedding(i as u32) {
            Some(e) => e,
            None => { i += 1; continue; }
        };
        let pos_i = match space.get_position(i as u32) {
            Some(p) => *p,
            None => { i += 1; continue; }
        };

        let mut j = i + 1;
        while j < n {
            let emb_j = match space.get_embedding(j as u32) {
                Some(e) => e,
                None => { j += 1; continue; }
            };

            let sim = emb_i.cosine_similarity(emb_j);
            if sim > config.similarity_threshold {
                let pos_j = match space.get_position(j as u32) {
                    Some(p) => *p,
                    None => { j += 1; continue; }
                };

                let dx = pos_j.x - pos_i.x;
                let dy = pos_j.y - pos_i.y;
                let dz = pos_j.z - pos_i.z;
                let dist_sq = dx * dx + dy * dy + dz * dz;
                let dist = sqrt_f32_local(dist_sq);

                if dist > 1e-10 {
                    let inv_dist = 1.0 / dist;
                    // Attract proportional to similarity
                    let force_mag = config.semantic_gravity * sim;
                    let fx = force_mag * dx * inv_dist;
                    let fy = force_mag * dy * inv_dist;
                    let fz = force_mag * dz * inv_dist;

                    forces_x[i] += fx;
                    forces_y[i] += fy;
                    forces_z[i] += fz;
                    forces_x[j] -= fx;
                    forces_y[j] -= fy;
                    forces_z[j] -= fz;
                }
            }
            j += 1;
        }
        i += 1;
    }

    // 4 & 5. Update velocities (with damping) and positions
    i = 0;
    while i < n {
        if let Some(vel) = space.get_velocity_mut(i as u32) {
            vel.dx = (vel.dx + forces_x[i] * config.dt) * config.damping;
            vel.dy = (vel.dy + forces_y[i] * config.dt) * config.damping;
            vel.dz = (vel.dz + forces_z[i] * config.dt) * config.damping;

            let vdx = vel.dx;
            let vdy = vel.dy;
            let vdz = vel.dz;

            if let Some(pos) = space.get_position_mut(i as u32) {
                pos.x += vdx * config.dt;
                pos.y += vdy * config.dt;
                pos.z += vdz * config.dt;
            }
        }
        i += 1;
    }
}

fn sqrt_f32_local(x: f32) -> f32 {
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
