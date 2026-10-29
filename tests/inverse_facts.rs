//! H2-14: a relation stored in the canonical direction ("Noah manages Mira")
//! must be found and rendered from the asked person's side when the question
//! uses the inverse phrasing ("who does Mira report to", "who is Mira's
//! manager", "who is Mira accountable to"). The opposite direction must stay
//! the opposite direction.
use neocortex::embed::HashEmbedder;
use neocortex::engine::{
    Engine, EntityIn, FactHit, RecallInput, RecallOutput, RelationIn, RememberInput,
};
use std::sync::Arc;

mod common;

use common::TempDir;

fn remember(
    engine: &Engine,
    content: &str,
    entities: &[(&str, &str)],
    relations: Vec<RelationIn>,
) -> i64 {
    engine
        .remember(RememberInput {
            content: content.into(),
            entities: entities
                .iter()
                .map(|(name, kind)| EntityIn {
                    name: (*name).into(),
                    kind: Some((*kind).into()),
                    ..Default::default()
                })
                .collect(),
            relations,
            ..Default::default()
        })
        .unwrap()
        .id
}

fn relation(source: &str, predicate: &str, target: &str, fact: &str) -> RelationIn {
    RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        fact: Some(fact.into()),
        ..Default::default()
    }
}

/// Noah manages Mira (stored canonically), Mira manages Priya, and Marta
/// mentors Rui. The manage facts share a group, so a direction bug would pick
/// the wrong one.
fn engine() -> Engine {
    let engine = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
    remember(
        &engine,
        "Noah Tansen manages Mira Shah.",
        &[("Noah Tansen", "person"), ("Mira Shah", "person")],
        vec![relation(
            "Noah Tansen",
            "manages",
            "Mira Shah",
            "Noah Tansen manages Mira Shah.",
        )],
    );
    remember(
        &engine,
        "Mira Shah manages Priya Raman.",
        &[("Mira Shah", "person"), ("Priya Raman", "person")],
        vec![relation(
            "Mira Shah",
            "manages",
            "Priya Raman",
            "Mira Shah manages Priya Raman.",
        )],
    );
    remember(
        &engine,
        "Marta Lima mentors Rui Costa.",
        &[("Marta Lima", "person"), ("Rui Costa", "person")],
        vec![relation(
            "Marta Lima",
            "mentors",
            "Rui Costa",
            "Marta Lima mentors Rui Costa.",
        )],
    );
    engine
}

fn recall(engine: &Engine, query: &str) -> RecallOutput {
    engine
        .recall(&RecallInput {
            query: query.into(),
            ..Default::default()
        })
        .unwrap()
}

fn find<'a>(out: &'a RecallOutput, source: &str, target: &str) -> Option<&'a FactHit> {
    out.facts
        .iter()
        .find(|fact| !fact.memory && fact.source == source && fact.target == target)
}

#[test]
fn inverse_manager_questions_answer_from_the_named_persons_side() {
    let engine = engine();
    for query in [
        "Who does Mira Shah report to?",
        "Who is Mira Shah accountable to?",
        "Who is Mira's manager?",
        "Who is Mira Shah's boss?",
        "Mira Shah supervisor",
    ] {
        let out = recall(&engine, query);
        let fact = find(&out, "Mira Shah", "Noah Tansen").unwrap_or_else(|| {
            panic!(
                "{query}: the manages fact must render from Mira's side: {:?}",
                out.facts
            )
        });
        assert!(
            fact.predicate.contains("report") || fact.predicate.contains("accountable"),
            "{query}: unexpected predicate {:?}",
            fact.predicate
        );
        assert_eq!(
            out.facts.iter().filter(|f| f.id == fact.id).count(),
            1,
            "{query}: one rendering per relation: {:?}",
            out.facts
        );
    }
}

#[test]
fn the_opposite_direction_stays_the_opposite_direction() {
    let engine = engine();
    let out = recall(&engine, "Who does Mira Shah manage?");
    let fact = find(&out, "Mira Shah", "Priya Raman")
        .unwrap_or_else(|| panic!("Mira manages Priya: {:?}", out.facts));
    assert_eq!(fact.predicate, "manages");
    assert!(
        !out.facts
            .iter()
            .any(|f| !f.memory && f.source == "Mira Shah" && f.target == "Noah Tansen"),
        "Mira does not manage Noah: {:?}",
        out.facts
    );
}

#[test]
fn a_question_about_who_manages_keeps_the_canonical_direction() {
    let engine = engine();
    let out = recall(&engine, "Who manages Mira Shah?");
    let fact = find(&out, "Noah Tansen", "Mira Shah")
        .unwrap_or_else(|| panic!("Noah manages Mira: {:?}", out.facts));
    assert_eq!(fact.predicate, "manages");
}

