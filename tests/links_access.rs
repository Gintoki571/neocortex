//! E4a/E4b/E5 experiments: temporal links, causal links, access reinforcement.
//!
//! The tests use the hash embedder: links and counters are write-path/ranking
//! behaviour and must not depend on the real model.

use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput};
use neocortex::temporal::Clock;
use std::sync::Arc;

mod common;
use common::TempDir;

const NOW: i64 = 1_700_000_000;
const DAY: i64 = 86_400;

fn clock() -> Clock {
    Clock {
        now: NOW,
        tz_offset_secs: 0,
    }
}

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(clock())
}

fn person(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
        ..EntityIn::default()
    }
}

fn cause(source: &str, target: &str) -> RelationIn {
    RelationIn {
        source: source.into(),
        predicate: "causes".into(),
        target: target.into(),
        ..RelationIn::default()
    }
}

fn link_count(engine: &Engine, id: i64) -> i64 {
    engine
        .db
        .sql()
        .query_row(
            "SELECT count(*) FROM memory_links WHERE a=?1 OR b=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

// ------------------------------------------------------------------- E4a

#[test]
fn temporal_links_are_capped_and_weighted_by_gap() {
    let mut e = engine();
    e.cfg.tlink_cap = 5;
    e.cfg.tlink_window_hours = 24.0;
    let mut ids = Vec::new();
    // One person, thirty events ten minutes apart. Every event links only to
    // the closest events, so no memory may end up over the fan-out cap.
    for i in 0..30 {
        let at = NOW + i * 600;
        let out = e
            .remember(RememberInput {
                content: format!("Ada attended planning meeting number {i}."),
                kind: Some("event".into()),
                valid_from: Some(at.to_string()),
                entities: vec![person("Ada")],
                ..RememberInput::default()
            })
            .unwrap();
        ids.push(out.id);
    }
    for id in &ids {
        let n = link_count(&e, *id);
        assert!(n <= 5, "m{id} has {n} temporal links, cap is 5");
    }
    let last = *ids.last().unwrap();
    let previous = ids[ids.len() - 2];
    let weight: f64 =
        e.db.sql()
            .query_row(
                "SELECT weight FROM memory_links WHERE (a=?1 AND b=?2) OR (a=?2 AND b=?1)",
                [last, previous],
                |r| r.get(0),
            )
            .unwrap();
    let expected = 1.0 - 600.0 / DAY as f64;
    assert!(
        (weight - expected).abs() < 1e-9,
        "weight {weight} != {expected}"
    );
}

#[test]
fn temporal_links_do_not_cross_entities_or_the_window() {
    let mut e = engine();
    e.cfg.tlink_cap = 20;
    e.cfg.tlink_window_hours = 24.0;
    let a = e
        .remember(RememberInput {
            content: "Ada joined the platform team.".into(),
            kind: Some("event".into()),
            valid_from: Some(NOW.to_string()),
            entities: vec![person("Ada")],
            ..RememberInput::default()
        })
        .unwrap();
    let b = e
        .remember(RememberInput {
            content: "Grace joined the platform team.".into(),
            kind: Some("event".into()),
            valid_from: Some((NOW + 60).to_string()),
            entities: vec![person("Grace")],
            ..RememberInput::default()
        })
        .unwrap();
    let c = e
        .remember(RememberInput {
            content: "Ada moved to the research team.".into(),
            kind: Some("event".into()),
            valid_from: Some((NOW + 2 * DAY).to_string()),
            entities: vec![person("Ada")],
            ..RememberInput::default()
        })
        .unwrap();
    assert_eq!(link_count(&e, a.id), 0, "different entities must not link");
    assert_eq!(link_count(&e, b.id), 0, "different entities must not link");
    assert_eq!(
        link_count(&e, c.id),
        0,
        "the same entity later than the window must not link"
    );
}

#[test]
fn a_v4_database_gains_the_additive_tables_on_open() {
    let dir = TempDir::new("links-migrate");
    let path = dir.join("links.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        e.db.sql()
            .execute_batch("DROP TABLE memory_links; DROP TABLE memory_access;")
            .unwrap();
        let version: i64 =
            e.db.sql()
                .query_row("SELECT version FROM schema_meta", [], |r| r.get(0))
                .unwrap();
        assert_eq!(version, 4, "the additive migration must not renumber v4");
    }
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    for table in ["memory_links", "memory_access"] {
        let exists: i64 =
            e.db.sql()
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
        assert_eq!(exists, 1, "{table} missing after reopening a v4 database");
    }
    drop(e);
}

#[test]
fn a_v4_database_gains_the_subject_index_and_the_planner_uses_it() {
    let dir = TempDir::new("subj-index");
    let path = dir.join("subj.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        e.db.sql()
            .execute_batch("DROP INDEX idx_rel_subj;")
            .unwrap();
    }
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    // The facts_for_view count and the write-path exclusivity lookup.
    for sql in [
        "SELECT count(*) FROM relations WHERE subj=1 AND predicate_id IN
           (SELECT id FROM predicates WHERE group_id=2) AND polarity=1
           AND expired_at IS NULL AND valid_from<=3 AND (valid_to IS NULL OR valid_to>3)",
        "SELECT r.id FROM relations r JOIN predicates p ON p.id=r.predicate_id
           WHERE r.subj=1 AND COALESCE(NULLIF(p.group_id,0),p.id)=2
           AND r.polarity=1 AND r.expired_at IS NULL",
    ] {
        let plan: String =
            e.db.sql()
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map([], |r| r.get::<_, String>(3))
                .unwrap()
                .map(|d| d.unwrap())
                .collect::<Vec<_>>()
                .join("; ");
        assert!(plan.contains("idx_rel_subj"), "no subj index in: {plan}");
    }
    drop(e);
}

// ------------------------------------------------------------------- E4b

#[test]
fn because_of_is_stored_in_the_canonical_cause_direction() {
    let e = engine();
    let out = e
        .remember(RememberInput {
            content: "The deploy failed because of the config change.".into(),
            kind: Some("event".into()),
            entities: vec![person("Deploy"), person("Config change")],
            relations: vec![RelationIn {
                source: "Deploy".into(),
                predicate: "because of".into(),
                target: "Config change".into(),
                ..RelationIn::default()
            }],
            ..RememberInput::default()
        })
        .unwrap();
    let (canonical, src, dst): (String, String, String) =
        e.db.sql()
            .query_row(
                "SELECT p.canonical,s.name,d.name FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             JOIN entities s ON s.id=r.src JOIN entities d ON d.id=r.dst
             WHERE r.id=?1",
                [out.relations[0].id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(canonical, "cause");
    assert_eq!(src, "Config change", "the cause must be the stored source");
    assert_eq!(dst, "Deploy", "the effect must be the stored target");
}

#[test]
fn causal_intent_recognises_why_questions() {
    assert!(neocortex::paths::causal_intent("why did the deploy fail"));
    assert!(neocortex::paths::causal_intent("what caused the outage"));
    assert!(neocortex::paths::causal_intent("reason for the delay"));
    assert!(!neocortex::paths::causal_intent("when did the deploy fail"));
}

#[test]
fn causal_queries_follow_causal_links_first() {
    let mut e = engine();
    e.cfg.w_causal = 4.0;
    let cause_memory = e
        .remember(RememberInput {
            content: "The deploy failed because the config changed.".into(),
            kind: Some("event".into()),
            valid_from: Some((NOW - DAY).to_string()),
            entities: vec![
                EntityIn {
                    name: "Deploy failure".into(),
                    aliases: vec!["deploy".into()],
                    ..EntityIn::default()
                },
                person("Config change"),
            ],
            relations: vec![cause("Config change", "Deploy failure")],
            ..RememberInput::default()
        })
        .unwrap();
    let distractor = e
        .remember(RememberInput {
            content: "The deploy was scheduled for Friday.".into(),
            kind: Some("event".into()),
            entities: vec![EntityIn {
                name: "Deploy failure".into(),
                aliases: vec!["deploy".into()],
                ..EntityIn::default()
            }],
            ..RememberInput::default()
        })
        .unwrap();
    assert_ne!(cause_memory.id, distractor.id);
    let out = e
        .recall(&RecallInput {
            query: "why did the deploy fail".into(),
            k: Some(5),
            ..RecallInput::default()
        })
        .unwrap();
    let top = out.memories.first().expect("causal answer");
    assert_eq!(top.id, cause_memory.id);
    assert!(
        top.reasons.iter().any(|r| r.starts_with("causal#1")),
        "reasons {:?}",
        top.reasons
    );
}

#[test]
fn text_cues_create_conservative_causal_relations() {
    let mut e = engine();
    e.cfg.causal_extract = true;
    let out = e
        .remember(RememberInput {
            content: "The rollout slipped because the cache migration failed.".into(),
            kind: Some("event".into()),
            entities: vec![person("Rollout"), person("Cache migration")],
            ..RememberInput::default()
        })
        .unwrap();
    assert_eq!(out.relations.len(), 1, "{:?}", out.relations);
    let (canonical, src, dst): (String, String, String) =
        e.db.sql()
            .query_row(
                "SELECT p.canonical,s.name,d.name FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             JOIN entities s ON s.id=r.src JOIN entities d ON d.id=r.dst
             WHERE r.id=?1",
                [out.relations[0].id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(canonical, "cause");
    assert_eq!((src.as_str(), dst.as_str()), ("Cache migration", "Rollout"));
}

// -------------------------------------------------------------------- E5

#[test]
fn recall_records_returned_memories_and_caps_recent_accesses() {
    let mut e = engine();
    e.cfg.access_keep = 3;
    let out = e
        .remember(RememberInput {
            content: "The backup runs nightly at three.".into(),
            kind: Some("fact".into()),
            ..RememberInput::default()
        })
        .unwrap();
    for step in 0..5 {
        e.fixed_clock = Some(Clock {
            now: NOW + step * 60,
            tz_offset_secs: 0,
        });
        let recalled = e
            .recall(&RecallInput {
                query: "backup nightly".into(),
                k: Some(1),
                ..RecallInput::default()
            })
            .unwrap();
        assert_eq!(recalled.memories[0].id, out.id);
    }
    let (count, last): (i64, Option<i64>) =
        e.db.sql()
            .query_row(
                "SELECT access_count,last_access FROM memories WHERE id=?1",
                [out.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(count, 5);
    assert_eq!(last, Some(NOW + 4 * 60));
    let kept: i64 =
        e.db.sql()
            .query_row(
                "SELECT count(*) FROM memory_access WHERE memory_id=?1",
                [out.id],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(kept, 3, "recent access times are capped by access_keep");
}

#[test]
fn retention_prefers_important_recent_and_accessed_memories() {
    let now = NOW;
    let old = now - 300 * DAY;
    assert!(neocortex::rank::retention(0.1, old, &[], now) < 0.05);
    assert!(neocortex::rank::retention(0.9, now, &[], now) > 0.8);
    let cold = neocortex::rank::retention(0.5, old, &[], now);
    let warm = neocortex::rank::retention(0.5, old, &[now - DAY], now);
    assert!(warm > cold, "{warm} must beat {cold}");
}

#[test]
fn concurrent_recalls_keep_access_counters_consistent() {
    let dir = TempDir::new("links-concurrent");
    let path = dir.join("links.db");
    let seeded = {
        let e = Engine::open(&path, Arc::new(HashEmbedder))
            .unwrap()
            .with_clock(clock());
        e.remember(RememberInput {
            content: "The release train leaves on Friday.".into(),
            kind: Some("fact".into()),
            ..RememberInput::default()
        })
        .unwrap()
        .id
    };
    let mut handles = Vec::new();
    for _ in 0..4 {
        let path = path.clone();
        handles.push(std::thread::spawn(move || {
            let e = Engine::open(&path, Arc::new(HashEmbedder))
                .unwrap()
                .with_clock(clock());
            for _ in 0..10 {
                let out = e
                    .recall(&RecallInput {
                        query: "release train Friday".into(),
                        k: Some(1),
                        ..RecallInput::default()
                    })
                    .unwrap();
                assert_eq!(out.memories[0].id, seeded);
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    let count: i64 =
        e.db.sql()
            .query_row(
                "SELECT access_count FROM memories WHERE id=?1",
                [seeded],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(count, 40);
    let kept: i64 =
        e.db.sql()
            .query_row(
                "SELECT count(*) FROM memory_access WHERE memory_id=?1",
                [seeded],
                |r| r.get(0),
            )
            .unwrap();
    assert!(kept <= e.cfg.access_keep as i64, "{kept}");
    drop(e);
}
