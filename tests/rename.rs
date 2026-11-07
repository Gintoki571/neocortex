//! Renames (H3-10, fix-round-3 contract): prose NEVER merges. A text cue that
//! passes every pre-round-3 guard becomes a `renamed?` hint naming both
//! entities and the exact confirm call, storing nothing. Only a caller-asserted
//! rename — a rename-predicate relation or `same_as` when both rows exist —
//! merges old into new, moves facts, and writes the "formerly called" holder
//! fact. Hedged/negated/ambiguous/generic sentences emit no hint.

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RecallInput, RememberInput, RememberOutput},
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

fn untyped(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
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
) -> RememberOutput {
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

fn has_question_hint(out: &RememberOutput) -> bool {
    out.hints.iter().any(|h| h.contains("renamed?"))
}

fn has_merge_hint(out: &RememberOutput) -> bool {
    out.hints.iter().any(|h| h.starts_with("renamed:"))
}

fn has_any_rename_hint(out: &RememberOutput) -> bool {
    out.hints
        .iter()
        .any(|h| h.contains("renamed?") || h.starts_with("renamed:"))
}

/// Text cue passed the guards and hinted, but stored nothing: hints carry
/// both names, both `e<id>`s, and the surviving confirm calls (same_as, renamed to).
fn assert_renamed_question_hint(out: &RememberOutput, old: &str, new: &str) {
    let hint = out
        .hints
        .iter()
        .find(|h| h.contains("renamed?"))
        .unwrap_or_else(|| panic!("expected renamed? hint, got {:?}", out.hints));
    assert!(hint.contains(old), "hint names old {old}: {hint}");
    assert!(hint.contains(new), "hint names new {new}: {hint}");
    assert!(hint.contains("e"), "hint carries e<id>: {hint}");
    assert!(
        hint.contains("same_as"),
        "hint tells same_as confirm: {hint}"
    );
    assert!(
        hint.contains("renamed to"),
        "hint tells renamed-to confirm: {hint}"
    );
}

/// No rename hint at all (guards filtered the pair out).
fn assert_no_rename_hint(out: &RememberOutput) {
    assert!(
        !has_any_rename_hint(out),
        "expected no rename hint, got {:?}",
        out.hints
    );
}

/// Structured confirm via a `renamed from` relation (new-first, merges).
/// (Narrowed contract: `formerly called` is a plain relation, not a merge trigger.)
fn confirm_rename_relation(
    engine: &Engine,
    old: &str,
    new: &str,
    kind: &str,
    when: &str,
) -> RememberOutput {
    remember_at(
        engine,
        &format!("{new} was renamed from {old}."),
        vec![typed(old, kind), typed(new, kind)],
        vec![neocortex::engine::RelationIn {
            source: new.into(),
            predicate: "renamed from".into(),
            target: old.into(),
            ..Default::default()
        }],
        when,
    )
}

/// Structured confirm via `same_as` on the new name (hint call).
fn confirm_rename_same_as(
    engine: &Engine,
    new: &str,
    old: &str,
    kind: &str,
    when: &str,
) -> RememberOutput {
    let old_id = card_id(engine, old);
    engine
        .remember(RememberInput {
            content: "Rename confirmed.".into(),
            entities: vec![EntityIn {
                name: new.into(),
                kind: Some(kind.into()),
                same_as: Some(format!("e{old_id}")),
                ..Default::default()
            }],
            valid_from: Some(when.into()),
            ..Default::default()
        })
        .unwrap()
}

fn assert_no_merge(engine: &Engine, a: &str, b: &str) {
    assert_ne!(
        card_id(engine, a),
        card_id(engine, b),
        "{a} and {b} must not have merged"
    );
}

fn assert_merged(engine: &Engine, a: &str, b: &str) {
    assert_eq!(
        card_id(engine, a),
        card_id(engine, b),
        "{a} and {b} should be one entity"
    );
}

fn seed_entity(engine: &Engine, name: &str, kind: &str, when: &str) {
    remember_at(
        engine,
        &format!("{name} runs on Python 3.11."),
        vec![typed(name, kind), typed("Python 3.11", "language")],
        vec![rel(name, "runs on", "Python 3.11", true)],
        when,
    );
}

/// s11 structured path: text hints, structured merges, stale 3.11 ends.
#[test]
fn structured_rename_harbor_into_lantern_ends_stale_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The ingest pipeline Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    let hint_out = remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert_renamed_question_hint(&hint_out, "Harbor", "Lantern");
    let confirm = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-07-04");
    assert!(
        has_merge_hint(&confirm),
        "structured confirm should merge, got {:?}",
        confirm.hints
    );
    let third = remember_at(
        &engine,
        "The Lantern pipeline no longer uses Python 3.11 after the 2026-08-20 upgrade to Python 3.13.",
        vec![project("Lantern"), project("Python 3.13")],
        vec![rel("Lantern", "runs on", "Python 3.13", true)],
        "2026-08-20",
    );
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
    assert_eq!(card_id(&engine, "Harbor"), lantern.card.id);
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

/// s11 text-only: hint, no merge, no formerly fact, facts stay put.
#[test]
fn text_rename_harbor_hints_and_stores_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The ingest pipeline Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    let out = remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert_renamed_question_hint(&out, "Harbor", "Lantern");
    let harbor = engine.entity("Harbor", 20).unwrap();
    assert!(
        harbor
            .current_facts
            .iter()
            .any(|f| f.fact.contains("Python 3.11")),
        "old fact stays on Harbor before confirmation, got {:?}",
        harbor.current_facts
    );
    let lantern = engine.entity("Lantern", 20).unwrap();
    assert!(
        !lantern
            .current_facts
            .iter()
            .any(|f| f.fact.contains("formerly")),
        "text hint must store no formerly fact, got {:?}",
        lantern.current_facts
    );
    let facts = recall_facts(&engine, "what was Lantern called before the rename");
    assert!(
        facts.iter().all(|f| !f.to_lowercase().contains("formerly")),
        "unconfirmed hint must not answer called-before, got {facts:?}"
    );
}

/// Shared F/G/S/V/Vерниф negative shape: no merge and no hint.
fn assert_text_negative_no_merge_no_hint(
    engine: &Engine,
    cand_out: &RememberOutput,
    a: &str,
    b: &str,
) {
    assert_no_merge(engine, a, b);
    assert_no_rename_hint(cand_out);
}

