use crate::math::sqrt_f32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Q8Block {
    pub scale: u16,
    pub quants: [i8; 32],
}

pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let frac = (h & 0x3FF) as u32;

    if exp == 0 {
        if frac == 0 {
            return f32::from_bits(sign << 31);
        }
        // subnormal
        let mut e = 0i32;
        let mut f = frac;
        while (f & 0x400) == 0 {
            f <<= 1;
            e -= 1;
        }
        f &= 0x3FF;
        let exp32 = (127 - 15 + 1 + e) as u32;
        return f32::from_bits((sign << 31) | (exp32 << 23) | (f << 13));
    }
    if exp == 31 {
        let bits = (sign << 31) | (0xFF << 23) | (frac << 13);
        return f32::from_bits(bits);
    }

    let exp32 = exp + 127 - 15;
    f32::from_bits((sign << 31) | (exp32 << 23) | (frac << 13))
}

pub fn matmul_f32(out: &mut [f32], x: &[f32], w: &[f32], n: usize, d: usize) {
    for i in 0..d {
        let mut sum = 0.0f32;
        let base = i * n;
        for j in 0..n {
            sum += w[base + j] * x[j];
        }
        out[i] = sum;
    }
}

pub fn matmul_q8(out: &mut [f32], x: &[f32], w: &[Q8Block], n: usize, d: usize) {
    let blocks_per_row = n / 32;
    for i in 0..d {
        let mut sum = 0.0f32;
        for b in 0..blocks_per_row {
            let block = &w[i * blocks_per_row + b];
            let scale = f16_to_f32(block.scale);
            let quants = block.quants;
            let x_offset = b * 32;
            for k in 0..32 {
                sum += (quants[k] as f32) * scale * x[x_offset + k];
            }
        }
        out[i] = sum;
    }
}

#[allow(dead_code)]
pub fn embed_token(out: &mut [f32], embedding: &[f32], token: u32, dim: usize) {
    let offset = token as usize * dim;
    for i in 0..dim {
        out[i] = embedding[offset + i];
    }
}

#[allow(dead_code)]
pub fn scaled_dot_product_attention(
    out: &mut [f32],
    q: &[f32],
    key_cache: &[f32],
    value_cache: &[f32],
    head_dim: usize,
    seq_len: usize,
    kv_stride: usize,
    att_buf: &mut [f32],
) {
    let scale = 1.0 / sqrt_f32(head_dim as f32);

    for t in 0..seq_len {
        let mut score = 0.0f32;
        let k_base = t * kv_stride;
        for d in 0..head_dim {
            score += q[d] * key_cache[k_base + d];
        }
        att_buf[t] = score * scale;
    }

    crate::math::softmax(&mut att_buf[..seq_len]);

    for d in 0..head_dim {
        let mut val = 0.0f32;
        for t in 0..seq_len {
            val += att_buf[t] * value_cache[t * kv_stride + d];
        }
        out[d] = val;
    }
}
