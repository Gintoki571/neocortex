//! Lane nc-hygiene: guard tests for NEGQ-FOLLOW, KG-GHOST, VALIDATE-NAMEPART.
//!
//! Legacy/corrupt states are set up with direct SQL because the normal write
//! path prevents them. That is the point: the graph/validation tools must stay
//! clean even when old data contains these shapes.

use neocortex::db::Db;
use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, EntityIn, RelationIn, RememberInput};
use neocortex::paths;
use neocortex::paths::FollowInput;
use std::sync::Arc;

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap()
}

fn remember_two(engine: &Engine) -> (i64, i64) {
    let out = engine
        .remember(RememberInput {
            content: "Alice manages Bob.".into(),
            entities: vec![
                EntityIn {
                    name: "Alice".into(),
                    kind: Some("person".into()),
                    ..Default::default()
                },
                EntityIn {
                    name: "Bob".into(),
                    kind: Some("person".into()),
                    ..Default::default()
                },
            ],
            relations: vec![RelationIn {
                source: "Alice".into(),
                predicate: "manages".into(),
                target: "Bob".into(),
                fact: Some("Alice manages Bob".into()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    (out.entities[0].id, out.entities[1].id)
}

/// Insert a legacy live self-loop relation (src == dst) directly. The write path
/// never stores these, so this simulates old/corrupt data.
fn insert_legacy_self_loop(db: &Db, entity_id: i64) {
    let pid: i64 = db
        .sql()
        .query_row(
            "SELECT id FROM predicates WHERE canonical='manage' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let now = 1_700_000_000_i64;
    db.sql()
        .execute(
            "INSERT INTO relations(src,dst,subj,predicate_id,fact,created_at,valid_from,expired_at,polarity,evidence_count)
             VALUES (?1,?1,?1,?2,'Alice manages Alice',?3,0,NULL,1,1)",
            rusqlite::params![entity_id, pid, now],
        )
        .unwrap();
}

// ---- NEGQ-FOLLOW ---------------------------------------------------------

#[test]
fn negq_follow_follow_never_shows_legacy_self_loop() {
    let engine = engine();
    let (alice, _bob) = remember_two(&engine);
    insert_legacy_self_loop(&engine.db, alice);

    let out = paths::follow(
        &engine,
        &FollowInput {
            from: "Alice".into(),
            depth: Some(1),
            k: Some(50),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        out.facts.iter().all(|f| f.source != f.target),
        "follow showed a self-loop: {:?}",
        out.facts
    );
    // The real relation (Alice -> Bob) must still be present.
    assert!(
        out.facts
            .iter()
            .any(|f| f.source == "Alice" && f.target == "Bob"),
        "follow lost the real relation: {:?}",
        out.facts
    );
}

#[test]
fn negq_follow_aggregate_never_shows_legacy_self_loop() {
    let engine = engine();
    let (alice, _bob) = remember_two(&engine);
    insert_legacy_self_loop(&engine.db, alice);

    let out = paths::aggregate(
        &engine,
        &paths::AggregateInput {
            source: Some("Alice".into()),
            predicate: Some("manages".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        out.items.iter().all(|i| i.name != "Alice"),
        "aggregate counted the self-loop as a target: {:?}",
        out.items
    );
    assert!(
        out.items.iter().any(|i| i.name == "Bob"),
        "aggregate lost the real target: {:?}",
        out.items
    );
}

#[test]
fn negq_follow_dropped_self_relation_hint_names_the_fact() {
    let engine = engine();
    let out = engine
        .remember(RememberInput {
            content: "Alice manages Alice.".into(),
            entities: vec![EntityIn {
                name: "Alice".into(),
                kind: Some("person".into()),
                ..Default::default()
            }],
            relations: vec![RelationIn {
                source: "Alice".into(),
                predicate: "manages".into(),
                target: "Alice".into(),
                fact: Some("Alice manages Alice".into()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let hint = out
        .hints
        .iter()
        .find(|h| h.contains("self-relation"))
        .unwrap_or_else(|| panic!("no self-relation hint: {:?}", out.hints));
    assert!(
        hint.contains("Alice manages Alice"),
        "self-relation hint did not name the fact: {hint:?}"
    );
    assert!(
        !hint.trim_end().ends_with("self-relation") && !hint.contains("1 self"),
        "self-relation hint looks like a bare count, not the fact: {hint:?}"
    );
}

#[test]
fn negq_follow_legacy_self_loop_not_projected_into_ppr_snapshot() {
    let engine = engine();
    let (alice, _bob) = remember_two(&engine);
    insert_legacy_self_loop(&engine.db, alice);
    // Project the self-loop edge the way rebuild_all would, simulating a legacy
    // projection that included it, then verify the snapshot builder does not
    // carry a self-edge for it (and the live DB never should produce one).
    neocortex::kg::rebuild_all(&engine.db).unwrap();
    let snap = neocortex::kg::snapshot(&engine.db, None, false).unwrap();
    let node = *snap.ent_node.get(&alice).expect("alice node");
    let start = snap.csr.offsets[node as usize];
    let end = snap.csr.offsets[node as usize + 1];
    let self_edges = (start..end)
        .filter(|&k| snap.csr.targets[k] == node)
        .count();
    assert_eq!(
        self_edges,
        0,
        "PPR snapshot carried a self-edge for alice (targets {:?})",
        &snap.csr.targets[start..end]
    );
}

// ---- KG-GHOST ------------------------------------------------------------

#[test]
fn kg_ghost_projection_creates_no_nameless_node_when_endpoint_unprojected() {
    let engine = engine();
    let (alice, bob) = remember_two(&engine);
    // Simulate an endpoint that lost its projection (node deleted, relational row remains).
    // DETACH DELETE also removes its mention/relation edges from the graph.
    neocortex::kg::remove_entity(&engine.db, bob).unwrap();
    // Projecting a relation that touches the missing endpoint must not leave a
    // bare-id node; the endpoint is re-projected from its relational row.
    let rid: i64 = engine
        .db
        .sql()
        .query_row("SELECT id FROM relations LIMIT 1", [], |r| r.get(0))
        .unwrap();
    neocortex::kg::project_relation(&engine.db, rid, alice, bob, "manages").unwrap();
    // Restore the mention edge DETACH DELETE removed, so validate stays clean.
    let mid: i64 = engine
        .db
        .sql()
        .query_row(
            "SELECT memory_id FROM mentions WHERE entity_id=?1 LIMIT 1",
            [bob],
            |r| r.get(0),
        )
        .unwrap();
    neocortex::kg::project_mention(&engine.db, mid, bob).unwrap();

    let unnamed: i64 = engine
        .db
        .graph()
        .cypher("MATCH (n) WHERE n.name IS NULL OR n.name = '' RETURN count(n) AS c")
        .unwrap()
        .iter()
        .next()
        .and_then(|r| r.get::<i64>("c").ok())
        .unwrap_or(0);
    assert_eq!(unnamed, 0, "projection left a nameless node");
    let problems = engine.validate().unwrap();
    assert!(
        problems.is_empty(),
        "validate not clean after re-projection: {problems:?}"
    );
}

// ---- VALIDATE-NAMEPART ---------------------------------------------------

#[test]
fn validate_namepart_counts_containment_edges_and_stays_clean() {
    let engine = engine();
    // Build a DB that has a B5 name_part CONTAINS edge.
    engine
        .remember(RememberInput {
            content: "The Cascais marina is near Cascais.".into(),
            entities: vec![
                EntityIn {
                    name: "Cascais marina".into(),
                    kind: Some("place".into()),
                    ..Default::default()
                },
                EntityIn {
                    name: "Cascais".into(),
                    kind: Some("place".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();
    let name_parts: i64 = engine
        .db
        .sql()
        .query_row("SELECT count(*) FROM name_part", [], |r| r.get(0))
        .unwrap();
    assert!(
        name_parts > 0,
        "test setup produced no name_part edges (B5 containment not triggered)"
    );
    let problems = engine.validate().unwrap();
    assert!(
        problems.is_empty(),
        "validate reported a false mismatch with name_part edges present: {problems:?}"
    );
}
