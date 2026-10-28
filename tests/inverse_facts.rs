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
