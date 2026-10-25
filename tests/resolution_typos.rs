//! Lane nc-f4 regression tests for the 2026-10-23 resolution failures:
//! ER-07 typo/transliteration dedup with hard blockers, Q-08 typo'd query
//! entity resolution, and REL-19 typo'd exclusive targets (tests are named
//! after the items in briefs/open_items.md).

use neocortex::{
    embed::{Embedder, HashEmbedder, Mode},
    engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput, RememberOutput},
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

fn place(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("place".into()),
        ..Default::default()
    }
}

fn rel(src: &str, predicate: &str, dst: &str, exclusive: bool) -> RelationIn {
    RelationIn {
        source: src.into(),
        predicate: predicate.into(),
        target: dst.into(),
        exclusive: Some(exclusive),
        ..Default::default()
    }
}

/// Fixed-vectors embedder for the cosine-agreement tests: texts are matched
/// by their prefix against the first key that applies, everything else falls
/// back to the hash embedder.
struct MapEmbedder {
    map: Vec<(String, Vec<f32>)>,
}

impl Embedder for MapEmbedder {
    fn embed(&self, texts: &[&str], mode: Mode) -> neocortex::Result<Vec<Vec<f32>>> {
        let fallback = HashEmbedder.embed(texts, mode)?;
        Ok(texts
            .iter()
            .zip(fallback)
            .map(|(text, fallback)| {
                self.map
                    .iter()
                    .find(|(key, _)| text.starts_with(key.as_str()))
                    .map(|(_, vector)| vector.clone())
                    .unwrap_or(fallback)
            })
            .collect())
    }

    fn name(&self) -> &str {
        "map"
    }
}

fn axis(i: usize) -> Vec<f32> {
    let mut v = vec![0.0; 768];
    v[i] = 1.0;
    v
}

