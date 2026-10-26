//! Cross-encoder reranker: cross-encoder/ms-marco-MiniLM-L-6-v2 (BERT, 6
//! layers, 384 hidden, Apache-2.0) on candle (CPU). Scores (query, text) pairs
//! jointly, so it closes paraphrase gaps ("partner of Ravi" vs "married to")
//! that independent embeddings miss. Optional: when the model directory is
//! absent `recall` behaves exactly as it does without a reranker.
//!
//! Forward pass: word + position + token-type embeddings -> LayerNorm -> 6 x
//! [self-attention -> residual + LayerNorm -> GELU FFN -> residual + LayerNorm]
//! -> pooler (dense + tanh on [CLS]) -> classifier (1 logit).

use crate::error::Result;
use candle_core::{DType, Device, Result as CandleResult, Tensor};
use candle_nn::{ops, VarBuilder};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokenizers::{Tokenizer, TruncationParams};

const HIDDEN: usize = 384;
const HEADS: usize = 12;
const HEAD_DIM: usize = HIDDEN / HEADS;
const INTERMEDIATE: usize = 1536;
const LAYERS: usize = 6;
const LAYER_NORM_EPS: f32 = 1e-12;
/// Longest (query, text) pair in tokens; the longer side is truncated first.
const MAX_PAIR_TOKENS: usize = 256;
/// Padded tokens per forward: bounds activation memory.
const MAX_BATCH_TOKENS: usize = 8192;

/// Scores how well each text answers a query.
pub trait Reranker: Send + Sync {
    /// One relevance logit per text, same order (higher = more relevant).
    fn score(&self, query: &str, texts: &[&str]) -> Result<Vec<f32>>;
}

struct Layer {
    /// query, key and value concatenated: (3 * HIDDEN, HIDDEN) and (3 * HIDDEN).
    qkv: Tensor,
    qkv_bias: Tensor,
    out: Tensor,
    out_bias: Tensor,
    attn_norm: (Tensor, Tensor),
    up: Tensor,
    up_bias: Tensor,
    down: Tensor,
    down_bias: Tensor,
    ffn_norm: (Tensor, Tensor),
}

struct MiniLm {
    word: Tensor,
    position: Tensor,
    token_type: Tensor,
    embed_norm: (Tensor, Tensor),
    layers: Vec<Layer>,
    pooler: Tensor,
    pooler_bias: Tensor,
    classifier: Tensor,
    classifier_bias: Tensor,
}

/// `x` (batch, seq, in) times `weight` (out, in) plus `bias` (out).
fn linear(x: &Tensor, weight: &Tensor, bias: &Tensor) -> CandleResult<Tensor> {
    let (batch, seq, width) = x.dims3()?;
    x.reshape((batch * seq, width))?
        .matmul(&weight.t()?)?
        .broadcast_add(bias)?
        .reshape((batch, seq, weight.dim(0)?))
}

fn norm(x: &Tensor, (weight, bias): &(Tensor, Tensor)) -> CandleResult<Tensor> {
    ops::layer_norm(&x.contiguous()?, weight, bias, LAYER_NORM_EPS)
}

fn split_heads(x: Tensor, batch: usize, seq: usize) -> CandleResult<Tensor> {
    x.reshape((batch, seq, HEADS, HEAD_DIM))?
        .transpose(1, 2)?
        .contiguous()
}

