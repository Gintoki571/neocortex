//! UltraBERT-Embed-150M forward pass on candle (CPU): token embeddings ->
//! 24 x [RMSNorm -> RoPE attention (sliding window ±128 on layers where
//! config.attention_windows[i] is set) -> residual -> RMSNorm -> SwiGLU MLP ->
//! residual] -> final RMSNorm. Pooling (CLS = position 0) and normalisation
//! happen in embed.rs.

use candle_core::quantized::{gguf_file, GgmlDType, QTensor};
use candle_core::{Device, Result as CandleResult, Tensor};
use candle_nn::ops;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const HIDDEN: usize = 768;
const HEADS: usize = 6;
const HEAD_DIM: usize = 128;
const INTERMEDIATE: usize = 1024;
/// RoPE tables are precomputed up to this many positions; longer inputs fall
/// back to computing the table on the fly.
const ROPE_CACHE: usize = 1024;

/// candle asks gemm for `get_num_threads()` (the physical core count unless
/// RAYON_NUM_THREADS is set) while rayon's shared pool defaults to logical
/// cores; that mismatch costs ~50% on small batches, so pin the pool to the
/// size gemm will request. Shared by every candle model in the process.
pub(crate) fn pin_rayon_threads() {
    if std::env::var_os("RAYON_NUM_THREADS").is_none() {
        std::env::set_var(
            "RAYON_NUM_THREADS",
            candle_core::utils::get_num_threads().to_string(),
        );
    }
}

struct Layer {
    input_norm: Tensor,
    post_norm: Tensor,
    /// q, k and v concatenated: (3 * HIDDEN, HIDDEN).
    qkv: Linear,
    o: Linear,
    /// gate and up concatenated: (2 * INTERMEDIATE, HIDDEN).
    gate_up: Linear,
    down: Linear,
    /// Index into `UltraBert::rope`.
    rope: usize,
    window: Option<usize>,
}

pub struct UltraBert {
    embedding: Embedding,
    layers: Vec<Layer>,
    norm: Tensor,
    /// (theta, cos, sin), each table (1, 1, ROPE_CACHE, HEAD_DIM).
    rope: Vec<(f64, Tensor, Tensor)>,
    device: Device,
}

/// A linear layer: f32 weights, or quantized weights dequantized to f32 for
/// each call and multiplied by the same runtime-dispatched gemm. candle's own
/// quantized matmul (`QMatMul`) picks its SIMD kernels at compile time
/// (`target_feature = "avx2"`/"neon"); the default x86-64 build falls to scalar
/// code that measured 60x slower than f32, so it is not used. The per-call
/// f32 copy is one layer matrix (<= 6.3 MB), never the whole model.
enum Linear {
    F32(Tensor),
    Quant(QTensor),
}

impl Linear {
    fn forward(&self, x: &Tensor, out: usize) -> CandleResult<Tensor> {
        let dims = x.dims();
        let x = x.reshape((dims[0] * dims[1], dims[2]))?;
        let y = match self {
            Linear::F32(weight) => x.matmul(&weight.t()?)?,
            Linear::Quant(q) => quant_matmul(&x, q)?,
        };
        y.reshape((dims[0], dims[1], out))
    }
}

/// Output rows dequantized per gemm call: 256 x 768 f32 = 768 KB stays in L2,
/// where one whole-matrix copy (up to 6.3 MB) made dequantizing bandwidth-bound.
const SLAB_ROWS: usize = 256;

