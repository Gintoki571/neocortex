//! Offline candidate-recall check for the sign-bit prefilter (lane nc-binvec).
//!
//! ```text
//! NEOCORTEX_EMBEDDER=hash cargo run --release --example bits_recall -- <db> <fixtures.json> \
//!     [--k 50] [--mult 4,10,20] [--limit N]
//! cargo run --release --example bits_recall -- <db> --drop-bits
//! ```
//!
//! For every query of the fixture it embeds the text, takes the exact
//! `memories_vec` top-k, then takes the top `mult * k` rows of `memories_bits`
//! by Hamming distance and reranks them with the exact stored vectors. It
//! reports recall@k, the mean candidate rank of the true top-k, and the p50 of
//! the exact scan against the Hamming scan + rerank. `--drop-bits` removes the
//! derived table and VACUUMs, for a size baseline.

use neocortex::db::Db;
use neocortex::embed::Mode;
use neocortex::engine::Engine;
use neocortex::rank::cosine;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

fn p50(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn db_bytes(path: &Path) -> u64 {
    let mut total = 0;
    for suffix in ["", "-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{suffix}", path.display()));
        total += std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    }
    total
}

fn vectors(db: &Db, ids: &[i64]) -> HashMap<i64, Vec<f32>> {
    let mut out = HashMap::new();
    let mut st = db
        .sql()
        .prepare("SELECT embedding FROM memories_vec WHERE rowid=?1")
        .unwrap();
    for id in ids {
        if let Some(v) = st
            .query_row([id], |r| r.get::<_, Vec<u8>>(0))
            .ok()
            .and_then(|b| db.blob_to_vec(&b))
        {
            out.insert(*id, v);
        }
    }
    out
}

fn main() -> neocortex::Result<()> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut args = argv.into_iter();
    let db_path = PathBuf::from(args.next().unwrap_or_else(|| {
        eprintln!("usage: bits_recall <db> <fixtures.json> [--k N] [--mult 4,10,20] [--limit N]");
        eprintln!("       bits_recall <db> --drop-bits");
        std::process::exit(2)
    }));
    let rest: Vec<String> = args.collect();
    if rest.iter().any(|a| a == "--drop-bits") {
        let db = Db::open(&db_path)?;
        db.sql().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        db.sql().execute_batch("VACUUM")?;
        let with_bits = db_bytes(&db_path);
        db.sql().execute_batch("DROP TABLE memories_bits")?;
        db.sql().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        db.sql().execute_batch("VACUUM")?;
        let without = db_bytes(&db_path);
        let rows: i64 = db
            .sql()
            .query_row("SELECT count(*) FROM memories_vec", [], |r| r.get(0))?;
        println!(
            "vacuumed db bytes with bits: {with_bits}  without: {without}  delta: {} ({:.0} B/memory)",
            with_bits - without,
            (with_bits - without) as f64 / rows.max(1) as f64
        );
        return Ok(());
    }
    let fixture = PathBuf::from(rest.first().cloned().unwrap_or_else(|| {
        eprintln!("usage: bits_recall <db> <fixtures.json> [--k N] [--mult 4,10,20] [--limit N]");
        std::process::exit(2)
    }));
    let (mut k, mut limit) = (50usize, usize::MAX);
    let mut mults = vec![4usize, 10, 20];
    let mut flags = rest[1..].iter();
    while let Some(flag) = flags.next() {
        let value = flags.next().expect("flag needs a value");
        match flag.as_str() {
            "--k" => k = value.parse().unwrap(),
            "--limit" => limit = value.parse().unwrap(),
            "--mult" => {
                mults = value
                    .split(',')
                    .map(|x| x.parse().expect("bad --mult"))
                    .collect()
            }
            other => return Err(neocortex::Error(format!("unknown flag {other}"))),
        }
    }
    let data: Value = serde_json::from_str(&std::fs::read_to_string(&fixture)?)?;
    let queries: Vec<String> = data["queries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|q| q["q"].as_str().map(str::to_string))
        .take(limit)
        .collect();
    let engine = Engine::open(&db_path, Arc::new(neocortex::mcp::LazyEmbedder::new()))?;
    let db = &engine.db;
    println!(
        "db: {}  mode: {}  bits: {}  queries: {}  k: {k}",
        db_path.display(),
        db.vec_mode().name(),
        db.has_bits(),
        queries.len()
    );
    let emb = engine.emb.embed(
        &queries.iter().map(String::as_str).collect::<Vec<_>>(),
        Mode::Query,
    )?;
    let exact_sql = format!(
        "SELECT rowid FROM memories_vec WHERE embedding MATCH {} AND k=?2 AND live=1 ORDER BY distance",
        db.vec_sql("?1")
    );
    let mut exact_ms = Vec::new();
    let mut exact_hits: Vec<Vec<i64>> = Vec::new();
    for q in &emb {
        let start = Instant::now();
        let mut st = db.sql().prepare(&exact_sql)?;
        let ids: Vec<i64> = st
            .query_map(rusqlite::params![db.vec_blob(q), k as i64], |r| {
                r.get::<_, i64>(0)
            })?
            .collect::<rusqlite::Result<_>>()?;
        exact_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        exact_hits.push(ids);
    }
    println!(
        "exact top-{k}: p50 {:.2} ms  mean {:.2} ms",
        p50(exact_ms.clone()),
        mean(&exact_ms)
    );
    for mult in mults {
        let n = mult * k;
        let mut found = 0usize;
        let mut want = 0usize;
        let mut scan_ms = Vec::new();
        let mut rerank_ms = Vec::new();
        let mut ranks = Vec::new();
        let mut top1 = 0usize;
        for (qi, q) in emb.iter().enumerate() {
            let start = Instant::now();
            let ids = db.bits_knn(q, n, true)?;
            scan_ms.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = Instant::now();
            let vecs = vectors(db, &ids);
            let mut scored: Vec<(f32, i64)> = ids
                .iter()
                .filter_map(|id| vecs.get(id).map(|v| (cosine(q, v), *id)))
                .collect();
            scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            rerank_ms.push(start.elapsed().as_secs_f64() * 1000.0);
            let exact = &exact_hits[qi];
            let position: HashMap<i64, usize> = scored
                .iter()
                .enumerate()
                .map(|(i, (_, id))| (*id, i))
                .collect();
            for id in exact.iter().take(k) {
                want += 1;
                if let Some(p) = position.get(id) {
                    if *p < k {
                        found += 1;
                    }
                    ranks.push(*p as f64 + 1.0);
                } else {
                    ranks.push(n as f64);
                }
            }
            if scored.first().map(|x| x.1) == exact.first().copied() {
                top1 += 1;
            }
        }
        println!(
            "N={n:>5} ({mult}x): recall@{k} {:.3}  top-1 {:.3}  mean cand rank {:.1}  \
             hamming p50 {:.2} ms  rerank p50 {:.2} ms  total p50 {:.2} ms",
            found as f64 / want as f64,
            top1 as f64 / emb.len() as f64,
            mean(&ranks),
            p50(scan_ms.clone()),
            p50(rerank_ms.clone()),
            p50(scan_ms.iter().zip(&rerank_ms).map(|(a, b)| a + b).collect())
        );
    }
    Ok(())
}
