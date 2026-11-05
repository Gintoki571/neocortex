//! Exclusive slots end only the fact they replace (hunt3 H3-01, H3-02, H3-03,
//! H3-07). Subject-keyed slots (seeded "lives in", "works at", ...) still end
//! every other live value; an agent-declared exclusive predicate ends a value
//! only when the text names it as replaced; a one-owner predicate ("leads",
//! "owns") is keyed on the object.

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, RelationIn, RememberInput},
    temporal::Clock,
};
use rusqlite::OptionalExtension;
use std::sync::Arc;

const NOW: i64 = 1_700_000_000;

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
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

/// Write one memory dated `day` (undated when None) with its relations.
fn write(e: &Engine, content: &str, day: Option<&str>, relations: Vec<RelationIn>) {
    e.remember(RememberInput {
        content: content.into(),
        valid_from: day.map(Into::into),
        relations,
        ..Default::default()
    })
    .unwrap();
}

/// Is `src predicate dst` still live (no end date, not retracted)?
fn current(e: &Engine, src: &str, canonical: &str, dst: &str) -> bool {
    let n: i64 =
        e.db.sql()
            .query_row(
                "SELECT COUNT(*) FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             JOIN entities es ON es.id=r.src
             JOIN entities ed ON ed.id=r.dst
             WHERE LOWER(es.name)=LOWER(?1) AND LOWER(ed.name)=LOWER(?2) AND p.canonical=?3
               AND r.polarity=1 AND r.valid_to IS NULL AND r.expired_at IS NULL",
                rusqlite::params![src, dst, canonical],
                |r| r.get(0),
            )
            .unwrap();
    n > 0
}

/// Is any live relation from `src` to `dst`, whatever the predicate?
fn live_to(e: &Engine, src: &str, dst: &str) -> bool {
    let n: i64 =
        e.db.sql()
            .query_row(
                "SELECT COUNT(*) FROM relations r
             JOIN entities es ON es.id=r.src
             JOIN entities ed ON ed.id=r.dst
             WHERE LOWER(es.name)=LOWER(?1) AND LOWER(ed.name)=LOWER(?2)
               AND r.polarity=1 AND r.valid_to IS NULL AND r.expired_at IS NULL",
                rusqlite::params![src, dst],
                |r| r.get(0),
            )
            .unwrap();
    n > 0
}

/// The stored end of `src predicate dst`, when the row exists at all.
fn valid_to(e: &Engine, src: &str, canonical: &str, dst: &str) -> Option<Option<i64>> {
    e.db.sql()
        .query_row(
            "SELECT r.valid_to FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             JOIN entities es ON es.id=r.src
             JOIN entities ed ON ed.id=r.dst
             WHERE LOWER(es.name)=LOWER(?1) AND LOWER(ed.name)=LOWER(?2) AND p.canonical=?3
             ORDER BY r.id DESC LIMIT 1",
            rusqlite::params![src, dst, canonical],
            |r| r.get(0),
        )
        .optional()
        .unwrap()
}

/// Entity id of `name` (the write path stores the name it was given).
fn eid(e: &Engine, name: &str) -> i64 {
    e.db.sql()
        .query_row("SELECT id FROM entities WHERE name=?1", [name], |r| {
            r.get(0)
        })
        .unwrap()
}

fn epoch(day: &str) -> i64 {
    neocortex::temporal::parse_instant(
        day,
        &Clock {
            now: NOW,
            tz_offset_secs: 0,
        },
    )
    .unwrap()
}

#[test]
fn h3_01_tools_of_different_kinds_stay_current() {
    let e = engine();
    write(
        &e,
        "Atlas uses npm.",
        Some("2026-01-10"),
        vec![rel("Atlas", "uses", "npm", true)],
    );
    write(
        &e,
        "Atlas uses ESLint.",
        Some("2026-02-10"),
        vec![rel("Atlas", "uses", "ESLint", true)],
    );
    write(
        &e,
        "Atlas uses Jest.",
        Some("2026-03-01"),
        vec![rel("Atlas", "uses", "Jest", true)],
    );
    assert!(current(&e, "Atlas", "use", "npm"));
    assert!(current(&e, "Atlas", "use", "eslint"));
    assert!(current(&e, "Atlas", "use", "jest"));
}

#[test]
fn h3_01_text_naming_the_old_tool_as_replaced_ends_it() {
    let e = engine();
    write(
        &e,
        "Atlas uses npm.",
        Some("2026-01-10"),
        vec![rel("Atlas", "uses", "npm", true)],
    );
    write(
        &e,
        "Atlas switched from npm to pnpm for installs.",
        Some("2026-02-01"),
        vec![rel("Atlas", "uses", "pnpm", true)],
    );
    assert!(!current(&e, "Atlas", "use", "npm"));
    assert!(current(&e, "Atlas", "use", "pnpm"));
}