impl MiniLm {
    fn from_file(weights: &Path) -> Result<Self> {
        crate::ultrabert::pin_rayon_threads();
        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights], DType::F32, &Device::Cpu) }?;
        let bert = vb.pp("bert");
        let layer_norm = |vb: VarBuilder| -> CandleResult<(Tensor, Tensor)> {
            Ok((vb.get((HIDDEN,), "weight")?, vb.get((HIDDEN,), "bias")?))
        };
        let embeddings = bert.pp("embeddings");
        let mut layers = Vec::with_capacity(LAYERS);
        for i in 0..LAYERS {
            let p = bert.pp(format!("encoder.layer.{i}"));
            let attn = p.pp("attention.self");
            let get =
                |vb: &VarBuilder, name: &str| vb.get((HIDDEN, HIDDEN), &format!("{name}.weight"));
            let bias = |vb: &VarBuilder, name: &str| vb.get((HIDDEN,), &format!("{name}.bias"));
            let out = p.pp("attention.output");
            let up = p.pp("intermediate.dense");
            let down = p.pp("output.dense");
            layers.push(Layer {
                qkv: Tensor::cat(
                    &[
                        get(&attn, "query")?,
                        get(&attn, "key")?,
                        get(&attn, "value")?,
                    ],
                    0,
                )?,
                qkv_bias: Tensor::cat(
                    &[
                        bias(&attn, "query")?,
                        bias(&attn, "key")?,
                        bias(&attn, "value")?,
                    ],
                    0,
                )?,
                out: get(&out, "dense")?,
                out_bias: bias(&out, "dense")?,
                attn_norm: layer_norm(out.pp("LayerNorm"))?,
                up: up.get((INTERMEDIATE, HIDDEN), "weight")?,
                up_bias: up.get((INTERMEDIATE,), "bias")?,
                down: down.get((HIDDEN, INTERMEDIATE), "weight")?,
                down_bias: down.get((HIDDEN,), "bias")?,
                ffn_norm: layer_norm(p.pp("output.LayerNorm"))?,
            });
        }
        let pooler = bert.pp("pooler.dense");
        let classifier = vb.pp("classifier");
        Ok(Self {
            word: embeddings.get((30522, HIDDEN), "word_embeddings.weight")?,
            position: embeddings.get((512, HIDDEN), "position_embeddings.weight")?,
            token_type: embeddings.get((2, HIDDEN), "token_type_embeddings.weight")?,
            embed_norm: layer_norm(embeddings.pp("LayerNorm"))?,
            layers,
            pooler: pooler.get((HIDDEN, HIDDEN), "weight")?,
            pooler_bias: pooler.get((HIDDEN,), "bias")?,
            classifier: classifier.get((1, HIDDEN), "weight")?,
            classifier_bias: classifier.get((1,), "bias")?,
        })
    }

    /// One logit per row. `ids` and `types` are (batch, seq), padded with 0;
    /// `lengths` holds each row's real token count.
    fn forward(&self, ids: &Tensor, types: &Tensor, lengths: &[usize]) -> CandleResult<Tensor> {
        let (batch, seq) = ids.dims2()?;
        let embed = |table: &Tensor, ids: &Tensor| -> CandleResult<Tensor> {
            table
                .index_select(&ids.flatten_all()?, 0)?
                .reshape((batch, seq, HIDDEN))
        };
        let mut x = embed(&self.word, ids)?
            .add(&embed(&self.token_type, types)?)?
            .broadcast_add(&self.position.narrow(0, 0, seq)?.unsqueeze(0)?)?;
        x = norm(&x, &self.embed_norm)?;
        // Keys past a row's length are masked out of the softmax; padded query
        // rows are computed but never read.
        let mask = if lengths.iter().any(|&len| len < seq) {
            let mut values = vec![0f32; batch * seq];
            for (row, &len) in lengths.iter().enumerate() {
                values[row * seq + len..(row + 1) * seq].fill(f32::MIN);
            }
            Some(Tensor::from_vec(values, (batch, 1, 1, seq), ids.device())?)
        } else {
            None
        };
        for layer in &self.layers {
            let qkv = linear(&x, &layer.qkv, &layer.qkv_bias)?;
            let q = split_heads(qkv.narrow(2, 0, HIDDEN)?, batch, seq)?;
            let k = split_heads(qkv.narrow(2, HIDDEN, HIDDEN)?, batch, seq)?;
            let v = split_heads(qkv.narrow(2, 2 * HIDDEN, HIDDEN)?, batch, seq)?;
            let scores = q
                .matmul(&k.transpose(2, 3)?)?
                .affine(1.0 / (HEAD_DIM as f64).sqrt(), 0.0)?;
            let scores = match &mask {
                Some(mask) => scores.broadcast_add(mask)?,
                None => scores,
            };
            let attended = ops::softmax_last_dim(&scores)?
                .matmul(&v)?
                .transpose(1, 2)?
                .contiguous()?
                .reshape((batch, seq, HIDDEN))?;
            x = norm(
                &x.add(&linear(&attended, &layer.out, &layer.out_bias)?)?,
                &layer.attn_norm,
            )?;
            let hidden = linear(&x, &layer.up, &layer.up_bias)?.gelu_erf()?;
            x = norm(
                &x.add(&linear(&hidden, &layer.down, &layer.down_bias)?)?,
                &layer.ffn_norm,
            )?;
        }
        let first = x.narrow(1, 0, 1)?.squeeze(1)?;
        let pooled = first
            .matmul(&self.pooler.t()?)?
            .broadcast_add(&self.pooler_bias)?
            .tanh()?;
        pooled
            .matmul(&self.classifier.t()?)?
            .broadcast_add(&self.classifier_bias)?
            .squeeze(1)
    }
}

