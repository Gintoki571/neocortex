//! Temporal text semantics (lane nc-f3): text-only retraction (TM-10),
//! planned and recurring memories (TM-11), ordering/duration answers (TM-12)
//! and date literals that must never become entities (TM-13).

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput},
    extract, render,
    temporal::{days_from_civil, Clock},
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

fn day(y: i64, m: u32, d: u32) -> i64 {
    days_from_civil(y, m, d) * 86_400
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

fn recall(e: &Engine, query: &str) -> neocortex::engine::RecallOutput {
    e.recall(&RecallInput {
        query: query.into(),
        ..Default::default()
    })
    .unwrap()
}

fn recall_as_of(e: &Engine, query: &str, as_of: &str) -> neocortex::engine::RecallOutput {
    e.recall(&RecallInput {
        query: query.into(),
        as_of: Some(as_of.into()),
        ..Default::default()
    })
    .unwrap()
}

fn text(e: &Engine, out: &neocortex::engine::RecallOutput) -> String {
    render::render_recall(out, &render::clock_fmt(&e.clock()))
}

fn tags(e: &Engine, id: i64) -> String {
    e.db.sql()
        .query_row("SELECT tags FROM memories WHERE id=?1", [id], |r| r.get(0))
        .unwrap()
}

fn valid_from(e: &Engine, id: i64) -> Option<i64> {
    e.db.sql()
        .query_row("SELECT valid_from FROM memories WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
}

fn relation_end(e: &Engine, rid: i64) -> Option<i64> {
    e.db.sql()
        .query_row("SELECT valid_to FROM relations WHERE id=?1", [rid], |r| {
            r.get(0)
        })
        .unwrap()
}

// ---------------------------------------------------------------- TM-10

fn ivan_works_at_acme(e: &Engine) -> i64 {
    let out = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            valid_from: Some("2020-01-01".into()),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    out.relations[0].id
}

#[test]
fn tm10_text_only_retraction_ends_the_work_relation() {
    let e = engine();
    let rid = ivan_works_at_acme(&e);
    let out = e
        .remember(RememberInput {
            content: "Ivan left Acme; he no longer works there.".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "retraction was not reported: {out:?}"
    );
    assert_eq!(
        relation_end(&e, rid),
        Some(NOW),
        "relation not ended at now"
    );
    let rendered = render::render_remember(&out);
    assert!(
        rendered.contains(&format!("invalidated: r{rid}")),
        "{rendered}"
    );
    // The old fact is history, not current truth.
    let live: i64 =
        e.db.sql()
            .query_row(
                "SELECT count(*) FROM relations WHERE id=?1 AND expired_at IS NULL
               AND (valid_to IS NULL OR valid_to>?2)",
                rusqlite::params![rid, NOW],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(live, 0);
}

#[test]
fn tm10_residence_and_partner_retractions_use_their_group() {
    let e = engine();
    let live = e
        .remember(RememberInput {
            content: "Alice lives in Austin.".into(),
            valid_from: Some("2020-01-01".into()),
            entities: vec![person("Alice"), place("Austin")],
            relations: vec![rel("Alice", "lives in", "Austin", true)],
            ..Default::default()
        })
        .unwrap();
    let married = e
        .remember(RememberInput {
            content: "Alice is married to Bob.".into(),
            valid_from: Some("2018-01-01".into()),
            entities: vec![person("Alice"), person("Bob")],
            relations: vec![rel("Alice", "married to", "Bob", true)],
            ..Default::default()
        })
        .unwrap();
    let out = e
        .remember(RememberInput {
            content: "Alice moved out of Austin and broke up with Bob.".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(out.invalidated_relations.contains(&live.relations[0].id));
    assert!(out.invalidated_relations.contains(&married.relations[0].id));
}

#[test]
fn tm10_a_retraction_only_ends_its_own_group() {
    let e = engine();
    let work = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            valid_from: Some("2020-01-01".into()),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let residence = e
        .remember(RememberInput {
            content: "Ivan lives in Acme.".into(),
            valid_from: Some("2020-01-01".into()),
            entities: vec![person("Ivan"), place("Acme")],
            relations: vec![rel("Ivan", "lives in", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let out = e
        .remember(RememberInput {
            content: "Ivan left Acme.".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(out.invalidated_relations.contains(&work.relations[0].id));
    assert!(!out
        .invalidated_relations
        .contains(&residence.relations[0].id));
    assert_eq!(relation_end(&e, residence.relations[0].id), None);
}

#[test]
fn tm10_ambiguous_retraction_reports_conflicts_and_ends_nothing() {
    let e = engine();
    e.remember(RememberInput {
        content: "Ivan works at Acme.".into(),
        valid_from: Some("2020-01-01".into()),
        entities: vec![person("Ivan"), org("Acme")],
        relations: vec![rel("Ivan", "works at", "Acme", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Ivan lives in Acme.".into(),
        valid_from: Some("2020-01-01".into()),
        entities: vec![person("Ivan"), place("Acme")],
        relations: vec![rel("Ivan", "lives in", "Acme", true)],
        ..Default::default()
    })
    .unwrap();
    // The generic "is no longer" cue applies to any group, so two live
    // relations match the pair: report them, end nothing.
    let out = e
        .remember(RememberInput {
            content: "Ivan is no longer at Acme.".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(out.invalidated_relations.is_empty(), "{out:?}");
    assert_eq!(out.conflicts.len(), 2, "{out:?}");
    let ends: i64 =
        e.db.sql()
            .query_row(
                "SELECT count(*) FROM relations WHERE valid_to IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(ends, 0);
}

#[test]
fn tm10_no_matching_relation_is_a_quiet_no_op() {
    let e = engine();
    let rid = ivan_works_at_acme(&e);
    let out = e
        .remember(RememberInput {
            content: "Ivan left Globex.".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(out.invalidated_relations.is_empty(), "{out:?}");
    assert_eq!(relation_end(&e, rid), None);
}

/// The world the review table runs against: Ivan at Acme, Maria married to
/// Tom, Ivan in Bergen.
struct World {
    work: i64,
    married: i64,
    residence: i64,
}

fn retraction_world() -> (Engine, World) {
    let e = engine();
    let work = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            valid_from: Some("2020-01-01".into()),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap()
        .relations[0]
        .id;
    let married = e
        .remember(RememberInput {
            content: "Maria is married to Tom.".into(),
            valid_from: Some("2018-01-01".into()),
            entities: vec![person("Maria"), person("Tom")],
            relations: vec![rel("Maria", "married to", "Tom", true)],
            ..Default::default()
        })
        .unwrap()
        .relations[0]
        .id;
    let residence = e
        .remember(RememberInput {
            content: "Ivan lives in Bergen.".into(),
            valid_from: Some("2019-01-01".into()),
            entities: vec![person("Ivan"), place("Bergen")],
            relations: vec![rel("Ivan", "lives in", "Bergen", true)],
            ..Default::default()
        })
        .unwrap()
        .relations[0]
        .id;
    (
        e,
        World {
            work,
            married,
            residence,
        },
    )
}

fn plain(e: &Engine, content: &str) -> neocortex::engine::RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        ..Default::default()
    })
    .unwrap()
}

/// A wrongly ended fact disappears from current recall, so every "not ended"
/// row also checks the relation is still open.
fn assert_kept(e: &Engine, out: &neocortex::engine::RememberOutput, rid: i64) {
    assert!(
        !out.invalidated_relations.contains(&rid),
        "relation r{rid} was ended: {out:?}"
    );
    assert_eq!(
        relation_end(e, rid),
        None,
        "relation r{rid} got an end date"
    );
}

fn assert_ended(out: &neocortex::engine::RememberOutput, rid: i64) {
    assert!(
        out.invalidated_relations.contains(&rid),
        "relation r{rid} was not ended: {out:?}"
    );
}

#[test]
fn tm10b_a_determiner_in_the_gap_is_not_a_retraction() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan left the Acme office at 5pm to pick up his kids.");
    assert_kept(&e, &out, w.work);
}

#[test]
fn tm10b_a_ditransitive_left_is_not_a_retraction() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan left Acme a voicemail about the invoice.");
    assert_kept(&e, &out, w.work);
}

#[test]
fn tm10b_a_possessive_object_is_not_a_retraction() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan left Acme's parking lot after the meeting.");
    assert_kept(&e, &out, w.work);
}

#[test]
fn tm10b_a_hedged_near_miss_is_a_conflict_candidate() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan almost quit Acme last year but stayed.");
    assert_kept(&e, &out, w.work);
    assert!(
        out.conflicts.iter().any(|c| c.relation_id == w.work),
        "hedged retraction was not reported as a candidate: {out:?}"
    );
}

#[test]
fn tm10b_a_negated_left_is_not_a_retraction() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan did not leave Acme despite the offer from Globex.");
    assert_kept(&e, &out, w.work);
}

#[test]
fn tm10b_a_reported_belief_is_not_a_retraction() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan thinks Bob left Acme.");
    assert_kept(&e, &out, w.work);
}

#[test]
fn tm10b_left_a_person_at_the_station_is_not_a_retraction() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Maria left Tom at the station and went home.");
    assert_kept(&e, &out, w.married);
}

#[test]
fn tm10b_left_a_place_for_a_purpose_is_not_a_retraction() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan left Bergen for a two-day conference in Oslo.");
    assert_kept(&e, &out, w.residence);
}

#[test]
fn tm10b_left_acme_with_a_time_expression_ends() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan left Acme last month and joined Globex.");
    assert_ended(&out, w.work);
}

#[test]
fn tm10b_no_longer_works_at_ends() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan no longer works at Acme.");
    assert_ended(&out, w.work);
}

#[test]
fn tm10b_moved_out_of_ends() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Ivan moved out of Bergen in June.");
    assert_ended(&out, w.residence);
}

#[test]
fn tm10b_divorced_ends_the_marriage() {
    let (e, w) = retraction_world();
    let out = plain(&e, "Maria divorced Tom in 2024.");
    assert_ended(&out, w.married);
}

#[test]
fn tm10b_split_and_separated_end_the_marriage() {
    for content in ["Maria split up with Tom.", "Maria separated from Tom."] {
        let (e, w) = retraction_world();
        let out = plain(&e, content);
        assert_ended(&out, w.married);
    }
}

// ---------------------------------------------------------------- TM-11

#[test]
fn tm11_plan_without_a_date_is_tagged_and_not_current() {
    let e = engine();
    let out = e
        .remember(RememberInput {
            content: "We will move to Lisbon next spring.".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(valid_from(&e, out.id), Some(NOW));
    assert!(tags(&e, out.id).contains("planned"), "{}", tags(&e, out.id));

    let current = recall(&e, "Lisbon move");
    assert!(
        current.memories.iter().all(|m| m.id != out.id),
        "a plan without a date counted as current: {:?}",
        current.memories
    );
    let planned = recall(&e, "what are our plans for Lisbon");
    let hit = planned
        .memories
        .iter()
        .find(|m| m.id == out.id)
        .unwrap_or_else(|| panic!("plan not recallable by plan intent: {planned:?}"));
    assert!(
        text(&e, &planned).contains("[planned]"),
        "{}",
        text(&e, &planned)
    );
    assert!(hit.tags.iter().any(|t| t == "planned"));
}

#[test]
fn tm11_plan_with_a_resolvable_date_starts_then() {
    let e = engine();
    let out = e
        .remember(RememberInput {
            content: "We will move to Lisbon in June 2027.".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(valid_from(&e, out.id), Some(day(2027, 6, 1)));
    assert!(tags(&e, out.id).contains("planned"));
    // Not current before it happens.
    let before = recall(&e, "where do we live in Lisbon");
    assert!(before.memories.iter().all(|m| m.id != out.id), "{before:?}");
    // It becomes current knowledge on its date.
    let after = recall_as_of(&e, "where do we live in Lisbon", "2027-07-01");
    assert!(
        after.memories.iter().any(|m| m.id == out.id),
        "future plan missing from as-of view: {after:?}"
    );
}

#[test]
fn tm11_in_n_units_is_a_future_start() {
    let e = engine();
    let out = e
        .remember(RememberInput {
            content: "We will ship the beta in 3 days.".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(valid_from(&e, out.id), Some(day(2026, 10, 26)));
    assert!(tags(&e, out.id).contains("planned"));
}

#[test]
fn tm11_planned_relation_is_not_a_current_fact() {
    let e = engine();
    e.remember(RememberInput {
        content: "Alice will move to Lisbon in June 2027.".into(),
        entities: vec![person("Alice"), place("Lisbon")],
        relations: vec![rel("Alice", "lives in", "Lisbon", true)],
        ..Default::default()
    })
    .unwrap();
    let profile = e.entity("Alice", 20).unwrap();
    assert!(
        profile.current_facts.iter().all(|f| f.target != "Lisbon"),
        "planned fact shown as current: {:?}",
        profile.current_facts
    );
    let out = recall(&e, "where does Alice live");
    assert!(out.facts.iter().all(|f| f.target != "Lisbon"), "{out:?}");
    assert!(out.memories.iter().all(|m| m.id != 1), "{out:?}");
}

#[test]
fn tm11_recurring_rule_is_tagged_and_kept_as_text() {
    let e = engine();
    let out = e
        .remember(RememberInput {
            content: "Team standup every Monday at 09:30.".into(),
            ..Default::default()
        })
        .unwrap();
    let t = tags(&e, out.id);
    assert!(t.contains("recurring"), "{t}");
    assert!(!t.contains("planned"), "{t}");
    assert_eq!(valid_from(&e, out.id), Some(NOW));
    let found = recall(&e, "when is the team standup");
    let hit = found
        .memories
        .iter()
        .find(|m| m.id == out.id)
        .unwrap_or_else(|| panic!("recurring rule not found: {found:?}"));
    assert!(hit.content.contains("every Monday at 09:30"), "{hit:?}");
    assert!(
        text(&e, &found).contains("[recurring]"),
        "{}",
        text(&e, &found)
    );
}

#[test]
fn tm11_weekly_and_monthly_cues_are_recurring() {
    let e = engine();
    for (i, content) in ["Weekly sync on Fridays.", "The report goes out monthly."]
        .iter()
        .enumerate()
    {
        let out = e
            .remember(RememberInput {
                content: (*content).into(),
                ..Default::default()
            })
            .unwrap();
        assert!(
            tags(&e, out.id).contains("recurring"),
            "case {i}: {}",
            tags(&e, out.id)
        );
    }
}

// ---------------------------------------------------------------- TM-12

#[test]
fn tm12_ordering_question_sorts_memories_by_valid_from() {
    let e = engine();
    e.remember(RememberInput {
        content: "Alice moved to Lisbon.".into(),
        valid_from: Some("2020-01-01".into()),
        entities: vec![person("Alice"), place("Lisbon")],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Alice started a new job at Acme.".into(),
        valid_from: Some("2022-06-01".into()),
        entities: vec![person("Alice"), org("Acme")],
        ..Default::default()
    })
    .unwrap();
    let out = recall(
        &e,
        "what happened first, the move to Lisbon or the new job at Acme?",
    );
    let ids: Vec<i64> = out.memories.iter().map(|m| m.id).collect();
    let first = ids.iter().position(|id| *id == 1);
    let second = ids.iter().position(|id| *id == 2);
    assert!(
        first.is_some() && second.is_some() && first < second,
        "not chronological: {ids:?}"
    );
    let rendered = text(&e, &out);
    assert!(
        rendered.find("[m1]").unwrap() < rendered.find("[m2]").unwrap(),
        "{rendered}"
    );
}

#[test]
fn tm12_duration_is_computed_from_the_relation_interval() {
    let e = engine();
    e.remember(RememberInput {
        content: "Alice worked at Acme.".into(),
        valid_from: Some("2020-01-01".into()),
        valid_to: Some("2023-03-01".into()),
        entities: vec![person("Alice"), org("Acme")],
        relations: vec![rel("Alice", "works at", "Acme", true)],
        ..Default::default()
    })
    .unwrap();
    let out = recall(&e, "how long did Alice work at Acme?");
    let rendered = text(&e, &out);
    assert!(
        rendered.contains("3 years 2 months"),
        "duration missing from\n{rendered}"
    );
    assert!(
        rendered.contains("2020-01-01 → 2023-03-01"),
        "interval missing from\n{rendered}"
    );
}

#[test]
fn tm12_ongoing_duration_says_since_when() {
    let e = engine();
    e.remember(RememberInput {
        content: "Alice works at Acme.".into(),
        valid_from: Some("2023-04-01".into()),
        entities: vec![person("Alice"), org("Acme")],
        relations: vec![rel("Alice", "works at", "Acme", true)],
        ..Default::default()
    })
    .unwrap();
    let out = recall(&e, "since when has Alice worked at Acme?");
    let rendered = text(&e, &out);
    assert!(
        rendered.contains("ongoing since 2023-04-01"),
        "ongoing duration missing from\n{rendered}"
    );
}

#[test]
fn tm12_non_temporal_questions_keep_relevance_order() {
    let e = engine();
    e.remember(RememberInput {
        content: "Alice moved to Lisbon.".into(),
        valid_from: Some("2020-01-01".into()),
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Alice started a new job at Acme.".into(),
        valid_from: Some("2022-06-01".into()),
        ..Default::default()
    })
    .unwrap();
    let out = recall(&e, "Alice");
    let scores: Vec<f64> = out.memories.iter().map(|m| m.score).collect();
    assert!(
        scores.windows(2).all(|w| w[0] >= w[1]),
        "non-temporal query was re-sorted: {scores:?}"
    );
}

// ---------------------------------------------------------------- TM-13

#[test]
fn tm13_date_and_time_literals_never_become_entities() {
    let cases: &[(&str, &[&str])] = &[
        ("on 2025-03-05", &["2025-03-05", "2025"]),
        ("March 2025", &["March", "March 2025"]),
        ("March 5, 2025", &["March", "March 5", "March 5, 2025"]),
        ("Ships Dec 2025", &["Dec", "Dec 2025"]),
        ("Jan 3", &["Jan", "Jan 3"]),
        ("Monday", &["Monday"]),
        ("on Mondays", &["Mondays"]),
        ("Tuesdays at 10:00", &["Tuesdays", "10:00"]),
        ("in 2019", &["2019"]),
        ("Q1 2026", &["Q1", "Q1 2026"]),
        ("3 days ago", &["3 days", "3 days ago"]),
        ("two weeks", &["two weeks"]),
        ("15 minutes", &["15 minutes"]),
        ("last week", &["last week"]),
        ("yesterday", &["yesterday"]),
        ("next month", &["next month"]),
        ("3 years", &["3 years"]),
        ("10:30", &["10:30"]),
        ("every Monday", &["Monday", "every Monday"]),
        ("September 2026", &["September", "September 2026"]),
    ];
    for (input, forbidden) in cases {
        let names: Vec<String> = extract::heuristic_entities(input)
            .into_iter()
            .map(|c| c.name)
            .collect();
        for bad in *forbidden {
            assert!(
                !names.iter().any(|n| n == bad),
                "{bad:?} became an entity in {input:?}: {names:?}"
            );
        }
    }
}

#[test]
fn tm13_real_names_survive_the_date_filter() {
    let names: Vec<String> = extract::heuristic_entities(
        "Alice Johnson deployed Postgres 16 in New York City on 2025-03-05.",
    )
    .into_iter()
    .map(|c| c.name)
    .collect();
    for kept in ["Alice Johnson", "Postgres 16", "New York City"] {
        assert!(
            names.iter().any(|n| n == kept),
            "{kept:?} missing: {names:?}"
        );
    }
    assert!(
        !names.iter().any(|n| n.contains("2025")),
        "date leaked: {names:?}"
    );
}
