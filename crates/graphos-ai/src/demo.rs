extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub struct DemoEngine {
    embed: Vec<Vec<f32>>,
    proj: Vec<Vec<f32>>,
    vocab_size: usize,
    dim: usize,
}

impl DemoEngine {
    pub fn new() -> Self {
        let vocab_size = 128;
        let dim = 32;

        let mut embed = Vec::with_capacity(vocab_size);
        for i in 0..vocab_size {
            let mut row = vec![0.0f32; dim];
            for j in 0..dim {
                let seed = ((i * 7 + j * 13) % 97) as f32;
                row[j] = (seed - 48.0) * 0.05;
            }
            embed.push(row);
        }

        let mut proj = Vec::with_capacity(vocab_size);
        for i in 0..vocab_size {
            let mut row = vec![0.0f32; dim];
            for j in 0..dim {
                let seed = ((i * 11 + j * 3) % 89) as f32;
                row[j] = (seed - 44.0) * 0.04;
            }
            // bias toward printable ASCII
            if (32..=126).contains(&i) {
                for j in 0..dim {
                    row[j] += 0.01;
                }
            }
            proj.push(row);
        }

        Self { embed, proj, vocab_size, dim }
    }

    pub fn generate(&self, prompt: &str, max_tokens: u32) -> String {
        let mut hidden = vec![0.0f32; self.dim];

        // encode prompt: accumulate embeddings
        for &b in prompt.as_bytes() {
            let idx = (b as usize) % self.vocab_size;
            for j in 0..self.dim {
                hidden[j] = hidden[j] * 0.8 + self.embed[idx][j] * 0.2;
            }
        }

        let mut output = String::new();

        for step in 0..max_tokens {
            // project hidden -> logits
            let mut best_score = f32::NEG_INFINITY;
            let mut best_idx: usize = 32; // default: space

            for i in 0..self.vocab_size {
                let mut score = 0.0f32;
                for j in 0..self.dim {
                    score += hidden[j] * self.proj[i][j];
                }
                // temperature-like perturbation based on step
                score += ((step as f32 * 0.1 + i as f32 * 0.01) % 0.3) - 0.15;

                if score > best_score {
                    best_score = score;
                    best_idx = i;
                }
            }

            // output token
            if best_idx < 128 {
                let ch = best_idx as u8;
                if ch.is_ascii_graphic() || ch == b' ' {
                    output.push(ch as char);
                } else {
                    output.push('.');
                }
            }

            // update hidden state with new token embedding
            let idx = best_idx % self.vocab_size;
            for j in 0..self.dim {
                hidden[j] = hidden[j] * 0.7 + self.embed[idx][j] * 0.3;
            }
        }

        output
    }
}
