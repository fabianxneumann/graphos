extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::gguf::{GGUFModel, ModelConfig, TensorInfo};
use crate::math::{rmsnorm, rope, silu, softmax};
use crate::tensor::{f16_to_f32, matmul_q8, Q8Block};
use crate::AiError;

pub struct LayerWeights {
    pub rms_att: *const f32,
    pub wq: *const Q8Block,
    pub wk: *const Q8Block,
    pub wv: *const Q8Block,
    pub wo: *const Q8Block,
    pub rms_ffn: *const f32,
    pub w1: *const Q8Block,
    pub w2: *const Q8Block,
    pub w3: *const Q8Block,
}

unsafe impl Send for LayerWeights {}
unsafe impl Sync for LayerWeights {}

pub struct TransformerWeights {
    pub token_embedding: *const f32,
    pub layers: Vec<LayerWeights>,
    pub rms_final: *const f32,
    pub output: *const Q8Block,
    pub output_is_f32: bool,
    pub output_f32: *const f32,
}

unsafe impl Send for TransformerWeights {}
unsafe impl Sync for TransformerWeights {}

pub struct InferenceState {
    pub x: Vec<f32>,
    pub xb: Vec<f32>,
    pub xb2: Vec<f32>,
    pub q: Vec<f32>,
    pub k: Vec<f32>,
    pub v: Vec<f32>,
    pub att: Vec<f32>,
    pub hb: Vec<f32>,
    pub hb2: Vec<f32>,
    pub logits: Vec<f32>,
    pub key_cache: Vec<f32>,
    pub value_cache: Vec<f32>,
    pub pos: u32,
}

impl InferenceState {
    pub fn new(config: &ModelConfig) -> Self {
        let dim = config.embed_dim;
        let kv_dim = (dim / config.n_heads) * config.n_kv_heads;
        let cache_size = config.n_layers * config.context_len * kv_dim;

        Self {
            x: vec![0.0; dim],
            xb: vec![0.0; dim],
            xb2: vec![0.0; dim],
            q: vec![0.0; dim],
            k: vec![0.0; kv_dim],
            v: vec![0.0; kv_dim],
            att: vec![0.0; config.n_heads * config.context_len],
            hb: vec![0.0; config.intermediate_size],
            hb2: vec![0.0; config.intermediate_size],
            logits: vec![0.0; config.vocab_size],
            key_cache: vec![0.0; cache_size],
            value_cache: vec![0.0; cache_size],
            pos: 0,
        }
    }

    pub fn logits(&self) -> &[f32] {
        &self.logits
    }

    pub fn reset(&mut self) {
        self.pos = 0;
        for v in self.key_cache.iter_mut() {
            *v = 0.0;
        }
        for v in self.value_cache.iter_mut() {
            *v = 0.0;
        }
    }
}

fn find_tensor<'a>(tensors: &'a [TensorInfo], name: &str) -> Option<&'a TensorInfo> {
    tensors.iter().find(|t| t.name == name)
}