/// `x @ q^T` with `q` dequantized one slab of output rows at a time.
fn quant_matmul(x: &Tensor, q: &QTensor) -> CandleResult<Tensor> {
    let (rows, cols) = q.shape().dims2()?;
    if q.dtype() != GgmlDType::Q8_0 || rows <= SLAB_ROWS {
        return x.matmul(&dequantize(q)?.t()?);
    }
    let data = q.data()?;
    let row_bytes = cols / 32 * 34;
    let mut parts = Vec::with_capacity(rows.div_ceil(SLAB_ROWS));
    for start in (0..rows).step_by(SLAB_ROWS) {
        let n = SLAB_ROWS.min(rows - start);
        let mut slab = vec![0f32; n * cols];
        dequantize_q8_0(&data[start * row_bytes..(start + n) * row_bytes], &mut slab);
        let w = Tensor::from_vec(slab, (n, cols), &Device::Cpu)?;
        parts.push(x.matmul(&w.t()?)?);
    }
    Tensor::cat(&parts, 1)
}

fn dequantize(q: &QTensor) -> CandleResult<Tensor> {
    if q.dtype() != GgmlDType::Q8_0 {
        return q.dequantize(&Device::Cpu);
    }
    let mut out = vec![0f32; q.shape().elem_count()];
    dequantize_q8_0(&q.data()?, &mut out);
    Tensor::from_vec(out, q.shape().clone(), &Device::Cpu)
}

/// Q8_0 blocks are 34 bytes: little-endian f16 scale, then 32 i8.
fn dequantize_q8_0(blocks: &[u8], out: &mut [f32]) {
    #[inline(always)]
    fn run(blocks: &[u8], out: &mut [f32]) {
        let (blocks, _) = blocks.as_chunks::<34>();
        let (out, _) = out.as_chunks_mut::<32>();
        for (block, y) in blocks.iter().zip(out) {
            let scale = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
            for (o, &q) in y.iter_mut().zip(&block[2..]) {
                *o = f32::from(q as i8) * scale;
            }
        }
    }
    // The same loop compiled for AVX2 when the CPU has it (runtime check);
    // the baseline build keeps the SSE2/NEON autovectorised copy.
    #[cfg(target_arch = "x86_64")]
    {
        #[target_feature(enable = "avx2")]
        unsafe fn run_avx2(blocks: &[u8], out: &mut [f32]) {
            run(blocks, out)
        }
        if std::is_x86_feature_detected!("avx2") {
            return unsafe { run_avx2(blocks, out) };
        }
    }
    run(blocks, out)
}

/// IEEE half to single (zero, subnormal, normal, inf/NaN).
fn f16_to_f32(h: u16) -> f32 {
    let sign = u32::from(h & 0x8000) << 16;
    let (exp, man) = (u32::from(h >> 10) & 0x1f, u32::from(h & 0x3ff));
    match exp {
        0 => f32::from_bits(sign) + (man as f32 / 16_777_216.0).copysign(f32::from_bits(sign)),
        31 => f32::from_bits(sign | 0x7f80_0000 | man << 13),
        _ => f32::from_bits(sign | (exp + 112) << 23 | man << 13),
    }
}

enum Embedding {
    F32(Tensor),
    Quant(QTensor),
}

impl Embedding {
    fn lookup(&self, ids: &Tensor) -> CandleResult<Tensor> {
        match self {
            Embedding::F32(table) => table.index_select(ids, 0),
            Embedding::Quant(table) => table.embedding(ids),
        }
    }
}

/// Safetensors read tensor by tensor with plain file reads, not an mmap:
/// candle tensors own their storage, so a mmap would hold the file's pages
/// AND a copy of every tensor (and `Tensor::cat` for the fused weights one more).
/// Reading straight into the final buffer keeps peak RSS at one copy of the
/// weights; the page cache is not charged to the process.
struct SafetensorsFile {
    file: File,
    /// Offset of the data section (8 + header length).
    base: u64,
    /// name -> (shape, begin, end) of F32 tensors, offsets relative to `base`.
    index: HashMap<String, (Vec<usize>, u64, u64)>,
}

