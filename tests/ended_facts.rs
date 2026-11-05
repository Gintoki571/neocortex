//! Lane nc-ended: a text-only "no longer" retraction ends the live fact with a
//! hyphenated or multi-word object (H3-09), and a yes/no "still" question
//! shows the matching ended fact with its end date (H3-11).

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput},
    render,
    temporal::Clock,
};
use std::sync::Arc;

/// 2026-10-23 12:00 UTC, a Friday.
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

fn tool(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("tool".into()),
        ..Default::default()
    }
}

fn project(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("project".into()),
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

fn recall(e: &Engine, query: &str) -> neocortex::engine::RecallOutput {
    e.recall(&RecallInput {
        query: query.into(),
        ..Default::default()
    })
    .unwrap()
}

fn text(e: &Engine, out: &neocortex::engine::RecallOutput) -> String {
    render::render_recall(out, &render::clock_fmt(&e.clock()))
}

/// The relation's end, if any: `valid_to` or the system retirement time.
fn relation_end(e: &Engine, rid: i64) -> Option<i64> {
    e.db.sql()
        .query_row(
            "SELECT COALESCE(valid_to, expired_at) FROM relations WHERE id=?1",
            [rid],
            |r| r.get(0),
        )
        .unwrap()
}

/// Ingest a relation-bearing memory and return its relation id.
fn tomasz_reviews_payment(e: &Engine) -> i64 {
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak is the security reviewer for all payment-adjacent code.".into(),
            valid_from: Some("2026-02-04".into()),
            entities: vec![person("Tomasz Nowak")],
            relations: vec![rel(
                "Tomasz Nowak",
                "reviews",
                "payment-adjacent code",
                false,
            )],
            ..Default::default()
        })
        .unwrap();
    out.relations[0].id
}

fn retract(e: &Engine, content: &str) -> neocortex::engine::RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        entities: vec![person("Tomasz Nowak"), person("Ines Park")],
        ..Default::default()
    })
    .unwrap()
}

// ------------------------------------------------------------- H3-09

#[test]
fn h3_09_no_longer_with_a_hyphenated_object_ends_the_fact() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer reviews payment-adjacent code.");
    assert!(
        out.invalidated_relations.contains(&rid),
        "hyphenated object not matched: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_09_a_trailing_date_phrase_does_not_hide_the_object() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(
        &e,
        "Tomasz Nowak no longer reviews payment-adjacent code since August 2026; Ines Park took over the security reviews.",
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "'since August 2026' tail kept the fact live: {out:?}"
    );
}

