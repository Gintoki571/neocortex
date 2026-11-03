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

mod common;
use common::TempDir;

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

fn project(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("project".into()),
        ..Default::default()
    }
}

/// Four tool entities, each used by one project. Returns the engine.
fn tools_world() -> Engine {
    let engine = engine();
    for (proj, name) in [
        ("Helios Engine", "C++"),
        ("Quasar Signal", "C#"),
        ("Atlas Kernel", "C"),
        ("Nimbus Cloud", "F#"),
    ] {
        remember(
            &engine,
            &format!("{proj} uses {name} for its core."),
            vec![project(proj), tool(name)],
            relation(proj, "uses", name),
        );
    }
    engine
}

#[test]
fn read_side_anchors_on_the_asked_symbol_tool() {
    let engine = tools_world();
    for (query, want) in [
        ("who uses C++", "Helios Engine uses C++"),
        ("who uses C++,", "Helios Engine uses C++"),
        ("Tell me about C#", "Quasar Signal uses C#"),
        ("Tell me about (C#)", "Quasar Signal uses C#"),
        ("Tell me about C#.", "Quasar Signal uses C#"),
        ("what uses C", "Atlas Kernel uses C"),
    ] {
        let out = engine
            .recall(&RecallInput {
                query: query.into(),
                ..Default::default()
            })
            .unwrap();
        let first = out.facts.first().map(|f| f.fact.as_str());
        assert_eq!(first, Some(want), "query {query:?}");
    }
}

#[test]
fn write_side_ignores_surrounding_punctuation() {
    for (input, want) in [
        ("C++,", "c plus plus"),
        ("(C#)", "c sharp"),
        ("C#.", "c sharp"),
        ("C.", "c"),
    ] {
        assert_eq!(norm_name(input), want, "input {input:?}");
    }
}

/// A database written before TOOL-NAMES stores C++/C#/C under norm `c`.
#[test]
fn old_symbol_keys_are_rekeyed_on_open() {
    let dir = TempDir::new("symbols-rekey");
    let path = dir.join("old.db");
    {
        let engine = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &engine,
            "Helios Engine uses C++ for its renderer.",
            vec![project("Helios Engine"), tool("C++")],
            relation("Helios Engine", "uses", "C++"),
        );
        // Rewrite to the pre-fix keys.
        let c = engine.db.sql();
        c.execute("UPDATE entities SET norm='c' WHERE name='C++'", [])
            .unwrap();
        c.execute(
            "UPDATE entity_aliases SET alias_norm='c' WHERE alias_norm='c plus plus'",
            [],
        )
        .unwrap();
        c.execute(
            "DELETE FROM name_tokens WHERE entity_id=(SELECT id FROM entities WHERE name='C++')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO name_tokens SELECT 'c', id FROM entities WHERE name='C++'",
            [],
        )
        .unwrap();
    }
    let engine = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    let norm: String = engine
        .db
        .sql()
        .query_row("SELECT norm FROM entities WHERE name='C++'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(norm, "c plus plus");
    remember(
        &engine,
        "Helios Engine uses C++ for its renderer.",
        vec![project("Helios Engine"), tool("C++")],
        relation("Helios Engine", "uses", "C++"),
    );
    remember(
        &engine,
        "Atlas Kernel uses C for its core.",
        vec![project("Atlas Kernel"), tool("C")],
        relation("Atlas Kernel", "uses", "C"),
    );
    assert_eq!(
        tool_norms(&engine),
        vec!["c".to_string(), "c plus plus".to_string()],
        "C++ must not be duplicated and C must not be swallowed"
    );
    assert_ne!(entity_for(&engine, "C"), entity_for(&engine, "C++"));
    assert_eq!(engine.validate().unwrap(), Vec::<String>::new());
    drop(engine);
    // Idempotent: a second open changes nothing.
    let engine = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(engine.validate().unwrap(), Vec::<String>::new());
}

/// Re-keying never merges: a stale row whose new key is taken stays, and
/// `validate` names it.
#[test]
fn rekey_conflict_is_left_for_validate() {
    let dir = TempDir::new("symbols-conflict");
    let path = dir.join("old.db");
    {
        let engine = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &engine,
            "Helios Engine uses C++ for its renderer.",
            vec![project("Helios Engine"), tool("C++")],
            relation("Helios Engine", "uses", "C++"),
        );
        engine
            .db
            .sql()
            .execute(
                "INSERT INTO entities(name,norm,type,created_at,updated_at) VALUES ('C++','c','tool',0,0)",
                [],
            )
            .unwrap();
    }
    let engine = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    let n: i64 = engine
        .db
        .sql()
        .query_row("SELECT count(*) FROM entities WHERE name='C++'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(n, 2, "nothing is merged or deleted");
    assert!(engine
        .validate()
        .unwrap()
        .iter()
        .any(|l| l.contains("is keyed")));
}
