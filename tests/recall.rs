use neocortex::embed::{Embedder, HashEmbedder, Mode, DIMS};
use neocortex::engine::{
    Config, Engine, EntityIn, RecallInput, RecallOutput, RelationIn, RememberInput,
};
use neocortex::paths;
use neocortex::temporal::{parse_instant, Clock};
use neocortex::Result;
use std::collections::HashMap;
use std::sync::Arc;

mod common;
use common::TempDir;

fn clock() -> Clock {
    Clock {
        now: parse_instant(
            "2026-10-21T15:00:00Z",
            &Clock {
                now: 0,
                tz_offset_secs: 0,
            },
        )
        .unwrap(),
        tz_offset_secs: 0,
    }
}

fn engine(emb: Arc<dyn Embedder>) -> Engine {
    Engine::open_in_memory(emb).unwrap().with_clock(clock())
}

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

fn ids(out: &RecallOutput) -> Vec<i64> {
    let mut v: Vec<i64> = out.memories.iter().map(|m| m.id).collect();
    v.sort_unstable();
    v
}

fn has_reason(out: &RecallOutput, id: i64, prefix: &str) -> bool {
    out.memories
        .iter()
        .find(|m| m.id == id)
        .is_some_and(|m| m.reasons.iter().any(|r| r.starts_with(prefix)))
}

/// A tiny relation graph where every relation is incident to the query anchor
/// "Atlas", and the answer memory text does not name the anchor or the tool.
fn relation_prefilter_fixture() -> (Engine, i64, i64) {
    let mut e = engine(Arc::new(HashEmbedder));
    // Access recording must not let the on/off comparison drift between calls.
    e.cfg.w_retention = 0.0;
    let answer = remember(&e, "The stack choice is settled.", |i| {
        i.entities = vec![entity("Atlas"), entity("Rust")];
        i.relations = vec![relation("Atlas", "uses", "Rust", "Atlas uses Rust")];
    });
    let sibling = remember(&e, "The build pipeline is documented.", |i| {
        i.entities = vec![entity("Atlas"), entity("Go")];
        i.relations = vec![relation("Atlas", "uses", "Go", "Atlas uses Go")];
    });
    (e, answer, sibling)
}

fn prefilter_query() -> RecallInput {
    RecallInput {
        query: "What does Atlas use?".into(),
        ..Default::default()
    }
}

#[test]
fn relation_prefilter_off_is_byte_identical() {
    let (e, _, _) = relation_prefilter_fixture();
    let base = serde_json::to_string(&e.recall(&prefilter_query()).unwrap()).unwrap();
    let mut on = e;
    on.cfg.rel_prefilter = true;
    let _ = on.recall(&prefilter_query()).unwrap();
    on.cfg.rel_prefilter = false;
    let off = serde_json::to_string(&on.recall(&prefilter_query()).unwrap()).unwrap();
    assert_eq!(off, base, "knob off restores the pre-existing output");
}

#[test]
fn relation_prefilter_keeps_small_relation_sets_identical() {
    let (off, answer, sibling) = relation_prefilter_fixture();
    let (mut on, _, _) = relation_prefilter_fixture();
    on.cfg.rel_prefilter = true;
    let off = off.recall(&prefilter_query()).unwrap();
    let on = on.recall(&prefilter_query()).unwrap();
    assert_eq!(ids(&on), ids(&off), "the prefilter keeps the top-k ids");
    assert_eq!(on.memories.len(), off.memories.len());
    assert!(ids(&on).contains(&answer));
    assert!(ids(&on).contains(&sibling));
}

#[test]
fn relation_prefilter_finds_relation_only_answer() {
    let (mut e, answer, _) = relation_prefilter_fixture();
    e.cfg.rel_prefilter = true;
    let out = e.recall(&prefilter_query()).unwrap();
    assert!(
        out.facts
            .iter()
            .any(|fact| fact.source == "Atlas" && fact.target == "Rust"),
        "relation-backed fact missing: {out:?}"
    );
    assert!(
        ids(&out).contains(&answer),
        "the memory that evidences the relation is missing: {out:?}"
    );
}

#[test]
fn relation_prefilter_keeps_global_knn_for_anchorless_queries() {
    // The question names no entity, so the prefilter has nothing to restrict
    // the relation-vector arm to and the global KNN must still run.
    let (off, _, _) = relation_prefilter_fixture();
    let (mut on, _, _) = relation_prefilter_fixture();
    on.cfg.rel_prefilter = true;
    let query = RecallInput {
        query: "Which stack was settled?".into(),
        ..Default::default()
    };
    let off = off.recall(&query).unwrap();
    let on = on.recall(&query).unwrap();
    assert_eq!(
        serde_json::to_string(&on).unwrap(),
        serde_json::to_string(&off).unwrap(),
        "anchor-less questions keep the global relation KNN"
    );
}