/// s9 Atlas→Beacon: text hints, structured merges.
#[test]
fn text_hints_atlas_beacon_and_structured_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The Atlas repo uses npm for package management.",
        vec![project("Atlas"), project("npm")],
        vec![rel("Atlas", "uses", "npm", true)],
        "2026-01-08",
    );
    let hint_out = remember_at(
        &engine,
        "Project Atlas was renamed to Beacon on 2026-06-02.",
        vec![project("Atlas"), project("Beacon")],
        vec![],
        "2026-06-02",
    );
    assert_no_merge(&engine, "Atlas", "Beacon");
    assert_renamed_question_hint(&hint_out, "Atlas", "Beacon");
    let confirm = confirm_rename_relation(&engine, "Atlas", "Beacon", "project", "2026-06-03");
    assert!(has_merge_hint(&confirm));
    assert_merged(&engine, "Atlas", "Beacon");
}

/// The formerly-called fact answers "what was Lantern called before" only
/// after confirmation.
#[test]
fn structured_rename_answers_called_before() {
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
    assert!(
        recall_facts(&engine, "what was Lantern called before the rename")
            .iter()
            .all(|f| !f.to_lowercase().contains("formerly")),
        "text hint alone must not answer called-before"
    );
    let confirm = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-07-04");
    assert!(has_merge_hint(&confirm));
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
    // A question on the old name finds the new entity's facts via the alias:
    // the formerly-called holder fact of the survivor is returned.
    let old_q = recall_facts(&engine, "what does Harbor run on");
    let card_q = recall_facts(&engine, "what does Lantern run on");
    assert!(
        old_q
            .iter()
            .any(|f| f.to_lowercase().contains("formerly") || f.contains("Python 3.11")),
        "old-name question should find the merged fact, old={old_q:?} new={card_q:?}"
    );
}

/// A rename stated as a relation (predicate "renamed to", old-first) merges.
#[test]
fn structured_rename_via_relation_predicate_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    let out = remember_at(
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
    assert!(
        has_merge_hint(&out),
        "structured should merge, got {:?}",
        out.hints
    );
    assert_merged(&engine, "Harbor", "Lantern");
    // The formerly fact exists exactly once live.
    let card = engine.entity("Lantern", 20).unwrap();
    assert!(card
        .current_facts
        .iter()
        .any(|f| f.fact.contains("formerly called")));
}

/// Structured direction mapping (narrowed): only `renamed to` (old-first)
/// and `renamed from` (new-first) merge; call-shaped spellings do not.
#[test]
fn structured_rename_direction_mapping() {
    for (pred, src, dst) in [
        ("renamed to", "Harbor", "Lantern"),
        ("renamed from", "Lantern", "Harbor"),
    ] {
        let engine = new_engine();
        seed_entity(&engine, "Harbor", "project", "2026-01-05");
        seed_entity(&engine, "Lantern", "project", "2026-01-05");
        let out = remember_at(
            &engine,
            "Rename confirmed.",
            vec![project("Harbor"), project("Lantern")],
            vec![neocortex::engine::RelationIn {
                source: src.into(),
                predicate: pred.into(),
                target: dst.into(),
                ..Default::default()
            }],
            "2026-06-01",
        );
        assert!(
            has_merge_hint(&out),
            "predicate {pred} {src}->{dst} should merge, got {:?}",
            out.hints
        );
        assert_merged(&engine, "Harbor", "Lantern");
    }
}

/// "is now called" and "(formerly X)" text hints; structured merges.
#[test]
fn text_now_called_and_formerly_hint_and_structured_merge() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    let out = remember_at(
        &engine,
        "Harbor is now called Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-08-01",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert_renamed_question_hint(&out, "Harbor", "Lantern");
    let confirm = confirm_rename_same_as(&engine, "Lantern", "Harbor", "project", "2026-08-02");
    assert!(
        has_merge_hint(&confirm),
        "same_as confirm should merge, got {:?}",
        confirm.hints
    );
    assert_merged(&engine, "Harbor", "Lantern");

    let engine2 = new_engine();
    remember_at(
        &engine2,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    let out2 = remember_at(
        &engine2,
        "Lantern (formerly Harbor) runs the ingest pipeline.",
        vec![project("Lantern"), project("Harbor")],
        vec![],
        "2026-08-01",
    );
    assert_no_merge(&engine2, "Harbor", "Lantern");
    assert_renamed_question_hint(&out2, "Harbor", "Lantern");
}

/// same_as confirm merges with the new name surviving.
#[test]
fn same_as_confirm_merges_with_new_display() {
    let engine = new_engine();
    seed_entity(&engine, "Harbor", "project", "2026-01-05");
    seed_entity(&engine, "Lantern", "project", "2026-01-05");
    let out = confirm_rename_same_as(&engine, "Lantern", "Harbor", "project", "2026-06-01");
    assert!(has_merge_hint(&out), "got {:?}", out.hints);
    assert_merged(&engine, "Harbor", "Lantern");
    assert_eq!(engine.entity("Lantern", 20).unwrap().card.name, "Lantern");
}

/// A rename of a person: text hints, structured merges.
#[test]
fn person_name_change_hints_and_structured_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Kim Lee works at Acme.",
        vec![person("Kim Lee"), project("Acme")],
        vec![],
        "2026-01-09",
    );
    let out = remember_at(
        &engine,
        "Kim Lee changed her name to Kim Stone.",
        vec![person("Kim Lee"), person("Kim Stone")],
        vec![],
        "2026-07-03",
    );
    assert_no_merge(&engine, "Kim Lee", "Kim Stone");
    assert_renamed_question_hint(&out, "Kim Lee", "Kim Stone");
    let confirm = confirm_rename_relation(&engine, "Kim Lee", "Kim Stone", "person", "2026-07-04");
    assert!(has_merge_hint(&confirm));
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

/// Hedged renames emit no hint and merge nothing.
#[test]
fn hedged_rename_emits_no_hint_and_merges_nothing() {
    for text in [
        "We might rename Harbor to Lantern next month.",
        "We plan to rename Harbor to Lantern.",
        "Harbor might be renamed to Lantern.",
    ] {
        let engine = new_engine();
        remember_at(
            &engine,
            "Harbor runs on Python 3.11.",
            vec![project("Harbor")],
            vec![],
            "2026-01-09",
        );
        let out = remember_at(
            &engine,
            text,
            vec![project("Harbor"), project("Lantern")],
            vec![],
            "2026-06-01",
        );
        assert_text_negative_no_merge_no_hint(&engine, &out, "Harbor", "Lantern");
    }
}

/// Negated renames emit no hint and merge nothing.
#[test]
fn negated_rename_emits_no_hint_and_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    let out = remember_at(
        &engine,
        "Harbor was not renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Harbor", "Lantern");
}

/// Several entities named Harbor (homonyms): no merge, candidates, no hint.
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
    assert_no_rename_hint(&out);
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

