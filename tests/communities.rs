//! Community detection and navigation summaries.

use neocortex::{
    embed::{Embedder, HashEmbedder},
    engine::{Engine, EntityIn, RelationIn, RememberInput},
    temporal::Clock,
};
use std::collections::HashMap;
use std::sync::Arc;

const NOW: i64 = 1_700_000_000;
const EARLIER: i64 = NOW - 86_400;

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder) as Arc<dyn Embedder>)
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn named(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        ..Default::default()
    }
}

fn rel(source: &str, predicate: &str, target: &str) -> RelationIn {
    RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        ..Default::default()
    }
}

fn remember(engine: &Engine, content: &str, valid_from: i64) {
    // The same two clusters, with no memory naming entities from both:
    // {Alice, Bob, Pune} and {Dave, Erin, Acme}.
    let (entities, relations) = match content {
        "Alice lives in Pune" => (
            vec![named("Alice"), named("Pune")],
            vec![rel("Alice", "lives in", "Pune")],
        ),
        "Bob lives in Pune" => (
            vec![named("Bob"), named("Pune")],
            vec![rel("Bob", "lives in", "Pune")],
        ),
        "Alice knows Bob" => (
            vec![named("Alice"), named("Bob")],
            vec![rel("Alice", "knows", "Bob")],
        ),
        "Carol lives in Pune" => (
            vec![named("Carol"), named("Pune")],
            vec![rel("Carol", "lives in", "Pune")],
        ),
        "Dave works at Acme" => (
            vec![named("Dave"), named("Acme")],
            vec![rel("Dave", "works at", "Acme")],
        ),
        "Erin works at Acme" => (
            vec![named("Erin"), named("Acme")],
            vec![rel("Erin", "works at", "Acme")],
        ),
        "Dave knows Erin" => (
            vec![named("Dave"), named("Erin")],
            vec![rel("Dave", "knows", "Erin")],
        ),
        other => panic!("unknown memory {other}"),
    };
    engine
        .remember(RememberInput {
            content: content.into(),
            entities,
            relations,
            valid_from: Some(valid_from.to_string()),
            ..Default::default()
        })
        .unwrap();
}

fn seed(engine: &Engine) {
    for content in [
        "Alice lives in Pune",
        "Bob lives in Pune",
        "Alice knows Bob",
    ] {
        remember(engine, content, EARLIER);
    }
    for content in [
        "Dave works at Acme",
        "Erin works at Acme",
        "Dave knows Erin",
    ] {
        remember(engine, content, NOW);
    }
}

fn mapping(engine: &Engine) -> HashMap<String, i64> {
    let mut st = engine
        .db
        .sql()
        .prepare("SELECT e.name,c.community FROM communities c JOIN entities e ON e.id=c.entity_id")
        .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn invalidate(engine: &Engine) {
    engine
        .db
        .sql()
        .execute("DELETE FROM meta WHERE key='communities_gen'", [])
        .unwrap();
}

#[test]
fn leiden_communities_are_deterministic_and_stable() {
    let engine = engine();
    seed(&engine);
    let first = engine.overview(None, 10).unwrap();
    assert!(first.communities.len() >= 2, "{first:?}");
    let before = mapping(&engine);

    assert_eq!(before["Alice"], before["Bob"], "cluster A: {before:?}");
    assert_eq!(before["Bob"], before["Pune"], "cluster A: {before:?}");
    assert_eq!(before["Dave"], before["Erin"], "cluster B: {before:?}");
    assert_eq!(before["Erin"], before["Acme"], "cluster B: {before:?}");
    assert_ne!(
        before["Alice"], before["Dave"],
        "clusters merged: {before:?}"
    );

    // A recomputation must reproduce the same partition and ids.
    invalidate(&engine);
    let second = engine.overview(None, 10).unwrap();
    assert_eq!(before, mapping(&engine));
    assert_eq!(first.communities.len(), second.communities.len());
}

#[test]
fn community_ids_survive_a_new_member() {
    let engine = engine();
    seed(&engine);
    engine.overview(None, 10).unwrap();
    let before = mapping(&engine);

    remember(&engine, "Carol lives in Pune", NOW);
    invalidate(&engine);
    engine.overview(None, 10).unwrap();
    let after = mapping(&engine);

    assert_eq!(
        after["Alice"], before["Alice"],
        "cluster A id moved: {after:?}"
    );
    assert_eq!(after["Bob"], before["Bob"], "cluster A id moved: {after:?}");
    assert_eq!(
        after["Dave"], before["Dave"],
        "cluster B id moved: {after:?}"
    );
    assert!(after.contains_key("Carol"), "new entity missing: {after:?}");
}

#[test]
fn overview_summarises_communities() {
    let engine = engine();
    seed(&engine);
    let out = engine.overview(None, 10).unwrap();
    let a = out
        .communities
        .iter()
        .find(|c| c.label.contains("Alice"))
        .expect("cluster A");
    assert!(a.label.contains("Pune"), "label = {}", a.label);
    assert!(a.size >= 3, "size = {}", a.size);
    assert!(
        a.predicates.iter().any(|p| p == "lives in"),
        "predicates = {:?}",
        a.predicates
    );
    assert_eq!(a.span, Some((EARLIER, EARLIER)));
    assert!(!a.facts.is_empty() && a.facts.len() <= 3);
    assert!(a.facts.iter().any(|f| f.fact.contains("Pune")));
}

#[test]
fn entity_card_carries_its_community() {
    let engine = engine();
    seed(&engine);
    let out = engine.overview(None, 10).unwrap();
    let profile = engine.entity("Alice", 20).unwrap();
    let community = profile.card.community.expect("community on the card");
    let shown = out
        .communities
        .iter()
        .find(|c| c.id == community.id)
        .expect("overview shows the card's community");
    assert_eq!(community.size, shown.size);
    assert_eq!(community.label, shown.label);

    // Recall cards stay lightweight: no community field there.
    let recalled = engine.recall(&neocortex::engine::RecallInput {
        query: "Alice lives in Pune".into(),
        ..Default::default()
    });
    assert!(recalled
        .unwrap()
        .entities
        .iter()
        .all(|e| e.community.is_none()));
}