fn map_engine(map: Vec<(&str, Vec<f32>)>) -> Engine {
    let map = map.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    Engine::open_in_memory(Arc::new(MapEmbedder { map }))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn sql_i64(e: &Engine, sql: &str) -> Option<i64> {
    e.db.sql().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn resolution<'a>(out: &'a RememberOutput, name: &str) -> &'a neocortex::engine::EntityResolution {
    out.entities
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no resolution for {name:?} in {:#?}", out.entities))
}

pub fn remember_named(
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

// ------------------------------------------------------------------ ER-07

#[test]
fn er07_typo_becomes_the_same_entity() {
    let e = engine();
    let first = remember_named(
        &e,
        "Jonathan Smith called about the servers.",
        vec![person("Jonathan Smith")],
        vec![],
    );
    let smith_id = resolution(&first, "Jonathan Smith").id;
    let second = remember_named(
        &e,
        "Jonathon Smith called about the servers.",
        vec![person("Jonathon Smith")],
        vec![],
    );
    let res = resolution(&second, "Jonathon Smith");
    assert_eq!(res.id, smith_id, "typo must resolve to the stored entity");
    assert_eq!(res.status, "near");
}

#[test]
fn er07_transliteration_becomes_the_same_entity() {
    let e = engine();
    let first = remember_named(
        &e,
        "Mueller joined the audit.",
        vec![person("Müller")],
        vec![],
    );
    let muller_id = resolution(&first, "Müller").id;
    let second = remember_named(
        &e,
        "Mueller joined the audit review.",
        vec![person("Mueller")],
        vec![],
    );
    assert_eq!(resolution(&second, "Mueller").id, muller_id);
}

#[test]
fn er07_digit_siblings_stay_separate_and_are_reported() {
    let e = engine();
    let first = remember_named(&e, "Room 101 was checked.", vec![org("Room 101")], vec![]);
    let room_id = resolution(&first, "Room 101").id;
    let second = remember_named(&e, "Room 102 was swept.", vec![org("Room 102")], vec![]);
    let res = resolution(&second, "Room 102");
    assert_ne!(res.id, room_id, "digit siblings must not merge");
    assert_eq!(res.status, "ambiguous", "blocked pair is never silent");
    let cands = &res.candidates;
    assert!(cands.iter().any(|c| c.id == room_id), "{cands:?}");
}

#[test]
fn er07_person_tokens_block_the_automatic_merge() {
    let e = engine();
    let first = remember_named(
        &e,
        "Teresa Mota signed the contract.",
        vec![person("Teresa Mota")],
        vec![],
    );
    let mota_id = resolution(&first, "Teresa Mota").id;
    let second = remember_named(
        &e,
        "Teresa Moura signed the contract too.",
        vec![person("Teresa Moura")],
        vec![],
    );
    let res = resolution(&second, "Teresa Moura");
    assert_ne!(res.id, mota_id, "Teresa Mota/Moura stay separate");
    assert_eq!(res.status, "ambiguous");
    assert!(res.candidates.iter().any(|c| c.id == mota_id));
}

#[test]
fn er07_cosine_disagreement_keeps_the_pair_separate() {
    let e = map_engine(vec![
        ("Bergen", axis(0)),
        ("Bergn", axis(2)),
        // Memory texts feed the context signal; keep them orthogonal as well.
        ("Dana lives in Bergen", axis(1)),
        ("Dana moves to Bergn", axis(3)),
    ]);
    let first = remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let bergen_id = resolution(&first, "Bergen").id;
    let second = remember_named(
        &e,
        "Dana moves to Bergn.",
        vec![person("Dana"), place("Bergn")],
        vec![rel("Dana", "moves to", "Bergn", true)],
    );
    let res = resolution(&second, "Bergn");
    assert_ne!(res.id, bergen_id, "cosine-disagreeing pair stays separate");
    assert_eq!(res.status, "ambiguous");
    assert!(res.candidates.iter().any(|c| c.id == bergen_id));
}

// ------------------------------------------------------------------- Q-08

#[test]
fn q08_typoed_query_resolves_the_entity() {
    let e = engine();
    remember_named(
        &e,
        "Moritz Kanold works at Nordwind.",
        vec![person("Moritz Kanold"), org("Nordwind")],
        vec![rel("Moritz Kanold", "works at", "Nordwind", true)],
    );
    let out = e
        .recall(&RecallInput {
            query: "who is Moritz Kanoldt".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.entities.iter().any(|x| x.name == "Moritz Kanold"),
        "typo'd query must seed the entity: {:?}",
        out.entities
    );
}

#[test]
fn q08_fuzzy_lookup_resolves_bergen_from_bergn() {
    let e = engine();
    remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let out = e.entity("Bergn", 10).unwrap();
    assert_eq!(out.card.name, "Bergen");
}

// ------------------------------------------------------------------ REL-19

#[test]
fn rel19_typo_target_resolves_to_the_same_value() {
    let e = engine();
    let first = remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let rid = first.relations[0].id;
    let bergen_id = resolution(&first, "Bergen").id;
    let second = remember_named(
        &e,
        "Dana lives in Bergn.",
        vec![person("Dana"), place("Bergn")],
        vec![rel("Dana", "lives in", "Bergn", true)],
    );
    assert!(
        second.invalidated_relations.is_empty(),
        "typo'd target must not end the correct fact: {:?}",
        second.invalidated_relations
    );
    assert_eq!(resolution(&second, "Bergn").id, bergen_id);
    let live = sql_i64(
        &e,
        &format!("SELECT EXISTS(SELECT 1 FROM relations WHERE id={rid} AND valid_to IS NULL AND expired_at IS NULL)"),
    );
    assert_eq!(live, Some(1), "Bergen fact must stay live");
}

#[test]
fn rel19_blocked_near_target_ends_nothing_and_reports() {
    let e = engine();
    let first = remember_named(
        &e,
        "Dana reports to Teresa Mota.",
        vec![person("Dana"), person("Teresa Mota")],
        vec![rel("Dana", "reports to", "Teresa Mota", true)],
    );
    let rid = first.relations[0].id;
    let second = remember_named(
        &e,
        "Dana reports to Teresa Moura.",
        vec![person("Dana"), person("Teresa Moura")],
        vec![rel("Dana", "reports to", "Teresa Moura", true)],
    );
    assert!(second.invalidated_relations.is_empty());
    assert!(
        second.conflicts.iter().any(|c| c.relation_id == rid),
        "blocked near target must report a conflict candidate: {:?}",
        second.conflicts
    );
    let live = sql_i64(
        &e,
        &format!("SELECT EXISTS(SELECT 1 FROM relations WHERE id={rid} AND valid_to IS NULL AND expired_at IS NULL)"),
    );
    assert_eq!(live, Some(1));
}

#[test]
fn rel19_true_move_to_another_city_still_ends_the_fact() {
    let e = engine();
    let first = remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let rid = first.relations[0].id;
    let second = remember_named(
        &e,
        "Dana lives in Oslo.",
        vec![person("Dana"), place("Oslo")],
        vec![rel("Dana", "lives in", "Oslo", true)],
    );
    assert!(
        second.invalidated_relations.contains(&rid),
        "a genuinely different city must end the old fact: {:?}",
        second.invalidated_relations
    );
    assert!(second.conflicts.is_empty(), "{:?}", second.conflicts);
}
