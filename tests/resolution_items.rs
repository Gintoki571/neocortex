//! Lane nc-ex01 regression tests (briefs/open_items.md EX-01..EX-04): unit
//! labels as entities, measures never entities, known-head spans, and the
//! write-time unique alias prefix. Hash embedder; names follow the real
//! extraction order.

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RememberInput, RememberOutput},
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

fn remember(e: &Engine, content: &str, entities: Vec<EntityIn>) -> RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        entities,
        ..Default::default()
    })
    .unwrap()
}

fn names(out: &RememberOutput) -> Vec<&str> {
    out.entities.iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn ex01_unit_labels_are_separate_entities() {
    let e = engine();
    let a = remember(
        &e,
        "Apt 3B in the Harbor Building has a leaking pipe.",
        vec![],
    );
    let b = remember(
        &e,
        "Apt 3C in the Harbor Building has a broken window.",
        vec![],
    );
    let id_b = b
        .entities
        .iter()
        .find(|r| r.name == "Apt 3C")
        .expect("Apt 3C");
    let id_a = a
        .entities
        .iter()
        .find(|r| r.name == "Apt 3B")
        .expect("Apt 3B");
    assert_ne!(id_a.id, id_b.id, "Apt 3B and Apt 3C must stay apart");
}

#[test]
fn ex02_measures_after_numbers_never_become_entities() {
    let e = engine();
    let out = remember(
        &e,
        "The fresh DB is 1.2 MB and recall takes 120 ms.",
        vec![],
    );
    let n = names(&out);
    assert!(!n.contains(&"MB") && !n.contains(&"ms"), "{n:?}");
    assert!(n.contains(&"DB"), "{n:?}");
}

#[test]
fn ex03_known_head_before_plural_noun_is_not_a_compound() {
    let e = engine();
    let cypher = EntityIn {
        name: "Cypher".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    remember(&e, "Cypher is the query language.", vec![cypher]);
    let out = remember(&e, "Cypher Memory nodes are readable now.", vec![]);
    let n = names(&out);
    assert!(n.contains(&"Cypher"), "{n:?}");
    assert!(!n.contains(&"Cypher Memory"), "{n:?}");
}

#[test]
fn ex04_unique_alias_prefix_resolves_on_write() {
    let e = engine();
    let embedder = EntityIn {
        name: "UltraBERT-Embed-150M".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    let first = remember(&e, "UltraBERT-Embed-150M is the embedder.", vec![embedder]);
    let first_id = first.entities[0].id;
    let out = remember(&e, "UltraBERT runs on CPU.", vec![]);
    let hit = out
        .entities
        .iter()
        .find(|r| r.name == "UltraBERT")
        .expect("UltraBERT mention");
    assert_eq!(hit.id, first_id, "{:?}", out.entities);
    assert_eq!(hit.status, "alias", "{:?}", out.entities);
}

#[test]
fn ex04_two_prefix_candidates_stay_ambiguous() {
    let e = engine();
    let a = EntityIn {
        name: "UltraBERT-Embed-150M".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    let b = EntityIn {
        name: "UltraBERT-Large".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    remember(&e, "UltraBERT-Embed-150M is one model.", vec![a]);
    remember(&e, "UltraBERT-Large is another model.", vec![b]);
    let out = remember(&e, "UltraBERT runs on CPU.", vec![]);
    let hit = out
        .entities
        .iter()
        .find(|r| r.name == "UltraBERT")
        .expect("UltraBERT mention");
    assert_ne!(hit.status, "alias", "{:?}", out.entities);
}
