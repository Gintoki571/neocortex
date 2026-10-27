//! Entity-merge audit over a generated world (`examples/gen_world.rs`).
//!
//! Ground truth is the world's people roster (every generated person has a
//! unique true full name), so an entity is a FALSE MERGE when its stored name
//! and aliases contain two or more different true people, and a MISSED MERGE
//! when one true person is spread over two or more entities.
//!
//! For every false merge the tool prints the write that most likely caused it
//! (the first memory naming the second person) and the entities co-mentioned
//! in that same write - the input any corroboration rule would see.
//!
//! Run:
//! ```text
//! cargo run --release --example gen_world -- --people 3000 --seed 3001 \
//!     --out lanes/tmp/nc-er08/world-3000.json --roster lanes/tmp/nc-er08/world-3000.roster.json
//! cargo run --release --example merge_audit -- lanes/tmp/nc-er08/world-3000.json \
//!     lanes/tmp/nc-er08/world-3000.roster.json
//! ```
//! `--real` uses NEOCORTEX_MODEL_DIR (default: hash embedder - the name layer
//! decides merges for a hash embedder, so it is enough to audit names; it is
//! also what makes 32k-memory worlds fast). `--db PATH` keeps the ingested
//! database for inspection.

use neocortex::{
    embed::{load, Embedder, HashEmbedder},
    engine::{Engine, RememberInput},
    temporal::{self, Clock},
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Deserialize)]
struct World {
    now: String,
    memories: Vec<MemoryFixture>,
}

#[derive(Deserialize)]
struct MemoryFixture {
    #[serde(rename = "key")]
    _key: String,
    #[serde(flatten)]
    input: RememberInput,
}

