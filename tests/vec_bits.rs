//! Binary prefilter over memory vectors (lane nc-binvec): the sign-bit table
//! keeps the exact KNN's top-k as a candidate set, writes keep it in sync, and
//! the knob off leaves recall output unchanged.

use neocortex::db::{Db, VecMode};
use neocortex::embed::{Embedder, HashEmbedder};
use neocortex::engine::{Engine, RecallInput, RememberInput};
use neocortex::rank::cosine;
use neocortex::temporal::{parse_instant, Clock};
use std::collections::HashSet;
use std::sync::Arc;

mod common;
use common::TempDir;

fn random_unit(seed: &mut u64) -> Vec<f32> {
    let mut v: Vec<f32> = (0..768)
        .map(|_| {
            *seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // Signed: a sign-bit table over an all-negative corpus would be
            // constant (the vec_int8 helper's [-1, 0) range is not a corpus).
            (*seed >> 33) as f32 / (1u64 << 31) as f32 * 2.0 - 1.0
        })
        .collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

fn exact_knn(db: &Db, q: &[f32], k: usize) -> Vec<i64> {
    let sql = format!(
        "SELECT rowid FROM memories_vec WHERE embedding MATCH {} AND k=?2 AND live=1 ORDER BY distance",
        db.vec_sql("?1")
    );
    let mut st = db.sql().prepare(&sql).unwrap();
    st.query_map(rusqlite::params![db.vec_blob(q), k as i64], |r| {
        r.get::<_, i64>(0)
    })
    .unwrap()
    .map(|r| r.unwrap())
    .collect()
}

/// 60 cluster centres x 50 docs: embeddings of related texts cluster, and a
/// uniform-random corpus is the worst case for sign hashing (measured there:
/// 0.90 at N = 10k, 0.98 at 20k), not what a memory store looks like.
fn clustered_corpus(seed: &mut u64, centres: usize, per: usize) -> Vec<Vec<f32>> {
    let centres: Vec<Vec<f32>> = (0..centres).map(|_| random_unit(seed)).collect();
    let mut docs = Vec::with_capacity(centres.len() * per);
    for c in &centres {
        for _ in 0..per {
            let n = random_unit(seed);
            docs.push(mix(c, &n, 0.6));
        }
    }
    docs
}

fn mix(a: &[f32], b: &[f32], w: f32) -> Vec<f32> {
    let mut v: Vec<f32> = a.iter().zip(b).map(|(x, y)| x + w * y).collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

#[test]
fn bit_prefilter_keeps_the_exact_top_k() {
    let db = Db::open_in_memory_with_mode(VecMode::F32).unwrap();
    let mut seed = 11u64;
    let docs = clustered_corpus(&mut seed, 60, 50);
    for (i, d) in docs.iter().enumerate() {
        db.insert_memory_vec(i as i64 + 1, d).unwrap();
    }
    db.ensure_bits().unwrap();
    let k = 50usize;
    let n = 10 * k;
    let (mut found, mut want) = (0usize, 0usize);
    for d in docs.iter().take(60) {
        let noise = random_unit(&mut seed);
        let q = mix(d, &noise, 0.3);
        let exact: HashSet<i64> = exact_knn(&db, &q, k).into_iter().collect();
        let mut reranked: Vec<(f32, i64)> = db
            .bits_knn(&q, n, true)
            .unwrap()
            .into_iter()
            .map(|id| (cosine(&q, &docs[id as usize - 1]), id))
            .collect();
        reranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let got: HashSet<i64> = reranked.into_iter().take(k).map(|(_, id)| id).collect();
        found += exact.intersection(&got).count();
        want += exact.len();
    }
    let recall = found as f64 / want as f64;
    assert!(recall >= 0.98, "recall@{k} over the bits = {recall:.3}");
}

#[test]
fn writes_keep_the_bit_table_in_sync() {
    let dir = TempDir::new("vec-bits-sync");
    let path = dir.join("b.db");
    let emb: Arc<dyn Embedder> = Arc::new(HashEmbedder);
    let clock = Clock {
        now: parse_instant(
            "2026-10-21T15:00:00Z",
            &Clock {
                now: 0,
                tz_offset_secs: 0,
            },
        )
        .unwrap(),
        tz_offset_secs: 0,
    };
    let e = Engine::open(&path, emb).unwrap().with_clock(clock);
    let remember = |content: &str| {
        e.remember(RememberInput {
            content: content.into(),
            ..Default::default()
        })
        .unwrap()
        .id
    };
    let a = remember("Ana works at the harbor clinic in Lisbon.");
    let b = remember("Ben moved to Porto last spring.");
    // Rows written before the table existed are backfilled once.
    e.db.ensure_bits().unwrap();
    let c = remember("Carla prefers oat milk in her coffee.");
    let live = |id: i64| -> Option<i64> {
        e.db.sql()
            .query_row("SELECT live FROM memories_bits WHERE rowid=?1", [id], |r| {
                r.get(0)
            })
            .ok()
    };
    for id in [a, b, c] {
        assert_eq!(live(id), Some(1), "m{id} has live bits");
    }
    // Supersede flips the target's live flag in both tables.
    let d = remember("Ben lives in Braga now.");
    let superseding = e
        .remember(RememberInput {
            content: "Ben moved from Porto to Braga.".into(),
            supersedes: vec![format!("m{d}")],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(superseding.superseded_memories, vec![d]);
    assert_eq!(live(d), Some(0));
    // Soft forget keeps the row as history; hard forget drops it.
    e.forget(&format!("m{c}"), false).unwrap();
    assert_eq!(live(c), Some(0));
    e.forget(&format!("m{a}"), true).unwrap();
    assert_eq!(live(a), None);
}

#[test]
fn backfill_resumes_a_partial_table() {
    let db = Db::open_in_memory_with_mode(VecMode::F32).unwrap();
    let mut seed = 21u64;
    let docs: Vec<Vec<f32>> = (0..20).map(|_| random_unit(&mut seed)).collect();
    for (i, d) in docs.iter().enumerate() {
        db.insert_memory_vec(i as i64 + 1, d).unwrap();
    }
    db.set_memory_vec_live(3, 0).unwrap();
    db.ensure_bits().unwrap();
    // Simulate a process killed mid-backfill: some rows missing, no marker.
    for id in [2, 4, 6, 8, 10, 3] {
        db.sql()
            .execute("DELETE FROM memories_bits WHERE rowid=?1", [id])
            .unwrap();
    }
    db.sql()
        .execute("DELETE FROM meta WHERE key='vec_bits'", [])
        .unwrap();
    db.ensure_bits().unwrap();
    let rows: Vec<(i64, i64)> = {
        let mut st = db
            .sql()
            .prepare("SELECT rowid, live FROM memories_bits ORDER BY rowid")
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(rows.len(), 20, "every vector has bits again");
    assert_eq!(
        rows.iter().find(|(id, _)| *id == 3).map(|(_, live)| *live),
        Some(0),
        "the resumed row keeps its live flag"
    );
}

#[test]
fn chunk_scan_matches_a_manual_hamming_top_k() {
    let db = Db::open_in_memory_with_mode(VecMode::F32).unwrap();
    let mut seed = 31u64;
    let docs: Vec<Vec<f32>> = (0..300).map(|_| random_unit(&mut seed)).collect();
    for (i, d) in docs.iter().enumerate() {
        db.insert_memory_vec(i as i64 + 1, d).unwrap();
    }
    db.set_memory_vec_live(7, 0).unwrap();
    db.ensure_bits().unwrap();
    let q = random_unit(&mut seed);
    let qb = neocortex::db::sign_bits(&q);
    let hamming =
        |a: &[u8], b: &[u8]| -> u32 { a.iter().zip(b).map(|(x, y)| (x ^ y).count_ones()).sum() };
    let mut manual: Vec<u32> = docs
        .iter()
        .enumerate()
        .filter(|(i, _)| *i + 1 != 7)
        .map(|(_, d)| hamming(&qb, &neocortex::db::sign_bits(d)))
        .collect();
    manual.sort_unstable();
    manual.truncate(20);
    let mut got: Vec<u32> = db
        .bits_knn(&q, 20, true)
        .unwrap()
        .into_iter()
        .map(|id| hamming(&qb, &neocortex::db::sign_bits(&docs[id as usize - 1])))
        .collect();
    got.sort_unstable();
    assert_eq!(
        got, manual,
        "the chunk scan returns the 20 smallest distances"
    );
    assert!(
        !db.bits_knn(&q, 300, true).unwrap().contains(&7),
        "a dead row is filtered by the live metadata"
    );
}

#[test]
fn knob_off_is_unchanged_and_bit_agrees() {
    let dir = TempDir::new("vec-bits-knob");
    let path = dir.join("k.db");
    let emb: Arc<dyn Embedder> = Arc::new(HashEmbedder);
    let clock = Clock {
        now: parse_instant(
            "2026-10-21T15:00:00Z",
            &Clock {
                now: 0,
                tz_offset_secs: 0,
            },
        )
        .unwrap(),
        tz_offset_secs: 0,
    };
    let mut e = Engine::open(&path, emb).unwrap().with_clock(clock);
    // Access recording changes retention scores between calls; the contract
    // here is the ranking, so switch the blend off.
    e.cfg.w_retention = 0.0;
    for i in 0..40 {
        e.remember(RememberInput {
            content: format!(
                "Person {i} works at site {i} in city {i}, on team {}",
                i % 7
            ),
            ..Default::default()
        })
        .unwrap();
    }
    let recall = |e: &Engine| {
        e.recall(&RecallInput {
            query: "where does person 3 work".into(),
            k: Some(10),
            ..Default::default()
        })
        .unwrap()
    };
    let base = recall(&e);
    let base_json = serde_json::to_string(&base).unwrap();
    assert!(!e.db.has_bits(), "knob off never creates the table");
    e.cfg.vec_prefilter = "bit".into();
    let on = recall(&e);
    assert!(e.db.has_bits(), "the first bit recall creates the table");
    e.cfg.vec_prefilter = "off".into();
    let off = recall(&e);
    assert_eq!(
        serde_json::to_string(&off).unwrap(),
        base_json,
        "knob off is unchanged after the table exists"
    );
    let ids = |o: &neocortex::engine::RecallOutput| -> Vec<i64> {
        let mut v: Vec<i64> = o.memories.iter().map(|m| m.id).collect();
        v.sort_unstable();
        v
    };
    // The exact rerank computes the cosine itself instead of reading vec0's
    // distance, so two near-tied candidates can swap places in the fused order;
    // the contract is the returned set.
    assert_eq!(ids(&on), ids(&base), "the prefilter keeps the top 10");
}