/// The ms-marco-MiniLM-L-6-v2 cross-encoder.
pub struct CrossEncoder {
    tokenizer: Tokenizer,
    model: MiniLm,
}

/// Model directory: `dir`, else NEOCORTEX_RERANK_MODEL, else
/// ~/.neocortex/models/ms-marco-MiniLM-L-6-v2.
pub fn model_dir(dir: Option<&Path>) -> PathBuf {
    dir.map(Path::to_path_buf)
        .or_else(|| std::env::var_os("NEOCORTEX_RERANK_MODEL").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|p| Path::new(&p).join(".neocortex/models/ms-marco-MiniLM-L-6-v2"))
        })
        .unwrap_or_else(|| PathBuf::from(".neocortex/models/ms-marco-MiniLM-L-6-v2"))
}

impl CrossEncoder {
    /// Load from `dir` (see `model_dir`). Errors if the directory is missing or
    /// is not the 6-layer, 384-wide model.
    pub fn load(dir: Option<&Path>) -> Result<CrossEncoder> {
        let path = model_dir(dir);
        if !path.is_dir() {
            return Err(crate::err!(
                "reranker model directory is missing: {}",
                path.display()
            ));
        }
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path.join("config.json"))?)?;
        let size = |key: &str| config[key].as_u64().unwrap_or(0) as usize;
        if size("hidden_size") != HIDDEN
            || size("num_hidden_layers") != LAYERS
            || size("num_attention_heads") != HEADS
        {
            return Err(crate::err!(
                "reranker config in {} is not ms-marco-MiniLM-L-6-v2 (needs hidden_size {HIDDEN}, {LAYERS} layers, {HEADS} heads)",
                path.display()
            ));
        }
        let mut tokenizer = Tokenizer::from_file(path.join("tokenizer.json"))?;
        tokenizer.with_truncation(Some(TruncationParams {
            max_length: MAX_PAIR_TOKENS,
            ..Default::default()
        }))?;
        let model = MiniLm::from_file(&path.join("model.safetensors"))?;
        Ok(CrossEncoder { tokenizer, model })
    }
}

impl Reranker for CrossEncoder {
    fn score(&self, query: &str, texts: &[&str]) -> Result<Vec<f32>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let encoded = self
            .tokenizer
            .encode_batch(texts.iter().map(|t| (query, *t)).collect::<Vec<_>>(), true)?;
        let mut order: Vec<usize> = (0..encoded.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(encoded[i].len()));
        // Group pairs of similar length: padding every pair up to the longest
        // one wastes most of the compute when lengths vary.
        let mut output = vec![0f32; texts.len()];
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
            let mut types = vec![0u32; chunk.len() * max_len];
            let mut lengths = Vec::with_capacity(chunk.len());
            for (row, &index) in chunk.iter().enumerate() {
                let (row_ids, row_types) =
                    (encoded[index].get_ids(), encoded[index].get_type_ids());
                lengths.push(row_ids.len());
                ids[row * max_len..row * max_len + row_ids.len()].copy_from_slice(row_ids);
                types[row * max_len..row * max_len + row_types.len()].copy_from_slice(row_types);
            }
            let shape = (chunk.len(), max_len);
            let ids = Tensor::from_vec(ids, shape, &Device::Cpu)?;
            let types = Tensor::from_vec(types, shape, &Device::Cpu)?;
            let logits = self
                .model
                .forward(&ids, &types, &lengths)?
                .to_vec1::<f32>()?;
            for (&index, logit) in chunk.iter().zip(logits) {
                output[index] = logit;
            }
            start = end;
        }
        Ok(output)
    }
}

/// Loads the cross-encoder on first use and caches the result (or the error),
/// so engines that never rerank never touch the model.
pub struct LazyReranker {
    dir: Option<PathBuf>,
    loaded: OnceLock<std::result::Result<CrossEncoder, String>>,
}

impl LazyReranker {
    pub fn new() -> LazyReranker {
        LazyReranker {
            dir: None,
            loaded: OnceLock::new(),
        }
    }

