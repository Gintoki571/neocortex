//! nc-passive: a two-hop question asked in the passive voice must reach the
//! same answer as the active shape.
//!
//! Stored: "Cedric Narmani leads the Vesper Monitor project." and "Vesper
//! Monitor is written in Ruby." The question "which language is used by
//! Cedric Narmani's project" names the language as the subject of a passive
//! `use` and reaches the project only through a possessive, so the hop
//! person -> project carries no predicate word of its own. The second shape,
//! "the main language of the project led by <person>", states that hop in a
//! passive relative clause (`led`). Without both read as their active
//! predicates the path channel never leaves the person and the answer memory
//! is not in the top-10 context `eval` scores.
use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, EntityIn, RecallInput, RecallOutput, RelationIn, RememberInput};
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

/// One person leading one project that is written in one language: the
/// world-10000 shape the two-hop questions are asked about.
fn lead(engine: &Engine, person: &str, project: &str, language: &str) {
    pair(engine, person, project, language, "leads");
}

/// The same pair stated with another active verb, for the passive forms that
/// name it ("the project managed by <person>").
fn pair(engine: &Engine, person: &str, project: &str, language: &str, verb: &str) {
    remember(
        engine,
        &format!("{person} {verb} the {project} project."),
        &[(person, "person"), (project, "project")],
        vec![relation(
            person,
            verb,
            project,
            &format!("{person} {verb} {project}"),
        )],
    );
    remember(
        engine,
        &format!("{project} is written in {language}."),
        &[(project, "project"), (language, "tool")],
        vec![relation(
            project,
            "uses",
            language,
            &format!("{project} uses {language}"),
        )],
    );
}

/// Twenty people, projects and languages plus two more memories each that
/// share the person, and one `manages` pair: eighty-four memories, so the
/// answer memory cannot reach the top 10 on the anchor alone. Only the path
/// channel can walk project -> language.
fn engine() -> Engine {
    let engine = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
    for (person, project, language) in [
        ("Cedric Narmani", "Vesper Monitor", "Ruby"),
        ("Ruby Chanyya", "Borealis Inbox", "Scala"),
        ("Ives Malcolm", "Halcyon Cache", "Elixir"),
        ("Nadia Ferrer", "Tessera Ledger", "Kotlin"),
        ("Oscar Bellini", "Quarry Signal", "Dart"),
        ("Priya Raman", "Wavelength Sync", "Haskell"),
        ("Tomas Ekland", "Juniper Relay", "Clojure"),
        ("Yara Odeh", "Meridian Trace", "Swift"),
        ("Bruno Sattler", "Vantage Grid", "Perl"),
        ("Lena Kovac", "Beacon Drift", "Lua"),
        ("Saanvi Barkumar", "Copper Loom", "Haskell"),
        ("Scott Patvari", "Granite Fathom", "Fortran"),
        ("Sofia Mahraj", "Indigo Parse", "Haskell"),
        ("Marek Dvorak", "Silver Kite", "Rust"),
        ("Hana Sato", "Paper Crane", "Go"),
        ("Owen Brady", "River Delta", "Python"),
        ("Amara Diallo", "Sable Ridge", "Julia"),
        ("Felix Braun", "Amber Switch", "Java"),
        ("Ines Rocha", "Coral Queue", "PHP"),
        ("Jonas Berg", "Fjord Signal", "C Sharp"),
    ] {
        lead(&engine, person, project, language);
        remember(
            &engine,
            &format!("{person} has settled in a city abroad."),
            &[(person, "person")],
            Vec::new(),
        );
        remember(
            &engine,
            &format!("{person} works at a company in the same city."),
            &[(person, "person")],
            Vec::new(),
        );
    }
    // The passive class is not specific to "lead": "the project managed by X"
    // states the same hop with `manage`.
    pair(&engine, "Dario Voss", "Umbra Stack", "Zig", "manages");
    remember(
        &engine,
        "Dario Voss has settled in a city abroad.",
        &[("Dario Voss", "person")],
        Vec::new(),
    );
    remember(
        &engine,
        "Dario Voss works at a company in the same city.",
        &[("Dario Voss", "person")],
        Vec::new(),
    );
    engine
}

/// The answer counts as found when its memory is in the top 10, exactly what
/// `eval` scores (acc@10); a rendered fact alone does not count.
fn in_top10(out: &RecallOutput, answer: &str) -> bool {
    out.memories
        .iter()
        .take(10)
        .any(|m| m.content.contains(answer))
}

fn rank(out: &RecallOutput, answer: &str) -> Option<usize> {
    out.memories.iter().position(|m| m.content.contains(answer))
}

fn shown(out: &RecallOutput) -> Vec<&str> {
    out.memories
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
}

#[test]
fn a_passive_possessive_two_hop_reaches_the_language() {
    let _tmp = TempDir::new("passive-a");
    let engine = engine();
    let out = engine
        .recall(&RecallInput {
            query: "which language is used by Cedric Narmani's project".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        in_top10(&out, "Vesper Monitor is written in Ruby"),
        "the language memory must be in the top 10, rank {:?}: {:?}",
        rank(&out, "Vesper Monitor is written in Ruby"),
        shown(&out)
    );
}

#[test]
fn a_passive_relative_clause_two_hop_reaches_the_language() {
    let _tmp = TempDir::new("passive-b");
    let engine = engine();
    let out = engine
        .recall(&RecallInput {
            query: "what is the main language of the project led by Ruby Chanyya".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        in_top10(&out, "Borealis Inbox is written in Scala"),
        "the language memory must be in the top 10, rank {:?}: {:?}",
        rank(&out, "Borealis Inbox is written in Scala"),
        shown(&out)
    );
}

#[test]
fn a_passive_managed_by_clause_reaches_the_language() {
    let _tmp = TempDir::new("passive-managed");
    let engine = engine();
    let out = engine
        .recall(&RecallInput {
            query: "what is the main language of the project managed by Dario Voss".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        in_top10(&out, "Umbra Stack is written in Zig"),
        "the language memory must be in the top 10, rank {:?}: {:?}",
        rank(&out, "Umbra Stack is written in Zig"),
        shown(&out)
    );
}

#[test]
fn the_active_shape_and_the_inverse_questions_still_answer() {
    let _tmp = TempDir::new("passive-controls");
    let engine = engine();
    // The active shape must keep working: it is the shape that already passes.
    let active = engine
        .recall(&RecallInput {
            query: "what language does the project Ives Malcolm leads use".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        in_top10(&active, "Halcyon Cache is written in Elixir"),
        "the active shape must stay answered: {:?}",
        shown(&active)
    );
    // "who uses X" asks for the project, not for a language.
    let who_uses = engine
        .recall(&RecallInput {
            query: "who uses Scala".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        in_top10(&who_uses, "Borealis Inbox is written in Scala"),
        "the inverse question must stay answered: {:?}",
        shown(&who_uses)
    );
    // "who leads X" asks for the person; the language must not crowd the top.
    let inverse = engine
        .recall(&RecallInput {
            query: "who leads the Borealis Inbox project".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        in_top10(&inverse, "Ruby Chanyya leads the Borealis Inbox"),
        "the inverse question must stay answered: {:?}",
        shown(&inverse)
    );
    let top3: Vec<&str> = inverse
        .memories
        .iter()
        .take(3)
        .map(|m| m.content.as_str())
        .collect();
    assert!(
        !top3.iter().any(|content| content.contains("Scala")),
        "the inverse question is not about the language: {top3:?}"
    );
}
