extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::mem;

use crate::AiError;

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub embed_dim: usize,
    pub n_layers: usize,
    pub n_heads: usize,
    pub n_kv_heads: usize,
    pub vocab_size: usize,
    pub context_len: usize,
    pub intermediate_size: usize,
    pub rope_theta: f32,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            embed_dim: 256,
            n_layers: 12,
            n_heads: 4,
            n_kv_heads: 4,
            vocab_size: 32000,
            context_len: 512,
            intermediate_size: 688,
            rope_theta: 10000.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TensorInfo {
    pub name: String,
    pub n_dims: u32,
    pub dims: [u64; 4],
    pub tensor_type: u32,
    pub offset: u64,
}

pub struct GGUFModel {
    pub config: ModelConfig,
    pub tensors: Vec<TensorInfo>,
    pub vocab: Vec<Vec<u8>>,
    pub scores: Vec<f32>,
    pub data_offset: usize,
}

const GGUF_MAGIC: u32 = 0x46475547;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> Result<u8, AiError> {
        if self.remaining() < 1 {
            return Err(AiError::InvalidFormat);
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u32(&mut self) -> Result<u32, AiError> {
        if self.remaining() < 4 {
            return Err(AiError::InvalidFormat);
        }
        let v = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_u64(&mut self) -> Result<u64, AiError> {
        if self.remaining() < 8 {
            return Err(AiError::InvalidFormat);
        }
        let v = u64::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    fn read_i32(&mut self) -> Result<i32, AiError> {
        Ok(self.read_u32()? as i32)
    }

    fn read_f32(&mut self) -> Result<f32, AiError> {
        let bits = self.read_u32()?;
        Ok(f32::from_bits(bits))
    }

    fn read_string(&mut self) -> Result<String, AiError> {
        let len = self.read_u64()? as usize;
        if self.remaining() < len {
            return Err(AiError::InvalidFormat);
        }
        let bytes = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], AiError> {
        if self.remaining() < len {
            return Err(AiError::InvalidFormat);
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn skip(&mut self, n: usize) -> Result<(), AiError> {
        if self.remaining() < n {
            return Err(AiError::InvalidFormat);
        }
        self.pos += n;
        Ok(())
    }

    fn align(&mut self, alignment: usize) {
        let rem = self.pos % alignment;
        if rem != 0 {
            self.pos += alignment - rem;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
enum GGUFValueType {
    Uint8 = 0,
    Int8 = 1,
    Uint16 = 2,
    Int16 = 3,
    Uint32 = 4,
    Int32 = 5,
    Float32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    Uint64 = 10,
    Int64 = 11,
    Float64 = 12,
}

impl GGUFValueType {
    fn from_u32(v: u32) -> Result<Self, AiError> {
        match v {
            0 => Ok(Self::Uint8),
            1 => Ok(Self::Int8),
            2 => Ok(Self::Uint16),
            3 => Ok(Self::Int16),
            4 => Ok(Self::Uint32),
            5 => Ok(Self::Int32),
            6 => Ok(Self::Float32),
            7 => Ok(Self::Bool),
            8 => Ok(Self::String),
            9 => Ok(Self::Array),
            10 => Ok(Self::Uint64),
            11 => Ok(Self::Int64),
            12 => Ok(Self::Float64),
            _ => Err(AiError::InvalidFormat),
        }
    }
}

enum MetaValue {
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Uint64(u64),
    StringVal(String),
    StringArray(Vec<Vec<u8>>),
    Float32Array(Vec<f32>),
    Other,
}

fn read_meta_value(reader: &mut Reader, vtype: GGUFValueType) -> Result<MetaValue, AiError> {
    match vtype {
        GGUFValueType::Uint8 => {
            let v = reader.read_u8()?;
            Ok(MetaValue::Uint32(v as u32))
        }
        GGUFValueType::Int8 => {
            let v = reader.read_u8()?;
            Ok(MetaValue::Int32(v as i32))
        }
        GGUFValueType::Uint16 => {
            let lo = reader.read_u8()? as u32;
            let hi = reader.read_u8()? as u32;
            Ok(MetaValue::Uint32(lo | (hi << 8)))
        }
        GGUFValueType::Int16 => {
            let lo = reader.read_u8()? as u16;
            let hi = reader.read_u8()? as u16;
            Ok(MetaValue::Int32((lo | (hi << 8)) as i16 as i32))
        }
        GGUFValueType::Uint32 => Ok(MetaValue::Uint32(reader.read_u32()?)),
        GGUFValueType::Int32 => Ok(MetaValue::Int32(reader.read_i32()?)),
        GGUFValueType::Float32 => Ok(MetaValue::Float32(reader.read_f32()?)),
        GGUFValueType::Bool => {
            let v = reader.read_u8()?;
            Ok(MetaValue::Uint32(v as u32))
        }
        GGUFValueType::String => Ok(MetaValue::StringVal(reader.read_string()?)),
        GGUFValueType::Uint64 => Ok(MetaValue::Uint64(reader.read_u64()?)),
        GGUFValueType::Int64 => {
            let v = reader.read_u64()?;
            Ok(MetaValue::Uint64(v))
        }
        GGUFValueType::Float64 => {
            reader.skip(8)?;
            Ok(MetaValue::Other)
        }
        GGUFValueType::Array => {
            let elem_type = GGUFValueType::from_u32(reader.read_u32()?)?;
            let count = reader.read_u64()? as usize;

            match elem_type {
                GGUFValueType::String => {
                    let mut arr = Vec::with_capacity(count);
                    for _ in 0..count {
                        let len = reader.read_u64()? as usize;
                        let bytes = reader.read_bytes(len)?;
                        arr.push(bytes.to_vec());
                    }
                    Ok(MetaValue::StringArray(arr))
                }
                GGUFValueType::Float32 => {
                    let mut arr = Vec::with_capacity(count);
                    for _ in 0..count {
                        arr.push(reader.read_f32()?);
                    }
                    Ok(MetaValue::Float32Array(arr))
                }
                _ => {
                    let elem_size = match elem_type {
                        GGUFValueType::Uint8 | GGUFValueType::Int8 | GGUFValueType::Bool => 1,
                        GGUFValueType::Uint16 | GGUFValueType::Int16 => 2,
                        GGUFValueType::Uint32 | GGUFValueType::Int32 | GGUFValueType::Float32 => 4,
                        GGUFValueType::Uint64 | GGUFValueType::Int64 | GGUFValueType::Float64 => 8,
                        _ => return Err(AiError::InvalidFormat),
                    };
                    reader.skip(count * elem_size)?;
                    Ok(MetaValue::Other)
                }
            }
        }
    }
}

pub fn parse_gguf(data: &[u8]) -> Result<GGUFModel, AiError> {
    let mut reader = Reader::new(data);

    let magic = reader.read_u32()?;
    if magic != GGUF_MAGIC {
        return Err(AiError::InvalidFormat);
    }

    let version = reader.read_u32()?;
    if version < 2 || version > 3 {
        return Err(AiError::UnsupportedVersion);
    }

    let tensor_count = reader.read_u64()? as usize;
    let metadata_kv_count = reader.read_u64()? as usize;

    let mut config = ModelConfig::default();
    let mut vocab: Vec<Vec<u8>> = Vec::new();
    let mut scores: Vec<f32> = Vec::new();

    for _ in 0..metadata_kv_count {
        let key = reader.read_string()?;
        let vtype = GGUFValueType::from_u32(reader.read_u32()?)?;
        let value = read_meta_value(&mut reader, vtype)?;

        match key.as_str() {
            "llama.embedding_length" | "gpt2.embedding_length" => {
                if let MetaValue::Uint32(v) = value {
                    config.embed_dim = v as usize;
                }
            }
            "llama.block_count" | "gpt2.block_count" => {
                if let MetaValue::Uint32(v) = value {
                    config.n_layers = v as usize;
                }
            }
            "llama.attention.head_count" | "gpt2.attention.head_count" => {
                if let MetaValue::Uint32(v) = value {
                    config.n_heads = v as usize;
                }
            }
            "llama.attention.head_count_kv" | "gpt2.attention.head_count_kv" => {
                if let MetaValue::Uint32(v) = value {
                    config.n_kv_heads = v as usize;
                }
            }
            "llama.context_length" | "gpt2.context_length" => {
                if let MetaValue::Uint32(v) = value {
                    config.context_len = v as usize;
                }
            }
            "llama.feed_forward_length" | "gpt2.feed_forward_length" => {
                if let MetaValue::Uint32(v) = value {
                    config.intermediate_size = v as usize;
                }
            }
            "llama.rope.freq_base" | "gpt2.rope.freq_base" => {
                if let MetaValue::Float32(v) = value {
                    config.rope_theta = v;
                }
            }
            "tokenizer.ggml.tokens" => {
                if let MetaValue::StringArray(v) = value {
                    vocab = v;
                }
            }
            "tokenizer.ggml.scores" => {
                if let MetaValue::Float32Array(v) = value {
                    scores = v;
                }
            }
            _ => {}
        }
    }

    if !vocab.is_empty() {
        config.vocab_size = vocab.len();
    }

    let mut tensors = Vec::with_capacity(tensor_count);
    for _ in 0..tensor_count {
        let name = reader.read_string()?;
        let n_dims = reader.read_u32()?;
        let mut dims = [0u64; 4];
        for d in 0..n_dims as usize {
            dims[d] = reader.read_u64()?;
        }
        let tensor_type = reader.read_u32()?;
        let offset = reader.read_u64()?;
        tensors.push(TensorInfo {
            name,
            n_dims,
            dims,
            tensor_type,
            offset,
        });
    }

    reader.align(mem::size_of::<u64>() * 4);
    let data_offset = reader.pos;

    Ok(GGUFModel {
        config,
        tensors,
        vocab,
        scores,
        data_offset,
    })
}