// Ensure the 800-cap + cosine>=0.5 filter does not drop the only relation
// that answers a question, even when 850 newer relations crowd it out.
#[test]
fn relation_prefilter_cap_buried_answer() {
    let _guard = TempDir::new("neocortex-verify-cap");
    let mut e = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();

    // The one old answering relation, dated 2020 (the prefilter pool keeps
    // the 800 newest incident relations, so this one falls out).
    let answer = e
        .remember(RememberInput {
            content: "The qzibble toolchain decision is recorded.".into(),
            entities: vec![entity("Atlas"), entity("Rust")],
            relations: vec![RelationIn {
                source: "Atlas".into(),
                predicate: "uses".into(),
                target: "Rust".into(),
                fact: Some("Atlas uses Rust".into()),
                ..Default::default()
            }],
            valid_from: Some("2020-01-01".into()),
            ..Default::default()
        })
        .unwrap()
        .id;

    // 850 newer incident relations on the same anchor, none token-matching
    // the question's content words except the shared "atlas".
    for n in 0..850 {
        e.remember(RememberInput {
            content: format!("Build note {n}: pipeline segment {n} documented."),
            entities: vec![entity("Atlas"), entity(&format!("Go{n}"))],
            relations: vec![RelationIn {
                source: "Atlas".into(),
                predicate: "depends on".into(),
                target: format!("Go{n}"),
                fact: Some(format!("Atlas depends on Go{n}")),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    }

    let query = RecallInput {
        query: "What does Atlas use?".into(),
        ..Default::default()
    };

    let off = e.recall(&query).unwrap();
    e.cfg.rel_prefilter = true;
    let on = e.recall(&query).unwrap();

    let fact_hit = |o: &neocortex::engine::RecallOutput| {
        o.facts
            .iter()
            .any(|f| f.source == "Atlas" && f.target == "Rust")
    };
    eprintln!(
        "off: answer memory in top-k = {}(Atlas->Rust fact = {})",
        off.memories.iter().any(|m| m.id == answer),
        fact_hit(&off)
    );
    eprintln!(
        "on:  answer memory in top-k = {}(Atlas->Rust fact = {})",
        on.memories.iter().any(|m| m.id == answer),
        fact_hit(&on)
    );
    eprintln!(
        "on.facts = {:?}",
        on.facts
            .iter()
            .map(|f| (&f.source, &f.predicate, &f.target))
            .collect::<Vec<_>>()
    );

    assert!(
        fact_hit(&on),
        "rel_prefilter dropped the only relation that answers the question"
    );
    assert!(
        on.memories.iter().any(|m| m.id == answer),
        "rel_prefilter dropped the answer memory from the context"
    );
}

#[test]
fn relation_prefilter_knob_parses_and_defaults_off() {
    let mut cfg = Config::default();
    assert!(!cfg.rel_prefilter);
    neocortex::eval::set_config(&mut cfg, "rel_prefilter", "on").unwrap();
    assert!(cfg.rel_prefilter);
    neocortex::eval::set_config(&mut cfg, "rel_prefilter", "off").unwrap();
    assert!(!cfg.rel_prefilter);
    assert!(neocortex::eval::set_config(&mut cfg, "rel_prefilter", "maybe").is_err());
}

#[test]
fn typed_bridge_pull_adds_an_answer_typed_relation() {
    let mut e = engine(Arc::new(HashEmbedder));
    remember(&e, "Noah manages Mira.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Noah".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
            EntityIn {
                name: "Mira".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
        ];
        i.relations = vec![relation("Noah", "manages", "Mira", "Noah manages Mira")];
    });
    remember(&e, "Mira lives in Oslo.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Mira".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
            EntityIn {
                name: "Oslo".into(),
                kind: Some("place".into()),
                ..Default::default()
            },
        ];
        i.relations = vec![relation("Mira", "lives in", "Oslo", "Mira lives in Oslo")];
    });
    e.cfg.w_sempath = 0.0;
    // Test with bridge_pull explicitly off (old default)
    let off = e
        .recall(&RecallInput {
            query: "Where does Noah's colleague live?".into(),
            k: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert!(off.facts.iter().any(|fact| fact.target.contains("Oslo")));

    // Test with bridge_pull on (new default)
    let on = e
        .recall(&RecallInput {
            query: "Where does Noah's colleague live?".into(),
            k: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert!(
        on.facts.iter().any(|fact| fact.target.contains("Oslo")),
        "{:?}",
        on.facts
    );
    assert!(on.facts.iter().any(|fact| fact.target.contains("Oslo")));
}

#[test]
fn config_default_bridge_pull_is_true() {
    let e = engine(Arc::new(HashEmbedder));
    assert!(
        e.cfg.bridge_pull,
        "Config::default().bridge_pull should be true"
    );
}

#[test]
fn bridge_pull_off_restores_old_output() {
    let mut e = engine(Arc::new(HashEmbedder));
    remember(&e, "Noah manages Mira.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Noah".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
            EntityIn {
                name: "Mira".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
        ];
        i.relations = vec![relation("Noah", "manages", "Mira", "Noah manages Mira")];
    });
    remember(&e, "Mira lives in Oslo.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Mira".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
            EntityIn {
                name: "Oslo".into(),
                kind: Some("place".into()),
                ..Default::default()
            },
        ];
        i.relations = vec![relation("Mira", "lives in", "Oslo", "Mira lives in Oslo")];
    });
    e.cfg.w_sempath = 0.0;
    // Explicitly turn bridge_pull off to verify old output
    e.cfg.bridge_pull = false;
    let off = e
        .recall(&RecallInput {
            query: "Where does Noah's colleague live?".into(),
            k: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert!(off.facts.iter().any(|fact| fact.target.contains("Oslo")));
}

#[test]
fn keyword_recall_returns_matching_memory() {
    let engine = Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now: 1_800_000_000,
            tz_offset_secs: 0,
        });
    engine
        .remember(RememberInput {
            content: "The production database runs PostgreSQL".into(),
            ..Default::default()
        })
        .unwrap();
    let out = engine
        .recall(&RecallInput {
            query: "production database".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(out.memories.len(), 1);
    assert!(out.memories[0].content.contains("PostgreSQL"));
}

#[test]
fn filters_apply_to_kind_tags_scope_and_validity() {
    let e = engine(Arc::new(HashEmbedder));
    let event_work = remember(&e, "deployment checklist for the ops rotation", |i| {
        i.kind = Some("event".into());
        i.tags = vec!["ops".into()];
        i.scope = Some("work".into());
        i.valid_from = Some("2026-01-05".into());
    });
    let fact_work = remember(&e, "deployment checklist for the database rotation", |i| {
        i.kind = Some("fact".into());
        i.tags = vec!["ops".into(), "db".into()];
        i.scope = Some("work".into());
        i.valid_from = Some("2026-06-01".into());
    });
    let event_home = remember(&e, "deployment checklist for the home network", |i| {
        i.kind = Some("event".into());
        i.tags = vec!["home".into()];
        i.scope = Some("personal".into());
        i.valid_from = Some("2026-09-01".into());
    });
    let recall = |input: RecallInput| ids(&e.recall(&input).unwrap());

    let kinds = recall(RecallInput {
        query: "deployment checklist".into(),
        kinds: vec!["event".into()],
        ..Default::default()
    });
    assert_eq!(kinds, vec![event_work, event_home]);

    let tags = recall(RecallInput {
        query: "deployment checklist".into(),
        tags: vec!["db".into()],
        ..Default::default()
    });
    assert_eq!(tags, vec![fact_work]);

    let scope = recall(RecallInput {
        query: "deployment checklist".into(),
        scope: Some("personal".into()),
        ..Default::default()
    });
    assert_eq!(scope, vec![event_home]);

    // Interval overlap: the open-ended January event still holds in May.
    let mut since = recall(RecallInput {
        query: "deployment checklist".into(),
        since: Some("2026-05-01".into()),
        ..Default::default()
    });
    since.sort();
    assert_eq!(since, vec![event_work, fact_work, event_home]);

    let until = recall(RecallInput {
        query: "deployment checklist".into(),
        until: Some("2026-06-01".into()),
        ..Default::default()
    });
    assert_eq!(until, vec![event_work]);
}

#[test]
fn as_of_returns_the_then_valid_memory() {
    let e = engine(Arc::new(HashEmbedder));
    let delhi = remember(&e, "The office is in Delhi", |i| {
        i.valid_from = Some("2025-01-01".into());
        i.valid_to = Some("2026-01-01".into());
    });
    let pune = remember(&e, "The office is in Pune", |i| {
        i.valid_from = Some("2026-01-01".into());
    });
    let out = e
        .recall(&RecallInput {
            query: "office".into(),
            as_of: Some("2025-06-01".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(ids(&out), vec![delhi]);
    assert!(!ids(&out).contains(&pune));
}

#[test]
fn as_of_graph_excludes_relations_that_start_in_the_future() {
    let e = engine(Arc::new(HashEmbedder));
    let answer = remember(&e, "Anika owns the audit binder.", |i| {
        i.entities = vec![entity("Anika")];
    });
    remember(&e, "Priya mentors Anika.", |i| {
        i.entities = vec![entity("Priya"), entity("Anika")];
        i.relations = vec![RelationIn {
            source: "Priya".into(),
            predicate: "mentors".into(),
            target: "Anika".into(),
            fact: Some("Priya mentors Anika".into()),
            valid_from: Some("2027-01-01".into()),
            ..Default::default()
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "What does Priya oversee?".into(),
            as_of: Some("2026-10-01".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(!has_reason(&out, answer, "graph#"));
}

#[test]
fn temporal_channel_scores_all_small_window_matches_not_only_newest_400() {
    let e = engine(Arc::new(HashEmbedder));
    let old = remember(&e, "quarterly archive marker", |i| {
        i.valid_from = Some("1577836800".into());
    });
    for day in 1..405 {
        remember(&e, &format!("quarterly archive filler {day}"), |i| {
            i.valid_from = Some((1577836800_i64 + day * 86_400).to_string());
        });
    }
    let out = e
        .recall(&RecallInput {
            query: "in 2020 quarterly archive marker".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&out).contains(&old));
}

#[test]
fn superseded_hidden_normally_returned_with_history() {
    let e = engine(Arc::new(HashEmbedder));
    let old = remember(&e, "The office is in Delhi", |_| {});
    let new = remember(&e, "The office is in Pune", |i| {
        i.supersedes = vec![format!("m{old}")];
    });

    let normal = e
        .recall(&RecallInput {
            query: "office".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(ids(&normal), vec![new]);
    assert!(!normal.memories[0].ended);

    let history = e
        .recall(&RecallInput {
            query: "office".into(),
            include_history: true,
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&history).contains(&old));
    let old_hit = history.memories.iter().find(|m| m.id == old).unwrap();
    assert!(old_hit.ended);

    let used_to = e
        .recall(&RecallInput {
            query: "where did the office used to be".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&used_to).contains(&old));
}

#[test]
fn ended_memory_ranks_below_current_without_history() {
    let e = engine(Arc::new(HashEmbedder));
    let current = remember(&e, "The team laptop stays in the Pune office", |i| {
        i.valid_from = Some("2026-01-01".into());
    });
    let ended = remember(&e, "The team laptop stays in the Delhi office", |i| {
        i.valid_from = Some("2026-01-01".into());
        i.valid_to = Some("2026-05-01".into());
    });
    let out = e
        .recall(&RecallInput {
            query: "team laptop office".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(out.memories[0].id, current);
    let ended_hit = out.memories.iter().find(|m| m.id == ended).unwrap();
    assert!(ended_hit.ended);
    let current_pos = out.memories.iter().position(|m| m.id == current).unwrap();
    let ended_pos = out.memories.iter().position(|m| m.id == ended).unwrap();
    assert!(current_pos < ended_pos);
}

#[test]
fn forgotten_memory_is_never_returned() {
    let e = engine(Arc::new(HashEmbedder));
    let id = remember(&e, "The vault code is 4417", |_| {});
    e.forget(&format!("m{id}"), false).unwrap();
    let normal = e
        .recall(&RecallInput {
            query: "vault code".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(normal.memories.is_empty());
    let history = e
        .recall(&RecallInput {
            query: "vault code".into(),
            include_history: true,
            ..Default::default()
        })
        .unwrap();
    assert!(history.memories.is_empty());
}

#[test]
fn multi_hop_reaches_memory_that_only_names_the_far_entity() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Priya leads the platform team.", |i| {
        i.entities = vec![entity("Priya")];
    });
    let answer = remember(&e, "Anika owns the quarterly audit binder.", |i| {
        i.entities = vec![entity("Anika")];
    });
    remember(&e, "Priya mentors Anika.", |i| {
        i.entities = vec![entity("Priya"), entity("Anika")];
        i.relations = vec![relation("Priya", "mentors", "Anika", "Priya mentors Anika")];
    });
    let out = e
        .recall(&RecallInput {
            query: "What does Priya oversee?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&out).contains(&answer));
    assert!(has_reason(&out, answer, "graph#"));
}

#[test]
fn two_hop_path_answers_with_the_manager_home_city() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Noah Tansen's manager is Mira Shah.", |i| {
        i.entities = vec![entity("Noah Tansen"), entity("Mira Shah")];
        i.relations = vec![relation(
            "Noah Tansen",
            "manager",
            "Mira Shah",
            "Noah Tansen's manager is Mira Shah.",
        )];
    });
    let answer = remember(&e, "Mira Shah lives in Oslo.", |i| {
        i.entities = vec![entity("Mira Shah"), entity("Oslo")];
        i.relations = vec![relation(
            "Mira Shah",
            "lives in",
            "Oslo",
            "Mira Shah lives in Oslo.",
        )];
    });
    let out = e
        .recall(&RecallInput {
            query: "where does the manager of Noah Tansen live".into(),
            k: Some(5),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&out).contains(&answer), "{:?}", out.memories);
    assert!(
        out.facts.iter().any(|f| f.target == "Oslo"),
        "the last hop must render, got {:?}",
        out.facts
    );
}

#[test]
fn three_hop_path_covers_sibling_manager_and_city() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Elsa's sister is Maria.", |i| {
        i.entities = vec![entity("Elsa"), entity("Maria")];
        i.relations = vec![relation(
            "Elsa",
            "sibling of",
            "Maria",
            "Elsa's sister is Maria.",
        )];
    });
    remember(&e, "Liam is Maria's manager.", |i| {
        i.entities = vec![entity("Liam"), entity("Maria")];
        i.relations = vec![relation(
            "Liam",
            "manager",
            "Maria",
            "Liam is Maria's manager.",
        )];
    });
    let answer = remember(&e, "Liam's home is in Denver.", |i| {
        i.entities = vec![entity("Liam"), entity("Denver")];
        i.relations = vec![relation(
            "Liam",
            "lives in",
            "Denver",
            "Liam's home is in Denver.",
        )];
    });
    let out = e
        .recall(&RecallInput {
            query: "what city is home to the manager of Elsa's sister".into(),
            k: Some(5),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&out).contains(&answer), "{:?}", out.memories);
    assert!(
        out.facts.iter().any(|f| f.target == "Denver"),
        "the last hop must render, got {:?}",
        out.facts
    );
}

#[test]
fn negation_question_returns_only_negative_memories() {
    let e = engine(Arc::new(HashEmbedder));
    for name in ["Priya", "Anika"] {
        remember(&e, &format!("{name} likes jazz."), |i| {
            i.entities = vec![entity(name)];
        });
    }
    let negative = remember(&e, "Bob does not like jazz.", |i| {
        i.entities = vec![entity("Bob")];
    });
    let out = e
        .recall(&RecallInput {
            query: "who does not like jazz".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&out).contains(&negative), "{:?}", out.memories);
    assert!(
        out.memories.iter().all(|m| m.content.contains("not")),
        "positive facts leaked into a negation answer: {:?}",
        out.memories.iter().map(|m| &m.content).collect::<Vec<_>>()
    );
    assert!(out.facts.iter().all(|f| f.fact.contains("not")));
}

#[test]
fn negation_question_keeps_negative_relations_without_a_cue_word() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Priya likes jazz.", |i| {
        i.relations = vec![relation("Priya", "likes", "jazz", "Priya likes jazz")];
    });
    let negative = remember(&e, "Bob refuses to listen to jazz.", |i| {
        i.relations = vec![RelationIn {
            negated: Some(true),
            ..relation("Bob", "likes", "jazz", "")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "who does not like jazz".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(ids(&out), vec![negative], "{:?}", out.memories);
    assert!(out.facts.iter().all(|f| f.polarity == 0), "{:?}", out.facts);
}

// ---------------------------------------------------------------- nc-fix2 H2-01

#[test]
fn meta_memory_questions_are_not_negation_queries() {
    let e = engine(Arc::new(HashEmbedder));
    let lumen = remember(&e, "Lumen uses SQLite.", |i| {
        i.entities = vec![entity("Lumen")];
        i.relations = vec![relation("Lumen", "uses", "SQLite", "Lumen uses SQLite")];
    });
    remember(&e, "Lumen used Postgres before the switch.", |i| {
        i.entities = vec![entity("Lumen")];
        i.relations = vec![RelationIn {
            valid_from: Some("2026-01-01".into()),
            valid_to: Some("2026-09-29".into()),
            ..relation("Lumen", "used", "Postgres", "Lumen used Postgres")
        }];
    });
    for query in [
        "what database does lumen use",
        "I don't remember what database lumen uses",
        "I cannot remember what database lumen uses",
        "I can't remember what database lumen uses",
        "I forgot what database lumen uses",
        "can you remind me what database lumen uses",
        "remind me what database lumen uses",
    ] {
        let out = e
            .recall(&RecallInput {
                query: query.into(),
                ..Default::default()
            })
            .unwrap();
        assert!(
            ids(&out).contains(&lumen),
            "{query:?} lost the positive fact: {:?}",
            out.memories
        );
    }
}

#[test]
fn negation_intent_ignores_negation_of_the_speakers_memory_verb() {
    for query in [
        "I don't remember what database lumen uses",
        "I cannot remember what database lumen uses",
        "I can't remember what database lumen uses",
        "I forgot what database lumen uses",
        "can you remind me what database lumen uses",
        "remind me what database lumen uses",
    ] {
        assert!(!paths::negation_intent(query), "{query:?}");
    }
    for query in [
        "who does not like jazz",
        "what no longer works",
        "what did we decide not to use",
    ] {
        assert!(paths::negation_intent(query), "{query:?}");
    }
}

// ---------------------------------------------------------------- nc-negq H2-02/03

#[test]
fn exclusion_questions_are_not_negation_questions() {
    for query in [
        "which service is not in rust",
        "which services are not written in rust",
        "what database does lumen not use",
        "which database does lumen not use",
        "which services don't use redis",
        "which of the services doesn't use redis",
        "services that don't use redis",
        "what do we use other than rust",
        "everything except rust",
    ] {
        assert!(paths::exclusion_intent(query), "{query:?}");
        assert!(!paths::negation_intent(query), "{query:?}");
    }
    for query in [
        "who does not like jazz",
        "what no longer works",
        "what did we decide not to use",
        "what did we decide to use",
    ] {
        assert!(!paths::exclusion_intent(query), "{query:?}");
    }
}

fn service(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("service".into()),
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

#[test]
fn an_exclusion_question_returns_the_item_that_lacks_the_property() {
    let e = engine(Arc::new(HashEmbedder));
    let mut rust_services = Vec::new();
    for name in ["auth-service", "billing-service"] {
        let id = remember(&e, &format!("{name} is written in Rust."), |i| {
            i.entities = vec![service(name), tool("Rust")];
            i.relations = vec![relation(name, "written in", "Rust", "")];
        });
        rust_services.push(id);
    }
    let python = remember(&e, "report-service is written in Python.", |i| {
        i.entities = vec![service("report-service"), tool("Python")];
        i.relations = vec![relation("report-service", "written in", "Python", "")];
    });
    let out = e
        .recall(&RecallInput {
            query: "which service is not in rust".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        out.memories.first().map(|m| m.id),
        Some(python),
        "the service without the property must rank first: {:?}",
        out.memories
            .iter()
            .map(|m| (m.id, m.content.as_str()))
            .collect::<Vec<_>>()
    );
    assert!(
        out.memories
            .get(1)
            .is_some_and(|m| rust_services.contains(&m.id)),
        "{:?}",
        out.memories.iter().map(|m| m.id).collect::<Vec<_>>()
    );
    // The Rust facts are the wrong answers for this question, so the fact that
    // answers it comes first.
    assert_eq!(
        out.facts
            .first()
            .map(|f| (f.source.as_str(), f.target.as_str())),
        Some(("report-service", "Python")),
        "{:?}",
        out.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
}

#[test]
fn an_exclusion_question_without_a_known_property_runs_the_plain_channels() {
    // "what database does lumen not use": no typed entities back the noun, so
    // the ordinary channels answer with the current and the past value.
    let e = engine(Arc::new(HashEmbedder));
    let sqlite = remember(&e, "Lumen uses SQLite.", |i| {
        i.entities = vec![entity("Lumen")];
        i.relations = vec![relation("Lumen", "uses", "SQLite", "Lumen uses SQLite")];
    });
    let postgres = remember(&e, "Lumen used Postgres before the switch.", |i| {
        i.entities = vec![entity("Lumen")];
        i.relations = vec![RelationIn {
            valid_from: Some("2026-01-01".into()),
            valid_to: Some("2026-09-29".into()),
            ..relation("Lumen", "used", "Postgres", "Lumen used Postgres")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "what database does lumen not use".into(),
            ..Default::default()
        })
        .unwrap();
    let got = ids(&out);
    assert!(got.contains(&sqlite), "{:?}", out.memories);
    assert!(got.contains(&postgres), "{:?}", out.memories);
    assert!(
        out.facts
            .iter()
            .any(|f| f.fact == "Lumen uses SQLite" || f.fact == "Lumen used Postgres"),
        "{:?}",
        out.facts
    );
}

#[test]
fn a_chain_link_stated_only_in_prose_is_followed() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Rui Carvalho is mentored by Marta Lobo.", |i| {
        i.relations = vec![relation(
            "Marta Lobo",
            "mentors",
            "Rui Carvalho",
            "Marta Lobo mentors Rui Carvalho",
        )];
    });
    for filler in [
        "Rui Carvalho runs the Saturday football drills.",
        "Rui Carvalho prefers espresso over filter coffee.",
        "Rui Carvalho presented the assay results.",
        "Rui Carvalho bought a new bicycle.",
        "Marta Lobo reviewed the grant budget.",
        "Marta Lobo chairs the hiring panel.",
    ] {
        remember(&e, filler, |_| {});
    }
    // No relations passed: the hometown link exists only as prose.
    let answer = remember(&e, "Marta Lobo spent her childhood in Braga.", |_| {});
    let out = e
        .recall(&RecallInput {
            query: "Which town did Rui Carvalho's mentor grow up in?".into(),
            ..Default::default()
        })
        .unwrap();
    // Only the semantic beam's prose hop reaches it as the best chain.
    let hit = out.memories.iter().find(|m| m.id == answer);
    assert!(
        hit.is_some_and(|m| m.reasons.iter().any(|r| r == "sem#1")),
        "{:?}",
        out.memories
    );
}

/// Rui/Marta chain whose last link ("grew up in Braga") is prose only.
fn prose_terminal_engine() -> (Engine, i64) {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Rui Carvalho is mentored by Marta Lobo.", |i| {
        i.relations = vec![relation(
            "Marta Lobo",
            "mentors",
            "Rui Carvalho",
            "Marta Lobo mentors Rui Carvalho",
        )];
    });
    for filler in [
        "Rui Carvalho runs the Saturday football drills.",
        "Rui Carvalho prefers espresso over filter coffee.",
        "Marta Lobo reviewed the grant budget.",
        "Marta Lobo chairs the hiring panel.",
    ] {
        remember(&e, filler, |_| {});
    }
    let answer = remember(&e, "Marta Lobo spent her childhood in Braga.", |_| {});
    (e, answer)
}

#[test]
fn a_prose_terminal_chain_renders_its_memory_as_a_fact() {
    let (mut e, answer) = prose_terminal_engine();
    let ask = |e: &Engine| {
        e.recall(&RecallInput {
            query: "Which town did Rui Carvalho's mentor grow up in?".into(),
            ..Default::default()
        })
        .unwrap()
    };
    // Opt-in: off by default.
    assert!(ask(&e).facts.iter().all(|f| !f.memory));
    e.cfg.prose_margin = 0.0;
    let out = ask(&e);
    let fact = out.facts.iter().find(|f| f.memory && f.id == answer);
    assert!(fact.is_some(), "{:?}", out.facts);
    assert!(fact.unwrap().fact.contains("Braga"));
    let text = neocortex::render::render_recall(&out, &|t| t.to_string());
    assert!(text.contains(&format!("[m{answer}] ")), "{text}");
    // Infinity switches the line off again.
    e.cfg.prose_margin = f64::INFINITY;
    assert!(ask(&e).facts.iter().all(|f| !f.memory));
}

/// Ingest an eval set the way `neocortex eval` does (same order, supersedes
/// keys resolved to ids) and return the query texts.
fn ingest_eval_set(path: &str) -> (Engine, Vec<String>) {
    let raw = std::fs::read_to_string(path).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let e = engine(Arc::new(HashEmbedder));
    let mut ids: HashMap<String, i64> = HashMap::new();
    for memory in doc["memories"].as_array().unwrap() {
        let mut input: RememberInput = serde_json::from_value(memory.clone()).unwrap();
        input.supersedes = input
            .supersedes
            .iter()
            .map(|key| format!("m{}", ids[key]))
            .collect();
        let out = e.remember(input).unwrap();
        ids.insert(memory["key"].as_str().unwrap().to_string(), out.id);
    }
    let queries = doc["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|query| query["q"].as_str().unwrap().to_string())
        .collect();
    (e, queries)
}

/// `prose_wide` may only add fact lines: on fixtures and holdout the returned
/// memories are the same ids in the same order with the knob on. Two identical
/// engines keep the access history (the retention blend) in step.
#[test]
fn prose_wide_never_changes_the_memory_ranking() {
    for path in ["eval/fixtures.json", "eval/holdout.json"] {
        let (off_engine, queries) = ingest_eval_set(path);
        let (mut on_engine, _) = ingest_eval_set(path);
        on_engine.cfg.prose_wide = true;
        for query in &queries {
            let ask = |e: &Engine| {
                e.recall(&RecallInput {
                    query: query.clone(),
                    ..Default::default()
                })
                .unwrap()
            };
            let off = ask(&off_engine);
            let on = ask(&on_engine);
            let ids = |out: &RecallOutput| -> Vec<i64> {
                out.memories.iter().map(|memory| memory.id).collect()
            };
            assert_eq!(ids(&off), ids(&on), "{path}: '{query}'");
        }
    }
}

/// nc-prose2: a chain whose middle hop is prose only ("trained at the Braga
/// observatory") reaches the context as a fact line when `prose_wide` is on,
/// without moving the memory list.
#[test]
fn prose_wide_reaches_a_middle_hop_without_moving_memories() {
    let mut e = engine(Arc::new(HashEmbedder));
    remember(&e, "Rui Carvalho is mentored by Marta Lobo.", |i| {
        i.entities = vec![entity("Rui Carvalho"), entity("Marta Lobo")];
        i.relations = vec![relation(
            "Marta Lobo",
            "mentors",
            "Rui Carvalho",
            "Marta Lobo mentors Rui Carvalho",
        )];
    });
    let bridge = remember(&e, "Marta Lobo trained at the Braga observatory.", |i| {
        i.entities = vec![entity("Marta Lobo"), entity("Braga observatory")];
    });
    // A plain memory that outranks the prose hop, so the bridge is reached
    // through the chain and not as one of the rendered memories.
    remember(&e, "The Lima array is hosted at the observatory.", |_| {});
    remember(&e, "The Braga observatory hosts the Lima array.", |i| {
        i.entities = vec![entity("Braga observatory"), entity("Lima array")];
        i.relations = vec![relation(
            "Braga observatory",
            "hosts",
            "Lima array",
            "Braga observatory hosts Lima array",
        )];
    });
    for filler in [
        "Tomas Vidal plays the cello on Sundays.",
        "Ines Barros keeps the greenhouse records.",
        "Pavel Orlov repairs vintage radios.",
        "Sofia Menezes paints the harbour at dawn.",
        "Kwame Adjei coaches the youth chess club.",
        "Lena Fischer audits the tenant ledgers.",
        "Diego Salas maps the coastal trails.",
        "Hana Okafor runs the neighbourhood food bank.",
        "Marek Dudek restores wooden boats.",
        "Priya Raghavan studies the migratory birds.",
        "Eero Nieminen tunes the church organ.",
        "Camila Duarte designs the festival posters.",
        "Yusuf Demir maintains the water pumps.",
        "Greta Lindqvist catalogues the seed library.",
        "Omar Haddad organises the winter market.",
        "Nadia Petrova teaches the folk dances.",
        "Sean O'Connor brews the seasonal ale.",
        "Mei Lin photographs the old quarter.",
    ] {
        remember(&e, filler, |_| {});
    }
    // prose_wide is scale-gated; fill to scale so the knob engages.
    at_scale(&e);
    e.cfg.prose_wide = false;
    let ask = |e: &Engine| {
        e.recall(&RecallInput {
            query: "Which array is hosted at the observatory where Rui Carvalho's mentor trained?"
                .into(),
            k: Some(1),
            ..Default::default()
        })
        .unwrap()
    };
    let off = ask(&e);
    assert!(
        !off.memories.iter().any(|m| m.id == bridge),
        "{:?}",
        off.memories
    );
    assert!(
        !off.facts.iter().any(|f| f.memory && f.id == bridge),
        "{:?}",
        off.facts
    );
    e.cfg.prose_wide = true;
    let on = ask(&e);
    assert_eq!(
        off.memories.iter().map(|m| m.id).collect::<Vec<_>>(),
        on.memories.iter().map(|m| m.id).collect::<Vec<_>>()
    );
    assert!(
        on.facts.iter().any(|f| f.memory && f.id == bridge),
        "{:?}",
        on.facts
    );
}

#[test]
fn a_history_question_still_reaches_the_second_hop() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Dana Reyes led the Mirante assay team.", |i| {
        i.valid_from = Some("2025-01-01".into());
        i.valid_to = Some("2026-01-01".into());
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            valid_to: Some("2026-01-01".into()),
            ..relation(
                "Dana Reyes",
                "leads",
                "Mirante assay team",
                "Dana Reyes led the Mirante assay team",
            )
        }];
    });
    for filler in [
        "Dana Reyes reviewed the plate reader budget.",
        "The Mirante assay team meets on Tuesdays.",
        "Dana Reyes runs the Thursday journal club.",
    ] {
        remember(&e, filler, |_| {});
    }
    let answer = remember(&e, "Dana Reyes has a severe peanut allergy.", |_| {});
    let out = e
        .recall(&RecallInput {
            query: "What is the allergy of whoever used to lead the Mirante assay team?".into(),
            ..Default::default()
        })
        .unwrap();
    let hit = out.memories.iter().find(|m| m.id == answer);
    assert!(
        hit.is_some_and(|m| m.reasons.iter().any(|r| r.starts_with("sem#"))),
        "{:?}",
        out.memories
    );
}

#[test]
fn unrelated_question_abstains_instead_of_ranking_noise() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "The office coffee machine is in the kitchen.", |_| {});
    remember(&e, "Priya leads the platform team.", |i| {
        i.entities = vec![entity("Priya")];
    });
    let out = e
        .recall(&RecallInput {
            query: "quantum chromodynamics lattice spacing".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(out.memories.is_empty(), "{:?}", out.memories);
    assert!(out.facts.is_empty());
    assert_eq!(out.candidates, 0);
}

#[test]
fn follow_and_aggregate_skip_stored_self_loops() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Robert Okafor leads payments rewrite.", |i| {
        i.entities = vec![entity("Robert Okafor"), entity("payments rewrite")];
        i.relations = vec![relation(
            "Robert Okafor",
            "lead",
            "payments rewrite",
            "Robert Okafor leads payments rewrite.",
        )];
    });
    let robert: i64 =
        e.db.sql()
            .query_row(
                "SELECT id FROM entities WHERE name='Robert Okafor'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let predicate_id: i64 =
        e.db.sql()
            .query_row("SELECT predicate_id FROM relations", [], |r| r.get(0))
            .unwrap();
    // An older build left a nickname self-loop in the database.
    e.db.sql()
        .execute(
            "INSERT INTO relations(src,dst,predicate_id,predicate_text,subj,fact,valid_from,created_at)
             VALUES(?1,?1,?2,'nickname','Robert Okafor','Robert Okafor nickname Robert Okafor',?3,?3)",
            rusqlite::params![robert, predicate_id, clock().now],
        )
        .unwrap();
    let out = paths::follow(
        &e,
        &paths::FollowInput {
            from: "Robert Okafor".into(),
            depth: Some(2),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        out.facts.iter().all(|f| f.source != f.target),
        "{:?}",
        out.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
    let agg = paths::aggregate(
        &e,
        &paths::AggregateInput {
            target: Some("Robert Okafor".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(agg.count, 0, "{:?}", agg.items);
    assert!(agg.items.is_empty(), "{:?}", agg.items);
}

#[test]
fn every_fact_channel_renders_one_predicate_spelling() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "The platform team uses Jenkins for CI.", |i| {
        i.entities = vec![entity("platform team"), entity("Jenkins")];
        i.relations = vec![relation(
            "platform team",
            "uses",
            "Jenkins",
            "platform team uses Jenkins",
        )];
    });
    remember(&e, "The platform team uses Buildkite.", |i| {
        i.entities = vec![entity("platform team"), entity("Buildkite")];
        i.relations = vec![relation(
            "platform team",
            "uses",
            "Buildkite",
            "platform team uses Buildkite",
        )];
    });
    // A query the lexical path channel answers itself: the path facts used to
    // render the stemmed canonical ("use") while the DB channel showed "uses".
    let out = e
        .recall(&RecallInput {
            query: "platform team uses Jenkins Buildkite".into(),
            ..Default::default()
        })
        .unwrap();
    let team: Vec<_> = out
        .facts
        .iter()
        .filter(|f| f.source == "platform team")
        .collect();
    assert!(!team.is_empty(), "{:?}", out.facts);
    assert!(
        team.iter().all(|f| f.predicate == "uses"),
        "{:?}",
        team.iter()
            .map(|f| (&f.predicate, &f.fact))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_negative_fact_carries_polarity_and_explicit_not_string() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Kepler does not use Redis.", |i| {
        i.entities = vec![entity("Kepler"), entity("Redis")];
        i.relations = vec![RelationIn {
            source: "Kepler".into(),
            predicate: "uses".into(),
            target: "Redis".into(),
            negated: Some(true),
            ..Default::default()
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "does Kepler use Redis".into(),
            ..Default::default()
        })
        .unwrap();
    let fact = out
        .facts
        .iter()
        .find(|f| f.target == "Redis")
        .unwrap_or_else(|| panic!("no Redis fact in {:?}", out.facts));
    assert_eq!(fact.polarity, 0);
    assert_eq!(fact.predicate, "uses");
    assert_eq!(fact.fact, "Kepler NOT uses Redis");

    // follow also carries polarity 0 and the explicit NOT fact string
    let fol = paths::follow(
        &e,
        &paths::FollowInput {
            from: "Kepler".into(),
            depth: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    let ffact = fol
        .facts
        .iter()
        .find(|f| f.target == "Redis")
        .unwrap_or_else(|| panic!("no Redis follow fact in {:?}", fol.facts));
    assert_eq!(ffact.polarity, 0);
    assert_eq!(ffact.predicate, "uses");
    assert_eq!(ffact.fact, "Kepler NOT uses Redis");

    // aggregate negative bucket also carries the explicit NOT fact string
    let agg = paths::aggregate(
        &e,
        &paths::AggregateInput {
            target: Some("Redis".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let neg = agg
        .negative
        .iter()
        .find(|item| item.name == "Kepler")
        .unwrap_or_else(|| panic!("no Kepler in negative aggregate {:?}", agg.negative));
    assert!(
        neg.facts.contains(&"Kepler NOT uses Redis".to_string()),
        "{:?}",
        neg.facts
    );
}

#[test]
fn replaced_by_renders_explicit_negation_marker() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Kepler uses Redis.", |i| {
        i.valid_from = Some("2026-01-01".into());
        i.entities = vec![entity("Kepler"), entity("Redis")];
        i.relations = vec![RelationIn {
            source: "Kepler".into(),
            predicate: "uses".into(),
            target: "Redis".into(),
            exclusive: Some(true),
            ..Default::default()
        }];
    });
    remember(&e, "Kepler does not use Redis.", |i| {
        i.valid_from = Some("2026-03-01".into());
        i.entities = vec![entity("Kepler"), entity("Redis")];
        i.relations = vec![RelationIn {
            source: "Kepler".into(),
            predicate: "uses".into(),
            target: "Redis".into(),
            negated: Some(true),
            ..Default::default()
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "Kepler Redis".into(),
            include_history: true,
            ..Default::default()
        })
        .unwrap();
    let old_fact = out
        .facts
        .iter()
        .find(|f| f.ended)
        .unwrap_or_else(|| panic!("no ended fact in {:?}", out.facts));
    let (_, repl_fact) = old_fact
        .replaced_by
        .as_ref()
        .unwrap_or_else(|| panic!("expected replaced_by in {:?}", old_fact));
    assert_eq!(repl_fact, "Kepler NOT uses Redis");

    let text = neocortex::render::render_recall(&out, &|t| t.to_string());
    assert!(
        text.contains("replaced by r2: Kepler NOT uses Redis"),
        "{text}"
    );
}

#[test]
fn follow_walks_to_a_named_target() {
    let e = engine(Arc::new(HashEmbedder));
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
    let out = paths::follow(
        &e,
        &paths::FollowInput {
            from: "Noah Tansen".into(),
            to: Some("Oslo".into()),
            depth: Some(3),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(out.reached);
    assert!(
        out.facts.iter().any(|f| f.target == "Oslo"),
        "{:?}",
        out.facts
    );
    assert!(out.facts.iter().any(|f| f.target == "Mira Shah"));
}

#[test]
fn follow_prefers_forward_edges_over_hub_noise() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Nimbus Prism is a project.", |i| {
        i.entities = vec![entity("Nimbus Prism")];
    });
    for name in ["Ada", "Bea", "Cleo", "Dora", "Eli"] {
        remember(&e, &format!("{name} prefers Nimbus Prism."), |i| {
            i.entities = vec![entity(name), entity("Nimbus Prism")];
            i.relations = vec![relation(
                name,
                "prefer",
                "Nimbus Prism",
                &format!("{name} prefers Nimbus Prism."),
            )];
        });
    }
    remember(&e, "Yash leads Nimbus Prism.", |i| {
        i.entities = vec![entity("Yash"), entity("Nimbus Prism")];
        i.relations = vec![relation(
            "Yash",
            "lead",
            "Nimbus Prism",
            "Yash leads Nimbus Prism.",
        )];
    });
    remember(&e, "Nimbus Prism uses Scala.", |i| {
        i.entities = vec![entity("Nimbus Prism"), entity("Scala")];
        i.relations = vec![relation(
            "Nimbus Prism",
            "use",
            "Scala",
            "Nimbus Prism uses Scala.",
        )];
    });
    let out = paths::follow(
        &e,
        &paths::FollowInput {
            from: "Yash".into(),
            depth: Some(2),
            k: Some(3),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        out.facts.iter().any(|f| f.target == "Scala"),
        "hub reverse edges crowded out the forward hop: {:?}",
        out.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
}

#[test]
fn aggregate_counts_entities_and_separates_negatives() {
    let e = engine(Arc::new(HashEmbedder));
    for name in ["Priya", "Anika"] {
        remember(&e, &format!("{name} works at the Solberg Lab."), |i| {
            i.entities = vec![entity(name), entity("Solberg Lab")];
            i.relations = vec![relation(
                name,
                "works at",
                "Solberg Lab",
                &format!("{name} works at the Solberg Lab."),
            )];
        });
    }
    remember(&e, "Dana does not work at the Solberg Lab.", |i| {
        i.entities = vec![entity("Dana"), entity("Solberg Lab")];
        i.relations = vec![relation(
            "Dana",
            "works at",
            "Solberg Lab",
            "Dana does not work at the Solberg Lab.",
        )];
    });
    let out = paths::aggregate(
        &e,
        &paths::AggregateInput {
            predicate: Some("works at".into()),
            target: Some("Solberg Lab".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(out.count, 2, "{:?}", out.items);
    assert!(out.items.iter().any(|item| item.name == "Priya"));
    assert!(out.items.iter().any(|item| item.name == "Anika"));
    assert_eq!(out.negative.len(), 1, "{:?}", out.negative);
    assert_eq!(out.negative[0].name, "Dana");
}

struct MapEmbedder {
    map: HashMap<String, Vec<f32>>,
}

impl Embedder for MapEmbedder {
    fn embed(&self, texts: &[&str], mode: Mode) -> Result<Vec<Vec<f32>>> {
        let fallback = HashEmbedder;
        Ok(texts
            .iter()
            .map(|text| match self.map.get(*text) {
                Some(vector) => vector.clone(),
                None => fallback.embed(&[text], mode).unwrap().remove(0),
            })
            .collect())
    }

    fn name(&self) -> &str {
        "map"
    }
}

fn basis(i: usize) -> Vec<f32> {
    let mut v = vec![0.0; DIMS];
    v[i] = 1.0;
    v
}

fn mix(a: f32, b: f32) -> Vec<f32> {
    let norm = (a * a + b * b).sqrt();
    let mut v = vec![0.0; DIMS];
    v[0] = a / norm;
    v[1] = b / norm;
    v
}

#[test]
fn entity_knn_seeds_graph_without_an_exact_name() {
    let mut map = HashMap::new();
    map.insert(
        "Kubernetes (tool): container orchestrator".to_string(),
        basis(0),
    );
    map.insert(
        "The cluster upgrade runs every quarter.".to_string(),
        basis(5),
    );
    // cosine 0.9 with the entity card, and the entity name is not in the query.
    map.insert(
        "container orchestrator upgrade schedule".to_string(),
        mix(0.9, 0.435_889_95),
    );
    let e = engine(Arc::new(MapEmbedder { map }));
    let answer = remember(&e, "The cluster upgrade runs every quarter.", |i| {
        i.entities = vec![EntityIn {
            name: "Kubernetes".into(),
            kind: Some("tool".into()),
            description: Some("container orchestrator".into()),
            ..Default::default()
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "container orchestrator upgrade schedule".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&out).contains(&answer));
    assert!(has_reason(&out, answer, "graph#"));
}

#[test]
fn relation_fact_seed_activates_both_endpoints() {
    let e = engine(Arc::new(HashEmbedder));
    let ravi_memory = remember(&e, "Ravi approved the budget.", |i| {
        i.entities = vec![entity("Ravi")];
    });
    let acme_memory = remember(&e, "Acme Corp shipped the order.", |i| {
        i.entities = vec![entity("Acme Corp")];
    });
    remember(&e, "Ravi signed the annual supply agreement.", |i| {
        i.entities = vec![entity("Ravi"), entity("Acme Corp")];
        i.relations = vec![relation(
            "Ravi",
            "signed",
            "Acme Corp",
            "Ravi signed the annual supply agreement",
        )];
    });
    let out = e
        .recall(&RecallInput {
            query: "who signed the annual supply agreement".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(ids(&out).contains(&ravi_memory));
    assert!(ids(&out).contains(&acme_memory));
    assert!(has_reason(&out, ravi_memory, "graph#"));
    assert!(has_reason(&out, acme_memory, "graph#"));
}

#[test]
fn temporal_window_ranks_the_in_window_event_first() {
    let e = engine(Arc::new(HashEmbedder));
    let in_window = remember(&e, "We shipped the beta release.", |i| {
        i.valid_from = Some("2026-10-14".into());
    });
    remember(&e, "We planned the beta release.", |i| {
        i.valid_from = Some("2026-09-01".into());
    });
    let out = e
        .recall(&RecallInput {
            query: "beta release shipped last week".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(out.memories[0].id, in_window);
    assert!(has_reason(&out, in_window, "time"));
}

#[test]
fn budget_packing_reports_omitted_memories() {
    let e = engine(Arc::new(HashEmbedder));
    for i in 0..5 {
        remember(
            &e,
            &format!(
                "quarterly compliance audit checklist item {i} {}",
                "detail ".repeat(60)
            ),
            |_| {},
        );
    }
    let out = e
        .recall(&RecallInput {
            query: "quarterly compliance audit checklist".into(),
            k: Some(5),
            budget: Some(60),
            ..Default::default()
        })
        .unwrap();
    assert!(out.omitted >= 1);
    assert!(out.memories.len() < out.candidates);
}

#[test]
fn facts_include_current_and_ended_with_history() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Ravi lives in Delhi.", |i| {
        i.entities = vec![entity("Ravi")];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            valid_to: Some("2026-01-01".into()),
            ..relation("Ravi", "lives in", "Delhi", "Ravi lives in Delhi")
        }];
    });
    remember(&e, "Ravi lives in Pune.", |i| {
        i.entities = vec![entity("Ravi")];
        i.relations = vec![RelationIn {
            valid_from: Some("2026-01-01".into()),
            ..relation("Ravi", "lives in", "Pune", "Ravi lives in Pune")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "where does Ravi live".into(),
            include_history: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(out.facts.len(), 2);
    assert!(out.facts.iter().any(|f| f.ended));
    assert!(out.facts.iter().any(|f| !f.ended));
}

#[test]
fn entity_profile_has_card_facts_and_timeline() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Priya reviews the roadmap.", |i| {
        i.entities = vec![EntityIn {
            name: "Priya".into(),
            kind: Some("person".into()),
            description: Some("the product lead".into()),
            aliases: vec!["Priya S".into()],
            ..Default::default()
        }];
    });
    remember(&e, "Priya mentors Anika.", |i| {
        i.entities = vec![entity("Priya"), entity("Anika")];
        i.relations = vec![relation("Priya", "mentors", "Anika", "Priya mentors Anika")];
    });
    let out = e.entity("Priya", 10).unwrap();
    assert_eq!(out.card.name, "Priya");
    assert_eq!(out.card.kind, "person");
    assert!(out.card.aliases.contains(&"priya s".to_string()));
    assert_eq!(out.timeline.len(), 2);
    assert!(!out.current_facts.is_empty());
}

#[test]
fn overview_finds_two_communities() {
    let e = engine(Arc::new(HashEmbedder));
    for (source, target) in [("Alpha", "Beta"), ("Beta", "Gamma"), ("Gamma", "Alpha")] {
        remember(&e, &format!("{source} works with {target}."), |i| {
            i.entities = vec![
                EntityIn {
                    name: source.into(),
                    kind: Some("project".into()),
                    ..Default::default()
                },
                EntityIn {
                    name: target.into(),
                    kind: Some("project".into()),
                    ..Default::default()
                },
            ];
            i.relations = vec![relation(
                source,
                "works with",
                target,
                &format!("{source} works with {target}"),
            )];
        });
    }
    for (source, target) in [("Delta", "Epsilon"), ("Epsilon", "Zeta"), ("Zeta", "Delta")] {
        remember(&e, &format!("{source} works with {target}."), |i| {
            i.entities = vec![entity(source), entity(target)];
            i.relations = vec![relation(
                source,
                "works with",
                target,
                &format!("{source} works with {target}"),
            )];
        });
    }
    let out = e.overview(None, 10).unwrap();
    assert!(out.communities.len() >= 2);
    assert!(out.communities.iter().map(|c| c.size).sum::<usize>() >= 6);
    assert!(out.communities.iter().all(|c| c.representative.is_some()));
}

#[test]
fn overview_drops_junk_and_chooses_typed_labels_and_representative() {
    let e = engine(Arc::new(HashEmbedder));
    let representative = remember(&e, "Alpha Beta Alpha roadmap decision with details.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Alpha".into(),
                kind: Some("project".into()),
                ..Default::default()
            },
            EntityIn {
                name: "Beta".into(),
                kind: Some("project".into()),
                ..Default::default()
            },
        ];
        i.relations = vec![relation(
            "Alpha",
            "works with",
            "Beta",
            "Alpha works with Beta",
        )];
    });
    remember(&e, "Alpha release planning.", |i| {
        i.entities = vec![EntityIn {
            name: "Alpha".into(),
            kind: Some("project".into()),
            ..Default::default()
        }];
    });
    remember(&e, "junk only.", |i| {
        i.entities = vec![entity("junk")];
    });
    let out = e.overview(None, 10).unwrap();
    assert_eq!(out.communities.len(), 1);
    let community = &out.communities[0];
    assert_eq!(community.entities[0].name, "Alpha");
    assert_eq!(
        community.representative.as_ref().unwrap().id,
        representative
    );
    assert!(community.representative.as_ref().unwrap().content.len() < 161);
}

#[test]
fn recall_bumps_access_count() {
    let e = engine(Arc::new(HashEmbedder));
    let id = remember(&e, "The backup runs nightly.", |_| {});
    let before: i64 =
        e.db.sql()
            .query_row("SELECT access_count FROM memories WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
    e.recall(&RecallInput {
        query: "backup nightly".into(),
        ..Default::default()
    })
    .unwrap();
    let (after, last): (i64, Option<i64>) =
        e.db.sql()
            .query_row(
                "SELECT access_count,last_access FROM memories WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(after, before + 1);
    assert_eq!(last, Some(clock().now));
}

#[test]
fn sweep_ingests_once_and_reports_each_variant() {
    let dir = TempDir::new("eval-sweep");
    let path = dir.join("sweep.json");
    std::fs::write(
        &path,
        r#"{
            "now": "2026-10-21T15:00:00Z",
            "memories": [
                {"key":"a","content":"The office is in Pune.","kind":"fact","tags":[],"importance":0.5,"valid_from":"2026-01-01"},
                {"key":"b","content":"The backup runs nightly.","kind":"fact","tags":[],"importance":0.5,"valid_from":"2026-01-01"}
            ],
            "queries": [{"q":"office","category":"keyword","expect":["a"]}]
        }"#,
    )
    .unwrap();
    let base = Config::default();
    let variants = vec![
        ("one".to_string(), base.clone()),
        ("two".to_string(), base.clone()),
    ];
    let rows = neocortex::eval::sweep(&path, Arc::new(HashEmbedder), base, variants).unwrap();
    assert_eq!(rows.len(), 2);
    for (_, report) in &rows {
        assert_eq!(report.ingested, 2, "fixture must be ingested exactly once");
        assert_eq!(report.queries, 1);
    }
}

#[test]
fn entity_lookup_of_a_shared_first_name_lists_the_other_person() {
    let e = engine(Arc::new(HashEmbedder));
    remember(
        &e,
        "Maya Chen is a data engineer on the analytics pipeline.",
        |i| {
            i.entities = vec![EntityIn {
                name: "Maya Chen".into(),
                kind: Some("person".into()),
                description: Some("data engineer".into()),
                ..Default::default()
            }];
        },
    );
    remember(
        &e,
        "Maya Lindqvist is a product designer on the onboarding flow.",
        |i| {
            i.entities = vec![EntityIn {
                name: "Maya Lindqvist".into(),
                kind: Some("person".into()),
                description: Some("product designer".into()),
                ..Default::default()
            }];
        },
    );
    let out = e.entity("Maya", 10).unwrap();
    assert!(
        out.card.name == "Maya Chen" || out.card.name == "Maya Lindqvist",
        "{}",
        out.card.name
    );
    // H2-13: a bare first name shared by 2 people is flagged, and every
    // person with that first name is listed (the card included).
    assert_eq!(out.status.as_deref(), Some("ambiguous"), "{out:#?}");
    assert_eq!(out.candidates.len(), 2, "{out:#?}");
    assert!(out.candidates.iter().any(|c| c.id == out.card.id));
    assert!(out.candidates.iter().any(|c| c.name == "Maya Chen"));
    assert!(out.candidates.iter().any(|c| c.name == "Maya Lindqvist"));
    assert!(out.candidates.iter().all(|c| c.kind == "person"));
    assert!(out.candidates.iter().all(|c| !c.description.is_empty()));

    // An exact full-name lookup stays unambiguous.
    let exact = e.entity("Maya Chen", 10).unwrap();
    assert_eq!(exact.card.name, "Maya Chen");
    assert!(exact.candidates.is_empty(), "{exact:#?}");
    assert!(exact.status.is_none(), "{exact:#?}");
}

#[test]
fn entity_lookup_includes_a_non_person_card_named_like_the_token() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Jasmine is the analytics project.", |i| {
        i.entities = vec![EntityIn {
            name: "Jasmine".into(),
            kind: Some("project".into()),
            ..Default::default()
        }];
    });
    remember(&e, "Jasmine Diaz leads the analytics project.", |i| {
        i.entities = vec![EntityIn {
            name: "Jasmine Diaz".into(),
            kind: Some("person".into()),
            ..Default::default()
        }];
    });
    remember(&e, "Jasmine Court hosts the standup.", |i| {
        i.entities = vec![EntityIn {
            name: "Jasmine Court".into(),
            kind: Some("place".into()),
            ..Default::default()
        }];
    });
    let out = e.entity("Jasmine", 10).unwrap();
    assert_eq!(out.card.name, "Jasmine");
    assert_eq!(out.status.as_deref(), Some("ambiguous"), "{out:#?}");
    // V2: the non-person card named exactly like the token is listed, not
    // only the people whose first name it is.
    assert!(
        out.candidates.iter().any(|c| c.id == out.card.id),
        "{out:#?}"
    );
    assert!(out.candidates.iter().any(|c| c.name == "Jasmine Diaz"));
    // A place that only starts with the token is a prefix match, not a homonym.
    assert!(
        !out.candidates.iter().any(|c| c.name == "Jasmine Court"),
        "{out:#?}"
    );
}

#[test]
fn entity_lookup_caps_the_homonym_list_and_reports_the_rest() {
    let e = engine(Arc::new(HashEmbedder));
    for i in 0..12 {
        remember(
            &e,
            &format!("Jasmine Person{i} works on pipeline {i}."),
            |inp| {
                inp.entities = vec![EntityIn {
                    name: format!("Jasmine Person{i}"),
                    kind: Some("person".into()),
                    ..Default::default()
                }];
            },
        );
    }
    let out = e.entity("Jasmine", 10).unwrap();
    assert_eq!(out.status.as_deref(), Some("ambiguous"), "{out:#?}");
    assert_eq!(out.candidates.len(), 10, "list is capped: {out:#?}");
    assert_eq!(out.candidates_more, Some(2), "{out:#?}");
    // The card itself stays in the capped list, so "the card included" holds
    // even when it is not among the most-mentioned ten.
    assert_eq!(out.candidates[0].id, out.card.id, "{out:#?}");
}

#[test]
fn entity_lookup_of_a_unique_first_name_is_unflagged() {
    let e = engine(Arc::new(HashEmbedder));
    remember(&e, "Jun Park owns the billing service.", |i| {
        i.entities = vec![EntityIn {
            name: "Jun Park".into(),
            kind: Some("person".into()),
            ..Default::default()
        }];
    });
    let out = e.entity("Jun", 10).unwrap();
    assert_eq!(out.card.name, "Jun Park");
    assert!(out.status.is_none(), "{out:#?}");
    assert!(out.candidates.is_empty(), "{out:#?}");
}

#[test]
fn entity_lookup_falls_back_to_prefix_and_suggests_closest() {
    let engine = engine(Arc::new(HashEmbedder));
    remember(&engine, "The lab runs UltraBERT-Embed-150M on CPU.", |m| {
        m.entities = vec![entity("UltraBERT-Embed-150M")];
    });
    let out = engine.entity("UltraBERT", 5).unwrap();
    assert_eq!(out.card.name, "UltraBERT-Embed-150M");
    let err = engine.entity("zzqx unknown", 5).unwrap_err().to_string();
    assert!(
        err.contains("unknown entity") && err.contains("closest:"),
        "{err}"
    );
}

// ---------------------------------------------------------------- nc-status H2-15/16

#[test]
fn status_intent_only_fires_on_unresolved_questions() {
    for query in [
        "what bug is still open in lumen",
        "which issues are unresolved",
        "what is still broken",
        "which bugs are not fixed yet",
        "any open bugs in lumen",
        "what defects are outstanding",
    ] {
        assert!(paths::status_intent(query), "{query:?}");
    }
    for query in [
        "what bugs did we fix",
        "what did we fix in v0.3.1",
        "what is the open question about workspaces",
        "why is the sky blue",
        "what is Mira working on",
    ] {
        assert!(!paths::status_intent(query), "{query:?}");
    }
}

#[test]
fn a_still_open_question_ranks_the_unfixed_bug_first() {
    let e = engine(Arc::new(HashEmbedder));
    let bug = remember(
        &e,
        "Bug on 2026-09-18: Lumen's sync loop panicked with DatabaseBusy when two devices synced at once.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-09-18".into());
            i.entities = vec![entity("Lumen")];
        },
    );
    let fixed = remember(
        &e,
        "Fix for the 2026-09-18 DatabaseBusy panic: set PRAGMA busy_timeout=5000 and retried the transaction once.",
        |i| {
            i.kind = Some("procedure".into());
            i.valid_from = Some("2026-09-19".into());
            i.entities = vec![entity("Lumen")];
        },
    );
    let open = remember(
        &e,
        "A user reported on 2026-10-15 that offline edits from two devices lost one note body; suspected bug in merge_note_versions.",
        |i| {
            i.kind = Some("observation".into());
            i.valid_from = Some("2026-10-15".into());
            i.entities = vec![entity("Lumen")];
        },
    );
    let question = remember(
        &e,
        "Open question 2026-10-15: whether Lumen should support team workspaces in v0.5; the decision was deferred.",
        |i| {
            i.kind = Some("note".into());
            i.valid_from = Some("2026-10-16".into());
            i.entities = vec![entity("Lumen")];
        },
    );
    let out = e
        .recall(&RecallInput {
            query: "what bug is still open in lumen".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let position = |id: i64| out.memories.iter().position(|m| m.id == id);
    let shown: Vec<(i64, &str)> = out
        .memories
        .iter()
        .map(|m| (m.id, m.content.as_str()))
        .collect();
    assert_eq!(out.memories.first().map(|m| m.id), Some(open), "{shown:?}");
    assert!(
        position(open) < position(bug),
        "the open bug must outrank the fixed one: {shown:?}"
    );
    assert!(
        position(open) < position(fixed),
        "the open bug must outrank its fix: {shown:?}"
    );
    assert!(
        position(open) < position(question),
        "an open question is not an open bug: {shown:?}"
    );
    // Re-ranking only: the fixed bug is demoted, never dropped.
    assert!(position(bug).is_some(), "{shown:?}");
}

#[test]
fn a_status_question_without_bugs_changes_nothing() {
    let mut e = engine(Arc::new(HashEmbedder));
    remember(&e, "Lumen is written in Rust.", |i| {
        i.entities = vec![entity("Lumen")];
    });
    remember(&e, "Lumen uses SQLite in WAL mode.", |i| {
        i.entities = vec![entity("Lumen")];
    });
    let query = "what bug is still open in lumen".to_string();
    let on = e
        .recall(&RecallInput {
            query: query.clone(),
            ..Default::default()
        })
        .unwrap();
    e.cfg.status_rank = false;
    let base = e
        .recall(&RecallInput {
            query,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(ids(&on), ids(&base));
}

#[test]
fn a_why_question_follows_the_cause_of_the_answering_memory() {
    let e = engine(Arc::new(HashEmbedder));
    let fix = remember(
        &e,
        "Mira Shah fixed the 2026-08-14 outage by adding a circuit breaker to the payments client.",
        |i| {
            i.entities = vec![entity("Mira Shah"), service("payments client")];
        },
    );
    let cause = remember(
        &e,
        "The 2026-08-14 outage was caused by a retry storm from the payments client, which amplified the failure.",
        |i| {
            i.entities = vec![service("payments client")];
            i.relations = vec![relation(
                "retry storm",
                "caused",
                "2026-08-14 outage",
                "retry storm caused 2026-08-14 outage",
            )];
        },
    );
    remember(
        &e,
        "Mira Shah prefers async written updates over meetings.",
        |i| {
            i.entities = vec![entity("Mira Shah")];
        },
    );
    let out = e
        .recall(&RecallInput {
            query: "why did Mira add a circuit breaker".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let shown: Vec<(i64, &str)> = out
        .memories
        .iter()
        .map(|m| (m.id, m.content.as_str()))
        .collect();
    assert_eq!(out.memories.first().map(|m| m.id), Some(fix), "{shown:?}");
    assert!(
        out.memories.iter().any(|m| m.id == cause),
        "the cause memory must be added: {shown:?}"
    );
    assert!(has_reason(&out, cause, "cause"), "{shown:?}");
    assert!(
        out.facts.iter().any(|f| f.source == "retry storm"),
        "the cause fact must be rendered: {:?}",
        out.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
}

#[test]
fn status_intent_needs_a_defect_in_the_question() {
    for query in [
        "what bug is still open in lumen",
        "which issues are unresolved",
        "what is still broken",
        "what is still failing in the sync loop",
        "which bugs are not fixed yet",
        "any open bugs in lumen",
        "what defects are outstanding",
        "which crashes are unresolved",
        "which incident tickets are still open for the 500 error",
    ] {
        assert!(paths::status_intent(query), "{query:?}");
    }
    // "still open" / "outstanding" describe a state, not a defect: a plain
    // question about an API or a PR is not a status question.
    for query in [
        "is the API still open to the public",
        "is the API gateway still open to the public",
        "which PRs are outstanding",
        "is the office still open on fridays",
        "which tickets are outstanding for the offsite",
        "what is the open question about workspaces",
    ] {
        assert!(!paths::status_intent(query), "{query:?}");
    }
}

#[test]
fn an_open_ended_question_is_not_a_status_question() {
    let mut e = engine(Arc::new(HashEmbedder));
    let api = remember(
        &e,
        "The API is open to the public; anonymous requests are rate limited to 60 per minute.",
        |i| {
            i.entities = vec![entity("API")];
        },
    );
    let gateway = remember(
        &e,
        "The API gateway is open to the public and routes anonymous traffic to the billing service.",
        |i| {
            i.entities = vec![entity("API gateway")];
        },
    );
    let pr = remember(
        &e,
        "PR 412 adds the export endpoint and is awaiting review.",
        |i| {
            i.entities = vec![entity("PR 412")];
        },
    );
    // Unrelated defect reports in the same database.
    remember(
        &e,
        "The ingest worker failed with a 500 error on 2026-10-02.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-02".into());
        },
    );
    remember(
        &e,
        "The smoke test failed against staging on 2026-10-03.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-03".into());
        },
    );
    for (query, allowed) in [
        ("is the API still open to the public", [api, gateway]),
        (
            "is the API gateway still open to the public",
            [gateway, api],
        ),
        ("which PRs are outstanding", [pr, pr]),
    ] {
        let on = e
            .recall(&RecallInput {
                query: query.into(),
                k: Some(8),
                ..Default::default()
            })
            .unwrap();
        let shown: Vec<(i64, &str)> = on
            .memories
            .iter()
            .map(|m| (m.id, m.content.as_str()))
            .collect();
        assert!(
            on.memories.first().is_some_and(|m| allowed.contains(&m.id)),
            "{query:?}: {shown:?}"
        );
        e.cfg.status_rank = false;
        let base = e
            .recall(&RecallInput {
                query: query.into(),
                k: Some(8),
                ..Default::default()
            })
            .unwrap();
        e.cfg.status_rank = true;
        assert_eq!(ids(&on), ids(&base), "{query:?}: {shown:?}");
    }
}

#[test]
fn an_unfixed_crash_report_is_ranked_as_open() {
    let e = engine(Arc::new(HashEmbedder));
    let crash = remember(
        &e,
        "The nightly export crashes with a null pointer in the CSV writer.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-15".into());
        },
    );
    remember(
        &e,
        "The nightly export writes one CSV file per table to the export bucket.",
        |i| {
            i.kind = Some("fact".into());
            i.valid_from = Some("2026-10-14".into());
        },
    );
    let out = e
        .recall(&RecallInput {
            query: "what is still broken in the export".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let shown: Vec<(i64, &str)> = out
        .memories
        .iter()
        .map(|m| (m.id, m.content.as_str()))
        .collect();
    assert!(
        has_reason(&out, crash, "status"),
        "not ranked as open: {shown:?}"
    );
    assert_eq!(out.memories.first().map(|m| m.id), Some(crash), "{shown:?}");
}

#[test]
fn a_report_that_says_not_fixed_yet_is_still_open() {
    let e = engine(Arc::new(HashEmbedder));
    let asked = remember(
        &e,
        "Bug 2026-10-08: the nightly export crashes with a null pointer in the CSV writer; it is not fixed yet.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-08".into());
        },
    );
    remember(
        &e,
        "Bug 2026-10-07: the search index rebuild hangs on large vaults.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-07".into());
        },
    );
    remember(
        &e,
        "Bug 2026-10-05: the login fails with a 500 when the session cookie is missing.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-05".into());
        },
    );
    for (day, fact) in [
        (
            1,
            "The nightly export writes one CSV file per table to the export bucket.",
        ),
        (2, "The export bucket keeps objects for 30 days."),
        (3, "The CSV writer quotes fields that contain a comma."),
        (4, "The search index is rebuilt on every deploy."),
    ] {
        remember(&e, fact, |i| {
            i.kind = Some("fact".into());
            i.valid_from = Some(format!("2026-10-{day:02}"));
        });
    }
    let out = e
        .recall(&RecallInput {
            query: "is the nightly export bug still open".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let shown: Vec<(i64, &str)> = out
        .memories
        .iter()
        .map(|m| (m.id, m.content.as_str()))
        .collect();
    assert_eq!(out.memories.first().map(|m| m.id), Some(asked), "{shown:?}");
    assert!(has_reason(&out, asked, "status"), "{shown:?}");
}

#[test]
fn a_shared_date_alone_does_not_close_a_bug() {
    // A release note dated the same day as a bug report is not its fix: the
    // two share a date and nothing else, so the report stays open and keeps
    // the top answer instead of being halved below a plain fact.
    let e = engine(Arc::new(HashEmbedder));
    let bug = remember(
        &e,
        "Bug 2026-10-15: the nightly export crashes with a null pointer in the CSV writer; it is not fixed yet.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-15".into());
        },
    );
    remember(
        &e,
        "Lumen v0.9.0 was released on 2026-10-15 with the new dashboard layout.",
        |i| {
            i.kind = Some("note".into());
            i.valid_from = Some("2026-10-15".into());
            i.entities = vec![entity("Lumen")];
        },
    );
    for (day, fact) in [
        (
            14,
            "The nightly export writes one CSV file per table to the export bucket.",
        ),
        (14, "The export bucket keeps objects for 30 days."),
        (13, "The CSV writer quotes fields that contain a comma."),
    ] {
        remember(&e, fact, |i| {
            i.kind = Some("fact".into());
            i.valid_from = Some(format!("2026-10-{day:02}"));
        });
    }
    let out = e
        .recall(&RecallInput {
            query: "what bugs are still open".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let shown: Vec<(i64, &str)> = out
        .memories
        .iter()
        .map(|m| (m.id, m.content.as_str()))
        .collect();
    assert_eq!(out.memories.first().map(|m| m.id), Some(bug), "{shown:?}");
    assert!(
        has_reason(&out, bug, "status"),
        "not ranked as open: {shown:?}"
    );
}

#[test]
fn a_question_about_a_fixed_bug_keeps_the_fix_in_front() {
    let e = engine(Arc::new(HashEmbedder));
    let bug = remember(
        &e,
        "Bug 2026-10-05: the login fails with a 500 when the session cookie is missing.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-05".into());
        },
    );
    let fix = remember(
        &e,
        "Fix for the 2026-10-05 login bug: the session cookie is now signed before it is stored.",
        |i| {
            i.kind = Some("procedure".into());
            i.valid_from = Some("2026-10-06".into());
        },
    );
    let export = remember(
        &e,
        "Bug 2026-10-02: the nightly export crashes with a null pointer in the CSV writer.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-02".into());
        },
    );
    let index = remember(
        &e,
        "Bug 2026-10-03: the search index rebuild hangs on large vaults.",
        |i| {
            i.kind = Some("mistake".into());
            i.valid_from = Some("2026-10-03".into());
        },
    );
    let out = e
        .recall(&RecallInput {
            query: "is the login bug still open".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let shown: Vec<(i64, &str)> = out
        .memories
        .iter()
        .map(|m| (m.id, m.content.as_str()))
        .collect();
    let position = |id: i64| out.memories.iter().position(|m| m.id == id);
    assert!(
        matches!(out.memories.first().map(|m| m.id), Some(first) if first == fix || first == bug),
        "the asked bug or its fix must answer: {shown:?}"
    );
    for id in [fix, bug] {
        for other in [export, index] {
            assert!(
                position(id) < position(other),
                "an unrelated open bug outranks the asked one: {shown:?}"
            );
        }
    }
}

#[test]
fn the_status_answer_does_not_depend_on_ingestion_order() {
    let subjects = [
        "proxy", "cache", "billing", "search", "export", "import", "auth", "session", "webhook",
        "queue", "worker", "invoice", "mailer", "router", "gateway", "storage", "backup",
        "restore", "report", "digest", "ledger", "hooks", "plugins", "themes",
    ];
    let build = |scheduler_at: usize| -> (Engine, i64) {
        let e = engine(Arc::new(HashEmbedder));
        let mut scheduler = 0;
        for (i, subject) in subjects.iter().enumerate() {
            let content = if i == scheduler_at {
                "Bug 2026-10-02: the scheduler job fails with a 500 error on retry.".to_string()
            } else {
                format!("Bug 2026-10-01: the {subject} path fails with a 500 error on retry.")
            };
            let id = remember(&e, &content, |i| {
                i.kind = Some("mistake".into());
                i.valid_from = Some("2026-10-01".into());
            });
            if i == scheduler_at {
                scheduler = id;
            }
        }
        (e, scheduler)
    };
    let mut tops = Vec::new();
    for (at, label) in [(0, "first"), (22, "last")] {
        let (e, scheduler) = build(at);
        let out = e
            .recall(&RecallInput {
                query: "is the scheduler bug still open".into(),
                k: Some(8),
                ..Default::default()
            })
            .unwrap();
        let shown: Vec<(i64, &str)> = out
            .memories
            .iter()
            .map(|m| (m.id, m.content.as_str()))
            .collect();
        assert_eq!(
            out.memories.first().map(|m| m.id),
            Some(scheduler),
            "scheduler bug stored {label}: {shown:?}"
        );
        assert!(has_reason(&out, scheduler, "status"), "{label}: {shown:?}");
        tops.push(out.memories.first().map(|m| m.content.clone()));
    }
    assert_eq!(tops[0], tops[1]);
}

#[test]
fn a_why_question_renders_each_cause_fact_once() {
    let e = engine(Arc::new(HashEmbedder));
    remember(
        &e,
        "The 2026-08-14 outage was caused by a retry storm from the payments client.",
        |i| {
            i.entities = vec![service("payments client")];
            i.relations = vec![relation(
                "retry storm",
                "caused",
                "2026-08-14 outage",
                "retry storm caused 2026-08-14 outage",
            )];
        },
    );
    remember(
        &e,
        "Mira Shah reviewed the incident report afterwards.",
        |i| {
            i.entities = vec![entity("Mira Shah")];
        },
    );
    let out = e
        .recall(&RecallInput {
            query: "why did the retry storm cause the 2026-08-14 outage".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let lines: Vec<&str> = out.facts.iter().map(|f| f.fact.as_str()).collect();
    let mut unique: Vec<i64> = out.facts.iter().map(|f| f.id).collect();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        out.facts.len(),
        "duplicate fact lines: {lines:?}"
    );
    assert!(
        lines.iter().any(|f| f.contains("retry storm")),
        "the cause fact must be rendered: {lines:?}"
    );
}

#[test]
fn a_why_question_ignores_a_cause_that_only_shares_a_hub_entity() {
    let mut e = engine(Arc::new(HashEmbedder));
    let slow = remember(
        &e,
        "The search service got slow after the 2026-10-10 deploy; p99 latency went from 80 ms to 900 ms.",
        |i| {
            i.entities = vec![service("search service"), entity("Platform team")];
        },
    );
    let storm = remember(
        &e,
        "The 2026-10-11 retry storm hit the payments client.",
        |i| {
            i.entities = vec![service("payments client"), entity("Platform team")];
            i.relations = vec![relation(
                "retry storm",
                "caused",
                "payments client outage",
                "retry storm caused payments client outage",
            )];
        },
    );
    // The team is a hub: every standup note mentions it, so sharing it says
    // nothing about what caused the search slowdown.
    for day in 1..=6 {
        remember(
            &e,
            &format!("Platform team standup on 2026-10-{day:02} covered the on-call rotation."),
            |i| {
                i.entities = vec![entity("Platform team")];
            },
        );
    }
    let out = e
        .recall(&RecallInput {
            query: "why is the search service slow".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    let shown: Vec<(i64, &str)> = out
        .memories
        .iter()
        .map(|m| (m.id, m.content.as_str()))
        .collect();
    assert_eq!(out.memories.first().map(|m| m.id), Some(slow), "{shown:?}");
    assert!(
        !has_reason(&out, storm, "cause"),
        "hub cause followed: {shown:?}"
    );
    // Nothing about the hub-only cause reaches the answer, facts included.
    let facts_on: Vec<i64> = out.facts.iter().map(|f| f.id).collect();
    e.cfg.cause_follow = false;
    let base = e
        .recall(&RecallInput {
            query: "why is the search service slow".into(),
            k: Some(8),
            ..Default::default()
        })
        .unwrap();
    e.cfg.cause_follow = true;
    assert_eq!(ids(&out), ids(&base), "{shown:?}");
    assert_eq!(
        facts_on,
        base.facts.iter().map(|f| f.id).collect::<Vec<_>>(),
        "{:?}",
        out.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
}

#[test]
fn a_why_question_without_causes_is_unchanged() {
    let mut e = engine(Arc::new(HashEmbedder));
    remember(
        &e,
        "Mira Shah prefers async written updates over meetings.",
        |i| {
            i.entities = vec![entity("Mira Shah")];
        },
    );
    remember(&e, "Mira Shah has a cat named Poppy.", |i| {
        i.entities = vec![entity("Mira Shah")];
    });
    let query = "why is the sky blue".to_string();
    let on = e
        .recall(&RecallInput {
            query: query.clone(),
            ..Default::default()
        })
        .unwrap();
    e.cfg.cause_follow = false;
    let base = e
        .recall(&RecallInput {
            query,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(ids(&on), ids(&base));
    assert!(on.facts.iter().all(|f| f.source != "retry storm"));
}

/// A store above the anchor channel's scale gate (50k memories): the same
/// tests run unlocked at 108k. Bulk-inserted (the crowd) through plain SQL
/// so the test does not ingest 50k memories through `remember`.
fn at_scale(e: &Engine) {
    let sql = e.db.sql();
    sql.execute_batch("BEGIN").unwrap();
    let mut st = sql
        .prepare(
            "INSERT INTO memories(content,kind,tags,importance,created_at,valid_from,content_hash) VALUES (?1,'note','',0.5,0,0,?1)",
        )
        .unwrap();
    for n in 0..50_000 {
        st.execute([format!("fill {n}")]).unwrap();
    }
    sql.execute_batch("COMMIT").unwrap();
    drop(st);
}

// A gold memory that only the graph channel can see (an exact anchor, no
// shared query words and no BM25 term) must still answer a question whose
// top dense/BM25 hits belong to other memories (nc-hay1: at 108k distractors
// a graph#1 hit loses the fused ranking to 2-channel modest hits).
#[test]
fn anchored_gold_reaches_top_k_despite_dense_crowding() {
    let mut e = engine(Arc::new(HashEmbedder));
    // Access recording must not let comparisons drift between calls.
    e.cfg.w_retention = 0.0;
    at_scale(&e);
    let gold = remember(
        &e,
        "The Titan Krios G4 is the lab's main high-resolution cryo-EM scope.",
        |i| {
            i.entities = vec![EntityIn {
                name: "Titan Krios G4".into(),
                aliases: vec!["microscope".into()],
                ..Default::default()
            }];
        },
    );
    // Distractors that merely repeat the question's words ("book a session"):
    // they take the top BM25 + vector ranks but say nothing about the anchor.
    for n in 0..6 {
        remember(
            &e,
            &format!("Book a session slot for the polishing machine, visit {n}."),
            |i| {
                i.entities = vec![entity("Polishing Machine")];
            },
        );
    }
    let query = RecallInput {
        query: "book a session on the microscope".into(),
        k: Some(5),
        ..Default::default()
    };
    e.cfg.w_anchor = 0.0;
    let off = e.recall(&query).unwrap();
    assert!(
        !ids(&off).contains(&gold),
        "precondition: without the anchor channel the gold is buried by crowded distractors"
    );
    e.cfg.w_anchor = 2.5;
    let on = e.recall(&query).unwrap();
    assert!(
        ids(&on).contains(&gold),
        "anchored gold must reach top-k despite dense distractors"
    );
    // And it should carry the anchor reason while doing so.
    assert!(
        has_reason(&on, gold, "anchor#"),
        "gold should get an anchor-channel reason: {:?}",
        on.memories
            .iter()
            .find(|m| m.id == gold)
            .map(|m| &m.reasons)
    );
}

// A list question names a category by the tags the caller stored: its members
// must reach the answer even when every one of them ranks far behind the
// crowd's memories (nc-agg). Scale-gated (50k) and shape-gated like the anchor
// channel, so the same store answers the same way with the channel off.
fn tag_store(e: &Engine) -> Vec<i64> {
    // The crowd: memories that repeat the question's words but are not the
    // category (a policy note about vacations, not a trip taken).
    for n in 0..12 {
        remember(
            e,
            &format!(
                "Personal vacation policy note {n}: request recreational days through the portal."
            ),
            |_| {},
        );
    }
    let mut golds = Vec::new();
    for (text, tag) in [
        (
            "Spent seven days in Paris during spring, admiring Impressionist masterpieces.",
            "vacation",
        ),
        (
            "Drove up Trail Ridge Road above twelve thousand feet during a camping trip.",
            "vacation",
        ),
        (
            "Took a five-day autumn vacation to London, staying in Covent Garden.",
            "vacation",
        ),
        (
            "Caught a Broadway performance of Hadestown and walked the High Line.",
            "vacation",
        ),
    ] {
        golds.push(remember(e, text, |i| i.tags = vec![tag.into()]));
    }
    // A career note that a loose prefix match ("care") would collect.
    remember(
        e,
        "Sarah Mercer is a landscape painter and botanist.",
        |i| i.tags = vec!["career".into()],
    );
    golds
}

#[test]
fn a_list_question_reaches_its_tagged_members_at_scale() {
    let mut e = engine(Arc::new(HashEmbedder));
    e.cfg.w_retention = 0.0;
    at_scale(&e);
    let golds = tag_store(&e);
    let query = RecallInput {
        query: "all personal vacations and recreational trips taken".into(),
        k: Some(8),
        ..Default::default()
    };
    e.cfg.w_tag = 0.0;
    let off = e.recall(&query).unwrap();
    let reached = golds.iter().filter(|g| ids(&off).contains(g)).count();
    e.cfg.w_tag = 4.0;
    let on = e.recall(&query).unwrap();
    let on_reached = golds.iter().filter(|g| ids(&on).contains(g)).count();
    assert!(
        on_reached >= 3 && on_reached > reached,
        "tag members must answer a list question: off {reached}/4, on {on_reached}/4"
    );
    assert!(
        has_reason(&on, golds[0], "tag#"),
        "a tagged member should carry the tag channel: {:?}",
        on.memories
            .iter()
            .find(|m| m.id == golds[0])
            .map(|m| &m.reasons)
    );
}

#[test]
fn the_tag_channel_is_shape_gated_and_exact() {
    let mut e = engine(Arc::new(HashEmbedder));
    e.cfg.w_retention = 0.0;
    at_scale(&e);
    tag_store(&e);
    // A single-answer question that names the same tag word keeps the plain
    // ranking: the category would flood it.
    let single = RecallInput {
        query: "which vacation trip was the longest".into(),
        k: Some(5),
        ..Default::default()
    };
    e.cfg.w_tag = 0.0;
    let base = e.recall(&single).unwrap();
    e.cfg.w_tag = 4.0;
    let gated = e.recall(&single).unwrap();
    assert_eq!(ids(&base), ids(&gated));
    assert!(gated
        .memories
        .iter()
        .all(|m| !m.reasons.iter().any(|r| r.starts_with("tag#"))));
    // A habitual "every <occasion>" asks for one member too: the tag word it
    // names ("vacation") must not collect the whole category (round 2).
    let habitual = RecallInput {
        query: "which vacation did Ana take every winter".into(),
        k: Some(5),
        ..Default::default()
    };
    e.cfg.w_tag = 0.0;
    let habitual_base = serde_json::to_string(&e.recall(&habitual).unwrap()).unwrap();
    e.cfg.w_tag = 4.0;
    let habitual_on = serde_json::to_string(&e.recall(&habitual).unwrap()).unwrap();
    assert_eq!(
        habitual_base, habitual_on,
        "a habitual every <occasion> question must be byte-identical with w_tag=0"
    );
    // A list question whose word only prefixes a tag ("care" / `career`) does
    // not collect that category.
    let prefix = RecallInput {
        query: "all personal care arrangements".into(),
        k: Some(8),
        ..Default::default()
    };
    let out = e.recall(&prefix).unwrap();
    assert!(out
        .memories
        .iter()
        .all(|m| !m.reasons.iter().any(|r| r.starts_with("tag#"))));
}

#[test]
fn anchor_knob_parses_and_defaults_on() {
    let mut cfg = Config::default();
    assert_eq!(cfg.w_anchor, 2.5);
    neocortex::eval::set_config(&mut cfg, "w_anchor", "0").unwrap();
    assert_eq!(cfg.w_anchor, 0.0);
    neocortex::eval::set_config(&mut cfg, "w_anchor", "2.5").unwrap();
    assert_eq!(cfg.w_anchor, 2.5);
    assert!(neocortex::eval::set_config(&mut cfg, "w_anchor", "many").is_err());
}

#[test]
fn anchor_knob_zero_is_byte_identical() {
    let mut e = engine(Arc::new(HashEmbedder));
    e.cfg.w_retention = 0.0;
    at_scale(&e);
    remember(&e, "The Titan Krios G4 is the lab's main scope.", |i| {
        i.entities = vec![EntityIn {
            name: "Titan Krios G4".into(),
            aliases: vec!["microscope".into()],
            ..Default::default()
        }];
    });
    remember(&e, "Book a session for the polisher.", |i| {
        i.entities = vec![entity("Polisher")];
    });
    let query = RecallInput {
        query: "book a session on the microscope".into(),
        ..Default::default()
    };
    // With the knob at 0 the channel is silent: on/off/on must round-trip.
    e.cfg.w_anchor = 0.0;
    let off = serde_json::to_string(&e.recall(&query).unwrap()).unwrap();
    e.cfg.w_anchor = 2.5;
    let on = serde_json::to_string(&e.recall(&query).unwrap()).unwrap();
    assert_ne!(
        on, off,
        "with the knob on the output must change (anchor channel active)"
    );
    e.cfg.w_anchor = 0.0;
    let off2 = serde_json::to_string(&e.recall(&query).unwrap()).unwrap();
    assert_eq!(
        off2, off,
        "knob off must give a deterministic, state-free output"
    );
}
/// Reverse-hop fixture: the question's answer frame is `where` (a place), the
/// bridge "Mira" is returned by the lexical channel, and the terminal
/// relation "studied at" is paraphrase-free but ranked nowhere by the forward
/// channels because the answer memory shares no word with the question.
fn rev_hop_fixture() -> Engine {
    let mut e = engine(Arc::new(HashEmbedder));
    e.cfg.w_retention = 0.0;
    e.cfg.w_sempath = 0.0;
    remember(&e, "Noor Haddad mentors under Mira Lindqvist.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Noor Haddad".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
            EntityIn {
                name: "Mira Lindqvist".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
        ];
        i.relations = vec![relation(
            "Mira Lindqvist",
            "mentors",
            "Noor Haddad",
            "Mira Lindqvist mentors Noor Haddad",
        )];
    });
    remember(&e, "Mira Lindqvist studied at Kestrel Polytechnic.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Mira Lindqvist".into(),
                kind: Some("person".into()),
                ..Default::default()
            },
            EntityIn {
                name: "Kestrel Polytechnic".into(),
                kind: Some("organization".into()),
                ..Default::default()
            },
        ];
        i.relations = vec![relation(
            "Mira Lindqvist",
            "studied at",
            "Kestrel Polytechnic",
            "Mira Lindqvist studied at Kestrel Polytechnic",
        )];
    });
    // Unrelated padding so the terminal memory is not trivially first.
    for n in 0..6 {
        remember(&e, &format!("Routine note {n} about scheduling."), |_| {});
    }
    // rev_hop is scale-gated (>=50k memories); bulk-fill so the hop engages.
    at_scale(&e);
    e
}

fn rev_hop_query() -> RecallInput {
    RecallInput {
        query: "Where did the engineer who guides Noor's early career get schooled?".into(),
        k: Some(3),
        ..Default::default()
    }
}

#[test]
fn rev_hop_default_is_off() {
    let e = engine(Arc::new(HashEmbedder));
    assert!(
        e.cfg.rev_hop,
        "Config::default().rev_hop should be on at scale"
    );
    let mut cfg = Config::default();
    neocortex::eval::set_config(&mut cfg, "rev_hop", "on").unwrap();
    assert!(cfg.rev_hop);
    neocortex::eval::set_config(&mut cfg, "rev_hop", "off").unwrap();
    assert!(!cfg.rev_hop);
    assert!(neocortex::eval::set_config(&mut cfg, "rev_hop", "maybe").is_err());
}

#[test]
fn rev_hop_reaches_a_terminal_fact_the_forward_channels_miss() {
    let mut e = rev_hop_fixture();
    e.cfg.rev_hop = false;
    let off = e.recall(&rev_hop_query()).unwrap();
    e.cfg.rev_hop = true;
    let on = e.recall(&rev_hop_query()).unwrap();
    let reached = |o: &RecallOutput| {
        o.facts
            .iter()
            .any(|fact| fact.target == "Kestrel Polytechnic")
            || o.memories
                .iter()
                .any(|m| m.content.contains("Kestrel Polytechnic"))
    };
    eprintln!(
        "off facts={:?}",
        off.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
    eprintln!(
        "on facts={:?}",
        on.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
    assert!(
        reached(&on),
        "rev_hop should reach the terminal relation: {:?}",
        on.facts.iter().map(|f| &f.fact).collect::<Vec<_>>()
    );
}

#[test]
fn rev_hop_off_is_byte_identical() {
    let mut e = rev_hop_fixture();
    e.cfg.rev_hop = false;
    let a = e.recall(&rev_hop_query()).unwrap();
    let b = e.recall(&rev_hop_query()).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

#[test]
fn rev_hop_caps_facts_and_never_floods_from_a_hub() {
    const REV_HUB: usize = 200;
    let mut e = engine(Arc::new(HashEmbedder));
    e.cfg.w_retention = 0.0;
    e.cfg.w_sempath = 0.0;
    // The anchor entity is a hub: 250 live relations to places.
    remember(&e, "Noor Haddad is on the staff.", |i| {
        i.entities = vec![EntityIn {
            name: "Noor Haddad".into(),
            kind: Some("person".into()),
            ..Default::default()
        }];
    });
    for n in 0..250 {
        let place = format!("Town{n}");
        remember(&e, &format!("Noor Haddad visited {place}."), |i| {
            i.entities = vec![
                EntityIn {
                    name: "Noor Haddad".into(),
                    kind: Some("person".into()),
                    ..Default::default()
                },
                EntityIn {
                    name: place.clone(),
                    kind: Some("place".into()),
                    ..Default::default()
                },
            ];
            i.relations = vec![relation(
                "Noor Haddad",
                "visited",
                &place,
                &format!("Noor Haddad visited {place}"),
            )];
        });
    }
    let query = RecallInput {
        query: "Where did Noor Haddad travel?".into(),
        k: Some(5),
        ..Default::default()
    };
    // rev_hop is scale-gated; fill to scale so the hub cap path engages.
    at_scale(&e);
    e.cfg.rev_hop = false;
    let off: Vec<i64> = e
        .recall(&query)
        .unwrap()
        .facts
        .iter()
        .map(|f| f.id)
        .collect();
    e.cfg.rev_hop = true;
    let out = e.recall(&query).unwrap();
    assert!(
        out.facts.len() <= 8,
        "rev_hop must keep the fact list capped: {}",
        out.facts.len()
    );
    let added = out.facts.iter().filter(|f| !off.contains(&f.id)).count();
    assert!(
        added == 0,
        "a hub bridge (> {REV_HUB} relations) must not contribute candidates, {added} added"
    );
}
