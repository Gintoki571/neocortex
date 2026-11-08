//! H4-01: a typo-sized near-name of a PERSON never merges without ER-08
//! corroboration (translit-key equality or a shared person/org named in the
//! same write); otherwise a new entity reported "ambiguous" with the candidate.
//! DB/DBs (EX-06b caps plural) still folds. A true same-person respelling with
//! a shared org in the same write still merges.

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RelationIn, RememberInput, RememberOutput},
    temporal::Clock,
};
use std::sync::Arc;

const NOW: i64 = 1_792_756_800;

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn person(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
        ..Default::default()
    }
}

fn org(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("org".into()),
        ..Default::default()
    }
}

fn tool(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("tool".into()),
        ..Default::default()
    }
}

fn rel(src: &str, predicate: &str, dst: &str) -> RelationIn {
    RelationIn {
        source: src.into(),
        predicate: predicate.into(),
        target: dst.into(),
        ..Default::default()
    }
}

fn resolution<'a>(out: &'a RememberOutput, name: &str) -> &'a neocortex::engine::EntityResolution {
    out.entities
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no resolution for {name:?} in {:#?}", out.entities))
}

fn remember(
    e: &Engine,
    content: &str,
    entities: Vec<EntityIn>,
    relations: Vec<RelationIn>,
) -> RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        entities,
        relations,
        ..Default::default()
    })
    .unwrap()
}

fn separate_persons(e: &Engine, a: &str, b: &str, a_owns: &str, b_owns: &str) {
    let out1 = remember(
        e,
        &format!("{a} owns the {a_owns}."),
        vec![person(a)],
        vec![rel(a, "owns", a_owns)],
    );
    let r1 = resolution(&out1, a);
    let id1 = r1.id;
    assert!(id1 > 0, "{a}: {r1:#?}");

    let out2 = remember(
        e,
        &format!("{b} owns the {b_owns}."),
        vec![person(b)],
        vec![rel(b, "owns", b_owns)],
    );
    let r2 = resolution(&out2, b);
    assert_ne!(
        r2.id, id1,
        "{b} must not merge into {a} (got {r2:#?}, {a} was {r1:#?})"
    );
    assert_eq!(
        r2.status, "ambiguous",
        "{b} must be reported ambiguous with the near-name candidate, got {r2:#?}"
    );
    assert!(
        r2.candidates.iter().any(|c| c.id == id1),
        "{b} must name {a} (e{id1}) as a candidate: {r2:#?}"
    );

    let ea = e.entity(&format!("e{id1}"), 50).unwrap();
    let facts: String = ea
        .current_facts
        .iter()
        .map(|f| f.fact.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(
        !facts.to_lowercase().contains(&b_owns.to_lowercase()),
        "{a}'s card must not hold {b}'s fact: {facts}"
    );
}

#[test]
fn sam_lee_and_sam_leeds_stay_separate() {
    separate_persons(
        &engine(),
        "Sam Lee",
        "Sam Leeds",
        "mobile app",
        "billing dashboard",
    );
}

#[test]
fn alex_chen_and_alex_cheng_stay_separate() {
    separate_persons(
        &engine(),
        "Alex Chen",
        "Alex Cheng",
        "search service",
        "data group",
    );
}

#[test]
fn jon_park_and_jon_parks_stay_separate() {
    separate_persons(
        &engine(),
        "Jon Park",
        "Jon Parks",
        "mobile app",
        "billing dashboard",
    );
}

#[test]
fn dan_ortiz_and_daniel_ortiz_stay_separate() {
    separate_persons(&engine(), "Dan Ortiz", "Daniel Ortiz", "api", "web app");
}

#[test]
fn tom_reed_and_tom_reid_stay_separate() {
    separate_persons(&engine(), "Tom Reed", "Tom Reid", "api", "web app");
}

#[test]
fn dbs_caps_plural_still_folds() {
    let e = engine();
    // EX-06b: "DBs" and "DB" share one entity (caps-abbreviation plural).
    let t1 = remember(&e, "Atlas uses DBs.", vec![tool("DBs")], vec![]);
    let id1 = resolution(&t1, "DBs").id;
    let t2 = remember(&e, "Atlas uses DB.", vec![tool("DB")], vec![]);
    let r2 = resolution(&t2, "DB");
    assert_eq!(
        r2.id, id1,
        "DB must resolve to the same entity as DBs (EX-06b): {r2:#?}"
    );
    assert_ne!(r2.status, "ambiguous", "DB/DBs must not be ambiguous");
}

#[test]
fn same_person_with_shared_org_still_merges() {
    // ER-08 allows a near-name merge when the write names a shared person/org
    // that a stored memory of the candidate also mentions.
    let e = engine();
    remember(
        &e,
        "Jonathon Smith works at Acme.",
        vec![person("Jonathon Smith"), org("Acme")],
        vec![rel("Jonathon Smith", "works at", "Acme")],
    );
    let out = remember(
        &e,
        "Jonathan Smith works at Acme.",
        vec![person("Jonathan Smith"), org("Acme")],
        vec![rel("Jonathan Smith", "works at", "Acme")],
    );
    let r = resolution(&out, "Jonathan Smith");
    assert!(
        matches!(
            r.status.as_str(),
            "near" | "matched" | "alias" | "same_as" | "lexical"
        ),
        "same person respelling with shared org must merge (ER-08 corroboration): {r:#?}"
    );
    assert!(r.id > 0);
}
