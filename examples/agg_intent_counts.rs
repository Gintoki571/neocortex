//! Counts `paths::aggregate_intent` triggers per eval set (lane nc-agg round 2).
//!
//! ```text
//! cargo run --release --example agg_intent_counts -- eval/fixtures.json eval/holdout.json eval/multihop-holdout.json eval/multihop-holdout2.json
//! ```
//!
//! Prints, per set and category, how many queries trigger the list shape and
//! which ones. The tag channel runs on this shape, so a habitual
//! "every/each <time>" that triggers it floods a single-answer question.

use neocortex::paths;
use std::collections::BTreeMap;

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let queries = doc["queries"].as_array().expect("queries array");
        let mut per: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut hits = Vec::new();
        for q in queries {
            let text = q["q"].as_str().unwrap_or("");
            let cat = q["category"].as_str().unwrap_or("?").to_string();
            let e = per.entry(cat).or_default();
            e.1 += 1;
            if paths::aggregate_intent(text) {
                e.0 += 1;
                hits.push(text.to_string());
            }
        }
        let total = queries.len();
        let fired: usize = per.values().map(|(f, _)| *f).sum();
        println!("== {path}: {fired}/{total} trigger aggregate_intent");
        for (cat, (f, n)) in &per {
            if *f > 0 {
                println!("   {cat}: {f}/{n}");
            }
        }
        for q in hits {
            println!("   + {q}");
        }
    }
}
