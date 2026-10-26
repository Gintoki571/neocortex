//! Tiered query anchors (`anchor_tiers`): first-name tokens, role nouns and
//! generic-alias demotion.
use neocortex::anchor::{query_anchors, Anchors};
use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput};
use neocortex::extract;
use neocortex::temporal::{parse_instant, Clock};
use neocortex::text;
use std::sync::Arc;

fn engine() -> Engine {
    let now = parse_instant(
        "2026-10-21T15:00:00Z",
        &Clock {
            now: 0,
            tz_offset_secs: 0,
        },
    )
    .unwrap();
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now,
            tz_offset_secs: 0,
        })
}

fn person(name: &str, aliases: &[&str]) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
        aliases: aliases.iter().map(|a| a.to_string()).collect(),
        ..Default::default()
    }
}

fn remember(e: &Engine, content: &str, entities: Vec<EntityIn>, relations: Vec<RelationIn>) {
    e.remember(RememberInput {
        content: content.into(),
        entities,
        relations,
        ..Default::default()
    })
    .unwrap();
}

fn rel(source: &str, predicate: &str, target: &str) -> RelationIn {
    RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        ..Default::default()
    }
}

fn id(e: &Engine, name: &str) -> i64 {
    e.db.sql()
        .query_row("SELECT id FROM entities WHERE name=?1", [name], |r| {
            r.get(0)
        })
        .unwrap()
}

fn anchors(e: &Engine, query: &str, tiers: &str) -> Anchors {
    let mut names: Vec<String> = extract::ngrams(query, 4).into_iter().map(|x| x.0).collect();
    names.sort();
    names.dedup();
    query_anchors(&e.db, query, &names, tiers, e.clock().now).unwrap()
}

#[test]
fn unique_first_name_is_a_weak_anchor() {
    let e = engine();
    remember(
        &e,
        "Noor Haddad is an engineer.",
        vec![person("Noor Haddad", &[])],
        vec![],
    );
    remember(
        &e,
        "Mira Lindqvist teaches.",
        vec![person("Mira Lindqvist", &[])],
        vec![],
    );
    let off = anchors(&e, "Where was Noor's mentor born?", "");
    assert!(off.strong.is_empty() && off.weak.is_empty(), "{off:?}");
    let on = anchors(&e, "Where was Noor's mentor born?", "1");
    assert!(on.strong.is_empty(), "{on:?}");
    assert_eq!(on.weak, vec![id(&e, "Noor Haddad")]);
}

#[test]
fn lower_case_common_noun_is_not_a_name_anchor() {
    let e = engine();
    remember(
        &e,
        "Rafael is a PhD student.",
        vec![person("PhD student", &[])],
        vec![],
    );
    let on = anchors(&e, "Which funding body supports the student?", "1");
    assert!(on.weak.is_empty(), "{on:?}");
}

#[test]
fn shared_first_name_is_not_an_anchor() {
    let e = engine();
    remember(
        &e,
        "Teresa Mota joined.",
        vec![person("Teresa Mota", &[])],
        vec![],
    );
    remember(
        &e,
        "Teresa Moura joined.",
        vec![person("Teresa Moura", &[])],
        vec![],
    );
    let on = anchors(&e, "Which company did Teresa join?", "1");
    assert!(on.strong.is_empty() && on.weak.is_empty(), "{on:?}");
}