impl SafetensorsFile {
    fn open(path: &Path) -> crate::Result<Self> {
        let mut file =
            File::open(path).map_err(|e| crate::err!("cannot open {}: {e}", path.display()))?;
        let mut len = [0u8; 8];
        file.read_exact(&mut len)?;
        let len = u64::from_le_bytes(len);
        if len > 100 << 20 {
            return Err(crate::err!("{} is not a safetensors file", path.display()));
        }
        let mut header = vec![0u8; len as usize];
        file.read_exact(&mut header)?;
        let header: serde_json::Value = serde_json::from_slice(&header)?;
        let mut index = HashMap::new();
        for (name, info) in header.as_object().into_iter().flatten() {
            if name == "__metadata__" {
                continue;
            }
            if info["dtype"] != "F32" {
                return Err(crate::err!(
                    "{name} is {}; only F32 safetensors are supported",
                    info["dtype"]
                ));
            }
            let shape: Vec<usize> = serde_json::from_value(info["shape"].clone())?;
            let offsets: Vec<u64> = serde_json::from_value(info["data_offsets"].clone())?;
            if offsets.len() != 2 {
                return Err(crate::err!("{name} has malformed data_offsets"));
            }
            index.insert(name.clone(), (shape, offsets[0], offsets[1]));
        }
        Ok(Self {
            file,
            base: 8 + len,
            index,
        })
    }

    /// Read `names` (each (rows, cols) or (len,)) back to back into one
    /// buffer: row-wise concatenation, no intermediate copy.
    fn read(&mut self, names: &[&str], shape: &[usize]) -> crate::Result<Tensor> {
        let total: usize = shape.iter().product();
        let mut data = vec![0f32; total];
        let mut at = 0;
        for name in names {
            let (found, begin, end) = self
                .index
                .get(*name)
                .ok_or_else(|| crate::err!("model is missing tensor {name}"))?;
            let n = found.iter().product::<usize>();
            if end.checked_sub(*begin) != Some(n as u64 * 4) || at + n > total {
                return Err(crate::err!(
                    "tensor {name} has shape {found:?}, does not fit {shape:?}"
                ));
            }
            self.file.seek(SeekFrom::Start(self.base + begin))?;
            // shortcut: assumes a little-endian host (x86-64, aarch64 Android/Linux); upgrade if a big-endian target appears.
            let bytes = unsafe {
                std::slice::from_raw_parts_mut(data[at..at + n].as_mut_ptr().cast::<u8>(), n * 4)
            };
            self.file.read_exact(bytes)?;
            at += n;
        }
        if at != total {
            return Err(crate::err!("{names:?} do not fill {shape:?}"));
        }
        Ok(Tensor::from_vec(data, shape, &Device::Cpu)?)
    }
}

/// Where `UltraBert::from_files` reads from.
enum Weights {
    Safetensors(SafetensorsFile),
    Gguf(gguf_file::Content, File),
}

impl Weights {
    fn open(path: &Path) -> crate::Result<Self> {
        if path.extension().is_some_and(|e| e == "gguf") {
            let mut file =
                File::open(path).map_err(|e| crate::err!("cannot open {}: {e}", path.display()))?;
            let content = gguf_file::Content::read(&mut file)?;
            Ok(Self::Gguf(content, file))
        } else {
            Ok(Self::Safetensors(SafetensorsFile::open(path)?))
        }
    }

    fn vector(&mut self, name: &str, len: usize) -> crate::Result<Tensor> {
        match self {
            Self::Safetensors(st) => st.read(&[name], &[len]),
            Self::Gguf(content, file) => {
                let q = content.tensor(file, name, &Device::Cpu)?;
                Ok(q.dequantize(&Device::Cpu)?.reshape(len)?)
            }
        }
    }

