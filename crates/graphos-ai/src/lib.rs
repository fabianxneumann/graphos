#![no_std]
extern crate alloc;

pub mod math;
pub mod tensor;
pub mod gguf;
pub mod tokenizer;
pub mod transformer;
pub mod sampler;
pub mod engine;
pub mod demo;

pub use engine::InferenceEngine;
pub use gguf::ModelConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiError {
    InvalidFormat,
    UnsupportedVersion,
    TensorNotFound,
    OutOfMemory,
    InvalidToken,
}