pub fn load_weights(model: &GGUFModel, data: &[u8]) -> Result<TransformerWeights, AiError> {
    let base = data.as_ptr() as usize + model.data_offset;
    let tensors = &model.tensors;
    let config = &model.config;

    let token_embedding = match find_tensor(tensors, "token_embd.weight") {
        Some(t) => (base + t.offset as usize) as *const f32,
        None => return Err(AiError::TensorNotFound),
    };

    let rms_final = match find_tensor(tensors, "output_norm.weight") {
        Some(t) => (base + t.offset as usize) as *const f32,
        None => return Err(AiError::TensorNotFound),
    };

    let (output, output_is_f32, output_f32) = match find_tensor(tensors, "output.weight") {
        Some(t) => {
            if t.tensor_type == 8 {
                // Q8_0
                (
                    (base + t.offset as usize) as *const Q8Block,
                    false,
                    core::ptr::null(),
                )
            } else {
                // F32 or F16 treated as f32 pointer
                (
                    core::ptr::null(),
                    true,
                    (base + t.offset as usize) as *const f32,
                )
            }
        }
        None => {
            // weight tying: output = token_embedding
            (core::ptr::null(), true, token_embedding)
        }
    };

    let mut layers = Vec::with_capacity(config.n_layers);
    for l in 0..config.n_layers {
        let prefix = alloc::format!("blk.{}", l);

        let rms_att = match find_tensor(tensors, &alloc::format!("{}.attn_norm.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const f32,
            None => return Err(AiError::TensorNotFound),
        };

        let wq = match find_tensor(tensors, &alloc::format!("{}.attn_q.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const Q8Block,
            None => return Err(AiError::TensorNotFound),
        };

        let wk = match find_tensor(tensors, &alloc::format!("{}.attn_k.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const Q8Block,
            None => return Err(AiError::TensorNotFound),
        };

        let wv = match find_tensor(tensors, &alloc::format!("{}.attn_v.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const Q8Block,
            None => return Err(AiError::TensorNotFound),
        };

        let wo = match find_tensor(tensors, &alloc::format!("{}.attn_output.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const Q8Block,
            None => return Err(AiError::TensorNotFound),
        };

        let rms_ffn = match find_tensor(tensors, &alloc::format!("{}.ffn_norm.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const f32,
            None => return Err(AiError::TensorNotFound),
        };

        let w1 = match find_tensor(tensors, &alloc::format!("{}.ffn_gate.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const Q8Block,
            None => return Err(AiError::TensorNotFound),
        };

        let w2 = match find_tensor(tensors, &alloc::format!("{}.ffn_down.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const Q8Block,
            None => return Err(AiError::TensorNotFound),
        };

        let w3 = match find_tensor(tensors, &alloc::format!("{}.ffn_up.weight", prefix)) {
            Some(t) => (base + t.offset as usize) as *const Q8Block,
            None => return Err(AiError::TensorNotFound),
        };

        layers.push(LayerWeights {
            rms_att,
            wq,
            wk,
            wv,
            wo,
            rms_ffn,
            w1,
            w2,
            w3,
        });
    }

    Ok(TransformerWeights {
        token_embedding,
        layers,
        rms_final,
        output,
        output_is_f32,
        output_f32,
    })
}

pub fn forward(
    state: &mut InferenceState,
    weights: &TransformerWeights,
    config: &ModelConfig,
    token: u32,
    pos: u32,
) {
    let dim = config.embed_dim;
    let head_dim = dim / config.n_heads;
    let kv_dim = head_dim * config.n_kv_heads;
    let kv_mul = config.n_heads / config.n_kv_heads;
    let seq_pos = pos as usize;

    // token embedding lookup
    unsafe {
        let emb = weights.token_embedding.add(token as usize * dim);
        for i in 0..dim {
            state.x[i] = *emb.add(i);
        }
    }

    for l in 0..config.n_layers {
        let layer = &weights.layers[l];

        // attention rmsnorm
        let rms_w = unsafe { core::slice::from_raw_parts(layer.rms_att, dim) };
        rmsnorm(&mut state.xb, &state.x, rms_w, 1e-5);

        // QKV projections
        let wq = unsafe {
            core::slice::from_raw_parts(layer.wq, dim * dim / 32)
        };
        matmul_q8(&mut state.q, &state.xb, wq, dim, dim);

        let wk = unsafe {
            core::slice::from_raw_parts(layer.wk, dim * kv_dim / 32)
        };
        matmul_q8(&mut state.k, &state.xb, wk, dim, kv_dim);

        let wv = unsafe {
            core::slice::from_raw_parts(layer.wv, dim * kv_dim / 32)
        };
        matmul_q8(&mut state.v, &state.xb, wv, dim, kv_dim);

        // RoPE
        for h in 0..config.n_kv_heads {
            let q_start = h * kv_mul * head_dim;
            let k_start = h * head_dim;
            // apply RoPE to each query head in this kv group
            for m in 0..kv_mul {
                let qs = q_start + m * head_dim;
                let q_slice = &mut state.q[qs..qs + head_dim];
                let k_slice = &mut state.k[k_start..k_start + head_dim];
                if m == 0 {
                    rope(q_slice, k_slice, head_dim, pos, config.rope_theta);
                } else {
                    // only apply RoPE to q for additional heads sharing same kv
                    let mut dummy_k = vec![0.0f32; head_dim];
                    rope(q_slice, &mut dummy_k, head_dim, pos, config.rope_theta);
                }
            }
        }

        // write K,V to cache
        let cache_offset = l * config.context_len * kv_dim + seq_pos * kv_dim;
        state.key_cache[cache_offset..cache_offset + kv_dim]
            .copy_from_slice(&state.k[..kv_dim]);
        state.value_cache[cache_offset..cache_offset + kv_dim]
            .copy_from_slice(&state.v[..kv_dim]);

        // multi-head attention
        for h in 0..config.n_heads {
            let q_offset = h * head_dim;
            let kv_head = h / kv_mul;
            let att_offset = h * config.context_len;

            for t in 0..=seq_pos {
                let k_cache_base = l * config.context_len * kv_dim + t * kv_dim + kv_head * head_dim;
                let mut score = 0.0f32;
                for d in 0..head_dim {
                    score += state.q[q_offset + d] * state.key_cache[k_cache_base + d];
                }
                state.att[att_offset + t] = score / crate::math::sqrt_f32(head_dim as f32);
            }

            softmax(&mut state.att[att_offset..att_offset + seq_pos + 1]);

            // weighted sum of values
            let xb_offset = q_offset;
            for d in 0..head_dim {
                let mut val = 0.0f32;
                for t in 0..=seq_pos {
                    let v_cache_base = l * config.context_len * kv_dim + t * kv_dim + kv_head * head_dim;
                    val += state.att[att_offset + t] * state.value_cache[v_cache_base + d];
                }
                state.xb2[xb_offset + d] = val;
            }
        }

        // output projection
        let wo = unsafe {
            core::slice::from_raw_parts(layer.wo, dim * dim / 32)
        };
        matmul_q8(&mut state.xb, &state.xb2, wo, dim, dim);

        // residual
        for i in 0..dim {
            state.x[i] += state.xb[i];
        }

        // FFN rmsnorm
        let rms_ffn_w = unsafe { core::slice::from_raw_parts(layer.rms_ffn, dim) };
        rmsnorm(&mut state.xb, &state.x, rms_ffn_w, 1e-5);

        // FFN: gate = silu(w1 @ x), up = w3 @ x, out = w2 @ (gate * up)
        let w1 = unsafe {
            core::slice::from_raw_parts(layer.w1, config.intermediate_size * dim / 32)
        };
        matmul_q8(&mut state.hb, &state.xb, w1, dim, config.intermediate_size);

        let w3 = unsafe {
            core::slice::from_raw_parts(layer.w3, config.intermediate_size * dim / 32)
        };
        matmul_q8(&mut state.hb2, &state.xb, w3, dim, config.intermediate_size);

        for i in 0..config.intermediate_size {
            state.hb[i] = silu(state.hb[i]) * state.hb2[i];
        }

        let w2 = unsafe {
            core::slice::from_raw_parts(layer.w2, dim * config.intermediate_size / 32)
        };
        matmul_q8(&mut state.xb, &state.hb, w2, config.intermediate_size, dim);

        // residual
        for i in 0..dim {
            state.x[i] += state.xb[i];
        }
    }

    // final rmsnorm
    let rms_final_w = unsafe { core::slice::from_raw_parts(weights.rms_final, dim) };
    rmsnorm(&mut state.xb, &state.x, rms_final_w, 1e-5);

    // output projection to logits
    if weights.output_is_f32 {
        let w = unsafe { core::slice::from_raw_parts(weights.output_f32, config.vocab_size * dim) };
        crate::tensor::matmul_f32(&mut state.logits, &state.xb, w, dim, config.vocab_size);
    } else {
        let w = unsafe {
            core::slice::from_raw_parts(weights.output, config.vocab_size * dim / 32)
        };
        matmul_q8(&mut state.logits, &state.xb, w, dim, config.vocab_size);
    }
}
