//! Lane nc-ex01 regression tests (briefs/open_items.md EX-01..EX-04): unit
//! labels as entities, measures never entities, known-head spans, and the
//! write-time unique alias prefix. Hash embedder; names follow the real
//! extraction order.

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RememberInput, RememberOutput},
    extract,
    temporal::Clock,
};
use std::sync::Arc;

const NOW: i64 = 1_792_756_800;

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn remember(e: &Engine, content: &str, entities: Vec<EntityIn>) -> RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        entities,
        ..Default::default()
    })
    .unwrap()
}

fn names(out: &RememberOutput) -> Vec<&str> {
    out.entities.iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn ex01_unit_labels_are_separate_entities() {
    let e = engine();
    let a = remember(
        &e,
        "Apt 3B in the Harbor Building has a leaking pipe.",
        vec![],
    );
    let b = remember(
        &e,
        "Apt 3C in the Harbor Building has a broken window.",
        vec![],
    );
    let id_b = b
        .entities
        .iter()
        .find(|r| r.name == "Apt 3C")
        .expect("Apt 3C");
    let id_a = a
        .entities
        .iter()
        .find(|r| r.name == "Apt 3B")
        .expect("Apt 3B");
    assert_ne!(id_a.id, id_b.id, "Apt 3B and Apt 3C must stay apart");
}

#[test]
fn ex02_measures_after_numbers_never_become_entities() {
    let e = engine();
    let out = remember(
        &e,
        "The fresh DB is 1.2 MB and recall takes 120 ms.",
        vec![],
    );
    let n = names(&out);
    assert!(!n.contains(&"MB") && !n.contains(&"ms"), "{n:?}");
    assert!(n.contains(&"DB"), "{n:?}");
}

#[test]
fn ex03_known_head_before_plural_noun_is_not_a_compound() {
    let e = engine();
    let cypher = EntityIn {
        name: "Cypher".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    remember(&e, "Cypher is the query language.", vec![cypher]);
    let out = remember(&e, "Cypher Memory nodes are readable now.", vec![]);
    let n = names(&out);
    assert!(n.contains(&"Cypher"), "{n:?}");
    assert!(!n.contains(&"Cypher Memory"), "{n:?}");
}

#[test]
fn ex04_unique_alias_prefix_resolves_on_write() {
    let e = engine();
    let embedder = EntityIn {
        name: "UltraBERT-Embed-150M".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    let first = remember(&e, "UltraBERT-Embed-150M is the embedder.", vec![embedder]);
    let first_id = first.entities[0].id;
    let out = remember(&e, "UltraBERT runs on CPU.", vec![]);
    let hit = out
        .entities
        .iter()
        .find(|r| r.name == "UltraBERT")
        .expect("UltraBERT mention");
    assert_eq!(hit.id, first_id, "{:?}", out.entities);
    assert_eq!(hit.status, "alias", "{:?}", out.entities);
}

#[test]
fn ex04_two_prefix_candidates_stay_ambiguous() {
    let e = engine();
    let a = EntityIn {
        name: "UltraBERT-Embed-150M".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    let b = EntityIn {
        name: "UltraBERT-Large".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    remember(&e, "UltraBERT-Embed-150M is one model.", vec![a]);
    remember(&e, "UltraBERT-Large is another model.", vec![b]);
    let out = remember(&e, "UltraBERT runs on CPU.", vec![]);
    let hit = out
        .entities
        .iter()
        .find(|r| r.name == "UltraBERT")
        .expect("UltraBERT mention");
    assert_ne!(hit.status, "alias", "{:?}", out.entities);
}

/// (name, hint) of every heuristic candidate in `text`.
fn candidates(text: &str) -> Vec<(String, &'static str)> {
    extract::heuristic_entities(text)
        .into_iter()
        .map(|c| (c.name, c.hint))
        .collect()
}

#[test]
fn ex06_quoted_phrases_that_are_not_names_are_not_entities() {
    for phrase in [
        "used by Priya's project",
        "the project led by Marco",
        "what is the Atlas build?",
        "Priya's",
        "the sqlite vec index",
    ] {
        let got = candidates(&format!("The notes say \"{phrase}\" in the review."));
        assert!(
            !got.iter().any(|(n, h)| *h == "quoted" && n == phrase),
            "{phrase}: {got:?}"
        );
    }
}

#[test]
fn ex06_quoted_names_and_identifiers_stay_entities() {
    for name in [
        "Harbor Building",
        "UltraBERT-Embed-150M",
        "ultrabert-embed-150m",
    ] {
        let got = candidates(&format!("The notes name \"{name}\" as the target."));
        assert!(
            got.iter().any(|(n, h)| *h == "quoted" && n == name),
            "{name}: {got:?}"
        );
    }
}

#[test]
fn ex06_quoted_lowercase_single_tokens_are_names() {
    for name in ["neocortex", "pgvector", "tokio", "sqlite-vec"] {
        let got = candidates(&format!("The notes name \"{name}\" as the target."));
        assert!(
            got.iter().any(|(n, h)| *h == "quoted" && n == name),
            "{name}: {got:?}"
        );
    }
}

#[test]
fn ex06b_plural_of_caps_abbreviation_resolves_to_singular_entity() {
    let e = engine();
    let a = remember(&e, "The fresh DB holds the memories.", vec![]);
    let db_id = a.entities.iter().find(|r| r.name == "DB").expect("DB").id;
    let b = remember(&e, "Each cluster runs DBs for the memories.", vec![]);
    assert_eq!(b.entities.len(), 1, "{:?}", names(&b));
    assert_eq!(b.entities[0].id, db_id, "DBs must resolve to DB");
}

#[test]
fn ex06b_plural_of_caps_abbreviations_share_one_norm() {
    use neocortex::text::norm_name;
    for (plural, singular) in [
        ("DBs", "DB"),
        ("APIs", "API"),
        ("GPUs", "GPU"),
        ("LLMs", "LLM"),
    ] {
        assert_eq!(norm_name(plural), norm_name(singular), "{plural}");
    }
    // "APIs Inc" stays one name; only the abbreviation's plural is folded.
    assert_eq!(norm_name("APIs Inc"), norm_name("API Inc"));
}

#[test]
fn ex02_units_after_numbers_are_never_entities() {
    for text in [
        "The fresh DB is 1.2 MB on disk.",
        "Recall takes 300 ms on the laptop.",
        "The image is 2 GiB in size.",
        "The disk is 40% full today.",
    ] {
        let got = candidates(text);
        for unit in ["MB", "ms", "GiB", "40%", "40", "%"] {
            assert!(!got.iter().any(|(n, _)| n == unit), "{text}: {got:?}");
        }
    }
}

#[test]
fn ex06_controls_caps_names_and_compound_names_still_extract() {
    let got = candidates("Deploy the service to AWS on Friday with APIs Inc.");
    for name in ["AWS", "APIs Inc"] {
        assert!(got.iter().any(|(n, _)| n == name), "{name}: {got:?}");
    }
}

// nc-ex03 / EX-03: a real recurring compound (capitalised again in the same
// text) is still created, even when its head is a known alias.
#[test]
fn ex03_recurring_compound_in_same_text_is_created() {
    let e = engine();
    let cypher = EntityIn {
        name: "Cypher".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    remember(&e, "Cypher is the query language.", vec![cypher]);
    let out = remember(
        &e,
        "Cypher Memory nodes are stored. Cypher Memory is useful.",
        vec![],
    );
    let n = names(&out);
    assert!(
        n.contains(&"Cypher Memory"),
        "recurring compound must be created: {n:?}"
    );
}

// nc-ex03 / EX-03: a non-recurring compound with a known head still splits to
// the head (control for the recurrence rule).
#[test]
fn ex03_non_recurring_compound_with_known_head_stays_split() {
    let e = engine();
    let cypher = EntityIn {
        name: "Cypher".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    remember(&e, "Cypher is the query language.", vec![cypher]);
    let out = remember(&e, "Cypher Memory nodes are stored.", vec![]);
    let n = names(&out);
    assert!(n.contains(&"Cypher"), "{n:?}");
    assert!(!n.contains(&"Cypher Memory"), "{n:?}");
}

// nc-ex03 / EX-05: a sentence-initial word is kept when it is already a stored
// alias (one indexed lookup via the known callback).
#[test]
fn ex05_sentence_initial_known_alias_is_kept() {
    let e = engine();
    let cypher = EntityIn {
        name: "Cypher".into(),
        kind: Some("tool".into()),
        ..Default::default()
    };
    remember(&e, "We use Cypher here.", vec![cypher]);
    let out = remember(&e, "Cypher is the query language.", vec![]);
    let n = names(&out);
    assert!(
        n.contains(&"Cypher"),
        "known sentence-initial alias must be kept: {n:?}"
    );
}

// nc-ex03 / EX-05: unknown sentence-initial words stay dropped (precision
// first); verb-like followers do not rescue an unknown.
#[test]
fn ex05_unknown_sentence_initial_words_still_dropped() {
    let e = engine();
    for (text, bad) in [
        ("Repo was renamed to Main.", "Repo"),
        ("The system runs fast.", "The"),
        ("Starting was slow.", "Starting"),
    ] {
        let out = remember(&e, text, vec![]);
        let n = names(&out);
        assert!(
            !n.contains(&bad),
            "{bad:?} must not be extracted from {text:?}: {n:?}"
        );
    }
}

// nc-ex03 / EX-05: two unknown capitalised words at sentence start are not
// created as a compound ("Release Notes:").
#[test]
fn ex05_release_notes_not_created_as_unknown_compound() {
    let e = engine();
    let out = remember(&e, "Release Notes: version 1.0 is ready.", vec![]);
    let n = names(&out);
    assert!(
        !n.contains(&"Release Notes"),
        "unknown compound must not be created: {n:?}"
    );
    assert!(!n.contains(&"Release"), "{n:?}");
    assert!(!n.contains(&"Notes"), "{n:?}");
}