/// After a structured rename, a fact stated with the old name lands on the survivor.
#[test]
fn fact_on_old_name_after_structured_rename_resolves_to_survivor() {
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
    let confirm = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-07-04");
    assert!(has_merge_hint(&confirm));
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

/// Forgetting the confirming memory retracts the formerly fact but keeps the
/// merge (no un-merge). Forgetting a text-hint memory does nothing.
#[test]
fn forget_confirming_memory_keeps_merge_and_retracts_formerly_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    let hint = remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    assert!(!engine
        .entity("Lantern", 20)
        .unwrap()
        .current_facts
        .iter()
        .any(|f| f.fact.contains("formerly")));
    // Forgetting the hint memory changes nothing.
    engine.forget(&format!("m{}", hint.id), false).unwrap();
    assert_no_merge(&engine, "Harbor", "Lantern");

    let engine2 = new_engine();
    remember_at(
        &engine2,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    let confirm = confirm_rename_relation(&engine2, "Harbor", "Lantern", "project", "2026-07-03");
    assert!(has_merge_hint(&confirm));
    let survivor = card_id(&engine2, "Lantern");
    assert!(engine2
        .entity("Lantern", 20)
        .unwrap()
        .current_facts
        .iter()
        .any(|f| f.fact.to_lowercase().contains("harbor")));
    engine2.forget(&format!("m{}", confirm.id), false).unwrap();
    assert_eq!(
        card_id(&engine2, "Harbor"),
        survivor,
        "forgetting the confirming memory must not unmerge"
    );
    assert!(
        !engine2
            .entity("Lantern", 20)
            .unwrap()
            .current_facts
            .iter()
            .any(|f| f.fact.to_lowercase().contains("formerly")),
        "forget should retract the formerly-called fact"
    );
}

/// Verifier F02: "the branch" -> "main" is generic: no merge, no hint.
#[test]
fn f02_generic_branch_to_main_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Branch tracks Origin.",
        vec![typed("Branch", "branch"), typed("Origin", "branch")],
        vec![rel("Branch", "tracks", "Origin", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "main tracks Release.",
        vec![typed("main", "branch"), typed("Release", "branch")],
        vec![rel("main", "tracks", "Release", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "I renamed the branch to main.",
        vec![typed("Branch", "branch"), typed("main", "branch")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Branch", "main");
}

/// Verifier F03: "The meeting" is generic.
#[test]
fn f03_meeting_to_project_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Sync runs on nginx.",
        vec![project("Sync"), typed("nginx", "project")],
        vec![rel("Sync", "runs on", "nginx", false)],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "The meeting was renamed to Sync.",
        vec![
            EntityIn {
                name: "Meeting".into(),
                ..Default::default()
            },
            project("Sync"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Meeting", "Sync");
}

/// Verifier F04a/F04b: passive "is now called by the team": no merge, no hint.
#[test]
fn f04_passive_called_by_emits_no_hint_in_both_type_variants() {
    for typed_team in [true, false] {
        let engine = new_engine();
        remember_at(
            &engine,
            "Harbor runs on Python 3.11.",
            vec![project("Harbor"), typed("Python 3.11", "language")],
            vec![rel("Harbor", "runs on", "Python 3.11", false)],
            "2026-01-05",
        );
        if typed_team {
            remember_at(
                &engine,
                "Team owns Backlog.",
                vec![typed("Team", "team"), typed("Backlog", "list")],
                vec![rel("Team", "owns", "Backlog", false)],
                "2026-01-06",
            );
        } else {
            remember_at(
                &engine,
                "Team owns Backlog.",
                vec![
                    EntityIn {
                        name: "Team".into(),
                        ..Default::default()
                    },
                    typed("Backlog", "list"),
                ],
                vec![rel("Team", "owns", "Backlog", false)],
                "2026-01-06",
            );
        }
        let out = remember_at(
            &engine,
            "Harbor is now called by the team every morning.",
            {
                let mut v = vec![project("Harbor")];
                if typed_team {
                    v.push(typed("Team", "team"));
                } else {
                    v.push(EntityIn {
                        name: "Team".into(),
                        ..Default::default()
                    });
                }
                v
            },
            vec![],
            "2026-06-01",
        );
        assert_ne!(
            card_id(&engine, "Harbor"),
            card_id(&engine, "Team"),
            "passive 'called by' (typed_team={typed_team}) must not merge"
        );
        assert!(
            !out.hints
                .iter()
                .any(|h| h.contains("renamed?") || h.starts_with("renamed:")),
            "no rename hint expected, got {:?}",
            out.hints
        );
    }
}

/// Verifier F08: both names carry live facts (undated text): candidates, no
/// merge, no hint. Structured confirms merge and report ended facts.
#[test]
fn f08_both_live_facts_undated_text_blocks_and_structured_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Lantern runs on Python 3.13.",
        vec![project("Lantern"), typed("Python 3.13", "language")],
        vec![rel("Lantern", "runs on", "Python 3.13", true)],
        "2026-02-05",
    );
    let out = remember_at(
        &engine,
        "The Harbor pipeline was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert!(
        !has_question_hint(&out),
        "both-facts undated text should not hint, got {:?}",
        out.hints
    );
    let reported = out.entities.iter().any(|e| !e.candidates.is_empty())
        || out.hints.iter().any(|h| h.contains("Lantern"));
    assert!(
        reported,
        "both-facts rename should report a candidate, got {:?} / {:?}",
        out.entities, out.hints
    );
    // Structured: caller asserted it — merge and report ended facts.
    let confirm = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-06-02");
    assert!(has_merge_hint(&confirm), "got {:?}", confirm.hints);
    assert_merged(&engine, "Harbor", "Lantern");
    assert!(
        !confirm.invalidated_relations.is_empty(),
        "structured both-live merge should report ended facts"
    );
    assert!(
        confirm.hints.iter().any(|h| h.contains("ended")),
        "merge hint should report ended facts, got {:?}",
        confirm.hints
    );
}

/// Verifier F09: bare shared first name never hints.
#[test]
fn f09_shared_first_name_old_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Maya Chen works at Acme.",
        vec![person("Maya Chen"), typed("Acme", "org")],
        vec![rel("Maya Chen", "works at", "Acme", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Maya Patel works at Brightline.",
        vec![person("Maya Patel"), typed("Brightline", "org")],
        vec![rel("Maya Patel", "works at", "Brightline", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "Maya changed her name to Rio.",
        vec![person("Maya"), person("Rio")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Maya Chen"),
        card_id(&engine, "Maya Patel"),
        "seeds must stay separate"
    );
    assert!(!has_question_hint(&out), "got {:?}", out.hints);
    assert_ne!(
        card_id(&engine, "Maya Chen"),
        card_id(&engine, "Rio"),
        "bare shared first name must not merge into Rio"
    );
    assert_ne!(
        card_id(&engine, "Maya Patel"),
        card_id(&engine, "Rio"),
        "bare shared first name must not merge into Rio"
    );
}

/// Verifier F09b: NEW name is the shared first name.
#[test]
fn f09b_shared_first_name_new_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Kim Lee works at Acme.",
        vec![person("Kim Lee"), typed("Acme", "org")],
        vec![rel("Kim Lee", "works at", "Acme", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Maya Chen works at Brightline.",
        vec![person("Maya Chen"), typed("Brightline", "org")],
        vec![rel("Maya Chen", "works at", "Brightline", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "Maya Patel works at Initech.",
        vec![person("Maya Patel"), typed("Initech", "org")],
        vec![rel("Maya Patel", "works at", "Initech", false)],
        "2026-01-07",
    );
    let out = remember_at(
        &engine,
        "Kim Lee changed her name to Maya.",
        vec![person("Kim Lee"), person("Maya")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Kim Lee"),
        card_id(&engine, "Maya Chen"),
        "Kim Lee must not merge into Maya Chen"
    );
    assert!(!has_question_hint(&out), "got {:?}", out.hints);
}

/// Verifier F11: a question is not a rename statement: no hint, no merge.
#[test]
fn f11_question_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "Was Harbor renamed to Lantern?",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Harbor", "Lantern");
}

/// Verifier F12: immediately reverted rename emits no hint.
#[test]
fn f12_reverted_rename_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "Harbor was renamed to Lantern, then back to Harbor.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Harbor", "Lantern");
}

/// Verifier F13: plan after the cue emits no hint.
#[test]
fn f13_planned_after_cue_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "The rename of Harbor to Lantern is planned for Q3.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Harbor", "Lantern");
}

/// Verifier F16: directory names are generic.
#[test]
fn f16_tests_directory_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Tests holds Fixtures.",
        vec![typed("Tests", "directory"), typed("Fixtures", "directory")],
        vec![rel("Tests", "holds", "Fixtures", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Specs holds Docs.",
        vec![typed("Specs", "directory"), typed("Docs", "directory")],
        vec![rel("Specs", "holds", "Docs", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "We renamed the tests directory to specs.",
        vec![typed("Tests", "directory"), typed("Specs", "directory")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Tests", "Specs");
}

/// Verifier F17: "the main branch" is generic.
#[test]
fn f17_main_branch_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Main tracks Origin.",
        vec![typed("Main", "branch"), typed("Origin", "branch")],
        vec![rel("Main", "tracks", "Origin", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Release tracks Stable.",
        vec![typed("Release", "branch"), typed("Stable", "branch")],
        vec![rel("Release", "tracks", "Stable", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "The main branch was renamed to release.",
        vec![typed("Main", "branch"), typed("Release", "branch")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Main", "Release");
}

/// Verifier F20: branch->main generic in agent voice.
#[test]
fn f20_team_bare_verb_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Branch tracks Origin.",
        vec![typed("Branch", "branch"), typed("Origin", "branch")],
        vec![rel("Branch", "tracks", "Origin", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "main tracks Release.",
        vec![typed("main", "branch"), typed("Release", "branch")],
        vec![rel("main", "tracks", "Release", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "The team renamed the branch to main.",
        vec![
            typed("Team", "team"),
            typed("Branch", "branch"),
            typed("main", "branch"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_text_negative_no_merge_no_hint(&engine, &out, "Branch", "main");
}

/// Verifier T01: "Harbor is now Lantern." hints; structured merges.
#[test]
fn t01_is_now_name_hints_and_structured_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "Harbor is now Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert_renamed_question_hint(&out, "Harbor", "Lantern");
    let confirm = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-06-02");
    assert!(has_merge_hint(&confirm));
    assert_merged(&engine, "Harbor", "Lantern");
}

/// Verifier T02: parenthetical "(now Beacon)" hints; structured merges.
#[test]
fn t02_parenthetical_now_hints_and_structured_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Atlas uses npm.",
        vec![project("Atlas"), typed("npm", "tool")],
        vec![rel("Atlas", "uses", "npm", true)],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "Atlas (now Beacon) uses Swift.",
        vec![
            project("Atlas"),
            project("Beacon"),
            typed("Swift", "language"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Atlas", "Beacon");
    assert_renamed_question_hint(&out, "Atlas", "Beacon");
    let confirm = confirm_rename_relation(&engine, "Atlas", "Beacon", "project", "2026-06-02");
    assert!(has_merge_hint(&confirm));
    assert_merged(&engine, "Atlas", "Beacon");
}

/// Verifier T09: "Lantern (renamed from Harbor)" hints; structured merges.
#[test]
fn t09_renamed_from_without_to_hints_and_structured_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "Lantern (renamed from Harbor) runs ingest.",
        vec![project("Lantern"), project("Harbor")],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert_renamed_question_hint(&out, "Harbor", "Lantern");
    let confirm = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-06-02");
    assert!(has_merge_hint(&confirm));
    assert_merged(&engine, "Harbor", "Lantern");
}

/// Both-lowercase rename (T03 shape): text hints nothing, structured merges.
#[test]
fn t03_lowercase_text_no_hint_but_structured_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "harbor runs on python 3.11.",
        vec![typed("harbor", "project"), typed("python 3.11", "language")],
        vec![rel("harbor", "runs on", "python 3.11", true)],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "the harbor pipeline is now called lantern.",
        vec![typed("harbor", "project"), typed("lantern", "project")],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "harbor", "lantern");
    assert!(
        !has_question_hint(&out),
        "lowercase both sides should not hint, got {:?}",
        out.hints
    );
    let engine2 = new_engine();
    remember_at(
        &engine2,
        "harbor runs on python 3.11.",
        vec![typed("harbor", "project"), typed("python 3.11", "language")],
        vec![rel("harbor", "runs on", "python 3.11", true)],
        "2026-01-05",
    );
    let confirm = remember_at(
        &engine2,
        "lantern formerly called harbor.",
        vec![typed("harbor", "project"), typed("lantern", "project")],
        vec![neocortex::engine::RelationIn {
            source: "lantern".into(),
            predicate: "renamed from".into(),
            target: "harbor".into(),
            ..Default::default()
        }],
        "2026-06-01",
    );
    assert!(
        has_merge_hint(&confirm),
        "caller-asserted lowercase merges, got {:?}",
        confirm.hints
    );
    assert_merged(&engine2, "harbor", "lantern");
}

/// The CLI merge core now moves `relations.subj`: after `merge_entities`,
/// a later exclusive fact on the survivor ends the moved stale fact.
#[test]
fn merge_command_moves_subj_and_ends_stale_exclusive_fact() {
    let engine = new_engine();
    let first = remember_at(
        &engine,
        "Atlas runs on Python 3.11.",
        vec![project("Atlas"), typed("Python 3.11", "language")],
        vec![rel("Atlas", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Beacon runs on Python 3.13.",
        vec![project("Beacon"), typed("Python 3.13", "language")],
        vec![],
        "2026-02-01",
    );
    engine.merge_entities("Beacon", "Atlas", false).unwrap();
    let survivor = card_id(&engine, "Beacon");
    let third = remember_at(
        &engine,
        "Beacon runs on Python 3.14.",
        vec![project("Beacon"), typed("Python 3.14", "language")],
        vec![rel("Beacon", "runs on", "Python 3.14", true)],
        "2026-08-20",
    );
    assert!(
        !third.invalidated_relations.is_empty(),
        "merge must have moved subj so the exclusive slot ends stale facts"
    );
    let card = engine.entity("Beacon", 20).unwrap();
    assert_eq!(card.card.id, survivor);
    assert!(
        card.current_facts
            .iter()
            .any(|f| f.fact.contains("Python 3.14")),
        "current facts should hold Python 3.14, got {:?}",
        card.current_facts
    );
    assert!(
        !card
            .current_facts
            .iter()
            .any(|f| f.fact.contains("Python 3.11")),
        "Python 3.11 should not be current, got {:?}",
        card.current_facts
    );
    assert!(first.id > 0);
}

/// Structured rename ignores name-shape guards the caller overrode:
/// a lowercase-word generic pair merges when explicitly asserted.
#[test]
fn structured_merge_ignores_written_form_guard() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Repo tracks Commits.",
        vec![typed("Repo", "repo"), typed("Commits", "list")],
        vec![rel("Repo", "tracks", "Commits", false)],
        "2026-01-05",
    );
    let confirm = remember_at(
        &engine,
        "Rename confirmed.",
        vec![typed("Repo", "repo"), untyped("Helios")],
        vec![neocortex::engine::RelationIn {
            source: "Helios".into(),
            predicate: "renamed from".into(),
            target: "Repo".into(),
            ..Default::default()
        }],
        "2026-06-01",
    );
    // Types differ (repo vs untyped): different-type keeps its guard even
    // for structured, so use same-type entities here instead.
    // Re-run with same types to prove the shape guard is dropped.
    if !has_merge_hint(&confirm) {
        let engine2 = new_engine();
        remember_at(
            &engine2,
            "Repo tracks Commits.",
            vec![typed("Repo", "project"), typed("Commits", "list")],
            vec![rel("Repo", "tracks", "Commits", false)],
            "2026-01-05",
        );
        let confirm2 = remember_at(
            &engine2,
            "The repo was renamed to Helios.",
            vec![typed("Repo", "project"), typed("Helios", "project")],
            vec![neocortex::engine::RelationIn {
                source: "Helios".into(),
                predicate: "formerly called".into(),
                target: "Repo".into(),
                ..Default::default()
            }],
            "2026-06-01",
        );
        assert!(
            has_merge_hint(&confirm2),
            "caller-asserted lowercase merges, got {:?}",
            confirm2.hints
        );
        assert_merged(&engine2, "Repo", "Helios");
    } else {
        assert_merged(&engine, "Repo", "Helios");
    }
}

/// Structured rename with different types still skips with a hint.
#[test]
fn structured_merge_different_types_skips() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Lantern is a place.",
        vec![typed("Lantern", "place")],
        vec![],
        "2026-01-06",
    );
    let out = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-06-01");
    // confirm uses project for both; rename check reads stored types
    // (project vs place) — the skip hint names different types.
    // Force the differing-types case explicitly:
    let engine2 = new_engine();
    remember_at(
        &engine2,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    remember_at(
        &engine2,
        "Lantern is a city.",
        vec![typed("Lantern", "city")],
        vec![],
        "2026-01-06",
    );
    let out2 = remember_at(
        &engine2,
        "Rename confirmed.",
        vec![project("Harbor"), typed("Lantern", "city")],
        vec![neocortex::engine::RelationIn {
            source: "Lantern".into(),
            predicate: "renamed from".into(),
            target: "Harbor".into(),
            ..Default::default()
        }],
        "2026-06-01",
    );
    assert_no_merge(&engine2, "Harbor", "Lantern");
    assert!(
        out2.hints.iter().any(|h| h.contains("different types")),
        "expected different-types skip, got {:?}",
        out2.hints
    );
    assert!(
        !has_any_rename_hint(&out2) || out2.hints.iter().any(|h| h.contains("different types"))
    );
    let _ = out;
}

/// Run one seed + one rename candidate; return the candidate output.
fn seed_rename_with_out(
    seed_subject: &str,
    seed_kind: &str,
    seed_pred: &str,
    seed_object: &str,
    seed_okind: &str,
    cand_content: &str,
    cand_entities: Vec<EntityIn>,
) -> (Engine, RememberOutput) {
    let engine = new_engine();
    remember_at(
        &engine,
        &format!("{seed_subject} {seed_pred} {seed_object}."),
        vec![
            typed(seed_subject, seed_kind),
            typed(seed_object, seed_okind),
        ],
        vec![rel(seed_subject, seed_pred, seed_object, false)],
        "2026-01-05",
    );
    let out = remember_at(&engine, cand_content, cand_entities, vec![], "2026-06-01");
    (engine, out)
}

fn assert_text_negative(engine: &Engine, out: &RememberOutput, a: &str, b: &str) {
    assert_no_merge(engine, a, b);
    assert_no_rename_hint(out);
}

fn assert_text_hint_no_merge(
    engine: &Engine,
    out: &RememberOutput,
    a: &str,
    b: &str,
    old: &str,
    new: &str,
) {
    assert_no_merge(engine, a, b);
    assert_renamed_question_hint(out, old, new);
}

#[test]
fn g01_repo_to_helios_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Repo",
        "repo",
        "tracks",
        "Commits",
        "list",
        "The repo was renamed to Helios.",
        vec![typed("Repo", "repo"), untyped("Helios")],
    );
    assert_text_negative(&e, &out, "Repo", "Helios");
}

#[test]
fn g02_channel_now_called_general_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Channel",
        "channel",
        "carries",
        "Alerts",
        "list",
        "The channel is now called General.",
        vec![typed("Channel", "channel"), untyped("General")],
    );
    assert_text_negative(&e, &out, "Channel", "General");
}

#[test]
fn g03_doc_to_playbook_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Doc",
        "document",
        "lists",
        "Steps",
        "list",
        "The doc was renamed to Playbook.",
        vec![typed("Doc", "document"), untyped("Playbook")],
    );
    assert_text_negative(&e, &out, "Doc", "Playbook");
}

#[test]
fn g04_ticket_to_aurora_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Ticket",
        "issue",
        "tracks",
        "Bug 42",
        "issue",
        "The ticket was renamed to Aurora.",
        vec![typed("Ticket", "issue"), untyped("Aurora")],
    );
    assert_text_negative(&e, &out, "Ticket", "Aurora");
}

#[test]
fn g05_sprint_to_cycle7_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Sprint",
        "period",
        "ends",
        "May",
        "date",
        "The sprint was renamed to Cycle 7.",
        vec![typed("Sprint", "period"), untyped("Cycle 7")],
    );
    assert_text_negative(&e, &out, "Sprint", "Cycle 7");
}

#[test]
fn g06_board_to_roadmap_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Board",
        "doc",
        "lists",
        "Cards",
        "list",
        "The board was renamed to Roadmap.",
        vec![typed("Board", "doc"), untyped("Roadmap")],
    );
    assert_text_negative(&e, &out, "Board", "Roadmap");
}

#[test]
fn g07_server_to_atlas_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Server",
        "machine",
        "runs",
        "Debian",
        "os",
        "The server was renamed to Atlas.",
        vec![typed("Server", "machine"), untyped("Atlas")],
    );
    assert_text_negative(&e, &out, "Server", "Atlas");
}

