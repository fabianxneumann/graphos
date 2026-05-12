extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::gguf::{self, GGUFModel, ModelConfig};
use crate::sampler::sample_greedy;
use crate::tokenizer::Tokenizer;
use crate::transformer::{self, forward, InferenceState, TransformerWeights};
use crate::AiError;

pub struct InferenceEngine {
    pub config: ModelConfig,
    pub weights: TransformerWeights,
    pub state: InferenceState,
    pub tokenizer: Tokenizer,
    pub loaded: bool,
}

unsafe impl Send for InferenceEngine {}
unsafe impl Sync for InferenceEngine {}

impl InferenceEngine {
    pub fn from_gguf_data(data: &'static [u8]) -> Result<Self, AiError> {
        let model = gguf::parse_gguf(data)?;
        let weights = transformer::load_weights(&model, data)?;
        let state = InferenceState::new(&model.config);
        let tokenizer = Tokenizer::from_gguf(&model);
        let config = model.config.clone();

        Ok(Self {
            config,
            weights,
            state,
            tokenizer,
            loaded: true,
        })
    }

    pub fn generate(&mut self, prompt: &str, max_tokens: u32) -> String {
        let tokens = self.tokenizer.encode(prompt);
        let mut output_tokens: Vec<u32> = Vec::new();

        if tokens.is_empty() {
            return String::new();
        }

        // prefill: process prompt tokens
        for (i, &tok) in tokens.iter().enumerate() {
            forward(
                &mut self.state,
                &self.weights,
                &self.config,
                tok,
                i as u32,
            );
        }

        let mut pos = tokens.len() as u32;
        let mut next_token = sample_greedy(self.state.logits());

        for _ in 0..max_tokens {
            output_tokens.push(next_token);
            if next_token == 2 {
                break; // EOS
            }
            if pos >= self.config.context_len as u32 {
                break;
            }
            forward(
                &mut self.state,
                &self.weights,
                &self.config,
                next_token,
                pos,
            );
            next_token = sample_greedy(self.state.logits());
            pos += 1;
        }

        self.tokenizer.decode(&output_tokens)
    }

    pub fn generate_with_temperature(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> String {
        let tokens = self.tokenizer.encode(prompt);
        let mut output_tokens: Vec<u32> = Vec::new();

        if tokens.is_empty() {
            return String::new();
        }

        for (i, &tok) in tokens.iter().enumerate() {
            forward(
                &mut self.state,
                &self.weights,
                &self.config,
                tok,
                i as u32,
            );
        }

        let mut pos = tokens.len() as u32;
        let mut next_token =
            crate::sampler::sample_temperature(&mut self.state.logits.clone(), temperature);

        for _ in 0..max_tokens {
            output_tokens.push(next_token);
            if next_token == 2 {
                break;
            }
            if pos >= self.config.context_len as u32 {
                break;
            }
            forward(
                &mut self.state,
                &self.weights,
                &self.config,
                next_token,
                pos,
            );
            let mut logits_copy = self.state.logits.clone();
            next_token = crate::sampler::sample_temperature(&mut logits_copy, temperature);
            pos += 1;
        }

        self.tokenizer.decode(&output_tokens)
    }

    pub fn reset(&mut self) {
        self.state.reset();
    }
}
