//! Embeddings: the `Embedder` trait, the UltraBERT implementation, and a
//! deterministic `HashEmbedder` for tests.

use crate::error::Result;
use crate::ultrabert::UltraBert;
use candle_core::{DType, Device, Tensor};
use std::fs;
use std::path::Path;
use tokenizers::{tokenizer::TruncationDirection, Tokenizer};

pub const DIMS: usize = 768;
/// Padded tokens per forward: bounds activation memory for large batches.
const MAX_BATCH_TOKENS: usize = 8192;
/// Prompt prepended to queries (config_sentence_transformers.json "query").
pub const QUERY_PREFIX: &str =
    "Instruct: Given a question, retrieve passages that answer the question\nQuery: ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Search query: QUERY_PREFIX is prepended.
    Query,
    /// Stored text (memories, entity cards, facts, predicates): no prefix.
    Document,
}

pub trait Embedder: Send + Sync {
    /// One L2-normalised DIMS-long vector per input, same order.
    fn embed(&self, texts: &[&str], mode: Mode) -> Result<Vec<Vec<f32>>>;
    /// Short label for stats ("ultrabert-150m", "hash").
    fn name(&self) -> &str;
}

/// Deterministic bag of word unigrams + char trigrams hashed into DIMS
/// buckets, L2-normalised. Only for tests and as an explicit fallback.
pub struct HashEmbedder;

impl Embedder for HashEmbedder {
    fn embed(&self, texts: &[&str], _mode: Mode) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut vector = vec![0f32; DIMS];
                for word in text.split_whitespace() {
                    add_hash(&mut vector, word.as_bytes());
                }
                let chars: Vec<char> = text.to_lowercase().chars().collect();
                for window in chars.windows(3) {
                    let mut bytes = [0u8; 12];
                    let mut n = 0;
                    for c in window {
                        for byte in c.to_string().as_bytes() {
                            if n < bytes.len() {
                                bytes[n] = *byte;
                                n += 1;
                            }
                        }
                    }
                    add_hash(&mut vector, &bytes[..n]);
                }
                normalise(&mut vector);
                vector
            })
            .collect())
    }
    fn name(&self) -> &str {
        "hash"
    }
}

/// Load UltraBERT from `dir` (or NEOCORTEX_MODEL_DIR, or
/// ~/.neocortex/models/UltraBERT-Embed-150M). Errors if the dir is missing.
pub fn load(dir: Option<&Path>) -> Result<Box<dyn Embedder>> {
    load_dtype(dir, std::env::var("NEOCORTEX_EMBED_DTYPE").ok().as_deref())
}

/// `load` with an explicit dtype (`f32`, `q8`, `q4k`; None = auto).
pub fn load_dtype(dir: Option<&Path>, dtype: Option<&str>) -> Result<Box<dyn Embedder>> {
    let path = dir
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("NEOCORTEX_MODEL_DIR").map(|p| Path::new(&p).to_path_buf()))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|p| Path::new(&p).join(".neocortex/models/UltraBERT-Embed-150M"))
        })
        .unwrap_or_else(|| Path::new(".neocortex/models/UltraBERT-Embed-150M").to_path_buf());
    if !path.is_dir() {
        return Err(crate::err!(
            "UltraBERT model directory is missing: {}",
            path.display()
        ));
    }
    let tokenizer = Tokenizer::from_file(path.join("tokenizer.json"))?;
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path.join("config.json"))?)?;
    let model = UltraBert::from_files(&weights_file(&path, dtype)?, &config)?;
    Ok(Box::new(UltraEmbedder { tokenizer, model }))
}

/// Weight file names inside a model dir, per `NEOCORTEX_EMBED_DTYPE`.
pub const WEIGHTS_F32: &str = "model.safetensors";
pub const WEIGHTS_Q8: &str = "model.q8_0.gguf";
pub const WEIGHTS_Q4K: &str = "model.q4_k.gguf";

/// `NEOCORTEX_EMBED_DTYPE=f32|q8|q4k` picks the weight file. Unset: f32 when
/// the safetensors is present, else q8 (a quantized-only download).
fn weights_file(dir: &Path, dtype: Option<&str>) -> Result<std::path::PathBuf> {
    let file = match dtype {
        Some("f32") => WEIGHTS_F32,
        Some("q8") => WEIGHTS_Q8,
        Some("q4k") => WEIGHTS_Q4K,
        Some("") | None if dir.join(WEIGHTS_F32).exists() => WEIGHTS_F32,
        Some("") | None => WEIGHTS_Q8,
        Some(other) => {
            return Err(crate::err!(
                "NEOCORTEX_EMBED_DTYPE must be f32, q8 or q4k, got '{other}'"
            ))
        }
    };
    let path = dir.join(file);
    if !path.is_file() {
        return Err(crate::err!(
            "{} is missing; create it with `neocortex quantize-model <model dir> <out dir>` or set NEOCORTEX_EMBED_DTYPE=f32",
            path.display()
        ));
    }
    Ok(path)
}

