//! `neocortex eval <set> --haystack <db>`: a hand-written set scored inside a
//! store of older memories. The source stays byte-identical, a missing or
//! locked source fails cleanly, and scoring without `--haystack` is unchanged.

mod common;

use common::TempDir;
use neocortex::{
    embed::HashEmbedder,
    engine::{Config, Engine, EntityIn, RelationIn, RememberInput},
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn clock_at(date: &str) -> neocortex::temporal::Clock {
    neocortex::temporal::Clock {
        now: neocortex::temporal::parse_instant(
            date,
            &neocortex::temporal::Clock {
                now: 0,
                tz_offset_secs: 0,
            },
        )
        .unwrap(),
        tz_offset_secs: 0,
    }
}

fn entity(name: &str, kind: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some(kind.into()),
        ..Default::default()
    }
}

fn relation(source: &str, predicate: &str, target: &str) -> RelationIn {
    RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        exclusive: None,
        ..Default::default()
    }
}

fn structured(
    content: &str,
    from: &str,
    entities: Vec<EntityIn>,
    relations: Vec<RelationIn>,
) -> RememberInput {
    RememberInput {
        content: content.into(),
        kind: Some("fact".into()),
        valid_from: Some(from.into()),
        entities,
        relations,
        ..Default::default()
    }
}

fn set_json() -> serde_json::Value {
    serde_json::json!({
        "now": "2026-10-21T15:00:00Z",
        "memories": [
            {
                "key": "s1",
                "content": "Mara Vale lives in Lisbon.",
                "kind": "fact",
                "valid_from": "2025-03-01",
                "entities": [
                    {"name": "Mara Vale", "type": "person"},
                    {"name": "Lisbon", "type": "place"}
                ],
                "relations": [
                    {"source": "Mara Vale", "predicate": "lives in", "target": "Lisbon", "exclusive": true}
                ]
            },
            {
                "key": "s2",
                "content": "Mara Vale works at Fjord Labs.",
                "kind": "fact",
                "valid_from": "2025-03-02",
                "entities": [
                    {"name": "Mara Vale", "type": "person"},
                    {"name": "Fjord Labs", "type": "org"}
                ],
                "relations": [
                    {"source": "Mara Vale", "predicate": "works at", "target": "Fjord Labs"}
                ]
            },
            {
                "key": "s3",
                "content": "Fjord Labs is based in Reykjavik.",
                "kind": "fact",
                "valid_from": "2025-03-03",
                "entities": [
                    {"name": "Fjord Labs", "type": "org"},
                    {"name": "Reykjavik", "type": "place"}
                ],
                "relations": [
                    {"source": "Fjord Labs", "predicate": "based in", "target": "Reykjavik"}
                ]
            },
            {
                "key": "s4",
                "content": "Jon Park lives in Oslo.",
                "kind": "fact",
                "valid_from": "2025-03-04",
                "entities": [
                    {"name": "Jon Park", "type": "person"},
                    {"name": "Oslo", "type": "place"}
                ],
                "relations": [
                    {"source": "Jon Park", "predicate": "lives in", "target": "Oslo", "exclusive": true}
                ]
            },
            {
                "key": "s5",
                "content": "Jon Park leads Jotun Project.",
                "kind": "fact",
                "valid_from": "2025-03-05",
                "entities": [
                    {"name": "Jon Park", "type": "person"},
                    {"name": "Jotun Project", "type": "org"}
                ],
                "relations": [
                    {"source": "Jon Park", "predicate": "leads", "target": "Jotun Project"}
                ]
            }
        ],
        "queries": [
            {
                "q": "Where does Mara Vale live?",
                "category": "current",
                "expect": ["s1"],
                "answers": ["Lisbon"]
            },
            {
                "q": "Where is Fjord Labs based?",
                "category": "current",
                "expect": ["s3"],
                "answers": ["Reykjavik"]
            },
            {
                "q": "Where does Jon Park live?",
                "category": "current",
                "expect": ["s4"],
                "answers": ["Oslo"]
            },
            {
                "q": "What does Jon Park lead?",
                "category": "current",
                "expect": ["s5"],
                "answers": ["Jotun Project"]
            },
            {
                "q": "Where does Mara Vale work?",
                "category": "current",
                "expect": ["s2"],
                "answers": ["Fjord Labs"]
            }
        ]
    })
}

fn write_set(dir: &Path) -> PathBuf {
    let path = dir.join("set.json");
    std::fs::write(&path, serde_json::to_string(&set_json()).unwrap()).unwrap();
    path
}

fn build_haystack(dir: &Path) -> PathBuf {
    let path = dir.join("haystack.db");
    {
        let engine = Engine::open(&path, Arc::new(HashEmbedder))
            .unwrap()
            .with_clock(clock_at("2025-06-01T00:00:00Z"));
        let distractors: Vec<(&str, &str, &str, &str)> = vec![
            ("Henry Cole", "Brno", "2024-02-01", "Alpha Group"),
            ("Iris Duarte", "Denver", "2024-03-01", "Beta Unit"),
            ("Ken Asano", "Perth", "2024-04-01", "Gamma Team"),
            ("Lena Fischer", "Quito", "2024-05-01", "Delta Lab"),
            ("Omar Haddad", "Osaka", "2024-06-01", "Epsilon Co"),
            ("Priya Nair", "Turin", "2024-07-01", "Zeta Hub"),
            ("Quinn Adler", "Wroclaw", "2024-08-01", "Eta Works"),
            ("Ruth Varga", "Krakow", "2024-09-01", "Theta Inc"),
            ("Sam Okafor", "Lagos", "2024-10-01", "Iota Studio"),
            ("Tara Iqbal", "Hobart", "2024-11-01", "Kappa Group"),
            ("Umar Farouk", "Catania", "2024-12-01", "Lambda Unit"),
            ("Vera Lindt", "Belfast", "2025-01-01", "Mu Team"),
        ];
        for (person, city, from, org) in distractors {
            let content = format!("{person} lives in {city}.");
            let mem = structured(
                &content,
                from,
                vec![
                    entity(person, "person"),
                    entity(city, "place"),
                    entity(org, "org"),
                ],
                vec![
                    relation(person, "lives in", city),
                    relation(person, "works at", org),
                ],
            );
            engine.remember(mem).unwrap();
        }
    }
    path
}

