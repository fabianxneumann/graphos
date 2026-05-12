use crate::math::softmax;

pub fn sample_greedy(logits: &[f32]) -> u32 {
    let mut max_idx = 0u32;
    let mut max_val = logits[0];
    for (i, &v) in logits.iter().enumerate().skip(1) {
        if v > max_val {
            max_val = v;
            max_idx = i as u32;
        }
    }
    max_idx
}

pub fn sample_temperature(logits: &mut [f32], temperature: f32) -> u32 {
    if temperature <= 0.0 || temperature == 1.0 {
        return sample_greedy(logits);
    }
    let inv_t = 1.0 / temperature;
    for l in logits.iter_mut() {
        *l *= inv_t;
    }
    softmax(logits);
    sample_greedy(logits)
}

pub fn sample_top_k(logits: &mut [f32], k: usize) -> u32 {
    if k == 0 || k >= logits.len() {
        return sample_greedy(logits);
    }

    // find the k-th largest value
    let mut threshold = f32::NEG_INFINITY;
    let mut top_indices: alloc::vec::Vec<(usize, f32)> = alloc::vec::Vec::with_capacity(k);

    for (i, &v) in logits.iter().enumerate() {
        if top_indices.len() < k {
            top_indices.push((i, v));
            if top_indices.len() == k {
                top_indices.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
                threshold = top_indices[k - 1].1;
            }
        } else if v > threshold {
            top_indices[k - 1] = (i, v);
            top_indices.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
            threshold = top_indices[k - 1].1;
        }
    }

    // zero out everything below threshold
    for l in logits.iter_mut() {
        if *l < threshold {
            *l = f32::NEG_INFINITY;
        }
    }

    softmax(logits);
    sample_greedy(logits)
}

extern crate alloc;