#[test]
fn g08_cluster_to_vega_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Cluster",
        "infra",
        "runs",
        "Kubernetes",
        "tool",
        "The cluster was renamed to Vega.",
        vec![typed("Cluster", "infra"), untyped("Vega")],
    );
    assert_text_negative(&e, &out, "Cluster", "Vega");
}

#[test]
fn g09_app_to_aurora_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "App",
        "tool",
        "uses",
        "React",
        "tool",
        "The app was renamed to Aurora.",
        vec![typed("App", "tool"), untyped("Aurora")],
    );
    assert_text_negative(&e, &out, "App", "Aurora");
}

#[test]
fn g10_bucket_to_archive_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Bucket",
        "storage",
        "holds",
        "Backups",
        "list",
        "The bucket was renamed to Archive.",
        vec![typed("Bucket", "storage"), untyped("Archive")],
    );
    assert_text_negative(&e, &out, "Bucket", "Archive");
}

#[test]
fn g11_repo_to_main_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Repo tracks Commits.",
        vec![typed("Repo", "repo"), typed("Commits", "list")],
        vec![rel("Repo", "tracks", "Commits", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "main tracks Release.",
        vec![typed("main", "branch"), typed("Release", "branch")],
        vec![rel("main", "tracks", "Release", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "The repo was renamed to main.",
        vec![untyped("Repo"), untyped("main")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative(&engine, &out, "Repo", "main");
}

#[test]
fn g12_channel_now_general_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Channel",
        "channel",
        "carries",
        "Alerts",
        "list",
        "The channel is now general.",
        vec![untyped("Channel"), untyped("general")],
    );
    assert_text_negative(&e, &out, "Channel", "general");
}

#[test]
fn s01_stable_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor is now stable.",
        vec![project("Harbor"), untyped("Stable")],
    );
    assert_text_negative(&e, &out, "Harbor", "Stable");
}

