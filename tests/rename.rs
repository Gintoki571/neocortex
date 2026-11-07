//! Rename joins the old and new name (H3-10): a rename statement merges the
//! two names into one entity, the old name survives as an alias, old facts
//! move to the surviving entity, and a "formerly called" fact keeps the old
//! name answerable. Hedged/negated/ambiguous renames merge nothing.

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RecallInput, RememberInput},
    temporal::Clock,
};
use std::sync::Arc;

const NOW: i64 = 1_800_000_000;

fn new_engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn project(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("project".into()),
        ..Default::default()
    }
}

fn person(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
        ..Default::default()
    }
}

fn typed(name: &str, kind: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some(kind.into()),
        ..Default::default()
    }
}

fn rel(
    source: &str,
    predicate: &str,
    target: &str,
    exclusive: bool,
) -> neocortex::engine::RelationIn {
    neocortex::engine::RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        exclusive: Some(exclusive),
        ..Default::default()
    }
}

fn remember_at(
    engine: &Engine,
    content: &str,
    entities: Vec<EntityIn>,
    relations: Vec<neocortex::engine::RelationIn>,
    valid_from: &str,
) -> neocortex::engine::RememberOutput {
    engine
        .remember(RememberInput {
            content: content.into(),
            entities,
            relations,
            valid_from: Some(valid_from.into()),
            ..Default::default()
        })
        .unwrap()
}

fn recall_facts(engine: &Engine, q: &str) -> Vec<String> {
    engine
        .recall(&RecallInput {
            query: q.into(),
            ..Default::default()
        })
        .unwrap()
        .facts
        .into_iter()
        .map(|f| f.fact)
        .collect()
}

fn card_id(engine: &Engine, name: &str) -> i64 {
    engine.entity(name, 20).unwrap().card.id
}

/// s11: Harbor renamed to Lantern. The stale Python 3.11 fact under the old
/// name ends when Lantern's 3.13 fact lands, and both names are one entity.
#[test]
fn rename_merges_harbor_into_lantern_and_ends_stale_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The ingest pipeline Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let third = remember_at(
        &engine,
        "The Lantern pipeline no longer uses Python 3.11 after the 2026-08-20 upgrade to Python 3.13.",
        vec![project("Lantern"), project("Python 3.13")],
        vec![rel("Lantern", "runs on", "Python 3.13", true)],
        "2026-08-20",
    );
    // One entity: the surviving display name is Lantern, Harbor is its alias.
    let lantern = engine.entity("Lantern", 20).unwrap();
    assert_eq!(lantern.card.name, "Lantern");
    assert!(
        lantern
            .card
            .aliases
            .iter()
            .any(|a| neocortex::text::norm_name(a) == "harbor"),
        "Lantern aliases should contain Harbor, got {:?}",
        lantern.card.aliases
    );
    assert_eq!(
        card_id(&engine, "Harbor"),
        lantern.card.id,
        "Harbor and Lantern should be one entity"
    );
    // The stale exclusive value ended by valid_from order; history keeps it.
    assert!(
        !third.invalidated_relations.is_empty(),
        "Lantern's 3.13 fact should end the 3.11 fact"
    );
    let current: Vec<String> = lantern
        .current_facts
        .iter()
        .map(|f| f.fact.clone())
        .collect();
    assert!(
        current.iter().any(|f| f.contains("Python 3.13")),
        "current facts should hold Python 3.13, got {current:?}"
    );
    assert!(
        !current.iter().any(|f| f.contains("Python 3.11")),
        "Python 3.11 should not be current, got {current:?}"
    );
    assert!(
        lantern
            .past_facts
            .iter()
            .any(|f| f.fact.contains("Python 3.11")),
        "history should keep the ended 3.11 fact, got {:?}",
        lantern.past_facts
    );
}

/// s9: Project Atlas was renamed to Beacon.
#[test]
fn rename_merges_atlas_into_beacon() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The Atlas repo uses npm for package management.",
        vec![project("Atlas"), project("npm")],
        vec![rel("Atlas", "uses", "npm", true)],
        "2026-01-08",
    );
    remember_at(
        &engine,
        "Project Atlas was renamed to Beacon on 2026-06-02.",
        vec![project("Atlas"), project("Beacon")],
        vec![],
        "2026-06-02",
    );
    assert_eq!(card_id(&engine, "Atlas"), card_id(&engine, "Beacon"));
}

/// The formerly-called fact answers "what was Lantern called before".
#[test]
fn rename_fact_answers_called_before() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The ingest pipeline Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let facts = recall_facts(&engine, "what was Lantern called before the rename");
    assert!(
        facts.iter().any(|f| f.to_lowercase().contains("harbor")),
        "recall should answer Harbor, got {facts:?}"
    );
    let card = engine.entity("Lantern", 20).unwrap();
    assert!(
        card.current_facts
            .iter()
            .any(|f| f.fact.to_lowercase().contains("harbor")),
        "entity card should carry the formerly fact, got {:?}",
        card.current_facts
    );
}