    /// `stored` is the tensor name in a GGUF file (fused weights are stored
    /// fused); `parts` are the safetensors tensors concatenated row-wise.
    fn linear(
        &mut self,
        stored: &str,
        parts: &[&str],
        rows: usize,
        cols: usize,
    ) -> crate::Result<Linear> {
        match self {
            Self::Safetensors(st) => Ok(Linear::F32(st.read(parts, &[rows, cols])?)),
            Self::Gguf(content, file) => {
                let q = content.tensor(file, stored, &Device::Cpu)?;
                if q.shape().dims() != [rows, cols] {
                    return Err(crate::err!(
                        "{stored} has shape {:?}, expected ({rows}, {cols})",
                        q.shape()
                    ));
                }
                Ok(Linear::Quant(q))
            }
        }
    }

    fn embedding(&mut self) -> crate::Result<Embedding> {
        match self {
            Self::Safetensors(st) => Ok(Embedding::F32(st.read(&[EMBED], &[VOCAB, HIDDEN])?)),
            Self::Gguf(content, file) => {
                let q = content.tensor(file, EMBED, &Device::Cpu)?;
                if q.shape().dims() != [VOCAB, HIDDEN] {
                    return Err(crate::err!("{EMBED} has shape {:?}", q.shape()));
                }
                Ok(Embedding::Quant(q))
            }
        }
    }
}

const VOCAB: usize = 49152;
const EMBED: &str = "embed_tokens.weight";
const FINAL_NORM: &str = "norm.weight";

/// Safetensors names of layer `i`'s tensors; the GGUF stores the fused ones
/// as `layers.{i}.qkv.weight` / `layers.{i}.gate_up.weight`.
struct LayerNames {
    qkv: [String; 3],
    gate_up: [String; 2],
    o: String,
    down: String,
    input_norm: String,
    post_norm: String,
}

impl LayerNames {
    fn new(i: usize) -> Self {
        let p = format!("layers.{i}");
        Self {
            qkv: ["q_proj", "k_proj", "v_proj"].map(|n| format!("{p}.self_attn.{n}.weight")),
            gate_up: ["gate_proj", "up_proj"].map(|n| format!("{p}.mlp.{n}.weight")),
            o: format!("{p}.self_attn.o_proj.weight"),
            down: format!("{p}.mlp.down_proj.weight"),
            input_norm: format!("{p}.input_layernorm.weight"),
            post_norm: format!("{p}.post_attention_layernorm.weight"),
        }
    }
}

/// Offline conversion: read the f32 safetensors tensor by tensor and write a
/// GGUF whose linear weights (fused q/k/v and gate/up as the forward pass
/// uses them) are `dtype`, the embedding table Q8_0, norms f32. Returns the
/// output size in bytes.
pub fn quantize_model(weights: &Path, out: &Path, dtype: GgmlDType) -> crate::Result<u64> {
    let mut st = SafetensorsFile::open(weights)?;
    let quant = |t: Tensor, dtype| QTensor::quantize(&t, dtype);
    let mut tensors: Vec<(String, QTensor)> = vec![(
        EMBED.into(),
        quant(st.read(&[EMBED], &[VOCAB, HIDDEN])?, GgmlDType::Q8_0)?,
    )];
    for i in 0..24 {
        let n = LayerNames::new(i);
        let p = format!("layers.{i}");
        let qkv: Vec<&str> = n.qkv.iter().map(String::as_str).collect();
        let gate_up: Vec<&str> = n.gate_up.iter().map(String::as_str).collect();
        for (stored, parts, shape) in [
            (format!("{p}.qkv.weight"), qkv, [3 * HIDDEN, HIDDEN]),
            (
                format!("{p}.gate_up.weight"),
                gate_up,
                [2 * INTERMEDIATE, HIDDEN],
            ),
            (n.o.clone(), vec![n.o.as_str()], [HIDDEN, HIDDEN]),
            (
                n.down.clone(),
                vec![n.down.as_str()],
                [HIDDEN, INTERMEDIATE],
            ),
        ] {
            tensors.push((stored, quant(st.read(&parts, &shape)?, dtype)?));
        }
        for norm in [&n.input_norm, &n.post_norm] {
            tensors.push((
                norm.clone(),
                quant(st.read(&[norm], &[HIDDEN])?, GgmlDType::F32)?,
            ));
        }
    }
    tensors.push((
        FINAL_NORM.into(),
        quant(st.read(&[FINAL_NORM], &[HIDDEN])?, GgmlDType::F32)?,
    ));
    let mut file = File::create(out)?;
    let refs: Vec<(&str, &QTensor)> = tensors.iter().map(|(n, t)| (n.as_str(), t)).collect();
    let arch = gguf_file::Value::String("ultrabert-embed".into());
    gguf_file::write(&mut file, &[("general.architecture", &arch)], &refs)?;
    Ok(file.metadata()?.len())
}