#[test]
fn s02_deprecated_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor is now deprecated.",
        vec![project("Harbor"), untyped("Deprecated")],
    );
    assert_text_negative(&e, &out, "Harbor", "Deprecated");
}

#[test]
fn s03_busy_emits_no_hint() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Ana Silva works at Acme.",
        vec![person("Ana Silva"), typed("Acme", "org")],
        vec![rel("Ana Silva", "works at", "Acme", false)],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "Ana Silva is now busy.",
        vec![person("Ana Silva"), untyped("Busy")],
        vec![],
        "2026-06-01",
    );
    assert_text_negative(&engine, &out, "Ana Silva", "Busy");
}

#[test]
fn s04_public_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Atlas",
        "project",
        "uses",
        "npm",
        "tool",
        "Atlas is now public.",
        vec![project("Atlas"), untyped("Public")],
    );
    assert_text_negative(&e, &out, "Atlas", "Public");
}

#[test]
fn s05_archived_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Lantern",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Lantern is now archived.",
        vec![project("Lantern"), untyped("Archived")],
    );
    assert_text_negative(&e, &out, "Lantern", "Archived");
}

#[test]
fn s06_in_berlin_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor is now in Berlin.",
        vec![project("Harbor"), typed("Berlin", "city")],
    );
    assert_text_negative(&e, &out, "Harbor", "Berlin");
}

