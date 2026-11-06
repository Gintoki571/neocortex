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

fn place(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("place".into()),
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

fn group(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("group".into()),
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

/// Ingest a "leads the team" relation and return its id.
fn tomasz_leads_team(e: &Engine) -> i64 {
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak leads the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            relations: vec![rel("Tomasz Nowak", "leads", "team", false)],
            ..Default::default()
        })
        .unwrap();
    out.relations[0].id
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

// ------------------------------------------------------------- H3-12

#[test]
fn h3_12_approves_does_not_end_reviews() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer approves payment-adjacent code.");
    assert!(
        !out.invalidated_relations.contains(&rid),
        "approves ended reviews fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_no_longer_only_ends_nothing() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(
        &e,
        "Tomasz Nowak no longer only reviews payment-adjacent code.",
    );
    assert!(
        !out.invalidated_relations.contains(&rid),
        "no longer only ended fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_same_verb_reviews_ends_fact() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer reviews payment-adjacent code.");
    assert!(
        out.invalidated_relations.contains(&rid),
        "same verb reviews failed to end fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_stopped_using_npm_ends_uses_npm() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Atlas uses npm for building packages.".into(),
            entities: vec![project("Atlas"), tool("npm")],
            relations: vec![rel("Atlas", "uses", "npm", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Atlas stopped using npm.".into(),
            entities: vec![project("Atlas"), tool("npm")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "stopped using did not end uses: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_left_acme_ends_works_at_acme() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Ivan left Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "left did not end works at: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_no_longer_lives_in_pune_ends_lives_in_pune() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Carol lives in Pune.".into(),
            entities: vec![person("Carol"), place("Pune")],
            relations: vec![rel("Carol", "lives in", "Pune", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Carol no longer lives in Pune.".into(),
            entities: vec![person("Carol"), place("Pune")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "no longer lives in did not end lives in: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_no_longer_works_with_does_not_end_works_at() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;

    let out2 = e
        .remember(RememberInput {
            content: "Ivan no longer works with Ana.".into(),
            entities: vec![person("Ivan"), person("Ana")],
            ..Default::default()
        })
        .unwrap();
    assert!(!out2.invalidated_relations.contains(&rid));
    assert_eq!(relation_end(&e, rid), None);

    let out3 = e
        .remember(RememberInput {
            content: "Ivan no longer works with Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        !out3.invalidated_relations.contains(&rid),
        "works with ended works at: {out3:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_retraction_with_relations_passed_by_caller_is_unchanged() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid1 = out1.relations[0].id;

    let out2 = e
        .remember(RememberInput {
            content: "Ivan joined Globex; he left Acme.".into(),
            entities: vec![person("Ivan"), org("Globex"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Globex", true)],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid1),
        "old relation not ended: {out2:?}"
    );
    let rid2 = out2.relations[0].id;
    assert!(
        !out2.invalidated_relations.contains(&rid2),
        "new relation ended itself: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid1), Some(NOW));
    assert_eq!(relation_end(&e, rid2), None);
}

/// S01: "heads" is outside the predicate ontology (it is also a role noun), so
/// the retraction matcher carries the heads/leads synonym pair explicitly.
#[test]
fn h3_12_no_longer_heads_ends_leads() {
    let e = engine();
    let rid = tomasz_leads_team(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer heads the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "heads did not end leads: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// R02: the same synonym in the past tense.
#[test]
fn h3_12_no_longer_headed_ends_leads() {
    let e = engine();
    let rid = tomasz_leads_team(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer headed the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "headed did not end leads: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// R01: the same verb still ends, so the synonym path did not replace it.
#[test]
fn h3_12_no_longer_leads_ends_leads() {
    let e = engine();
    let rid = tomasz_leads_team(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer leads the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "leads did not end leads: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// X01: an -ly adverb between the cue and the verb is not part of the verb.
#[test]
fn h3_12_adverb_between_cue_and_verb_ends() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer actively reviews payment-adjacent code.".into(),
            entities: vec![person("Tomasz Nowak")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "adverb hid the verb: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// X02: the adverb is stripped, the preposition still tells "works at" apart.
#[test]
fn h3_12_no_longer_works_remotely_ends_works_at() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Ivan no longer works remotely at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "remotely hid works at: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// N02: "no longer just X" negates exclusivity, like "no longer only X".
#[test]
fn h3_12_no_longer_just_does_not_end() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer just reviews payment-adjacent code.".into(),
            entities: vec![person("Tomasz Nowak")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        !out.invalidated_relations.contains(&rid),
        "no longer just ended the fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

/// The run/manage synonym pair: a second entry in the table, and its negative
/// side, so the table cannot silently over-match.
#[test]
fn h3_12_no_longer_runs_ends_manages() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Tomasz Nowak manages the platform team.".into(),
            entities: vec![person("Tomasz Nowak"), group("platform team")],
            relations: vec![rel("Tomasz Nowak", "manages", "platform team", false)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer runs the platform team.".into(),
            entities: vec![person("Tomasz Nowak"), group("platform team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "runs did not end manages: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// A synonym pair never matches outside itself: "oversees" is paired with
/// "supervises", so it must not end "reviews".
#[test]
fn h3_12_oversees_does_not_end_reviews() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer oversees payment-adjacent code.");
    assert!(
        !out.invalidated_relations.contains(&rid),
        "oversees ended reviews: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_coordinated_pair_retraction_unchanged() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Acme Corp and Northwind are partners since 2023.".into(),
            entities: vec![org("Acme Corp"), org("Northwind")],
            relations: vec![rel("Acme Corp", "partners with", "Northwind", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Acme Corp and Northwind are no longer partners.".into(),
            entities: vec![org("Acme Corp"), org("Northwind")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "pair partners not ended: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}