fn rope_tables(theta: f64, seq: usize, device: &Device) -> CandleResult<(Tensor, Tensor)> {
    let mut cos = Vec::with_capacity(seq * HEAD_DIM);
    let mut sin = Vec::with_capacity(seq * HEAD_DIM);
    for pos in 0..seq {
        // Both halves of the width carry the same angle; `rope` rotates each
        // half against the other.
        for _ in 0..2 {
            for i in 0..HEAD_DIM / 2 {
                let angle = pos as f64 / theta.powf((2 * i) as f64 / HEAD_DIM as f64);
                cos.push(angle.cos() as f32);
                sin.push(angle.sin() as f32);
            }
        }
    }
    Ok((
        Tensor::from_vec(cos, (1, 1, seq, HEAD_DIM), device)?,
        Tensor::from_vec(sin, (1, 1, seq, HEAD_DIM), device)?,
    ))
}

fn rope(x: &Tensor, cos: &Tensor, sin: &Tensor) -> CandleResult<Tensor> {
    let left = x.narrow(3, 0, HEAD_DIM / 2)?;
    let right = x.narrow(3, HEAD_DIM / 2, HEAD_DIM / 2)?;
    let rotated = Tensor::cat(&[&right.neg()?, &left], 3)?;
    x.broadcast_mul(cos)?.add(&rotated.broadcast_mul(sin)?)
}

fn attention_mask(
    batch: usize,
    seq: usize,
    lengths: &[usize],
    window: Option<usize>,
    device: &Device,
) -> CandleResult<Tensor> {
    let mut values = vec![0f32; batch * seq * seq];
    for b in 0..batch {
        for i in 0..seq {
            for j in 0..seq {
                let valid =
                    j < lengths[b] && i < lengths[b] && window.is_none_or(|w| i.abs_diff(j) <= w);
                values[(b * seq + i) * seq + j] = if valid { 0.0 } else { f32::MIN };
            }
        }
    }
    Tensor::from_vec(values, (batch, 1, seq, seq), device)
}

impl UltraBert {
    pub fn from_files(
        weights: &std::path::Path,
        config: &serde_json::Value,
    ) -> crate::Result<Self> {
        pin_rayon_threads();
        let device = Device::Cpu;
        let mut weights = Weights::open(weights)?;
        let rope_theta: Vec<f64> = serde_json::from_value(config["rope_theta"].clone())?;
        let windows: Vec<Option<usize>> =
            serde_json::from_value(config["attention_windows"].clone())?;
        // A wrong config must be an error, not an index panic that kills the MCP server.
        if rope_theta.len() < 24 || windows.len() < 24 {
            return Err(crate::err!(
                "model config has {} rope_theta and {} attention_windows entries; UltraBERT-Embed-150M needs 24 each",
                rope_theta.len(),
                windows.len()
            ));
        }

        let mut rope: Vec<(f64, Tensor, Tensor)> = Vec::new();
        let mut rope_index = |theta: f64| -> crate::Result<usize> {
            if let Some(index) = rope.iter().position(|(t, _, _)| *t == theta) {
                return Ok(index);
            }
            let (cos, sin) = rope_tables(theta, ROPE_CACHE, &device)?;
            rope.push((theta, cos, sin));
            Ok(rope.len() - 1)
        };

        let mut layers = Vec::with_capacity(24);
        for i in 0..24 {
            let n = LayerNames::new(i);
            let p = format!("layers.{i}");
            let qkv: Vec<&str> = n.qkv.iter().map(String::as_str).collect();
            let gate_up: Vec<&str> = n.gate_up.iter().map(String::as_str).collect();
            layers.push(Layer {
                input_norm: weights.vector(&n.input_norm, HIDDEN)?,
                post_norm: weights.vector(&n.post_norm, HIDDEN)?,
                qkv: weights.linear(&format!("{p}.qkv.weight"), &qkv, 3 * HIDDEN, HIDDEN)?,
                o: weights.linear(&n.o, &[&n.o], HIDDEN, HIDDEN)?,
                gate_up: weights.linear(
                    &format!("{p}.gate_up.weight"),
                    &gate_up,
                    2 * INTERMEDIATE,
                    HIDDEN,
                )?,
                down: weights.linear(&n.down, &[&n.down], HIDDEN, INTERMEDIATE)?,
                rope: rope_index(rope_theta[i])?,
                window: windows[i],
            });
        }
        Ok(Self {
            embedding: weights.embedding()?,
            layers,
            norm: weights.vector(FINAL_NORM, HIDDEN)?,
            rope,
            device,
        })
    }

