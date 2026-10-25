use neocortex::embed::HashEmbedder;
use neocortex::engine::{
    Config, Engine, EntityIn, NextHop, RecallInput, RecallOutput, RelationIn, RememberInput,
};
use serde_json::json;
use std::sync::Arc;

fn remember(engine: &Engine, content: &str, setup: impl FnOnce(&mut RememberInput)) -> i64 {
    let mut input = RememberInput {
        content: content.into(),
        ..Default::default()
    };
    setup(&mut input);
    engine.remember(input).unwrap().id
}

fn entity(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        ..Default::default()
    }
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

fn two_hop_engine() -> Engine {
    let e = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
    remember(&e, "Noah Tansen's manager is Mira Shah.", |i| {
        i.entities = vec![entity("Noah Tansen"), entity("Mira Shah")];
        i.relations = vec![relation(
            "Noah Tansen",
            "manager",
            "Mira Shah",
            "Noah Tansen's manager is Mira Shah.",
        )];
    });
    remember(&e, "Mira Shah lives in Oslo.", |i| {
        i.entities = vec![entity("Mira Shah"), entity("Oslo")];
        i.relations = vec![relation(
            "Mira Shah",
            "lives in",
            "Oslo",
            "Mira Shah lives in Oslo.",
        )];
    });
    e
}

#[test]
fn an_answered_two_hop_question_carries_no_hint() {
    let e = two_hop_engine();
    let out = e
        .recall(&RecallInput {
            query: "Where does the manager of Noah Tansen live?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.memories.iter().any(|m| m.content.contains("Oslo")),
        "the fixture must answer: {:?}",
        out.memories
    );
    assert!(out.next.is_empty(), "{:?}", out.next);
    let value = serde_json::to_value(&out).unwrap();
    assert!(value.get("next").is_none(), "{value}");
}

#[test]
fn hints_are_disabled_by_a_zero_tau() {
    let mut e = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
    e.cfg = Config {
        hint_tau: 0.0,
        ..Config::default()
    };
    let out = e
        .recall(&RecallInput {
            query: "Where does the manager of Noah Tansen live?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(out.next.is_empty());
}

#[test]
fn next_hints_serialize_the_follow_call() {
    let out = RecallOutput {
        memories: Vec::new(),
        facts: Vec::new(),
        entities: Vec::new(),
        next: vec![NextHop {
            entity: "e151".into(),
            name: "Mira Lindqvist".into(),
            want: "where live".into(),
            why: "chain ends at a person".into(),
        }],
        window: None,
        candidates: 0,
        tokens_used: 0,
        tokens_budget: 1200,
        omitted: 0,
        window_empty: false,
        over_budget: false,
    };
    let value = serde_json::to_value(&out).unwrap();
    assert_eq!(
        value["next"],
        json!([{
            "entity": "e151",
            "name": "Mira Lindqvist",
            "want": "where live",
            "why": "chain ends at a person"
        }])
    );
}