/// A rename stated as a relation (predicate "renamed to") merges too.
#[test]
fn rename_via_relation_predicate_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Harbor was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![neocortex::engine::RelationIn {
            source: "Harbor".into(),
            predicate: "renamed to".into(),
            target: "Lantern".into(),
            ..Default::default()
        }],
        "2026-07-03",
    );
    assert_eq!(card_id(&engine, "Harbor"), card_id(&engine, "Lantern"));
}

/// "is now called" and "(formerly X)" phrasings merge too.
#[test]
fn rename_now_called_and_formerly_phrasings_merge() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Harbor is now called Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-08-01",
    );
    assert_eq!(card_id(&engine, "Harbor"), card_id(&engine, "Lantern"));

    let engine2 = new_engine();
    remember_at(
        &engine2,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine2,
        "Lantern (formerly Harbor) runs the ingest pipeline.",
        vec![project("Lantern"), project("Harbor")],
        vec![],
        "2026-08-01",
    );
    assert_eq!(card_id(&engine2, "Harbor"), card_id(&engine2, "Lantern"));
}

/// A rename of a person uses the same path.
#[test]
fn person_name_change_joins() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Kim Lee works at Acme.",
        vec![person("Kim Lee"), project("Acme")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Kim Lee changed her name to Kim Stone.",
        vec![person("Kim Lee"), person("Kim Stone")],
        vec![],
        "2026-07-03",
    );
    let stone = engine.entity("Kim Stone", 20).unwrap();
    assert_eq!(stone.card.id, card_id(&engine, "Kim Lee"));
    assert!(
        stone
            .card
            .aliases
            .iter()
            .any(|a| neocortex::text::norm_name(a) == "kim lee"),
        "old name should be an alias of the survivor"
    );
}

/// Hedged renames merge nothing.
#[test]
fn hedged_rename_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "We might rename Harbor to Lantern next month.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "a hedged rename must not merge"
    );

    let engine2 = new_engine();
    remember_at(
        &engine2,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine2,
        "We plan to rename Harbor to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine2, "Harbor"),
        card_id(&engine2, "Lantern"),
        "a planned rename must not merge"
    );
}

/// Negated renames merge nothing.
#[test]
fn negated_rename_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Harbor was not renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "a negated rename must not merge"
    );
}

/// Several entities named Harbor: merge nothing, report candidates.
#[test]
fn ambiguous_old_name_merges_nothing_and_reports_candidates() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor is a project that runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    // A second, different Harbor (a place) so the old name is ambiguous.
    remember_at(
        &engine,
        "Harbor is a city on the coast.",
        vec![typed("Harbor", "place")],
        vec![],
        "2026-02-01",
    );
    let out = remember_at(
        &engine,
        "Harbor was renamed to Lantern.",
        vec![
            EntityIn {
                name: "Harbor".into(),
                ..Default::default()
            },
            project("Lantern"),
        ],
        vec![],
        "2026-07-03",
    );
    let reported = out.entities.iter().any(|e| !e.candidates.is_empty())
        || out.hints.iter().any(|h| h.contains("Harbor"));
    assert!(
        reported,
        "ambiguous rename should report candidates, got entities={:?} hints={:?}",
        out.entities, out.hints
    );
    let lantern = engine.entity("Lantern", 20).unwrap();
    assert_ne!(
        lantern.card.id,
        card_id(&engine, "Harbor"),
        "ambiguous rename must not have merged Lantern into a Harbor"
    );
}

/// After the rename, a fact stated with the old name lands on the survivor.
#[test]
fn fact_on_old_name_after_rename_resolves_to_survivor() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let survivor = card_id(&engine, "Lantern");
    let out = remember_at(
        &engine,
        "Harbor runs on Python 3.12 now.",
        vec![project("Harbor"), project("Python 3.12")],
        vec![],
        "2026-09-01",
    );
    let resolution = out
        .entities
        .iter()
        .find(|e| e.name == "Harbor")
        .expect("Harbor resolution present");
    assert_eq!(
        resolution.id, survivor,
        "old-name mention should resolve to the merged entity"
    );
}

/// Forgetting the rename memory removes the formerly-called fact but keeps
/// the merge (the alias stays).
#[test]
fn forget_rename_memory_keeps_merge_and_retracts_formerly_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    let rename = remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let survivor = card_id(&engine, "Lantern");
    assert!(engine
        .entity("Lantern", 20)
        .unwrap()
        .current_facts
        .iter()
        .any(|f| f.fact.to_lowercase().contains("harbor")));
    engine.forget(&format!("m{}", rename.id), false).unwrap();
    assert_eq!(
        card_id(&engine, "Harbor"),
        survivor,
        "forgetting the rename memory must not unmerge"
    );
    assert!(
        !engine
            .entity("Lantern", 20)
            .unwrap()
            .current_facts
            .iter()
            .any(|f| f.fact.to_lowercase().contains("formerly")),
        "forget should retract the formerly-called fact"
    );
}
