//! Verb-similarity calibration probe for the text-retraction synonym decision
//! (H3-12): cosine between retraction verb phrases and live predicate
//! canonicals under the real embedder. Prints the best threshold's precision
//! and recall over the pair list, then whether the same/different score ranges
//! are separable at all. Measured 2026-11-06: they are not (max same-pair
//! 0.7457 vs min different-pair 0.4707, best F1 0.57 at threshold 0.48), which
//! is why `remember::RETRACTION_VERB_SYNONYMS` is an explicit table instead.
//!
//!   cargo run --release --example verb_sim

use neocortex::embed::{load, Mode};

fn cos(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn main() {
    let model = load(None).expect("model");
    // (a, b, same?)
    let pairs: &[(&str, &str, bool)] = &[
        ("head", "lead", true),
        ("headed", "lead", true),
        ("run", "manage", true),
        ("oversee", "supervise", true),
        ("reside", "live", true),
        ("employ", "work", true),
        ("head", "manage", true),
        ("lead", "manage", true),
        ("approve", "review", false),
        ("work with", "work at", false),
        ("sell", "buy", false),
        ("own", "use", false),
        ("review", "manage", false),
        ("head", "own", false),
        ("lead", "work at", false),
        ("supervise", "review", false),
        ("run", "review", false),
        ("head", "review", false),
        ("oversee", "work at", false),
        ("sell", "own", false),
        ("buy", "use", false),
    ];
    let texts: Vec<&str> = pairs.iter().flat_map(|(a, b, _)| [*a, *b]).collect();
    let vecs = model.embed(&texts, Mode::Document).expect("embed");
    let mut rows = Vec::new();
    for (i, (a, b, same)) in pairs.iter().enumerate() {
        let s = cos(&vecs[2 * i], &vecs[2 * i + 1]);
        rows.push((*a, *b, *same, s));
        println!("{a}\t{b}\t{same}\t{s:.4}");
    }
    // Best threshold: positives are same=true.
    let mut best = (0.0f32, 0.0f32, 0.0f32, 0.0f32); // f1, prec, rec, thr
    for k in 0..=100 {
        let thr = k as f32 / 100.0;
        let mut tp = 0;
        let mut fp = 0;
        let mut fnn = 0;
        for (_, _, same, s) in &rows {
            if *s >= thr {
                if *same {
                    tp += 1
                } else {
                    fp += 1
                }
            } else if *same {
                fnn += 1
            }
        }
        let prec = if tp + fp == 0 {
            0.0
        } else {
            tp as f32 / (tp + fp) as f32
        };
        let rec = if tp + fnn == 0 {
            0.0
        } else {
            tp as f32 / (tp + fnn) as f32
        };
        let f1 = if prec + rec == 0.0 {
            0.0
        } else {
            2.0 * prec * rec / (prec + rec)
        };
        if f1 > best.0 {
            best = (f1, prec, rec, thr);
        }
    }
    println!(
        "BEST thr={:.2} precision={:.2} recall={:.2} f1={:.2}",
        best.3, best.1, best.2, best.0
    );
    let max_same = rows
        .iter()
        .filter(|r| r.2)
        .map(|r| r.3)
        .fold(0.0f32, f32::max);
    let min_diff = rows
        .iter()
        .filter(|r| !r.2)
        .map(|r| r.3)
        .fold(1.0f32, f32::min);
    println!("max same-pair cosine={max_same:.4}  min different-pair cosine={min_diff:.4}");
    println!(
        "separable={}",
        if max_same < min_diff {
            "yes"
        } else {
            "no (score ranges overlap)"
        }
    );
    for thr in [0.60f32, 0.65, 0.70, 0.75] {
        let tp = rows.iter().filter(|r| r.2 && r.3 >= thr).count();
        let fp = rows.iter().filter(|r| !r.2 && r.3 >= thr).count();
        let fnn = rows.iter().filter(|r| r.2 && r.3 < thr).count();
        let prec = if tp + fp == 0 {
            0.0
        } else {
            tp as f32 / (tp + fp) as f32
        };
        let rec = if tp + fnn == 0 {
            0.0
        } else {
            tp as f32 / (tp + fnn) as f32
        };
        println!("thr={thr:.2} precision={prec:.2} recall={rec:.2}");
    }
}