#[derive(Deserialize)]
struct Roster {
    people: Vec<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("merge_audit: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut real = false;
    let mut db_path: Option<PathBuf> = None;
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--real" => real = true,
            "--db" => {
                i += 1;
                db_path = Some(PathBuf::from(
                    args.get(i).ok_or("--db needs a path")?.clone(),
                ));
            }
            other if other.starts_with("--") => return Err(format!("unknown argument '{other}'")),
            other => files.push(other.to_string()),
        }
        i += 1;
    }
    if files.len() != 2 {
        return Err("usage: merge_audit <world.json> <roster.json> [--real] [--db PATH]".into());
    }
    let World { now, memories } =
        serde_json::from_str(&std::fs::read_to_string(&files[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let Roster { people } =
        serde_json::from_str(&std::fs::read_to_string(&files[1]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let now = temporal::parse_instant(
        &now,
        &Clock {
            now: 0,
            tz_offset_secs: 0,
        },
    )
    .ok_or_else(|| format!("invalid world now '{now}'"))?;
    let clock = Clock {
        now,
        tz_offset_secs: 0,
    };
    let embedder: Arc<dyn Embedder> = if real {
        let dir = std::env::var_os("NEOCORTEX_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(format!(
                    "{}/.neocortex/models/UltraBERT-Embed-150M",
                    std::env::var("HOME").unwrap_or_default()
                ))
            });
        if !dir.is_dir() {
            return Err(format!(
                "no model dir at {}; unset --real or set NEOCORTEX_MODEL_DIR",
                dir.display()
            ));
        }
        Arc::from(load(Some(&dir)).map_err(|e| e.to_string())?)
    } else {
        Arc::new(HashEmbedder)
    };
    let engine = match &db_path {
        Some(path) => {
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
            }
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            Engine::open(path, embedder).map_err(|e| e.to_string())?
        }
        None => Engine::open_in_memory(embedder).map_err(|e| e.to_string())?,
    }
    .with_clock(clock);
    println!(
        "world {}: {} memories, {} people",
        files[0],
        memories.len(),
        people.len()
    );

    let start = std::time::Instant::now();
    let inputs: Vec<RememberInput> = memories.into_iter().map(|m| m.input).collect();
    let total = inputs.len();
    for (i, result) in engine.remember_batch(inputs).into_iter().enumerate() {
        if let Err(e) = result {
            return Err(format!("memory {i}: {e}"));
        }
    }
    println!(
        "ingested {total} memories in {:.1}s",
        start.elapsed().as_secs_f64()
    );

    let true_by_norm: HashMap<String, String> = people
        .iter()
        .map(|p| (neocortex::text::norm_name(p), p.clone()))
        .collect();
    // entity id -> (name, type, aliases in true-name order)
    let mut entities: BTreeMap<i64, (String, String, Vec<String>)> = BTreeMap::new();
    {
        let mut st = engine
            .db
            .sql()
            .prepare("SELECT id,name,type FROM entities ORDER BY id")
            .unwrap();
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        });
        for row in rows.unwrap() {
            let (id, name, kind) = row.unwrap();
            entities.insert(id, (name, kind, Vec::new()));
        }
    }
    {
        let mut st = engine
            .db
            .sql()
            .prepare(
                "SELECT entity_id, alias_norm FROM entity_aliases ORDER BY entity_id, alias_norm",
            )
            .unwrap();
        let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)));
        for row in rows.unwrap() {
            let (id, alias) = row.unwrap();
            if let Some(name) = true_by_norm.get(&alias) {
                if let Some(entity) = entities.get_mut(&id) {
                    if !entity.2.contains(name) {
                        entity.2.push(name.clone());
                    }
                }
            }
        }
    }

    let mut false_merges = Vec::new();
    for (id, (name, kind, true_names)) in &entities {
        if true_names.len() >= 2 {
            false_merges.push((*id, name.clone(), kind.clone(), true_names.clone()));
        }
    }
    // A true person on two entities: the person's name must be one entity's
    // own name/alias on each of them.
    let mut by_person: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for (id, (_, _, true_names)) in &entities {
        for name in true_names {
            by_person.entry(name.as_str()).or_default().push(*id);
        }
    }
    let missed: Vec<(&str, Vec<i64>)> = by_person
        .iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .map(|(name, ids)| (*name, ids.clone()))
        .collect();

    println!(
        "entities: {} (true people: {})   false merges: {}   missed merges: {}",
        entities.len(),
        people.len(),
        false_merges.len(),
        missed.len()
    );
    for (id, name, kind, true_names) in &false_merges {
        println!(
            "  FALSE e{id} {name:?} [{kind}] aliases={:?} people={:?}",
            entities.get(id).map(|e| e.2.clone()).unwrap_or_default(),
            true_names
        );
        if let Some((mid, content, co)) = merge_write(&engine, *id, true_names) {
            let co: Vec<String> = co
                .iter()
                .map(|(ename, ekind)| format!("{ename} [{ekind}]"))
                .collect();
            println!("    caused at m{mid}: {content:?}");
            println!(
                "    co-mentioned in that write: {}",
                if co.is_empty() {
                    "-".into()
                } else {
                    co.join(", ")
                }
            );
        }
    }
    for (name, ids) in &missed {
        let where_: Vec<String> = ids
            .iter()
            .map(|id| {
                let (n, k, _) = &entities[id];
                format!("e{id} {n:?} [{k}]")
            })
            .collect();
        println!("  MISSED {name:?} split over {}", where_.join(" + "));
    }
    Ok(())
}

/// The write that first names both true people: memory id, content, and the
/// entities co-mentioned with it.
type MergeWrite = (i64, String, Vec<(String, String)>);

/// The first memory of `entity` that names all but the earliest true person,
/// with the entities co-mentioned in the same memory (the write's evidence).
fn merge_write(engine: &Engine, entity: i64, true_names: &[String]) -> Option<MergeWrite> {
    let mut st = engine
        .db
        .sql()
        .prepare(
            "SELECT m.id, m.content FROM memories m
             JOIN mentions me ON me.memory_id=m.id
             WHERE me.entity_id=?1 ORDER BY m.id",
        )
        .ok()?;
    let rows = st
        .query_map([entity], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .ok()?;
    for row in rows.flatten() {
        let (mid, content) = row;
        let named: Vec<&String> = true_names
            .iter()
            .filter(|name| content.contains(name.as_str()))
            .collect();
        if named.len() < 2 {
            continue;
        }
        let mut st = engine
            .db
            .sql()
            .prepare(
                "SELECT DISTINCT e.name, e.type FROM mentions me
                 JOIN entities e ON e.id=me.entity_id
                 WHERE me.memory_id=?1 AND me.entity_id<>?2 ORDER BY e.name",
            )
            .ok()?;
        let co: Vec<(String, String)> = st
            .query_map(rusqlite::params![mid, entity], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .ok()?
            .flatten()
            .collect();
        return Some((mid, content, co));
    }
    None
}