#[test]
fn generic_or_shared_alias_is_demoted_to_weak() {
    let e = engine();
    let org = EntityIn {
        name: "Mirante Bio".into(),
        kind: Some("organization".into()),
        aliases: vec!["the company".into(), "Mirante".into()],
        ..Default::default()
    };
    remember(&e, "Mirante Bio is a biotech.", vec![org], vec![]);
    remember(
        &e,
        "Simão Pais sponsors a club.",
        vec![person("Simão Pais", &["Simão"])],
        vec![],
    );
    remember(
        &e,
        "Simão Pinho runs a co-op.",
        vec![person("Simão Pinho", &["Simão"])],
        vec![],
    );
    let mirante = id(&e, "Mirante Bio");
    // Without demotion every alias hit is a strong anchor (today's behaviour).
    let off = anchors(&e, "Where is the headquarters of the company?", "");
    assert!(off.strong.contains(&mirante), "{off:?}");
    // "company" is not a word of the name: weak.
    let on = anchors(&e, "Where is the headquarters of the company?", "w");
    assert!(
        !on.strong.contains(&mirante) && on.weak.contains(&mirante),
        "{on:?}"
    );
    // A word of the name stays strong.
    let own = anchors(&e, "Where is Mirante based?", "w");
    assert!(own.strong.contains(&mirante), "{own:?}");
    // An alias naming two entities is weak for both.
    let shared = anchors(&e, "What does Simão run?", "w");
    assert!(shared.strong.is_empty(), "{shared:?}");
    assert_eq!(shared.weak.len(), 2, "{shared:?}");
    // The full name is still strong.
    let full = anchors(&e, "What does Simão Pinho run?", "w");
    assert!(full.strong.contains(&id(&e, "Simão Pinho")), "{full:?}");
}

#[test]
fn role_noun_resolves_through_a_relation_to_an_anchor() {
    let e = engine();
    let org = EntityIn {
        name: "Mirante Bio".into(),
        kind: Some("organization".into()),
        ..Default::default()
    };
    remember(
        &e,
        "Ricardo Faria is the CFO of Mirante Bio.",
        vec![person("Ricardo Faria", &[]), org],
        vec![rel("Ricardo Faria", "is the CFO of", "Mirante Bio")],
    );
    remember(
        &e,
        "Noah Tansen reports to Mira Shah.",
        vec![person("Noah Tansen", &[]), person("Mira Shah", &[])],
        vec![rel("Noah Tansen", "reports to", "Mira Shah")],
    );
    let cfo = anchors(&e, "Where does the CFO of Mirante Bio live?", "2");
    assert_eq!(cfo.weak, vec![id(&e, "Ricardo Faria")], "{cfo:?}");
    let boss = anchors(&e, "Where does Noah Tansen's boss live?", "2");
    assert_eq!(boss.weak, vec![id(&e, "Mira Shah")], "{boss:?}");
    // No anchor in the question: a role noun alone resolves nothing.
    let bare = anchors(&e, "Where does the CFO live?", "2");
    assert!(bare.weak.is_empty(), "{bare:?}");
}

#[test]
fn first_name_anchor_starts_the_path_channel() {
    let mut e = engine();
    remember(
        &e,
        "Noah Tansen's manager is Mira Shah.",
        vec![person("Noah Tansen", &[]), person("Mira Shah", &[])],
        vec![RelationIn {
            fact: Some("Noah Tansen's manager is Mira Shah.".into()),
            ..rel("Noah Tansen", "manager", "Mira Shah")
        }],
    );
    remember(
        &e,
        "Mira Shah lives in Oslo.",
        vec![
            person("Mira Shah", &[]),
            EntityIn {
                name: "Oslo".into(),
                ..Default::default()
            },
        ],
        vec![RelationIn {
            fact: Some("Mira Shah lives in Oslo.".into()),
            ..rel("Mira Shah", "lives in", "Oslo")
        }],
    );
    let query = RecallInput {
        query: "where does Noah's manager live".into(),
        k: Some(5),
        ..Default::default()
    };
    // The lexical path channel starts only at anchors: without tier 1 the
    // question names no entity ("Noah's" is not an alias).
    let path_hit = |e: &Engine| {
        let out = e.recall(&query).unwrap();
        out.memories
            .iter()
            .any(|m| m.content.contains("Oslo") && m.reasons.iter().any(|r| r.starts_with("path")))
    };
    assert_eq!(text::norm_name("Noah's"), "noah");
    assert!(!path_hit(&e), "no anchor without tier 1");
    e.cfg.anchor_tiers = "1".into();
    assert!(
        path_hit(&e),
        "the first-name anchor must start the path channel"
    );
}
