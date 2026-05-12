extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::gguf::GGUFModel;

pub struct Tokenizer {
    pub vocab: Vec<Vec<u8>>,
    pub scores: Vec<f32>,
    pub vocab_size: u32,
}

impl Tokenizer {
    pub fn from_gguf(model: &GGUFModel) -> Self {
        Self {
            vocab: model.vocab.clone(),
            scores: model.scores.clone(),
            vocab_size: model.config.vocab_size as u32,
        }
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Vec::new();
        }

        let mut tokens: Vec<u32> = Vec::with_capacity(bytes.len());
        for &b in bytes {
            let tid = self.find_single_byte(b);
            tokens.push(tid);
        }

        loop {
            let mut best_score = f32::NEG_INFINITY;
            let mut best_idx = usize::MAX;
            let mut best_token = 0u32;

            for i in 0..tokens.len().saturating_sub(1) {
                let merged = self.try_merge(tokens[i], tokens[i + 1]);
                if let Some((tid, score)) = merged {
                    if score > best_score {
                        best_score = score;
                        best_idx = i;
                        best_token = tid;
                    }
                }
            }

            if best_idx == usize::MAX {
                break;
            }

            tokens[best_idx] = best_token;
            tokens.remove(best_idx + 1);
        }

        tokens
    }

    pub fn decode(&self, tokens: &[u32]) -> String {
        let mut bytes: Vec<u8> = Vec::new();
        for &tid in tokens {
            if (tid as usize) < self.vocab.len() {
                bytes.extend_from_slice(&self.vocab[tid as usize]);
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn find_single_byte(&self, b: u8) -> u32 {
        for (i, token) in self.vocab.iter().enumerate() {
            if token.len() == 1 && token[0] == b {
                return i as u32;
            }
        }
        0
    }

    fn try_merge(&self, t1: u32, t2: u32) -> Option<(u32, f32)> {
        if t1 as usize >= self.vocab.len() || t2 as usize >= self.vocab.len() {
            return None;
        }
        let mut merged = Vec::with_capacity(
            self.vocab[t1 as usize].len() + self.vocab[t2 as usize].len(),
        );
        merged.extend_from_slice(&self.vocab[t1 as usize]);
        merged.extend_from_slice(&self.vocab[t2 as usize]);

        for (i, token) in self.vocab.iter().enumerate() {
            if token == &merged {
                let score = if i < self.scores.len() {
                    self.scores[i]
                } else {
                    0.0
                };
                return Some((i as u32, score));
            }
        }
        None
    }
}
