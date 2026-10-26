//! Quantized (GGUF Q8_0 / Q4_K) UltraBERT: conversion round trip, loading and
//! cosine parity against the f32 reference fixture. Model-gated like
//! `embed_parity.rs`: skips when the f32 model dir is absent.

mod common;

use common::TempDir;
use neocortex::embed::{self, Mode, WEIGHTS_Q4K, WEIGHTS_Q8};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Fixture {
    docs: Vec<Entry>,
    queries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    text: String,
    emb: Vec<f32>,
}

fn model_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("NEOCORTEX_MODEL_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".neocortex/models/UltraBERT-Embed-150M"))
        })?;
    if dir.join(embed::WEIGHTS_F32).is_file() {
        Some(dir)
    } else {
        println!(
            "skipping quantized parity: f32 model absent: {}",
            dir.display()
        );
        None
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm(a) * norm(b))
}

/// (min, mean) cosine of `dtype` embeddings against the sentence-transformers fp32 fixture.
fn parity(dir: &std::path::Path, dtype: &str) -> (f32, f32) {
    let f: Fixture = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/ultrabert_fixture.json").unwrap(),
    )
    .unwrap();
    let model = embed::load_dtype(Some(dir), Some(dtype)).unwrap();
    let mut cosines = Vec::new();
    for (entries, mode) in [(&f.docs, Mode::Document), (&f.queries, Mode::Query)] {
        let texts: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
        let got = model.embed(&texts, mode).unwrap();
        cosines.extend(got.iter().zip(entries).map(|(g, e)| cosine(g, &e.emb)));
    }
    let min = cosines.iter().cloned().fold(f32::MAX, f32::min);
    let mean = cosines.iter().sum::<f32>() / cosines.len() as f32;
    println!(
        "{dtype}: min cosine {min:.5}, mean {mean:.5} over {}",
        cosines.len()
    );
    (min, mean)
}

#[test]
fn q8_round_trip_loads_without_the_f32_file_and_matches_f32() {
    let Some(dir) = model_dir() else { return };
    let out = TempDir::new("quant");
    let bytes = embed::quantize_dir(&dir, &out, "q8").unwrap();
    assert!(out.join(WEIGHTS_Q8).is_file() && out.join("config.json").is_file());
    // About a quarter of the 604 MB f32 file (embedding table and linears in Q8_0).
    assert!(bytes < 175 << 20, "q8 weights are {bytes} bytes");
    // The converted dir has no safetensors: auto-detect must pick the gguf.
    assert!(embed::load(Some(&out)).is_ok());
    let (min, mean) = parity(&out, "q8");
    assert!(
        mean >= 0.995 && min >= 0.99,
        "q8 parity min {min} mean {mean}"
    );
}

/// q8 vs f32 on 40 memories and 40 queries of each eval set (the fixture above
/// has only 8 texts). Measured over all 927 texts of the four eval sets
/// (2026-10-26): min 0.99836, mean 0.99959. Capped here because the test
/// profile compiles the dequant loop unoptimised.
#[test]
fn q8_matches_f32_on_the_eval_corpora() {
    let Some(dir) = model_dir() else { return };
    let out = TempDir::new("quant");
    embed::quantize_dir(&dir, &out, "q8").unwrap();
    let (f32m, q8m) = (
        embed::load_dtype(Some(&dir), Some("f32")).unwrap(),
        embed::load_dtype(Some(&out), Some("q8")).unwrap(),
    );
    let mut cosines = Vec::new();
    for set in ["eval/fixtures.json", "eval/holdout.json"] {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(set).unwrap()).unwrap();
        for (field, key, mode) in [
            ("memories", "content", Mode::Document),
            ("queries", "q", Mode::Query),
        ] {
            let texts: Vec<&str> = v[field]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|m| m[key].as_str())
                .take(40)
                .collect();
            let (a, b) = (
                f32m.embed(&texts, mode).unwrap(),
                q8m.embed(&texts, mode).unwrap(),
            );
            cosines.extend(a.iter().zip(&b).map(|(x, y)| cosine(x, y)));
        }
    }
    let min = cosines.iter().cloned().fold(f32::MAX, f32::min);
    let mean = cosines.iter().sum::<f32>() / cosines.len() as f32;
    println!(
        "q8 vs f32 on {} eval texts: min {min:.5}, mean {mean:.5}",
        cosines.len()
    );
    assert!(mean >= 0.995 && min >= 0.99, "min {min} mean {mean}");
}

#[test]
fn q4k_parity_is_reported_and_stays_usable() {
    let Some(dir) = model_dir() else { return };
    let out = TempDir::new("quant");
    embed::quantize_dir(&dir, &out, "q4k").unwrap();
    assert!(out.join(WEIGHTS_Q4K).is_file());
    // Not a default candidate; only guard against a broken kernel.
    let (_, mean) = parity(&out, "q4k");
    assert!(mean >= 0.9, "q4k mean cosine {mean}");
}

#[test]
fn f32_loader_is_unchanged() {
    let Some(dir) = model_dir() else { return };
    let (min, mean) = parity(&dir, "f32");
    assert!(min >= 0.999 && mean >= 0.9999, "f32 min {min} mean {mean}");
}

#[test]
fn bad_dtype_and_missing_file_are_errors() {
    let Some(dir) = model_dir() else { return };
    assert!(embed::load_dtype(Some(&dir), Some("q5")).is_err());
    // The model dir ships no q8 file: the error names the conversion command.
    if !dir.join(WEIGHTS_Q8).exists() {
        let err = embed::load_dtype(Some(&dir), Some("q8")).err().unwrap();
        assert!(err.to_string().contains("quantize-model"), "{err}");
    }
}
