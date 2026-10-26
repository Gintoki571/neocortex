//! Cross-encoder reranker: numeric parity with the reference model (skips when
//! the model directory is absent) and the recall integration (no model needed).

use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, RecallInput, RememberInput};
use neocortex::rerank::{model_dir, CrossEncoder, LazyReranker, Reranker};
use neocortex::temporal::Clock;
use neocortex::Result;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct Fixture {
    pairs: Vec<Pair>,
}

#[derive(Deserialize)]
struct Pair {
    query: String,
    text: String,
    logit: f32,
}

fn fixture() -> Fixture {
    serde_json::from_str(&std::fs::read_to_string("tests/fixtures/rerank_fixture.json").unwrap())
        .unwrap()
}

fn model() -> Option<CrossEncoder> {
    let dir = model_dir(None);
    if !dir.is_dir() {
        println!(
            "skipping reranker parity: model directory absent: {}",
            dir.display()
        );
        return None;
    }
    Some(CrossEncoder::load(Some(&dir)).unwrap())
}

#[test]
fn model_card_scores_match() {
    let Some(model) = model() else { return };
    let berlin = "How many people live in Berlin?";
    let scores = model
        .score(
            berlin,
            &[
                "Berlin had a population of 3,520,031 registered inhabitants in an area of 891.82 square kilometers.",
                "Berlin is well known for its museums.",
            ],
        )
        .unwrap();
    // Printed on the model card: [ 8.607138 -4.320078 ].
    println!(
        "model card diff: {:e} {:e}",
        scores[0] - 8.607138,
        scores[1] + 4.320078
    );
    assert!((scores[0] - 8.607138).abs() < 1e-3, "{scores:?}");
    assert!((scores[1] + 4.320078).abs() < 1e-3, "{scores:?}");
}

#[test]
fn logits_match_transformers_reference_in_one_padded_batch() {
    let Some(model) = model() else { return };
    let f = fixture();
    // Different queries in one call go through the per-query path below; the
    // fixture mixes queries, so score each query's texts together.
    let mut worst = 0f32;
    let mut queries: Vec<&str> = f.pairs.iter().map(|p| p.query.as_str()).collect();
    queries.dedup();
    for query in queries {
        let group: Vec<&Pair> = f.pairs.iter().filter(|p| p.query == query).collect();
        let texts: Vec<&str> = group.iter().map(|p| p.text.as_str()).collect();
        let got = model.score(query, &texts).unwrap();
        for (got, pair) in got.iter().zip(&group) {
            worst = worst.max((got - pair.logit).abs());
        }
    }
    println!(
        "max abs logit diff vs transformers over {} pairs: {worst:e}",
        f.pairs.len()
    );
    assert!(worst < 1e-3, "max abs logit diff {worst}");
}

#[test]
fn batching_does_not_change_scores() {
    let Some(model) = model() else { return };
    let f = fixture();
    let query = f.pairs[0].query.as_str();
    let texts: Vec<&str> = f.pairs[..4].iter().map(|p| p.text.as_str()).collect();
    let together = model.score(query, &texts).unwrap();
    for (i, text) in texts.iter().enumerate() {
        let alone = model.score(query, &[text]).unwrap()[0];
        assert!(
            (alone - together[i]).abs() < 1e-3,
            "{alone} vs {}",
            together[i]
        );
    }
}

fn clock() -> Clock {
    Clock {
        now: 1_800_000_000,
        tz_offset_secs: 0,
    }
}

fn engine(reranker: Arc<dyn Reranker>, top: usize, weight: f64) -> Engine {
    let mut engine = Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(clock())
        .with_reranker(reranker);
    engine.cfg.rerank_top = top;
    engine.cfg.w_rerank = weight;
    for content in [
        "Ravi Menon enjoys partner dancing on weekends.",
        "Ravi Menon works at Initech as a data engineer.",
        "Ravi Menon is married to Meera Iyer.",
        "The production database runs PostgreSQL 16.",
    ] {
        engine
            .remember(RememberInput {
                content: content.into(),
                ..Default::default()
            })
            .unwrap();
    }
    engine
}

fn recall(engine: &Engine, query: &str) -> String {
    let out = engine
        .recall(&RecallInput {
            query: query.into(),
            ..Default::default()
        })
        .unwrap();
    serde_json::to_string(&out).unwrap()
}

/// Scores texts containing "married" high: stands in for a model that knows
/// "partner" and "married to" are the same relation.
struct MarriedStub;

impl Reranker for MarriedStub {
    fn score(&self, _query: &str, texts: &[&str]) -> Result<Vec<f32>> {
        Ok(texts
            .iter()
            .map(|t| if t.contains("married") { 10.0 } else { -10.0 })
            .collect())
    }
}

#[test]
fn absent_model_recall_equals_reranker_off() {
    let off = engine(
        Arc::new(LazyReranker::with_dir("/nonexistent/rerank")),
        0,
        0.5,
    );
    let on_without_model = engine(
        Arc::new(LazyReranker::with_dir("/nonexistent/rerank")),
        30,
        0.5,
    );
    for query in [
        "who is Ravi's partner",
        "where does Ravi work",
        "database",
        "Ravi",
    ] {
        assert_eq!(
            recall(&off, query),
            recall(&on_without_model, query),
            "{query}"
        );
    }
}

#[test]
fn reranker_reorders_the_fused_head() {
    let query = "who is Ravi's partner";
    let off = engine(Arc::new(MarriedStub), 0, 1.0);
    let on = engine(Arc::new(MarriedStub), 30, 1.0);
    let top = |e: &Engine| {
        e.recall(&RecallInput {
            query: query.into(),
            ..Default::default()
        })
        .unwrap()
        .memories[0]
            .content
            .clone()
    };
    assert!(
        !top(&off).contains("married"),
        "stub must have something to fix"
    );
    assert!(top(&on).contains("married"));
    // top 1 limits the head to the first candidate: nothing to reorder.
    let one = engine(Arc::new(MarriedStub), 1, 1.0);
    assert_eq!(recall(&one, query), recall(&off, query));
}

#[test]
fn rerank_is_off_by_default() {
    assert_eq!(neocortex::engine::Config::default().rerank_top, 0);
}