#[test]
fn mentor_questions_render_from_the_named_side() {
    let engine = engine();
    let out = recall(&engine, "Who is Rui's mentor?");
    let fact = find(&out, "Rui Costa", "Marta Lima")
        .unwrap_or_else(|| panic!("Rui is mentored by Marta: {:?}", out.facts));
    assert!(fact.predicate.contains("mentor"), "{:?}", fact.predicate);

    let out = recall(&engine, "Who does Marta mentor?");
    let fact = find(&out, "Marta Lima", "Rui Costa")
        .unwrap_or_else(|| panic!("Marta mentors Rui: {:?}", out.facts));
    assert_eq!(fact.predicate, "mentors");

    let out = recall(&engine, "Who does Rui mentor?");
    assert!(
        !out.facts
            .iter()
            .any(|f| !f.memory && f.source == "Rui Costa" && f.target == "Marta Lima"),
        "Rui does not mentor Marta: {:?}",
        out.facts
    );
}

#[test]
fn accountable_to_is_a_direction_safe_write_alias() {
    let engine = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
    remember(
        &engine,
        "Mira Shah is accountable to Noah Tansen.",
        &[("Noah Tansen", "person"), ("Mira Shah", "person")],
        vec![relation(
            "Mira Shah",
            "accountable to",
            "Noah Tansen",
            "Mira Shah is accountable to Noah Tansen.",
        )],
    );
    let (manager, reporter): (String, String) = engine
        .db
        .sql()
        .query_row(
            "SELECT e.norm, d.norm
             FROM relations r JOIN predicates p ON p.id=r.predicate_id
             JOIN entities e ON e.id=r.src JOIN entities d ON d.id=r.dst
             WHERE (SELECT canonical FROM predicates WHERE id=COALESCE(NULLIF(p.group_id,0),p.id))='manage'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        manager, "noah tansen",
        "the manager is the canonical source"
    );
    assert_eq!(reporter, "mira shah");
}

/// A person can have several mentors: the seeded `mentored by` write alias must
/// not end the previous mentor's fact ("Rui Costa is mentored by Marta Lima"
/// then "Rui Costa is mentored by Ken Ito" keeps both), the way "reports to"
/// behaves like the exclusive one and mentors do not.
#[test]
fn a_second_mentored_by_write_keeps_the_first_mentors_fact() {
    let engine = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
    let people = &[
        ("Rui Costa", "person"),
        ("Marta Lima", "person"),
        ("Ken Ito", "person"),
    ];
    let first = engine
        .remember(RememberInput {
            content: "Rui Costa is mentored by Marta Lima.".into(),
            entities: people
                .iter()
                .map(|(name, kind)| EntityIn {
                    name: (*name).into(),
                    kind: Some((*kind).into()),
                    ..Default::default()
                })
                .collect(),
            relations: vec![relation(
                "Rui Costa",
                "mentored by",
                "Marta Lima",
                "Rui Costa is mentored by Marta Lima.",
            )],
            ..Default::default()
        })
        .unwrap();
    let second = engine
        .remember(RememberInput {
            content: "Rui Costa is mentored by Ken Ito.".into(),
            entities: people
                .iter()
                .map(|(name, kind)| EntityIn {
                    name: (*name).into(),
                    kind: Some((*kind).into()),
                    ..Default::default()
                })
                .collect(),
            relations: vec![relation(
                "Rui Costa",
                "mentored by",
                "Ken Ito",
                "Rui Costa is mentored by Ken Ito.",
            )],
            ..Default::default()
        })
        .unwrap();
    // Inverse writes store the canonical `mentor` direction (mentor -> mentee),
    // so both live facts have Rui as the canonical end.
    let mentees_of: Vec<String> = engine
        .db
        .sql()
        .prepare(
            "SELECT e.norm FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             JOIN entities e ON e.id=r.src
             WHERE p.canonical='mentor' AND r.expired_at IS NULL
             ORDER BY r.id",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<String>, _>>()
        .unwrap();
    assert_eq!(
        mentees_of,
        vec!["marta lima", "ken ito"],
        "both mentors must stay live"
    );
    assert!(first.invalidated_relations.is_empty());
    assert_eq!(
        second.invalidated_relations.len(),
        0,
        "no mentor fact ends: {:?} (conflicts {:?})",
        second.invalidated_relations,
        second.conflicts
    );
}

/// A database opened by the previous build carries `mentored by` seeded
/// exclusive: 1. The seed re-applies its policy on every open, so the next
/// open must correct it back to non-exclusive.
#[test]
fn reopening_a_previous_build_database_fixes_the_seeded_mentored_by_exclusivity() {
    let dir = TempDir::new("inverse-mentor-seed");
    let path = dir.join("mem.db");
    {
        let engine = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        // Simulate the previous build: the seed wrote exclusive = 1.
        engine
            .db
            .sql()
            .execute(
                "UPDATE predicates SET exclusive=1 WHERE canonical='mentored by'",
                [],
            )
            .unwrap();
    }
    let engine = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    let exclusive: i64 = engine
        .db
        .sql()
        .query_row(
            "SELECT exclusive FROM predicates WHERE canonical='mentored by'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        exclusive, 0,
        "an old database must be corrected to the non-exclusive seed on open"
    );
}
