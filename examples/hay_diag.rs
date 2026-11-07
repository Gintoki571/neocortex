//! Diagnosis of recall loss inside a haystack store (lane nc-hay1, step 1).
//!
//! ```text
//! cargo run --release --example hay_diag -- eval/fixtures.json <db> <db>.keys.json [--all]
//! ```
//!
//! For each query it prints, side by side in one row: the raw BM25 rank and
//! vector rank of the gold memory (top-2000 scans, so "beyond the pool" reads
//! `>2000`), whether the gold memory's entities are query anchors, its PPR
//! rank (the exact `recall` seed pipeline, minus the relation-seed step the
//! graph channel also feeds), where the gold landed in the returned output
//! (memory rank + per-channel reasons, facts, context), and whether a gold
//! entity is merged with haystack mentions (collision check: mentions by
//! memories outside the set's key map).
//!
//! Query selection: only queries that answer (acc pass and recall==1) in the
//! alone DB but miss in this DB, when given `<db>`/keys for the haystack file
//! plus `--alone <db> <keys>`; without `--alone` it prints every query.

use neocortex::anchor::query_anchors;
use neocortex::embed::{Embedder, Mode};
use neocortex::engine::{Engine, RecallInput};
use neocortex::graph;
use neocortex::kg;
use neocortex::mcp::LazyEmbedder;
use neocortex::temporal::{self, Clock};
use neocortex::text;
use rusqlite::params;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Deserialize)]
struct Fixture {
    now: String,
    #[allow(dead_code)]
    memories: Vec<serde_json::Value>,
    queries: Vec<Query>,
}

#[derive(Deserialize, Clone)]
struct Query {
    q: String,
    category: String,
    expect: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    expect_absent: Vec<String>,
    #[serde(default)]
    answers: Vec<String>,
    #[serde(default)]
    as_of: Option<String>,
    #[serde(default)]
    include_history: bool,
    #[serde(default)]
    k: Option<usize>,
}

type Keys = HashMap<String, i64>;

struct QResult {
    answered: bool,
    recall_hit: bool,
    bm25_rank: Option<usize>,
    vec_rank: Option<usize>,
    vec_cosine: f64,
    graph_rank: Option<usize>,
    anchor_hit: bool,
    collision: bool,
    mem_rank: Option<usize>,
    reasons: String,
    fact_in: bool,
    ctx_answer: bool,
    detail: String,
}

fn context_text(out: &neocortex::engine::RecallOutput, k: usize) -> String {
    let mut t = String::new();
    for m in out.memories.iter().take(k) {
        t.push_str(&m.content);
        t.push('\n');
    }
    for f in &out.facts {
        t.push_str(&f.fact);
        t.push('\n');
        t.push_str(&f.source);
        t.push(' ');
        t.push_str(&f.target);
        t.push('\n');
    }
    for e in &out.entities {
        t.push_str(&e.name);
        t.push(' ');
        t.push_str(&e.aliases.join(" "));
        t.push(' ');
        t.push_str(&e.description);
        t.push('\n');
    }
    t.to_lowercase()
}

/// Raw BM25 rank of `gold` under the recall FTS query (top-2000 scan).
fn bm25_rank(engine: &Engine, query: &str, gold: &[i64]) -> Option<usize> {
    let q = text::fts_query_with(query, &[], engine.cfg.fts_prefix_min)?;
    let mut st = engine
        .db
        .sql()
        .prepare(
            "SELECT rowid FROM memories_fts WHERE memories_fts MATCH ?1
         ORDER BY bm25(memories_fts,1.0,2.0) LIMIT 2000",
        )
        .ok()?;
    let ids: Vec<i64> = st
        .query_map(params![q], |r| r.get(0))
        .ok()?
        .collect::<Result<_, _>>()
        .ok()?;
    ids.iter().position(|id| gold.contains(id)).map(|p| p + 1)
}

