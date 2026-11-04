//! Correction hints (H3-04/H3-05): with `correction_hints` on, a remember
//! reports older memories the new text corrects and recall marks them. Nothing
//! is ended, and knob off changes no output.

use neocortex::embed::HashEmbedder;
use neocortex::engine::{Config, Engine, EntityIn, RecallInput, RelationIn, RememberInput};
use neocortex::render;
use std::sync::Arc;

fn engine(on: bool) -> Engine {
    let mut e = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
    e.cfg = Config {
        correction_hints: on,
        ..Config::default()
    };
    e
}

fn put(e: &Engine, content: &str, valid_from: Option<&str>) -> i64 {
    let inp = RememberInput {
        content: content.into(),
        valid_from: valid_from.map(Into::into),
        ..Default::default()
    };
    e.remember(inp).unwrap().id
}

const OLD: &str = "The Beacon API rate limit is 500 requests per minute per key after the second partner onboarding.";
const NEW: &str =
    "Correction to my June note: the Beacon API rate limit is 400 requests per minute, not 500.";

fn expired(e: &Engine, id: i64) -> bool {
    e.db.sql()
        .query_row(
            "SELECT expired_at IS NOT NULL FROM memories WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn a_correction_proposes_the_old_memory_and_ends_nothing() {
    let e = engine(true);
    let old = put(&e, OLD, Some("2026-06-20"));
    let out = e
        .remember(RememberInput {
            content: NEW.into(),
            valid_from: Some("2026-07-01".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(out.possible_supersedes.len(), 1);
    assert_eq!(out.possible_supersedes[0].memory_id, old);
    assert!(out.possible_supersedes[0]
        .snippet
        .starts_with("The Beacon API rate limit"));
    assert!(out.superseded_memories.is_empty());
    assert!(!expired(&e, old), "a hint must not end the old memory");
    let text = render::render_remember(&out);
    assert!(
        text.contains(&format!("possible_supersedes: m{old} ")),
        "{text}"
    );
}

#[test]
fn knob_off_reports_no_candidates() {
    let e = engine(false);
    put(&e, OLD, Some("2026-06-20"));
    let out = e
        .remember(RememberInput {
            content: NEW.into(),
            valid_from: Some("2026-07-01".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(out.possible_supersedes.is_empty());
    let json = serde_json::to_string(&out).unwrap();
    assert!(!json.contains("possible_supersedes"), "{json}");
}

#[test]
fn build_numbers_produce_no_candidate() {
    let e = engine(true);
    put(
        &e,
        "Build 12 now passes the smoke suite.",
        Some("2026-06-20"),
    );
    let out = e
        .remember(RememberInput {
            content: "Correction: build 13 now passes the smoke suite.".into(),
            valid_from: Some("2026-07-01".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(out.possible_supersedes.is_empty());
}

#[test]
fn relation_backed_memory_is_never_proposed() {
    let e = engine(true);
    let old = e
        .remember(RememberInput {
            content: "Harbor's batch window is 01:00 to 03:00 UTC.".into(),
            valid_from: Some("2026-06-20".into()),
            entities: vec![EntityIn {
                name: "Harbor".into(),
                ..Default::default()
            }],
            relations: vec![RelationIn {
                source: "Harbor".into(),
                predicate: "batch window".into(),
                target: "01:00 to 03:00 UTC".into(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap()
        .id;
    let out = e
        .remember(RememberInput {
            content:
                "Correction: the Harbor batch window is 22:00 to 00:00 UTC, not 01:00 to 03:00 UTC."
                    .into(),
            valid_from: Some("2026-07-01".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.possible_supersedes.iter().all(|c| c.memory_id != old),
        "{:?}",
        out.possible_supersedes
    );
}

#[test]
fn later_dated_memory_is_never_proposed() {
    let e = engine(true);
    let old = put(&e, OLD, Some("2026-09-01"));
    let out = e
        .remember(RememberInput {
            content: NEW.into(),
            valid_from: Some("2026-08-01".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(out.possible_supersedes.iter().all(|c| c.memory_id != old));
}

#[test]
fn recall_marks_the_corrected_hit_with_a_later_correction() {
    let e = engine(true);
    let old = put(&e, OLD, Some("2026-06-20"));
    let new = put(&e, NEW, Some("2026-07-01"));
    let out = e
        .recall(&RecallInput {
            query: "what is the beacon api rate limit".into(),
            ..Default::default()
        })
        .unwrap();
    let hit = out
        .memories
        .iter()
        .find(|m| m.id == old)
        .expect("old memory is recalled");
    assert_eq!(hit.maybe_superseded_by, Some(new));
    assert!(render::render_recall(&out, &|t| t.to_string())
        .contains(&format!("[maybe superseded by m{new}]")));
    let newest = out
        .memories
        .iter()
        .find(|m| m.id == new)
        .expect("new memory is recalled");
    assert_eq!(newest.maybe_superseded_by, None);
}

#[test]
fn recall_with_knob_off_has_no_marks() {
    let e = engine(false);
    let old = put(&e, OLD, Some("2026-06-20"));
    put(&e, NEW, Some("2026-07-01"));
    let out = e
        .recall(&RecallInput {
            query: "what is the beacon api rate limit".into(),
            ..Default::default()
        })
        .unwrap();
    let hit = out
        .memories
        .iter()
        .find(|m| m.id == old)
        .expect("old memory is recalled");
    assert_eq!(hit.maybe_superseded_by, None);
    assert!(!render::render_recall(&out, &|t| t.to_string()).contains("maybe superseded"));
}