    pub fn forward(&self, ids: &Tensor, lengths: &[usize]) -> CandleResult<Tensor> {
        let (batch, seq) = (ids.dims()[0], ids.dims()[1]);
        let mut x = self
            .embedding
            .lookup(&ids.flatten_all()?)?
            .reshape((batch, seq, HIDDEN))?;
        // Masks only matter for padding or when a window is narrower than the
        // sequence; build each distinct one at most once per forward.
        let padded = lengths.iter().any(|&len| len < seq);
        let mut masks: Vec<(Option<usize>, Tensor)> = Vec::new();
        // RoPE tables beyond the cached range, one per distinct theta.
        let long_rope = if seq > ROPE_CACHE {
            Some(
                self.rope
                    .iter()
                    .map(|(theta, _, _)| rope_tables(*theta, seq, &self.device))
                    .collect::<CandleResult<Vec<_>>>()?,
            )
        } else {
            None
        };
        for layer in &self.layers {
            let mask = if padded || layer.window.is_some_and(|w| seq > w) {
                let index = match masks.iter().position(|(w, _)| *w == layer.window) {
                    Some(index) => index,
                    None => {
                        let mask = attention_mask(batch, seq, lengths, layer.window, &self.device)?;
                        masks.push((layer.window, mask));
                        masks.len() - 1
                    }
                };
                Some(&masks[index].1)
            } else {
                None
            };

            let (cos, sin) = match &long_rope {
                Some(tables) => {
                    let (cos, sin) = &tables[layer.rope];
                    (cos.clone(), sin.clone())
                }
                None => {
                    let (_, cos, sin) = &self.rope[layer.rope];
                    (cos.narrow(2, 0, seq)?, sin.narrow(2, 0, seq)?)
                }
            };

            let normalized = ops::rms_norm(&x, &layer.input_norm, 1e-6)?;
            let qkv = layer.qkv.forward(&normalized, 3 * HIDDEN)?;
            let q = qkv
                .narrow(2, 0, HIDDEN)?
                .reshape((batch, seq, HEADS, HEAD_DIM))?
                .transpose(1, 2)?
                .contiguous()?;
            let k = qkv
                .narrow(2, HIDDEN, HIDDEN)?
                .reshape((batch, seq, HEADS, HEAD_DIM))?
                .transpose(1, 2)?
                .contiguous()?;
            let v = qkv
                .narrow(2, 2 * HIDDEN, HIDDEN)?
                .reshape((batch, seq, HEADS, HEAD_DIM))?
                .transpose(1, 2)?
                .contiguous()?;
            let q = rope(&q, &cos, &sin)?;
            let k = rope(&k, &cos, &sin)?;
            let scores = q
                .matmul(&k.transpose(2, 3)?)?
                .affine(1.0 / (HEAD_DIM as f64).sqrt(), 0.0)?;
            let scores = match mask {
                Some(mask) => scores.broadcast_add(mask)?,
                None => scores,
            };
            let weights = ops::softmax_last_dim(&scores)?;
            let attended = weights
                .matmul(&v)?
                .transpose(1, 2)?
                .contiguous()?
                .reshape((batch, seq, HEADS * HEAD_DIM))?;
            x = x.add(&layer.o.forward(&attended, HIDDEN)?)?;

            let normalized = ops::rms_norm(&x, &layer.post_norm, 1e-6)?;
            let gate_up = layer.gate_up.forward(&normalized, 2 * INTERMEDIATE)?;
            let gated = ops::silu(&gate_up.narrow(2, 0, INTERMEDIATE)?)?.mul(&gate_up.narrow(
                2,
                INTERMEDIATE,
                INTERMEDIATE,
            )?)?;
            x = x.add(&layer.down.forward(&gated, HIDDEN)?)?;
        }
        ops::rms_norm(&x, &self.norm, 1e-6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safetensors_reader_fuses_rows_and_rejects_bad_files() {
        let dir = std::env::temp_dir().join(format!("nc-st-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let header = r#"{"__metadata__":{"format":"pt"},"a":{"dtype":"F32","shape":[2,3],"data_offsets":[0,24]},"b":{"dtype":"F32","shape":[1,3],"data_offsets":[24,36]},"h":{"dtype":"F16","shape":[1],"data_offsets":[36,38]}}"#;
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend(header.as_bytes());
        for v in 0..9 {
            bytes.extend((v as f32).to_le_bytes());
        }
        bytes.extend([0u8, 0]);
        let path = dir.join("m.safetensors");
        std::fs::write(&path, &bytes).unwrap();
        // The F16 entry makes the whole file unsupported, with the tensor named.
        let err = SafetensorsFile::open(&path).err().unwrap().to_string();
        assert!(err.contains("F32"), "{err}");
        let header = header.replace(
            r#","h":{"dtype":"F16","shape":[1],"data_offsets":[36,38]}"#,
            "",
        );
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend(header.as_bytes());
        for v in 0..9 {
            bytes.extend((v as f32).to_le_bytes());
        }
        std::fs::write(&path, &bytes).unwrap();
        let mut st = SafetensorsFile::open(&path).unwrap();
        let fused = st.read(&["a", "b"], &[3, 3]).unwrap();
        assert_eq!(
            fused.flatten_all().unwrap().to_vec1::<f32>().unwrap(),
            (0..9).map(|v| v as f32).collect::<Vec<_>>()
        );
        assert!(st.read(&["a"], &[3, 3]).is_err(), "short fill is an error");
        assert!(
            st.read(&["a", "b"], &[2, 3]).is_err(),
            "overflow is an error"
        );
        assert!(
            st.read(&["zzz"], &[1]).is_err(),
            "missing tensor is an error"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn q8_dequant_matches_candle_and_f16_decode_is_exact() {
        let w = Tensor::randn(0f32, 0.3, (16, 96), &Device::Cpu).unwrap();
        let q = QTensor::quantize(&w, GgmlDType::Q8_0).unwrap();
        let ours = dequantize(&q)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        let theirs = q
            .dequantize(&Device::Cpu)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        assert_eq!(ours, theirs);
        for (bits, value) in [
            (0x3c00, 1.0),
            (0xc000, -2.0),
            (0x7bff, 65504.0),
            (0x0400, 2f32.powi(-14)),
            (0x0001, 2f32.powi(-24)),
            (0x8001, -(2f32.powi(-24))),
            (0x0000, 0.0),
        ] {
            assert_eq!(f16_to_f32(bits), value, "{bits:#x}");
        }
    }
}