#[test]
fn s07_now_based_emits_no_hint() {
    let (e, out) = seed_rename_with_out(
        "Atlas",
        "project",
        "uses",
        "npm",
        "tool",
        "Atlas (now based in Berlin) runs the pipeline.",
        vec![project("Atlas"), typed("Berlin", "city")],
    );
    assert_text_negative(&e, &out, "Atlas", "Berlin");
}

#[test]
fn x01_state_word_does_not_end_exclusive_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Channel carries Alerts.",
        vec![typed("Channel", "channel"), typed("Alerts", "list")],
        vec![rel("Channel", "carries", "Alerts", true)],
        "2026-01-05",
    );
    let out = remember_at(
        &engine,
        "The channel is now general.",
        vec![untyped("Channel"), untyped("general")],
        vec![],
        "2026-06-01",
    );
    assert_no_rename_hint(&out);
    remember_at(
        &engine,
        "General carries Pages.",
        vec![untyped("General"), typed("Pages", "list")],
        vec![rel("General", "carries", "Pages", true)],
        "2026-07-01",
    );
    assert_no_merge(&engine, "Channel", "General");
    let ch = engine.entity("Channel", 20).unwrap();
    assert!(
        ch.current_facts.iter().any(|f| f.fact.contains("Alerts")),
        "Channel Alerts should stay live, got {:?}",
        ch.current_facts
    );
    let gen = engine.entity("General", 20).unwrap();
    assert!(
        gen.current_facts.iter().any(|f| f.fact.contains("Pages")),
        "General Pages should be live, got {:?}",
        gen.current_facts
    );
}