#[test]
fn h3_09_since_a_noun_is_not_a_time_tail() {
    // "since the migration" continues the clause, so the object is not whole.
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(
        &e,
        "Tomasz Nowak no longer reviews payment-adjacent code since the migration.",
    );
    assert!(
        !out.invalidated_relations.contains(&rid),
        "'since the migration' ended the fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_09_a_different_object_does_not_end_the_fact() {
    // "payment code" is not "payment-adjacent code": the live fact stays.
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer reviews payment code.");
    assert!(
        !out.invalidated_relations.contains(&rid),
        "a different object ended the fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

// ------------------------------------------------------------- H3-13

fn trial_handover(e: &Engine) {
    e.remember(RememberInput {
        content: "Tunde Kalu led the Harbor trial.".into(),
        valid_from: Some("2026-01-10".into()),
        entities: vec![person("Tunde Kalu"), project("Harbor trial")],
        relations: vec![rel("Tunde Kalu", "led", "Harbor trial", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Amara Nwosu now leads the Harbor trial.".into(),
        valid_from: Some("2026-02-10".into()),
        entities: vec![person("Amara Nwosu"), project("Harbor trial")],
        relations: vec![rel("Amara Nwosu", "leads", "Harbor trial", true)],
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn h3_13_past_slot_questions_retrieve_ended_memory_but_current_does_not() {
    let e = engine();
    trial_handover(&e);
    for query in [
        "which trial did Tunde Kalu lead",
        "the trial that Tunde Kalu led",
        "who used to lead the Harbor trial",
        "who is the former lead of the Harbor trial",
    ] {
        let out = recall(&e, query);
        assert_eq!(
            out.memories.first().map(|m| m.id),
            Some(1),
            "{query}: {:?}",
            out.memories
        );
        assert!(
            out.facts.iter().any(|fact| fact.id == 1 && fact.ended),
            "{query}: {:?}",
            out.facts
        );
    }
    let current = recall(&e, "who leads the Harbor trial");
    assert_eq!(current.memories.first().map(|m| m.id), Some(2));
    assert!(!current.facts.iter().any(|fact| fact.id == 1));
}

#[test]
fn h3_13_past_state_boost_requires_the_asked_predicate() {
    let e = engine();
    trial_handover(&e);
    let out = recall(&e, "which tool did Tunde Kalu use");
    let old = out.memories.iter().find(|m| m.id == 1).unwrap();
    assert!(
        old.score < 2.0,
        "unrelated ended predicate was boosted: {}",
        old.score
    );
}

#[test]
fn h3_13_negated_retraction_is_not_resurrected_as_live() {
    let e = engine();
    trial_handover(&e);
    let out = recall(&e, "Tunde Kalu did not lead the Harbor trial");
    assert!(!out.facts.iter().any(|fact| fact.id == 1 && !fact.ended));
}

// ------------------------------------------------------------- H3-11

/// Atlas used npm, then switched to pnpm: the npm fact is ended. Eight other
/// Atlas tools fill the fact channels so the ended row competes for a slot.
fn atlas_switched_to_pnpm(e: &Engine) {
    for (i, tool_name) in [
        "Vitest",
        "Playwright",
        "Docker",
        "Terraform",
        "Redis",
        "Postgres",
        "Sentry",
        "Renovate",
    ]
    .iter()
    .enumerate()
    {
        e.remember(RememberInput {
            content: format!("Atlas depends on {tool_name} in its build pipeline."),
            valid_from: Some(format!("2025-0{}-01", i % 9 + 1)),
            entities: vec![project("Atlas"), tool(tool_name)],
            relations: vec![rel("Atlas", "depends on", tool_name, false)],
            ..Default::default()
        })
        .unwrap();
    }
    e.remember(RememberInput {
        content: "The Atlas repo uses npm for package management.".into(),
        valid_from: Some("2025-03-01".into()),
        entities: vec![project("Atlas"), tool("npm")],
        relations: vec![rel("Atlas", "uses", "npm", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Decision on 2026-02-10: switched Atlas from npm to pnpm because installs were slow. npm is no longer used in Atlas.".into(),
        valid_from: Some("2026-02-10".into()),
        entities: vec![project("Atlas"), tool("pnpm")],
        relations: vec![rel("Atlas", "uses", "pnpm", true)],
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn h3_11_still_question_shows_the_ended_object_with_its_end_date() {
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is npm still used in atlas");
    let shown = text(&e, &out);
    let npm_line = shown
        .lines()
        .find(|l| l.contains("→ npm"))
        .unwrap_or_else(|| panic!("no npm fact in the answer:\n{shown}"));
    assert!(
        npm_line.contains("ended"),
        "npm fact not marked ended: {npm_line}"
    );
    assert!(
        npm_line.contains("2026-02-10"),
        "end date missing: {npm_line}"
    );
}

#[test]
fn h3_11_still_question_never_shows_a_live_fact_as_ended() {
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is pnpm still used in atlas");
    let shown = text(&e, &out);
    let pnpm_line = shown
        .lines()
        .find(|l| l.contains("→ pnpm"))
        .unwrap_or_else(|| panic!("no pnpm fact in the answer:\n{shown}"));
    assert!(
        !pnpm_line.contains("ended"),
        "live pnpm fact shown as ended: {pnpm_line}"
    );
}

#[test]
fn h3_11_question_naming_only_the_subject_is_unchanged() {
    // The channel needs both ends: "is npm still used" names no subject.
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is npm still used");
    let shown = text(&e, &out);
    assert!(
        !shown
            .lines()
            .any(|l| l.contains("→ npm") && l.contains("ended")),
        "a subject-only question showed the ended fact:\n{shown}"
    );
}

#[test]
fn h3_11_question_without_still_is_unchanged() {
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is npm used in atlas");
    let shown = text(&e, &out);
    assert!(
        !shown
            .lines()
            .any(|l| l.contains("→ npm") && l.contains("ended")),
        "a question without a status cue showed the ended fact:\n{shown}"
    );
}
