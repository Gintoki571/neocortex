//! TOOL-NAMES: `#`, `+` and `.` inside or at the end of a short name carry
//! meaning. `text::norm_name` used to fold them to a space, so "C#", "C++"
//! and "C" all normalised to `c` and became one `tool` entity: every fact
//! about C++ shared the card of "Quasar Signal uses C#". The fix maps the
//! suffix to a word ("c# -> c sharp", "c++ -> c plus plus") so the names stay
//! apart, while ordinary names keep their old keys.

use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput};
use neocortex::text::norm_name;
use std::sync::Arc;

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap()
}

fn tool(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("tool".into()),
        ..Default::default()
    }
}

fn relation(source: &str, predicate: &str, target: &str) -> RelationIn {
    RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        ..Default::default()
    }
}

fn remember(engine: &Engine, content: &str, entities: Vec<EntityIn>, rel: RelationIn) {
    engine
        .remember(RememberInput {
            content: content.into(),
            entities,
            relations: vec![rel],
            ..Default::default()
        })
        .unwrap();
}

/// Stored norm of every entity of type `tool`, sorted.
fn tool_norms(engine: &Engine) -> Vec<String> {
    let mut stmt = engine
        .db
        .sql()
        .prepare("SELECT norm FROM entities WHERE type='tool' ORDER BY norm")
        .unwrap();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    rows.map(|r| r.unwrap()).collect()
}

/// Entity id an alias norm resolves to, if any.
fn entity_for(engine: &Engine, name: &str) -> Option<i64> {
    engine
        .db
        .sql()
        .query_row(
            "SELECT entity_id FROM entity_aliases WHERE alias_norm=?1",
            [norm_name(name)],
            |r| r.get(0),
        )
        .ok()
}

#[test]
fn norm_name_keeps_symbol_names_apart() {
    for (input, want) in [
        ("C#", "c sharp"),
        ("c#", "c sharp"),
        ("C++", "c plus plus"),
        ("F#", "f sharp"),
        ("A+", "plus"), // leading "a" is a determiner; still not "a"
        ("C", "c"),
        ("Node.js", "node js"),
        ("Node", "node"),
        ("Objective-C", "objective c"),
        ("Müller", "muller"),
        ("O'Brien", "obrien"),
        ("AT&T", "at t"),
        // Multi-word names keep the mapping inside them.
        ("C# 12", "c sharp 12"),
        // Ordinary words are untouched (no symbol, or a core longer than 3).
        ("Rust", "rust"),
        ("PostgreSQL 16", "postgresql 16"),
    ] {
        assert_eq!(norm_name(input), want, "input {input:?}");
    }
}

#[test]
fn symbol_tools_become_distinct_entities() {
    let engine = engine();
    for (name, content) in [
        ("C#", "C# is the managed language of the dotnet platform."),
        ("C++", "C++ is the systems language of the STL."),
        ("C", "C is the language of the POSIX API."),
        ("F#", "F# is the functional language of dotnet."),
    ] {
        remember(
            &engine,
            content,
            vec![tool(name)],
            relation("C", "compiles", name),
        );
    }
    assert_eq!(
        tool_norms(&engine),
        vec![
            "c".to_string(),
            "c plus plus".to_string(),
            "c sharp".to_string(),
            "f sharp".to_string()
        ],
        "C#, C++, C and F# must be four entities"
    );
    // Each mention resolves to its own card, never to a sibling.
    for name in ["C#", "C++", "C", "F#"] {
        assert!(
            entity_for(&engine, name).is_some(),
            "{name} has no alias row"
        );
    }
    let ids: Vec<i64> = ["C#", "C++", "C", "F#"]
        .iter()
        .map(|n| entity_for(&engine, n).unwrap())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 4, "ids {ids:?} must be distinct");
    // The spelling mapped to a word is the same entity, not a fifth copy.
    assert_eq!(entity_for(&engine, "C Sharp"), entity_for(&engine, "C#"));
    assert_eq!(
        entity_for(&engine, "C Plus Plus"),
        entity_for(&engine, "C++")
    );
}

#[test]
fn recall_answers_the_asked_tool() {
    let engine = engine();
    remember(
        &engine,
        "Quasar Signal uses C# for its query planner.",
        vec![
            EntityIn {
                name: "Quasar Signal".into(),
                kind: Some("project".into()),
                ..Default::default()
            },
            tool("C#"),
        ],
        relation("Quasar Signal", "uses", "C#"),
    );
    remember(
        &engine,
        "Helios Engine uses C++ for its renderer.",
        vec![
            EntityIn {
                name: "Helios Engine".into(),
                kind: Some("project".into()),
                ..Default::default()
            },
            tool("C++"),
        ],
        relation("Helios Engine", "uses", "C++"),
    );
    let out = engine
        .recall(&RecallInput {
            query: "what does Quasar Signal use".into(),
            ..Default::default()
        })
        .unwrap();
    let facts: Vec<String> = out.facts.iter().map(|f| f.fact.clone()).collect();
    let rendered = facts.join(" | ");
    assert_eq!(
        facts.first().map(String::as_str),
        Some("Quasar Signal uses C#"),
        "the C# fact must answer first, got {rendered:?}"
    );
    assert!(
        !facts
            .iter()
            .any(|f| f.starts_with("Quasar Signal uses C++")),
        "Quasar Signal must never answer with C++, got {rendered:?}"
    );
    assert!(
        out.memories
            .first()
            .is_some_and(|m| m.content.contains("Quasar Signal uses C#")),
        "the Quasar Signal memory must rank first, got {:?}",
        out.memories
            .iter()
            .map(|m| (&m.content, m.score))
            .collect::<Vec<_>>()
    );
}
