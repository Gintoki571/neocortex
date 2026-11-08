//! BM25 score probe for nc-hay2 (diagnosis only, not shipped logic).
//!
//! ```text
//! cargo run --release --example bm25_probe -- <fixture.json> <db> <db>.keys.json <query...>
//! ```
//!
//! Prints the top-6 BM25 (recall FTS, exact `text::fts_query_with`) hits with
//! scores and the gold memories' BM25 ranks + scores, so a BM25-only loss can
//! be judged by score dominance, not just rank.

use neocortex::embed::{Embedder, Mode};
use neocortex::engine::Engine;
use neocortex::mcp::LazyEmbedder;
use neocortex::temporal::{self, Clock};
use neocortex::text;
use rusqlite::params;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Deserialize)]
struct Fixture {
    now: String,
    #[allow(dead_code)]
    memories: Vec<serde_json::Value>,
    queries: Vec<Query>,
}

#[derive(Deserialize)]
struct Query {
    q: String,
    category: String,
    expect: Vec<String>,
}

type Keys = HashMap<String, i64>;

fn main() -> neocortex::Result<()> {
    let mut args = std::env::args().skip(1);
    let fixture = PathBuf::from(args.next().expect("fixture.json"));
    let db_path = PathBuf::from(args.next().expect("db path"));
    let keys_path = PathBuf::from(args.next().expect("keys.json"));
    let want: Vec<String> = args.collect();
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
    let engine = Engine::open(&db_path, emb.clone())?.with_clock(clock);
    let keys: Keys = serde_json::from_str(&std::fs::read_to_string(&keys_path)?)?;
    for q in &data.queries {
        if !want.is_empty() && !want.iter().any(|w| q.q.contains(w)) {
            continue;
        }
        let query_text =
            q.q.split_whitespace()
                .take(64)
                .collect::<Vec<_>>()
                .join(" ");
        let Some(fq) = text::fts_query_with(&query_text, &[], engine.cfg.fts_prefix_min) else {
            println!("{} [{}]: NO FTS QUERY", q.q, q.category);
            continue;
        };
        let gold: Vec<i64> = q
            .expect
            .iter()
            .filter_map(|k| keys.get(k).copied())
            .collect();
        let mut st = engine.db.sql().prepare(
            "SELECT rowid,bm25(memories_fts,1.0,2.0) FROM memories_fts WHERE memories_fts MATCH ?1 ORDER BY bm25(memories_fts,1.0,2.0) LIMIT 20",
        )?;
        let rows: Vec<(i64, f64)> = st
            .query_map(params![fq], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        println!("Q [{}]: {}", q.category, q.q);
        for (i, (id, s)) in rows.iter().take(6).enumerate() {
            println!("  #{} m{} score {:.3}", i + 1, id, s);
        }
        for (i, (id, s)) in rows.iter().enumerate() {
            if gold.contains(id) {
                println!("  GOLD m{} rank {} score {:.3}", id, i + 1, s);
            }
        }
        let _ = Mode::Query;
    }
    Ok(())
}