/// Offline conversion behind `neocortex quantize-model`: writes `out` as a
/// complete model dir (quantized weights + config.json + tokenizer.json) from
/// the f32 model dir `dir`. `dtype` is "q8" or "q4k". Returns the weight file size.
pub fn quantize_dir(dir: &Path, out: &Path, dtype: &str) -> Result<u64> {
    let (file, ggml) = match dtype {
        "q8" => (WEIGHTS_Q8, candle_core::quantized::GgmlDType::Q8_0),
        "q4k" => (WEIGHTS_Q4K, candle_core::quantized::GgmlDType::Q4K),
        other => return Err(crate::err!("dtype must be q8 or q4k, got '{other}'")),
    };
    fs::create_dir_all(out)?;
    for name in ["config.json", "tokenizer.json"] {
        fs::copy(dir.join(name), out.join(name))
            .map_err(|e| crate::err!("cannot copy {name} from {}: {e}", dir.display()))?;
    }
    crate::ultrabert::quantize_model(&dir.join(WEIGHTS_F32), &out.join(file), ggml)
}

/// Embedder identity stored in a database. f32 and q8 share it on purpose
/// (same model, cosine parity >= 0.995 against the f32 fixture, so vectors
/// are interchangeable); q4k drifts further and gets its own.
pub fn identity() -> &'static str {
    match std::env::var("NEOCORTEX_EMBED_DTYPE").as_deref() {
        Ok("q4k") => "ultrabert-150m-q4k",
        _ => "ultrabert-150m",
    }
}

fn add_hash(vector: &mut [f32], bytes: &[u8]) {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    vector[(hash as usize) % DIMS] += 1.0;
}

fn normalise(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

struct UltraEmbedder {
    tokenizer: Tokenizer,
    model: UltraBert,
}

impl Embedder for UltraEmbedder {
    fn embed(&self, texts: &[&str], mode: Mode) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let max_tokens = std::env::var("NEOCORTEX_MAX_TOKENS")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(1024usize);
        let mut encoded = Vec::with_capacity(texts.len());
        for text in texts {
            let text = if mode == Mode::Query {
                format!("{QUERY_PREFIX}{text}")
            } else {
                (*text).to_owned()
            };
            let mut encoding = self.tokenizer.encode(text, true)?;
            if encoding.len() > max_tokens {
                encoding.truncate(max_tokens, 0, TruncationDirection::Right);
            }
            encoded.push(encoding);
        }
        let mut order: Vec<usize> = (0..encoded.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(encoded[i].len()));
        // Group texts of similar length: padding a short text up to the longest
        // text of the whole call wastes most of the compute when lengths vary.
        let mut output = vec![Vec::new(); texts.len()];
        let mut start = 0;
        while start < order.len() {
            let max_len = encoded[order[start]].len();
            let mut end = start + 1;
            while end < order.len() {
                let len = encoded[order[end]].len();
                if len * 2 < max_len || (end - start + 1) * max_len > MAX_BATCH_TOKENS {
                    break;
                }
                end += 1;
            }
            let chunk = &order[start..end];
            let mut ids = vec![0u32; chunk.len() * max_len];
            let mut lengths = Vec::with_capacity(chunk.len());
            for (row, &index) in chunk.iter().enumerate() {
                let row_ids = encoded[index].get_ids();
                lengths.push(row_ids.len());
                ids[row * max_len..row * max_len + row_ids.len()].copy_from_slice(row_ids);
            }
            let ids = Tensor::from_vec(ids, (chunk.len(), max_len), &Device::Cpu)?;
            let hidden = self.model.forward(&ids, &lengths)?;
            let pooled = hidden.narrow(1, 0, 1)?.squeeze(1)?;
            let norms = pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
            let result = pooled
                .broadcast_div(&norms)?
                .to_dtype(DType::F32)?
                .to_vec2::<f32>()?;
            for (row, &index) in chunk.iter().enumerate() {
                output[index] = result[row].clone();
            }
            start = end;
        }
        Ok(output)
    }
    fn name(&self) -> &str {
        identity()
    }
}
