//! ER-07 threshold measurement: sweep the near-name auto-merge threshold
//! over the labelled pairs in tests/data/name_pairs.json (>= 40 pairs with
//! hard negatives) and report precision/recall per threshold.
//!
//! The decision under measurement is the name layer only: JW on the
//! transliteration key >= threshold AND no hard blocker. The entity-cosine
//! layer reuses the measured `entity_conflict = 0.73` threshold (see
//! Cargo's Config doc), so it is not swept here.
//!
//! Run: cargo run --release --example name_eval -- [pairs.json]

use neocortex::text::{jaro_winkler, name_blocker, norm_name, translit_norm};
use serde::Deserialize;

#[derive(Deserialize)]
struct Pairs {
    #[serde(default)]
    note: Option<String>,
    pairs: Vec<Pair>,
}

#[derive(Deserialize)]
struct Pair {
    a: String,
    b: String,
    label: String,
    kind: String,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tests/data/name_pairs.json".into());
    let raw = std::fs::read_to_string(&path).expect("read pairs file");
    let pairs: Pairs = serde_json::from_str(&raw).expect("parse pairs file");
    println!("{}", pairs.note.as_deref().unwrap_or_default());

    // Normalise once; report the raw JW and blocker per pair.
    struct Scored {
        a: String,
        b: String,
        label: String,
        kind: String,
        key_jw: f64,
        blocked: bool,
    }
    let scored: Vec<Scored> = pairs
        .pairs
        .iter()
        .map(|p| {
            let (na, nb) = (norm_name(&p.a), norm_name(&p.b));
            let (ka, kb) = (translit_norm(&na), translit_norm(&nb));
            Scored {
                a: p.a.clone(),
                b: p.b.clone(),
                label: p.label.clone(),
                kind: p.kind.clone(),
                key_jw: jaro_winkler(&ka, &kb),
                blocked: name_blocker(&na, &nb).is_some(),
            }
        })
        .collect();

    // Per-threshold sweep.
    println!("\nthreshold  precision  recall  f1     misses");
    for t in [
        70, 75, 80, 82, 84, 85, 86, 88, 90, 91, 92, 93, 94, 95, 96, 97, 98,
    ] {
        let t = t as f64 / 100.0;
        let (mut tp, mut fp, mut fn_) = (0usize, 0usize, 0usize);
        let mut miss = Vec::new();
        for p in &scored {
            let want = p.label == "same";
            let got = p.key_jw >= t && !p.blocked;
            match (want, got) {
                (true, true) => tp += 1,
                (false, true) => {
                    fp += 1;
                    miss.push(format!("FP {} / {}", p.a, p.b));
                }
                (true, false) => {
                    fn_ += 1;
                    miss.push(format!(
                        "FN {} / {} (jw {:.3}{})",
                        p.a,
                        p.b,
                        p.key_jw,
                        if p.blocked { ", blocked" } else { "" }
                    ));
                }
                (false, false) => {}
            }
        }
        let precision = if tp + fp == 0 {
            0.0
        } else {
            (tp as f64) / ((tp + fp) as f64)
        };
        let recall = if tp + fn_ == 0 {
            0.0
        } else {
            (tp as f64) / ((tp + fn_) as f64)
        };
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };
        println!(
            "{t:<9.2} {precision:<9.3} {recall:<7.3} {f1:<7.3} {}",
            miss.join("; "),
        );
    }

    // Per-pair table at the default threshold for the report.
    let t = 0.92;
    println!("\nper-pair (kind, key JW, blocker, decision at {t}):");
    for p in &scored {
        println!(
            "  {:<24.24} {:<22.22} {:<18.18} {:>6}  {:.3}  {:<5}  {}",
            p.a,
            p.b,
            p.kind,
            p.label,
            p.key_jw,
            if p.blocked { "block" } else { "-" },
            if p.key_jw >= t && !p.blocked {
                "merge"
            } else {
                "no"
            },
        );
    }
}