/// Raw vector rank of `gold` (memories_vec KNN, top-2000 scan).
fn vec_rank(engine: &Engine, qvec: &[f32], gold: &[i64]) -> (Option<usize>, f64) {
    let sql = format!(
        "SELECT rowid,1-distance FROM memories_vec WHERE embedding MATCH {} AND k=2000 AND live=1 ORDER BY distance",
        engine.db.vec_sql("?1")
    );
    let Ok(mut st) = engine.db.sql().prepare(&sql) else {
        return (None, 0.0);
    };
    let hits: Vec<(i64, f32)> = st
        .query_map(params![engine.db.vec_blob(qvec)], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .ok()
        .and_then(|rows| rows.collect::<Result<_, _>>().ok())
        .unwrap_or_default();
    let pos = hits
        .iter()
        .position(|(id, _)| gold.contains(id))
        .map(|p| p + 1);
    let cos = hits
        .iter()
        .find(|(id, _)| gold.contains(id))
        .map(|(_, c)| *c as f64)
        .unwrap_or(0.0);
    (pos, cos)
}

/// Gold memory -> its entity ids; gold memory -> its live relation ids.
fn gold_entities(engine: &Engine, gold: &[i64]) -> HashMap<i64, Vec<i64>> {
    let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
    let sql = engine.db.sql();
    for id in gold {
        let mut st = sql
            .prepare("SELECT entity_id FROM mentions WHERE memory_id=?1")
            .unwrap();
        let v: Vec<i64> = st
            .query_map(params![id], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        out.insert(*id, v);
    }
    out
}

fn gold_relations(engine: &Engine, gold: &[i64]) -> Vec<i64> {
    let sql = engine.db.sql();
    let mut out = Vec::new();
    for id in gold {
        let mut st = sql
            .prepare("SELECT relation_id FROM relation_evidence WHERE memory_id=?1")
            .unwrap();
        let v: Vec<i64> = st
            .query_map(params![id], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        out.extend(v);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The exact graph-channel pipeline of `recall` (anchors + entity KNN +
/// relation seeds replicated from recall.rs), returning memory PPR ranks.
fn graph_ranks(engine: &Engine, query: &str, qvec: &[f32], history: bool) -> HashMap<i64, usize> {
    let name_parts = engine.cfg.name_part_hop.is_finite() && engine.cfg.name_part_hop > 0.0;
    let gen: i64 = engine
        .db
        .sql()
        .query_row(
            "SELECT COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key='gen'),0)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if let Some((g, a, h, snap)) = engine.snapshot_cache.lock().unwrap().as_ref() {
        if *g == gen && a.is_none() && *h == history {
            return graph_ranks_with_snap(
                engine,
                query,
                qvec,
                std::sync::Arc::clone(snap),
                name_parts,
            );
        }
    }
    let snap =
        std::sync::Arc::new(kg::snapshot_with(&engine.db, None, history, name_parts).unwrap());
    *engine.snapshot_cache.lock().unwrap() =
        Some((gen, None, history, std::sync::Arc::clone(&snap)));
    graph_ranks_with_snap(engine, query, qvec, snap, name_parts)
}

fn graph_ranks_with_snap(
    engine: &Engine,
    query: &str,
    qvec: &[f32],
    snap: std::sync::Arc<kg::Snapshot>,
    _name_parts: bool,
) -> HashMap<i64, usize> {
    let now = engine.clock().now;
    let mut names: Vec<String> = neocortex::extract::ngrams(query, 4)
        .into_iter()
        .map(|x| x.0)
        .collect();
    names.sort();
    names.dedup();
    let anchors = query_anchors(&engine.db, query, &names, &engine.cfg.anchor_tiers, now).unwrap();
    let mut ent_seeds: HashMap<i64, f64> = HashMap::new();
    for id in anchors.all() {
        ent_seeds.insert(id, 1.0);
    }
    // entity KNN
    {
        let mut st = engine.db.sql().prepare(&format!(
            "SELECT rowid,1-distance FROM entities_vec WHERE embedding MATCH {} AND k=10 ORDER BY distance",
            engine.db.vec_sql("?1")
        )).unwrap();
        let hits: Vec<(i64, f32)> = st
            .query_map(params![engine.db.vec_blob(qvec)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for (id, c) in hits {
            if c >= engine.cfg.entity_seed {
                ent_seeds.entry(id).or_insert(c as f64);
            }
        }
    }
    // relation seeds (FTS top-10 + relations_vec KNN top-10), as in recall.rs
    let mut rel_entities: HashMap<i64, f64> = HashMap::new();
    if let Some(q) = text::fts_query_with(query, &[], engine.cfg.fts_prefix_min) {
        let mut st = engine.db.sql().prepare(
            "SELECT r.src,r.dst FROM relations_fts JOIN relations r ON r.id=relations_fts.rowid
             WHERE relations_fts MATCH ?1 AND r.valid_from<=?2 AND (r.valid_to IS NULL OR r.valid_to>?2) AND r.expired_at IS NULL
             ORDER BY bm25(relations_fts,1.0,1.0) LIMIT 10").unwrap();
        let hits: Vec<(i64, i64)> = st
            .query_map(params![q, now], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        for (rank_, (src, dst)) in hits.iter().enumerate() {
            let s = 0.5 / (1.0 + rank_ as f64);
            for id in [src, dst] {
                rel_entities.entry(*id).or_insert(s);
            }
        }
    }
    {
        let mut st = engine.db.sql().prepare(&format!(
            "SELECT rowid,1-distance FROM relations_vec WHERE embedding MATCH {} AND k=10 ORDER BY distance",
            engine.db.vec_sql("?1")
        )).unwrap();
        let hits: Vec<(i64, f32)> = st
            .query_map(params![engine.db.vec_blob(qvec)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        for (id, c) in hits {
            if c < 0.5 {
                continue;
            }
            let row: Option<(i64, i64, Option<i64>, Option<i64>)> = engine
                .db
                .sql()
                .query_row(
                    "SELECT src,dst,valid_to,expired_at FROM relations WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .ok();
            if let Some((src, dst, vt, expired)) = row {
                if expired.is_none() && vt.is_none_or(|v| v > now) {
                    for e in [src, dst] {
                        rel_entities.entry(e).or_insert(c as f64 * 0.5);
                    }
                }
            }
        }
    }
    for (id, w) in &rel_entities {
        ent_seeds
            .entry(*id)
            .and_modify(|e| *e = e.max(*w))
            .or_insert(*w);
    }
    // specificity
    let seed_ids: Vec<i64> = ent_seeds.keys().copied().collect();
    let mut spec: HashMap<i64, i64> = HashMap::new();
    // mentions counts via one aggregate query
    if !seed_ids.is_empty() {
        let marks = std::iter::repeat_n("?", seed_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT entity_id,COUNT(*) FROM mentions WHERE entity_id IN ({marks}) GROUP BY entity_id");
        let mut st = engine.db.sql().prepare(&sql).unwrap();
        let mut rows = st
            .query(rusqlite::params_from_iter(seed_ids.iter()))
            .unwrap();
        while let Ok(Some(r)) = rows.next() {
            let id: i64 = r.get(0).unwrap();
            let n: i64 = r.get(1).unwrap();
            spec.insert(id, n);
        }
    }
    for (id, w) in ent_seeds.iter_mut() {
        *w /= spec.get(id).copied().unwrap_or(0).max(1) as f64;
    }
    // memory seeds: top-20 dense (vec ranked)
    let mut mem_seeds: HashMap<i64, f64> = HashMap::new();
    {
        let sql = format!("SELECT rowid,1-distance FROM memories_vec WHERE embedding MATCH {} AND k=20 AND live=1 ORDER BY distance", engine.db.vec_sql("?1"));
        let mut st = engine.db.sql().prepare(&sql).unwrap();
        let hits: Vec<(i64, f32)> = st
            .query_map(params![engine.db.vec_blob(qvec)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        for (id, c) in hits {
            mem_seeds.insert(id, (c as f64).max(0.0));
        }
    }
    let ent_total: f64 = ent_seeds.values().sum();
    let mem_total: f64 = mem_seeds.values().sum();
    let mut seeds: Vec<(u32, f64)> = Vec::new();
    if ent_total > 0.0 {
        for (id, w) in &ent_seeds {
            if let Some(&node) = snap.ent_node.get(id) {
                seeds.push((node, w * (1.0 - engine.cfg.mem_seed_share) / ent_total));
            }
        }
    }
    if mem_total > 0.0 {
        let target = if ent_total > 0.0 {
            engine.cfg.mem_seed_share
        } else {
            1.0
        };
        for (id, w) in &mem_seeds {
            if let Some(&node) = snap.mem_node.get(id) {
                seeds.push((node, w * target / mem_total));
            }
        }
    }
    let ppr = graph::ppr_push(&snap.csr, &seeds, engine.cfg.alpha, engine.cfg.eps, 200_000);
    let mem_order: Vec<i64> = ppr
        .iter()
        .filter_map(|(node, _)| match snap.node_kind.get(*node as usize) {
            Some(('m', id)) => Some(*id),
            _ => None,
        })
        .collect();
    mem_order
        .into_iter()
        .enumerate()
        .map(|(i, id)| (id, i + 1))
        .collect()
}

fn diagnose(engine: &Engine, q: &Query, ids: &Keys, set_ids: &HashSet<i64>) -> QResult {
    let gold: Vec<i64> = q
        .expect
        .iter()
        .filter_map(|k| ids.get(k).copied())
        .collect();
    let k = q.k.unwrap_or(8);
    let requested = if q.answers.is_empty() {
        q.k
    } else {
        Some(k.max(10))
    };
    let out = engine
        .recall(&RecallInput {
            query: q.q.clone(),
            k: requested,
            as_of: q.as_of.clone(),
            include_history: q.include_history,
            ..Default::default()
        })
        .expect("recall");
    let found: Vec<i64> = out.memories.iter().take(k).map(|m| m.id).collect();
    let recall_hit = !gold.is_empty() && gold.iter().all(|id| found.contains(id));
    let ctx = context_text(&out, 10);
    let ctx_answer =
        q.answers.is_empty() || q.answers.iter().all(|a| ctx.contains(&a.to_lowercase()));
    let _answered = recall_hit && ctx_answer;
    // channel ranks
    let qvec = engine.emb.embed(&[&q.q], Mode::Query).unwrap().remove(0);
    let b = bm25_rank(engine, &q.q, &gold);
    let (v, vc) = vec_rank(engine, &qvec, &gold);
    let history = q.include_history || q.as_of.is_some();
    let gr = graph_ranks(engine, &q.q, &qvec, history);
    let best_g = gold.iter().filter_map(|id| gr.get(id).copied()).min();
    // anchors + entities of gold
    let now = engine.clock().now;
    let mut names: Vec<String> = neocortex::extract::ngrams(&q.q, 4)
        .into_iter()
        .map(|x| x.0)
        .collect();
    names.sort();
    names.dedup();
    let anchors = query_anchors(&engine.db, &q.q, &names, &engine.cfg.anchor_tiers, now).unwrap();
    let aset: HashSet<i64> = anchors.all().into_iter().collect();
    let ge = gold_entities(engine, &gold);
    let anchor_hit = ge.values().flatten().any(|e| aset.contains(e));
    // collision: a gold entity with mentions by non-set memories
    let mut collision = false;
    for id in &gold {
        for e in ge.get(id).cloned().unwrap_or_default() {
            let _cnt: i64 = engine
                .db
                .sql()
                .query_row(
                    "SELECT count(*) FROM mentions WHERE entity_id=?1",
                    [e],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let set_cnt: i64 = engine.db.sql().query_row(
                "SELECT count(*) FROM mentions WHERE entity_id=?1 AND memory_id IN (SELECT id FROM memories WHERE id=?)",
                params![e, id], |r| r.get(0)).unwrap_or(0);
            let _ = set_cnt;
            // non-set mentions: recompute via app-side set membership
            let all: Vec<i64> = {
                let mut st = engine
                    .db
                    .sql()
                    .prepare("SELECT memory_id FROM mentions WHERE entity_id=?1")
                    .unwrap();
                st.query_map(params![e], |r| r.get(0))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap_or_default()
            };
            let outside = all.iter().filter(|m| !set_ids.contains(m)).count();
            if outside > 0 {
                collision = true;
            }
        }
    }
    let mem_rank = gold
        .iter()
        .filter_map(|id| out.memories.iter().position(|m| m.id == *id).map(|p| p + 1))
        .min();
    let acc_ok = ctx_answer;
    let reasons = gold
        .iter()
        .find_map(|id| {
            out.memories
                .iter()
                .find(|m| m.id == *id)
                .map(|m| m.reasons.join(" "))
        })
        .unwrap_or_default();
    let grels = gold_relations(engine, &gold);
    let rel_info = {
        // gold relation cosine + its rank inside relations_vec
        if grels.is_empty() {
            String::new()
        } else {
            let sql = format!("SELECT rowid,1-distance FROM relations_vec WHERE embedding MATCH {} AND k=2000 ORDER BY distance", engine.db.vec_sql("?1"));
            let rows: Vec<(i64, f32)> = {
                let mut st = engine.db.sql().prepare(&sql).unwrap();
                st.query_map(params![engine.db.vec_blob(&qvec)], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap_or_default()
            };
            let pos: Vec<String> = grels
                .iter()
                .map(|g| {
                    let p = rows.iter().position(|(id, _)| id == g).map(|p| p + 1);
                    let c = rows
                        .iter()
                        .find(|(id, _)| id == g)
                        .map(|(_, c)| format!("{:.3}", c))
                        .unwrap_or_else(|| {
                            // exact cosine by point read when outside the top-2000
                            let v: Option<Vec<f32>> = engine
                                .db
                                .sql()
                                .prepare("SELECT embedding FROM relations_vec WHERE rowid=?1")
                                .ok()
                                .and_then(|mut st| {
                                    st.query_row([g], |r| r.get::<_, Vec<u8>>(0)).ok()
                                })
                                .and_then(|blob| engine.db.blob_to_vec(&blob));
                            v.map(|v| format!("{:.3}", neocortex::rank::cosine(&qvec, &v)))
                                .unwrap_or_else(|| "n/a".into())
                        });
                    format!(
                        "r{}@{}(cos {})",
                        g,
                        p.map(|p| p.to_string()).unwrap_or_else(|| ">2000".into()),
                        c
                    )
                })
                .collect();
            format!(" | relvec: {}", pos.join(" "))
        }
    };
    let fact_in = grels
        .iter()
        .any(|r| out.facts.iter().any(|f| f.id == *r && !f.memory));
    // detail: anchors names, gold memory content, top retrieved, missing answers
    let anchor_names: Vec<String> = anchors
        .all()
        .iter()
        .map(|id| {
            engine
                .db
                .sql()
                .query_row("SELECT name FROM entities WHERE id=?1", [id], |r| r.get(0))
                .unwrap_or_else(|_| format!("e{id}"))
        })
        .collect();
    let gold_text = gold
        .iter()
        .map(|id| {
            engine
                .db
                .sql()
                .query_row("SELECT content FROM memories WHERE id=?1", [id], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap_or_else(|_| "?".into())
        })
        .collect::<Vec<_>>()
        .join(" | ");
    let missing: Vec<String> = q
        .answers
        .iter()
        .filter(|a| !ctx.contains(&a.to_lowercase()))
        .cloned()
        .collect();
    let detail = format!(
        "anchors=[{}] goldent={:?} gold={} missing={:?}{} facts=[{}]",
        anchor_names.join(","),
        ge.values()
            .flatten()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        gold_text,
        missing,
        rel_info,
        out.facts
            .iter()
            .map(|f| format!("{}-{}->{}", f.id, f.source, f.target))
            .collect::<Vec<_>>()
            .join("; ")
    );
    QResult {
        answered: acc_ok,
        recall_hit,
        bm25_rank: b,
        vec_rank: v,
        vec_cosine: vc,
        graph_rank: best_g,
        anchor_hit,
        collision,
        mem_rank,
        reasons,
        fact_in,
        ctx_answer,
        detail,
    }
}

fn main() -> neocortex::Result<()> {
    let mut args = std::env::args().skip(1);
    let fixture = PathBuf::from(args.next().expect("fixture.json"));
    let db_path = PathBuf::from(args.next().expect("db path"));
    let keys_path = PathBuf::from(args.next().expect("keys.json"));
    let mut alone: Option<(PathBuf, PathBuf)> = None;
    let mut only_cat = String::new();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--alone" => {
                let d = args.next().expect("--alone db");
                let k = args.next().expect("--alone keys");
                alone = Some((PathBuf::from(d), PathBuf::from(k)));
            }
            "--category" => {
                only_cat = args.next().expect("--category");
            }
            other => eprintln!("warn: unknown flag {other}"),
        }
    }
    let data: Fixture = serde_json::from_str(&std::fs::read_to_string(&fixture)?)?;
    let now = temporal::parse_instant(
        &data.now,
        &Clock {
            now: 0,
            tz_offset_secs: 0,
        },
    )
    .unwrap();
    let clock = Clock {
        now,
        tz_offset_secs: 0,
    };
    let emb: Arc<dyn Embedder> = Arc::new(LazyEmbedder::new());
    let open =
        |path: &PathBuf, keys_file: &PathBuf| -> neocortex::Result<(Engine, Keys, HashSet<i64>)> {
            let engine = Engine::open(path, emb.clone())?.with_clock(clock);
            let keys: Keys = serde_json::from_str(&std::fs::read_to_string(keys_file)?)?;
            let set_ids: HashSet<i64> = keys.values().copied().collect();
            Ok((engine, keys, set_ids))
        };
    let (engine, keys, set_ids) = open(&db_path, &keys_path)?;
    let alone_engine = match &alone {
        Some((d, k)) => Some(open(d, k)?),
        None => None,
    };
    println!("query\tcat\talone_ok\thay_ok\tbm25\thay_bm25\tvec\thay_vec\tgraph\thay_graph\tanchor\thay_anchor\tcollision\tmem_rank\tfact_in\tctx");
    for q in &data.queries {
        if !only_cat.is_empty() && q.category != only_cat {
            continue;
        }
        let hay = diagnose(&engine, q, &keys, &set_ids);
        let (ao, a, h) = match &alone_engine {
            Some((ae, akeys, aset)) => {
                let a = diagnose(ae, q, akeys, aset);
                (Some(a.answered), a, hay)
            }
            None => {
                let a = QResult {
                    answered: hay.answered,
                    recall_hit: hay.recall_hit,
                    bm25_rank: hay.bm25_rank,
                    vec_rank: hay.vec_rank,
                    vec_cosine: hay.vec_cosine,
                    graph_rank: hay.graph_rank,
                    anchor_hit: hay.anchor_hit,
                    collision: hay.collision,
                    mem_rank: hay.mem_rank,
                    reasons: hay.reasons.clone(),
                    fact_in: hay.fact_in,
                    ctx_answer: hay.ctx_answer,
                    detail: String::new(),
                };
                (None, a, hay)
            }
        };
        let alone_ok = ao.unwrap_or(h.answered);
        // print only queries answered alone but not in the haystack, or when alone info missing all misses
        if alone.is_some() && !(alone_ok && !h.answered) {
            continue;
        }
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            q.q.replace(['\t', '\n'], " "),
            q.category,
            alone_ok,
            h.answered,
            fmt_rank(a.bm25_rank),
            fmt_rank(h.bm25_rank),
            fmt_rank(a.vec_rank),
            fmt_rank(h.vec_rank),
            fmt_rank(a.graph_rank),
            fmt_rank(h.graph_rank),
            a.anchor_hit,
            h.anchor_hit,
            h.collision,
            fmt_rank(h.mem_rank),
            h.fact_in,
            h.ctx_answer,
        );
        println!("   REASONS hay: {} | detail: {}", h.reasons, h.detail);
        println!(
            "   REASONS alone: {} | bm25 alone detail: anchors/detail from hay row above",
            a.reasons
        );
        println!(
            "   alone vec cosine {:.3} vs hay vec cosine {:.3}",
            a.vec_cosine, h.vec_cosine
        );
    }
    Ok(())
}

fn fmt_rank(r: Option<usize>) -> String {
    match r {
        Some(v) => v.to_string(),
        None => ">2000".into(),
    }
}