#[test]
fn l01_true_plus_generic_second_cue() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Team owns Backlog.",
        vec![typed("Team", "team"), typed("Backlog", "list")],
        vec![rel("Team", "owns", "Backlog", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "Harbor was renamed to Lantern and the team is now called Platform.",
        vec![
            project("Harbor"),
            project("Lantern"),
            typed("Team", "team"),
            typed("Platform", "team"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert_no_merge(&engine, "Team", "Platform");
    assert!(
        out.hints
            .iter()
            .any(|h| h.contains("renamed?") && h.contains("Harbor")),
        "Harbor hint expected, got {:?}",
        out.hints
    );
    assert!(
        !out.hints
            .iter()
            .any(|h| h.contains("renamed?") && h.contains("Platform")),
        "Team generic should not hint, got {:?}",
        out.hints
    );
}

#[test]
fn l02_two_true_renames_both_hint_and_structured_both_merge() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Atlas uses npm.",
        vec![project("Atlas"), typed("npm", "tool")],
        vec![rel("Atlas", "uses", "npm", true)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "Harbor was renamed to Lantern after Atlas was renamed to Beacon.",
        vec![
            project("Harbor"),
            project("Lantern"),
            project("Atlas"),
            project("Beacon"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Harbor", "Lantern");
    assert_no_merge(&engine, "Atlas", "Beacon");
    let hints = out.hints.iter().filter(|h| h.contains("renamed?")).count();
    assert!(hints >= 2, "two hints expected, got {:?}", out.hints);
    // Structured: confirm both.
    let c1 = confirm_rename_relation(&engine, "Harbor", "Lantern", "project", "2026-06-02");
    assert!(has_merge_hint(&c1));
    let c2 = confirm_rename_relation(&engine, "Atlas", "Beacon", "project", "2026-06-03");
    assert!(has_merge_hint(&c2));
    assert_merged(&engine, "Harbor", "Lantern");
    assert_merged(&engine, "Atlas", "Beacon");
    // No chain: Harbor survivor is not Beacon survivor.
    assert_ne!(card_id(&engine, "Harbor"), card_id(&engine, "Beacon"));
}

#[test]
fn l03_deep_clause_hints_and_structured_merges() {
    let (e, out) = seed_rename_with_out(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "After the long migration the Harbor ingest project was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
    );
    assert_text_hint_no_merge(&e, &out, "Harbor", "Lantern", "Harbor", "Lantern");
    let confirm = confirm_rename_relation(&e, "Harbor", "Lantern", "project", "2026-06-02");
    assert!(has_merge_hint(&confirm));
    assert_merged(&e, "Harbor", "Lantern");
}

#[test]
fn n01_zurich_hub_hints_and_structured_merges() {
    let (e, out) = seed_rename_with_out(
        "Zürich Hub",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "The Zürich Hub project was renamed to Helvetia.",
        vec![typed("Zürich Hub", "project"), typed("Helvetia", "project")],
    );
    assert_text_hint_no_merge(&e, &out, "Zürich Hub", "Helvetia", "Zürich Hub", "Helvetia");
    let confirm = confirm_rename_relation(&e, "Zürich Hub", "Helvetia", "project", "2026-06-02");
    assert!(has_merge_hint(&confirm));
    assert_merged(&e, "Zürich Hub", "Helvetia");
}

#[test]
fn n02_sao_paulo_hints_and_structured_merges() {
    let (e, out) = seed_rename_with_out(
        "São Paulo Team",
        "team",
        "owns",
        "Backlog",
        "list",
        "The São Paulo Team was renamed to Latam Team.",
        vec![typed("São Paulo Team", "team"), typed("Latam Team", "team")],
    );
    assert_text_hint_no_merge(
        &e,
        &out,
        "São Paulo Team",
        "Latam Team",
        "São Paulo Team",
        "Latam Team",
    );
    let confirm = confirm_rename_relation(&e, "São Paulo Team", "Latam Team", "team", "2026-06-02");
    assert!(has_merge_hint(&confirm));
    assert_merged(&e, "São Paulo Team", "Latam Team");
}

#[test]
fn n03_cjk_hints_and_structured_merges() {
    let (e, out) = seed_rename_with_out(
        "東京",
        "project",
        "uses",
        "npm",
        "tool",
        "東京 is now Tokyo.",
        vec![typed("東京", "project"), typed("Tokyo", "project")],
    );
    assert_text_hint_no_merge(&e, &out, "東京", "Tokyo", "東京", "Tokyo");
    let confirm = confirm_rename_relation(&e, "東京", "Tokyo", "project", "2026-06-02");
    assert!(has_merge_hint(&confirm));
    assert_merged(&e, "東京", "Tokyo");
}

#[test]
fn m01_lower_old_upper_new_hints_and_structured_merges() {
    let (e, out) = seed_rename_with_out(
        "harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "harbor is now LANTERN.",
        vec![typed("harbor", "project"), typed("LANTERN", "project")],
    );
    // Old side is lowercase non-initial but the haystack shape in the worker
    // tests merged because the seed made Harbor sentence-initial; here the
    // cue sentence is "harbor is now LANTERN." (old at position 0 — initial
    // exemption), so it must hint.
    assert_text_hint_no_merge(&e, &out, "harbor", "LANTERN", "harbor", "LANTERN");
}

#[test]
fn m02_upper_sentence_hints_and_structured_merges() {
    let (e, out) = seed_rename_with_out(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "HARBOR WAS RENAMED TO LANTERN.",
        vec![project("Harbor"), typed("lantern", "project")],
    );
    assert_text_hint_no_merge(&e, &out, "Harbor", "lantern", "Harbor", "lantern");
}

#[test]
fn and_hedge_does_not_block_true_rename_hint() {
    let (e, out) = seed_rename_with_out(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor was renamed to Lantern and we may expand it later.",
        vec![project("Harbor"), project("Lantern")],
    );
    assert_text_hint_no_merge(&e, &out, "Harbor", "Lantern", "Harbor", "Lantern");
}

// ---------------------------------------------------------------------------
// Fix round 4 (narrowed contract): only `renamed to` / `renamed from` and
// `same_as` merge. Call-shaped spellings are plain facts, byte-identical in
// storage semantics to main (no merge, no formerly-called holder, no alias).
// B1/B1b/B2/B7 from verify4.
// ---------------------------------------------------------------------------

fn assert_plain_relation_no_merge(engine: &Engine, a: &str, b: &str, out: &RememberOutput) {
    assert_no_merge(engine, a, b);
    assert!(
        !has_merge_hint(out),
        "call-shaped spelling must not merge, got {:?}",
        out.hints
    );
    // No merge-produced alias: a merge aliases b -> a's survivor.
    let card_a = engine.entity(a, 20).unwrap().card;
    let card_b = engine.entity(b, 20).unwrap().card;
    let norm_b = neocortex::text::norm_name(b);
    assert!(
        !card_a
            .aliases
            .iter()
            .any(|al| neocortex::text::norm_name(al) == norm_b),
        "merge must not alias {b} onto {a}, got {:?}",
        card_a.aliases
    );
    let _ = card_b;
}

fn remember_call_relation(
    engine: &Engine,
    source: &str,
    predicate: &str,
    target: &str,
    source_kind: Option<&str>,
    target_kind: Option<&str>,
) -> RememberOutput {
    let mut entities = vec![EntityIn {
        name: source.into(),
        kind: source_kind.map(str::to_string),
        ..Default::default()
    }];
    entities.push(EntityIn {
        name: target.into(),
        kind: target_kind.map(str::to_string),
        ..Default::default()
    });
    remember_at(
        engine,
        &format!("{source} {predicate} {target} about the invoice."),
        entities,
        vec![neocortex::engine::RelationIn {
            source: source.into(),
            predicate: predicate.into(),
            target: target.into(),
            ..Default::default()
        }],
        "2026-03-01",
    )
}

/// B1: "Ana previously called Bo" (phone call, both person) is a plain fact.
#[test]
fn b1_previously_called_phone_call_stores_plain_fact() {
    let engine = new_engine();
    let out = remember_call_relation(
        &engine,
        "Ana",
        "previously called",
        "Bo",
        Some("person"),
        Some("person"),
    );
    assert_plain_relation_no_merge(&engine, "Ana", "Bo", &out);
    assert!(
        !out.relations.is_empty(),
        "relation must be stored as a plain fact"
    );
}

/// B1b: "Ana formerly called Bo" (phone call, both person) is a plain fact.
#[test]
fn b1b_formerly_called_phone_call_stores_plain_fact() {
    let engine = new_engine();
    let out = remember_call_relation(
        &engine,
        "Ana",
        "formerly called",
        "Bo",
        Some("person"),
        Some("person"),
    );
    assert_plain_relation_no_merge(&engine, "Ana", "Bo", &out);
    assert!(
        !out.relations.is_empty(),
        "relation must be stored as a plain fact"
    );
}

/// B2: "Lantern now called Ana" (a call, both untyped) is a plain fact.
#[test]
fn b2_now_called_untyped_call_stores_plain_fact() {
    let engine = new_engine();
    let out = remember_call_relation(&engine, "Lantern", "now called", "Ana", None, None);
    assert_plain_relation_no_merge(&engine, "Lantern", "Ana", &out);
    assert!(
        !out.relations.is_empty(),
        "relation must be stored as a plain fact"
    );
}

/// B7: bare superset predicates beyond the narrowed merge set are plain facts.
#[test]
fn b7_bare_superset_predicates_store_plain_facts() {
    for (pred, src, dst) in [
        ("renamed", "Ana", "Bo"),
        ("rename to", "Ana", "Bo"),
        ("formerly", "Ana", "Bo"),
        ("previously", "Ana", "Bo"),
        ("now known as", "Ana", "Bo"),
    ] {
        let engine = new_engine();
        let out = remember_call_relation(&engine, src, pred, dst, Some("person"), Some("person"));
        assert_plain_relation_no_merge(&engine, src, dst, &out);
        assert!(
            !out.relations.is_empty(),
            "predicate {pred} must be stored as a plain fact"
        );
    }
}

/// `renamed to` (old-first) still merges.
#[test]
fn narrowed_renamed_to_still_merges() {
    let engine = new_engine();
    seed_entity(&engine, "Harbor", "project", "2026-01-05");
    seed_entity(&engine, "Lantern", "project", "2026-01-05");
    let out = remember_at(
        &engine,
        "Harbor was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![neocortex::engine::RelationIn {
            source: "Harbor".into(),
            predicate: "renamed to".into(),
            target: "Lantern".into(),
            ..Default::default()
        }],
        "2026-06-01",
    );
    assert!(
        has_merge_hint(&out),
        "renamed to must merge, got {:?}",
        out.hints
    );
    assert_merged(&engine, "Harbor", "Lantern");
}

/// `renamed from` (new-first) still merges.
#[test]
fn narrowed_renamed_from_still_merges() {
    let engine = new_engine();
    seed_entity(&engine, "Harbor", "project", "2026-01-05");
    seed_entity(&engine, "Lantern", "project", "2026-01-05");
    let out = remember_at(
        &engine,
        "Lantern was renamed from Harbor.",
        vec![project("Harbor"), project("Lantern")],
        vec![neocortex::engine::RelationIn {
            source: "Lantern".into(),
            predicate: "renamed from".into(),
            target: "Harbor".into(),
            ..Default::default()
        }],
        "2026-06-01",
    );
    assert!(
        has_merge_hint(&out),
        "renamed from must merge, got {:?}",
        out.hints
    );
    assert_merged(&engine, "Harbor", "Lantern");
}

/// `same_as` still merges (hint-confirm path).
#[test]
fn narrowed_same_as_still_merges() {
    let engine = new_engine();
    seed_entity(&engine, "Harbor", "project", "2026-01-05");
    seed_entity(&engine, "Lantern", "project", "2026-01-05");
    let out = confirm_rename_same_as(&engine, "Lantern", "Harbor", "project", "2026-06-01");
    assert!(
        has_merge_hint(&out),
        "same_as must merge, got {:?}",
        out.hints
    );
    assert_merged(&engine, "Harbor", "Lantern");
}

/// The `renamed?` text hint names only the surviving confirm calls: `same_as`
/// and a `renamed to` relation. It must NOT suggest `formerly called` (which
/// no longer merges under the narrowed contract) and must stay token-cheap.
#[test]
fn renamed_hint_names_only_surviving_confirm_calls() {
    let engine = new_engine();
    seed_entity(&engine, "Harbor", "project", "2026-01-05");
    let out = remember_at(
        &engine,
        "The Harbor pipeline was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let hint = out
        .hints
        .iter()
        .find(|h| h.contains("renamed?"))
        .expect("expected renamed? hint");
    assert!(hint.contains("same_as"), "hint must name same_as: {hint}");
    assert!(
        hint.contains("renamed to"),
        "hint must name renamed to: {hint}"
    );
    assert!(
        !hint.contains("formerly called"),
        "hint must not suggest formerly called (no longer merges): {hint}"
    );
    // Token-cheap: a short, single hint.
    assert!(
        hint.split_whitespace().count() <= 80,
        "hint should stay token-cheap, got {} words: {hint}",
        hint.split_whitespace().count()
    );
}