fn opts() -> neocortex::eval::EvalOptions {
    neocortex::eval::EvalOptions {
        limit_queries: None,
        save_db: None,
        load_db: None,
        haystack: None,
    }
}

#[test]
fn haystack_answers_still_found() {
    let dir = TempDir::new("haystack-found");
    let haystack = build_haystack(&dir);
    let set = write_set(&dir);
    let mut o = opts();
    o.haystack = Some(haystack);
    let report =
        neocortex::eval::run_eval(&set, Arc::new(HashEmbedder), Config::default(), &o).unwrap();
    assert_eq!(report.queries, 5);
    assert!(
        report.recall >= 0.8,
        "recall {} (failures: {:?})",
        report.recall,
        report.failures
    );
    let acc = report.acc.unwrap_or(0.0);
    assert!(acc >= 0.8, "acc {acc}");
    assert!(report.ingested >= 5, "ingested {}", report.ingested);
}

#[test]
fn haystack_source_byte_identical_after_run() {
    let dir = TempDir::new("haystack-identical");
    let haystack = build_haystack(&dir);
    let before = std::fs::read(&haystack).unwrap();
    let set = write_set(&dir);
    let copy = dir.join("copy.db");
    let mut o = opts();
    o.haystack = Some(haystack.clone());
    o.save_db = Some(copy.clone());
    let report =
        neocortex::eval::run_eval(&set, Arc::new(HashEmbedder), Config::default(), &o).unwrap();
    let after = std::fs::read(&haystack).unwrap();
    assert_eq!(before, after, "source database changed");
    assert!(copy.exists(), "the --save-db copy is kept");
    assert!(
        report.ingested > 5,
        "copy holds haystack + set: {}",
        report.ingested
    );
    // The kept copy must have the key map the set needs.
    let keys = std::fs::read_to_string(format!("{}.keys.json", copy.display())).unwrap();
    assert!(keys.contains("\"s1\""), "key map {keys}");
    // The copy itself answers a query for the set.
    let copy_engine = Engine::open(&copy, Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(clock_at("2026-10-21T15:00:00Z"));
    let out = copy_engine
        .recall(&neocortex::engine::RecallInput {
            query: "Where does Mara Vale live?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(!out.memories.is_empty());
}

#[test]
fn haystack_missing_fails_cleanly() {
    let dir = TempDir::new("haystack-missing");
    let set = write_set(&dir);
    let mut o = opts();
    o.haystack = Some(dir.join("does-not-exist.db"));
    let err =
        neocortex::eval::run_eval(&set, Arc::new(HashEmbedder), Config::default(), &o).unwrap_err();
    assert!(err.to_string().contains("does not exist"), "{err}");
}

#[test]
fn haystack_locked_fails_cleanly() {
    let dir = TempDir::new("haystack-locked");
    let haystack = build_haystack(&dir);
    let set = write_set(&dir);
    let holder = rusqlite::Connection::open(&haystack).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut o = opts();
    o.haystack = Some(haystack);
    let err =
        neocortex::eval::run_eval(&set, Arc::new(HashEmbedder), Config::default(), &o).unwrap_err();
    holder.execute_batch("ROLLBACK").unwrap();
    assert!(err.to_string().to_lowercase().contains("locked"), "{err}");
}

#[test]
fn haystack_copy_target_exists_is_refused() {
    let dir = TempDir::new("haystack-target-exists");
    let haystack = build_haystack(&dir);
    let set = write_set(&dir);
    let copy = dir.join("copy.db");
    std::fs::write(&copy, b"do-not-touch").unwrap();
    let before = std::fs::read(&copy).unwrap();
    let mut o = opts();
    o.haystack = Some(haystack);
    o.save_db = Some(copy.clone());
    let err =
        neocortex::eval::run_eval(&set, Arc::new(HashEmbedder), Config::default(), &o).unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
    assert_eq!(std::fs::read(&copy).unwrap(), before);
}

#[test]
fn without_haystack_unchanged() {
    let dir = TempDir::new("haystack-plain");
    let set = write_set(&dir);
    let report =
        neocortex::eval::run_eval(&set, Arc::new(HashEmbedder), Config::default(), &opts())
            .unwrap();
    assert_eq!(report.queries, 5);
    assert!(report.recall >= 0.8, "{}", report.recall);
    assert_eq!(report.ingested, 5);
}

#[test]
fn cli_haystack_missing_is_an_error() {
    let dir = TempDir::new("haystack-cli");
    let set = write_set(&dir);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_neocortex"))
        .env("NEOCORTEX_EMBEDDER", "hash")
        .env("NEOCORTEX_DB", dir.join("never.db"))
        .env("TMPDIR", dir.as_ref())
        .args([
            "eval",
            set.to_str().unwrap(),
            "--haystack",
            dir.join("missing.db").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success(), "status {:?}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not exist"), "{stderr}");
}