#[test]
fn h3_03_removed_tool_named_in_text_is_retired() {
    let e = engine();
    write(
        &e,
        "Atlas uses Jest.",
        Some("2026-01-10"),
        vec![rel("Atlas", "uses", "Jest", true)],
    );
    write(
        &e,
        "Atlas switched from Jest to Vitest for unit tests and Jest was removed.",
        Some("2026-03-15"),
        vec![rel("Atlas", "uses", "Vitest", true)],
    );
    assert!(!current(&e, "Atlas", "use", "jest"));
    assert!(current(&e, "Atlas", "use", "vitest"));
}

#[test]
fn h3_02_undated_write_does_not_end_a_later_dated_fact() {
    let e = engine();
    write(
        &e,
        "Atlas uses Biome.",
        Some("2026-04-02"),
        vec![rel("Atlas", "uses", "Biome", true)],
    );
    // Undated: stamped at ingest, which is before the April fact.
    write(
        &e,
        "Atlas uses npm.",
        None,
        vec![rel("Atlas", "uses", "npm", true)],
    );
    assert!(current(&e, "Atlas", "use", "biome"));
    assert!(current(&e, "Atlas", "use", "npm"));
}

#[test]
fn h3_02_dated_replacement_retires_an_undated_fact_instead_of_backfilling() {
    let e = engine();
    // Undated: its start is the ingest stamp (NOW), later than the dated write.
    write(
        &e,
        "Atlas uses Jest.",
        None,
        vec![rel("Atlas", "uses", "Jest", true)],
    );
    write(
        &e,
        "Atlas switched from Jest to Vitest for unit tests and Jest was removed.",
        Some("2023-06-01"),
        vec![rel("Atlas", "uses", "Vitest", true)],
    );
    assert!(!current(&e, "Atlas", "use", "jest"));
    assert!(current(&e, "Atlas", "use", "vitest"));
}

#[test]
fn h3_07_new_leader_ends_the_old_leader_of_the_same_project() {
    let e = engine();
    write(
        &e,
        "Sasha Lindqvist leads the portal project.",
        Some("2026-01-14"),
        vec![rel("Sasha Lindqvist", "leads", "portal project", true)],
    );
    write(
        &e,
        "Lars Haugen leads the portal project.",
        Some("2026-04-03"),
        vec![rel("Lars Haugen", "leads", "portal project", true)],
    );
    assert!(!current(&e, "Sasha Lindqvist", "lead", "portal project"));
    assert!(current(&e, "Lars Haugen", "lead", "portal project"));
}

#[test]
fn h3_07_control_one_lead_for_two_projects_both_stay_current() {
    let e = engine();
    write(
        &e,
        "Sam leads team A.",
        Some("2026-01-10"),
        vec![rel("Sam", "leads", "team A", true)],
    );
    write(
        &e,
        "Sam leads team B.",
        Some("2026-02-10"),
        vec![rel("Sam", "leads", "team B", true)],
    );
    assert!(current(&e, "Sam", "lead", "team a"));
    assert!(current(&e, "Sam", "lead", "team b"));
}

#[test]
fn control_agent_exclusive_non_use_predicate_still_supersedes() {
    let e = engine();
    write(
        &e,
        "Rita hosts the club at Biblioteca.",
        Some("2019-01-01"),
        vec![rel("Rita", "hosts the club at", "Biblioteca", true)],
    );
    write(
        &e,
        "Since 2026 Rita hosts the club at Cafe Central.",
        Some("2026-01-01"),
        vec![rel("Rita", "hosts the club at", "Cafe Central", true)],
    );
    assert!(!live_to(&e, "Rita", "biblioteca"));
    assert!(live_to(&e, "Rita", "cafe central"));
}

#[test]
fn control_subject_slot_still_ends_the_old_value() {
    let e = engine();
    write(
        &e,
        "Alice lives in Delhi.",
        Some("2026-01-10"),
        vec![rel("Alice", "lives in", "Delhi", true)],
    );
    write(
        &e,
        "Alice lives in Pune.",
        Some("2026-03-01"),
        vec![rel("Alice", "lives in", "Pune", true)],
    );
    assert!(!current(&e, "Alice", "live in", "delhi"));
    assert!(current(&e, "Alice", "live in", "pune"));
}

/// A dated historical write ("Ann lived in Rome in 2019") says nothing about
/// now: it is history up to the ingest stamp of the undated fact, and the
/// undated fact stays current. Retiring Paris here was a false retirement.
#[test]
fn dated_history_never_retires_the_current_fact() {
    let e = engine();
    write(
        &e,
        "Ann lives in Paris.",
        None,
        vec![rel("Ann", "lives in", "Paris", true)],
    );
    write(
        &e,
        "Ann lived in Rome in 2019.",
        Some("2019-01-01"),
        vec![rel("Ann", "lives in", "Rome", true)],
    );
    assert!(current(&e, "Ann", "live in", "paris"));
    assert!(!current(&e, "Ann", "live in", "rome"));
    assert_eq!(
        valid_to(&e, "Ann", "live in", "rome"),
        Some(Some(NOW)),
        "Rome is history up to the stamp of the undated Paris fact"
    );
}