    /// Load from an explicit model dir instead of NEOCORTEX_RERANK_MODEL.
    pub fn with_dir(dir: impl Into<PathBuf>) -> LazyReranker {
        LazyReranker {
            dir: Some(dir.into()),
            loaded: OnceLock::new(),
        }
    }

    fn get(&self) -> Result<&CrossEncoder> {
        match self
            .loaded
            .get_or_init(|| CrossEncoder::load(self.dir.as_deref()).map_err(|e| e.to_string()))
        {
            Ok(model) => Ok(model),
            Err(msg) => Err(crate::error::Error(msg.clone())),
        }
    }
}

impl Default for LazyReranker {
    fn default() -> Self {
        Self::new()
    }
}

impl Reranker for LazyReranker {
    fn score(&self, query: &str, texts: &[&str]) -> Result<Vec<f32>> {
        self.get()?.score(query, texts)
    }
}

/// Blend cross-encoder `logits` into the leading candidates of a list sorted
/// best first (`.1` is the fused score) and re-sort that head. Each candidate's
/// head-normalised fused score is mixed with sigmoid(logit) at `weight` (0 =
/// fused order, 1 = cross-encoder order); the head keeps its score range, so
/// the tail stays below it.
pub fn blend<T>(
    mut candidates: Vec<(i64, f64, T)>,
    logits: &[f32],
    weight: f64,
) -> Vec<(i64, f64, T)> {
    let tail = candidates.split_off(logits.len().min(candidates.len()));
    let lo = candidates.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let hi = candidates
        .iter()
        .map(|c| c.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let span = hi - lo;
    let weight = weight.clamp(0.0, 1.0);
    let mut mixed: Vec<(f64, (i64, f64, T))> = candidates
        .into_iter()
        .zip(logits)
        .map(|(candidate, &logit)| {
            let fused = if span > 0.0 {
                (candidate.1 - lo) / span
            } else {
                1.0
            };
            let relevance = 1.0 / (1.0 + (-f64::from(logit)).exp());
            ((1.0 - weight) * fused + weight * relevance, candidate)
        })
        .collect();
    mixed.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1 .0.cmp(&b.1 .0)));
    let mut out: Vec<(i64, f64, T)> = mixed
        .into_iter()
        .map(|(b, mut candidate)| {
            if span > 0.0 {
                candidate.1 = lo + b * span;
            }
            candidate
        })
        .collect();
    out.extend(tail);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(candidates: &[(i64, f64, ())]) -> Vec<i64> {
        candidates.iter().map(|c| c.0).collect()
    }

    #[test]
    fn blend_weight_zero_keeps_order_and_one_follows_logits() {
        let base = vec![(1, 0.9, ()), (2, 0.8, ()), (3, 0.7, ()), (4, 0.1, ())];
        let kept = blend(base.clone(), &[-5.0, 5.0, 0.0], 0.0);
        assert_eq!(ids(&kept), vec![1, 2, 3, 4]);
        let flipped = blend(base, &[-5.0, 5.0, 0.0], 1.0);
        assert_eq!(ids(&flipped), vec![2, 3, 1, 4]);
        // The head keeps its score range and stays above the tail.
        assert!(flipped[..3].iter().all(|c| c.1 <= 0.9 && c.1 >= 0.7));
        assert!(flipped.windows(2).all(|w| w[0].1 >= w[1].1));
    }

    #[test]
    fn blend_mixes_fused_and_logit_and_ignores_empty_input() {
        let candidates = vec![(1, 1.0, ()), (2, 0.9, ()), (3, 0.0, ())];
        let same = blend(candidates.clone(), &[], 0.5);
        assert_eq!(ids(&same), vec![1, 2, 3]);
        // Close fused scores, decisive logits: the relevant one wins at 0.5.
        let mixed = blend(candidates, &[-6.0, 6.0, -6.0], 0.5);
        assert_eq!(mixed[0].0, 2);
        // Tied fused scores fall back to the logits.
        let tied = blend(vec![(7, 0.5, ()), (8, 0.5, ())], &[-3.0, 3.0], 0.5);
        assert_eq!(ids(&tied), vec![8, 7]);
    }

    #[test]
    fn missing_model_dir_is_an_error_not_a_panic() {
        let reranker = LazyReranker::with_dir("/nonexistent/ms-marco-MiniLM-L-6-v2");
        let err = reranker.score("q", &["a"]).unwrap_err();
        assert!(err.to_string().contains("missing"), "{err}");
    }
}