/// A closed historical interval can never replace a current fact, and keeps
/// its own end.
#[test]
fn closed_history_never_retires_the_current_fact() {
    let e = engine();
    write(
        &e,
        "Ann lives in Paris.",
        None,
        vec![rel("Ann", "lives in", "Paris", true)],
    );
    let mut history = rel("Ann", "lives in", "Rome", true);
    history.valid_from = Some("2019-01-01".into());
    history.valid_to = Some("2021-01-01".into());
    write(
        &e,
        "Ann lived in Rome from 2019 to 2021.",
        None,
        vec![history],
    );
    assert!(current(&e, "Ann", "live in", "paris"));
    assert_eq!(
        valid_to(&e, "Ann", "live in", "rome"),
        Some(Some(epoch("2021-01-01")))
    );
}

/// The same for an object-keyed slot: a dated past leadership does not end the
/// current leader.
#[test]
fn dated_past_leadership_never_retires_the_current_leader() {
    let e = engine();
    write(
        &e,
        "Sasha leads the portal.",
        None,
        vec![rel("Sasha", "leads", "portal", true)],
    );
    let mut past = rel("Lars", "leads", "portal", true);
    past.valid_from = Some("2022-01-01".into());
    past.valid_to = Some("2023-01-01".into());
    write(
        &e,
        "Lars led the portal from 2022 to 2023.",
        None,
        vec![past],
    );
    assert!(current(&e, "Sasha", "lead", "portal"));
    assert_eq!(
        valid_to(&e, "Lars", "lead", "portal"),
        Some(Some(epoch("2023-01-01")))
    );
}

/// A merge is not evidence about an unrelated slot: `lead` is keyed on the
/// project and `use` is multi-valued, so a merge elsewhere changes nothing.
#[test]
fn a_merge_does_not_reapply_subject_slots() {
    let e = engine();
    write(
        &e,
        "Sam leads Project Alpha.",
        Some("2026-01-10"),
        vec![rel("Sam", "leads", "Project Alpha", true)],
    );
    write(
        &e,
        "Sam leads Project Beta.",
        Some("2026-02-10"),
        vec![rel("Sam", "leads", "Project Beta", true)],
    );
    write(
        &e,
        "Kim uses npm.",
        Some("2026-01-10"),
        vec![rel("Kim", "uses", "npm", true)],
    );
    write(
        &e,
        "Kim uses ESLint.",
        Some("2026-02-10"),
        vec![rel("Kim", "uses", "ESLint", true)],
    );
    write(
        &e,
        "Zed Qux works at Acme.",
        Some("2026-01-10"),
        vec![rel("Zed Qux", "works at", "Acme", false)],
    );
    write(
        &e,
        "Mira Vance works at Acme.",
        Some("2026-01-10"),
        vec![rel("Mira Vance", "works at", "Acme", false)],
    );
    let keep = eid(&e, "Zed Qux");
    let gone = eid(&e, "Mira Vance");
    e.merge_entities(&format!("e{keep}"), &format!("e{gone}"), false)
        .unwrap();
    assert!(current(&e, "Sam", "lead", "project alpha"));
    assert!(current(&e, "Sam", "lead", "project beta"));
    assert!(current(&e, "Kim", "use", "npm"));
    assert!(current(&e, "Kim", "use", "eslint"));
}

/// A generic object is not a one-owner slot: two people can own a laptop.
#[test]
fn a_shared_generic_object_is_not_a_slot() {
    let e = engine();
    write(
        &e,
        "Alex owns a laptop.",
        Some("2026-01-10"),
        vec![rel("Alex", "owns", "laptop", true)],
    );
    write(
        &e,
        "Blake owns a laptop.",
        Some("2026-02-10"),
        vec![rel("Blake", "owns", "laptop", true)],
    );
    assert!(current(&e, "Alex", "own", "laptop"));
    assert!(current(&e, "Blake", "own", "laptop"));
}

/// A named, definite object is: the repo has one owner.
#[test]
fn a_named_object_is_a_slot() {
    let e = engine();
    write(
        &e,
        "Ann owns the repo.",
        Some("2026-01-10"),
        vec![rel("Ann", "owns", "repo", true)],
    );
    write(
        &e,
        "Bo owns the repo.",
        Some("2026-02-10"),
        vec![rel("Bo", "owns", "repo", true)],
    );
    assert!(!current(&e, "Ann", "own", "repo"));
    assert!(current(&e, "Bo", "own", "repo"));
}
