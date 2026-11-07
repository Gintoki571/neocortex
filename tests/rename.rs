//! Rename joins the old and new name (H3-10): a rename statement merges the
//! two names into one entity, the old name survives as an alias, old facts
//! move to the surviving entity, and a "formerly called" fact keeps the old
//! name answerable. Hedged/negated/ambiguous renames merge nothing.

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RecallInput, RememberInput},
    temporal::Clock,
};
use std::sync::Arc;

const NOW: i64 = 1_800_000_000;

fn new_engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn project(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("project".into()),
        ..Default::default()
    }
}

fn person(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
        ..Default::default()
    }
}

fn typed(name: &str, kind: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some(kind.into()),
        ..Default::default()
    }
}

fn rel(
    source: &str,
    predicate: &str,
    target: &str,
    exclusive: bool,
) -> neocortex::engine::RelationIn {
    neocortex::engine::RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        exclusive: Some(exclusive),
        ..Default::default()
    }
}

fn remember_at(
    engine: &Engine,
    content: &str,
    entities: Vec<EntityIn>,
    relations: Vec<neocortex::engine::RelationIn>,
    valid_from: &str,
) -> neocortex::engine::RememberOutput {
    engine
        .remember(RememberInput {
            content: content.into(),
            entities,
            relations,
            valid_from: Some(valid_from.into()),
            ..Default::default()
        })
        .unwrap()
}

fn recall_facts(engine: &Engine, q: &str) -> Vec<String> {
    engine
        .recall(&RecallInput {
            query: q.into(),
            ..Default::default()
        })
        .unwrap()
        .facts
        .into_iter()
        .map(|f| f.fact)
        .collect()
}

fn card_id(engine: &Engine, name: &str) -> i64 {
    engine.entity(name, 20).unwrap().card.id
}

/// s11: Harbor renamed to Lantern. The stale Python 3.11 fact under the old
/// name ends when Lantern's 3.13 fact lands, and both names are one entity.
#[test]
fn rename_merges_harbor_into_lantern_and_ends_stale_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The ingest pipeline Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let third = remember_at(
        &engine,
        "The Lantern pipeline no longer uses Python 3.11 after the 2026-08-20 upgrade to Python 3.13.",
        vec![project("Lantern"), project("Python 3.13")],
        vec![rel("Lantern", "runs on", "Python 3.13", true)],
        "2026-08-20",
    );
    // One entity: the surviving display name is Lantern, Harbor is its alias.
    let lantern = engine.entity("Lantern", 20).unwrap();
    assert_eq!(lantern.card.name, "Lantern");
    assert!(
        lantern
            .card
            .aliases
            .iter()
            .any(|a| neocortex::text::norm_name(a) == "harbor"),
        "Lantern aliases should contain Harbor, got {:?}",
        lantern.card.aliases
    );
    assert_eq!(
        card_id(&engine, "Harbor"),
        lantern.card.id,
        "Harbor and Lantern should be one entity"
    );
    // The stale exclusive value ended by valid_from order; history keeps it.
    assert!(
        !third.invalidated_relations.is_empty(),
        "Lantern's 3.13 fact should end the 3.11 fact"
    );
    let current: Vec<String> = lantern
        .current_facts
        .iter()
        .map(|f| f.fact.clone())
        .collect();
    assert!(
        current.iter().any(|f| f.contains("Python 3.13")),
        "current facts should hold Python 3.13, got {current:?}"
    );
    assert!(
        !current.iter().any(|f| f.contains("Python 3.11")),
        "Python 3.11 should not be current, got {current:?}"
    );
    assert!(
        lantern
            .past_facts
            .iter()
            .any(|f| f.fact.contains("Python 3.11")),
        "history should keep the ended 3.11 fact, got {:?}",
        lantern.past_facts
    );
}

/// s9: Project Atlas was renamed to Beacon.
#[test]
fn rename_merges_atlas_into_beacon() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The Atlas repo uses npm for package management.",
        vec![project("Atlas"), project("npm")],
        vec![rel("Atlas", "uses", "npm", true)],
        "2026-01-08",
    );
    remember_at(
        &engine,
        "Project Atlas was renamed to Beacon on 2026-06-02.",
        vec![project("Atlas"), project("Beacon")],
        vec![],
        "2026-06-02",
    );
    assert_eq!(card_id(&engine, "Atlas"), card_id(&engine, "Beacon"));
}

/// The formerly-called fact answers "what was Lantern called before".
#[test]
fn rename_fact_answers_called_before() {
    let engine = new_engine();
    remember_at(
        &engine,
        "The ingest pipeline Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let facts = recall_facts(&engine, "what was Lantern called before the rename");
    assert!(
        facts.iter().any(|f| f.to_lowercase().contains("harbor")),
        "recall should answer Harbor, got {facts:?}"
    );
    let card = engine.entity("Lantern", 20).unwrap();
    assert!(
        card.current_facts
            .iter()
            .any(|f| f.fact.to_lowercase().contains("harbor")),
        "entity card should carry the formerly fact, got {:?}",
        card.current_facts
    );
}

/// A rename stated as a relation (predicate "renamed to") merges too.
#[test]
fn rename_via_relation_predicate_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Harbor was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![neocortex::engine::RelationIn {
            source: "Harbor".into(),
            predicate: "renamed to".into(),
            target: "Lantern".into(),
            ..Default::default()
        }],
        "2026-07-03",
    );
    assert_eq!(card_id(&engine, "Harbor"), card_id(&engine, "Lantern"));
}

/// "is now called" and "(formerly X)" phrasings merge too.
#[test]
fn rename_now_called_and_formerly_phrasings_merge() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Harbor is now called Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-08-01",
    );
    assert_eq!(card_id(&engine, "Harbor"), card_id(&engine, "Lantern"));

    let engine2 = new_engine();
    remember_at(
        &engine2,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine2,
        "Lantern (formerly Harbor) runs the ingest pipeline.",
        vec![project("Lantern"), project("Harbor")],
        vec![],
        "2026-08-01",
    );
    assert_eq!(card_id(&engine2, "Harbor"), card_id(&engine2, "Lantern"));
}

/// A rename of a person uses the same path.
#[test]
fn person_name_change_joins() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Kim Lee works at Acme.",
        vec![person("Kim Lee"), project("Acme")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Kim Lee changed her name to Kim Stone.",
        vec![person("Kim Lee"), person("Kim Stone")],
        vec![],
        "2026-07-03",
    );
    let stone = engine.entity("Kim Stone", 20).unwrap();
    assert_eq!(stone.card.id, card_id(&engine, "Kim Lee"));
    assert!(
        stone
            .card
            .aliases
            .iter()
            .any(|a| neocortex::text::norm_name(a) == "kim lee"),
        "old name should be an alias of the survivor"
    );
}

/// Hedged renames merge nothing.
#[test]
fn hedged_rename_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "We might rename Harbor to Lantern next month.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "a hedged rename must not merge"
    );

    let engine2 = new_engine();
    remember_at(
        &engine2,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine2,
        "We plan to rename Harbor to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine2, "Harbor"),
        card_id(&engine2, "Lantern"),
        "a planned rename must not merge"
    );
}

/// Negated renames merge nothing.
#[test]
fn negated_rename_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Harbor was not renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "a negated rename must not merge"
    );
}

/// Several entities named Harbor: merge nothing, report candidates.
#[test]
fn ambiguous_old_name_merges_nothing_and_reports_candidates() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor is a project that runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-09",
    );
    // A second, different Harbor (a place) so the old name is ambiguous.
    remember_at(
        &engine,
        "Harbor is a city on the coast.",
        vec![typed("Harbor", "place")],
        vec![],
        "2026-02-01",
    );
    let out = remember_at(
        &engine,
        "Harbor was renamed to Lantern.",
        vec![
            EntityIn {
                name: "Harbor".into(),
                ..Default::default()
            },
            project("Lantern"),
        ],
        vec![],
        "2026-07-03",
    );
    let reported = out.entities.iter().any(|e| !e.candidates.is_empty())
        || out.hints.iter().any(|h| h.contains("Harbor"));
    assert!(
        reported,
        "ambiguous rename should report candidates, got entities={:?} hints={:?}",
        out.entities, out.hints
    );
    let lantern = engine.entity("Lantern", 20).unwrap();
    assert_ne!(
        lantern.card.id,
        card_id(&engine, "Harbor"),
        "ambiguous rename must not have merged Lantern into a Harbor"
    );
}

/// After the rename, a fact stated with the old name lands on the survivor.
#[test]
fn fact_on_old_name_after_rename_resolves_to_survivor() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let survivor = card_id(&engine, "Lantern");
    let out = remember_at(
        &engine,
        "Harbor runs on Python 3.12 now.",
        vec![project("Harbor"), project("Python 3.12")],
        vec![],
        "2026-09-01",
    );
    let resolution = out
        .entities
        .iter()
        .find(|e| e.name == "Harbor")
        .expect("Harbor resolution present");
    assert_eq!(
        resolution.id, survivor,
        "old-name mention should resolve to the merged entity"
    );
}

/// Forgetting the rename memory removes the formerly-called fact but keeps
/// the merge (the alias stays).
#[test]
fn forget_rename_memory_keeps_merge_and_retracts_formerly_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), project("Python 3.11")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    let rename = remember_at(
        &engine,
        "The harbor-ingest project was renamed to Lantern in July 2026.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-07-03",
    );
    let survivor = card_id(&engine, "Lantern");
    assert!(engine
        .entity("Lantern", 20)
        .unwrap()
        .current_facts
        .iter()
        .any(|f| f.fact.to_lowercase().contains("harbor")));
    engine.forget(&format!("m{}", rename.id), false).unwrap();
    assert_eq!(
        card_id(&engine, "Harbor"),
        survivor,
        "forgetting the rename memory must not unmerge"
    );
    assert!(
        !engine
            .entity("Lantern", 20)
            .unwrap()
            .current_facts
            .iter()
            .any(|f| f.fact.to_lowercase().contains("formerly")),
        "forget should retract the formerly-called fact"
    );
}

/// Verifier F02: "the branch" -> "main" is a common-noun phrase, never a rename.
#[test]
fn f02_generic_branch_to_main_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Branch tracks Origin.",
        vec![typed("Branch", "branch"), typed("Origin", "branch")],
        vec![rel("Branch", "tracks", "Origin", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "main tracks Release.",
        vec![typed("main", "branch"), typed("Release", "branch")],
        vec![rel("main", "tracks", "Release", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "I renamed the branch to main.",
        vec![typed("Branch", "branch"), typed("main", "branch")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Branch"),
        card_id(&engine, "main"),
        "generic branch->main rename must not merge"
    );
}

/// Verifier F03: "The meeting" is a common noun; it must not merge into the
/// existing project Sync.
#[test]
fn f03_meeting_to_project_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Sync runs on nginx.",
        vec![project("Sync"), typed("nginx", "project")],
        vec![rel("Sync", "runs on", "nginx", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "The meeting was renamed to Sync.",
        vec![
            EntityIn {
                name: "Meeting".into(),
                ..Default::default()
            },
            project("Sync"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Meeting"),
        card_id(&engine, "Sync"),
        "meeting->Sync must not merge"
    );
}

/// Verifier F04a/F04b: passive "is now called by the team" is not a rename
/// of Harbor. The two verifier variants differ only in Team's seeded type
/// (typed "team" vs untyped ""); the fix must block both deterministically.
#[test]
fn f04_passive_called_by_merges_nothing_in_both_type_variants() {
    for typed_team in [true, false] {
        let engine = new_engine();
        remember_at(
            &engine,
            "Harbor runs on Python 3.11.",
            vec![project("Harbor"), typed("Python 3.11", "language")],
            vec![rel("Harbor", "runs on", "Python 3.11", false)],
            "2026-01-05",
        );
        if typed_team {
            remember_at(
                &engine,
                "Team owns Backlog.",
                vec![typed("Team", "team"), typed("Backlog", "list")],
                vec![rel("Team", "owns", "Backlog", false)],
                "2026-01-06",
            );
        } else {
            remember_at(
                &engine,
                "Team owns Backlog.",
                vec![
                    EntityIn {
                        name: "Team".into(),
                        ..Default::default()
                    },
                    typed("Backlog", "list"),
                ],
                vec![rel("Team", "owns", "Backlog", false)],
                "2026-01-06",
            );
        }
        let out = remember_at(
            &engine,
            "Harbor is now called by the team every morning.",
            {
                let mut v = vec![project("Harbor")];
                if typed_team {
                    v.push(typed("Team", "team"));
                } else {
                    v.push(EntityIn {
                        name: "Team".into(),
                        ..Default::default()
                    });
                }
                v
            },
            vec![],
            "2026-06-01",
        );
        assert_ne!(
            card_id(&engine, "Harbor"),
            card_id(&engine, "Team"),
            "passive 'called by' (typed_team={typed_team}) must not merge"
        );
        assert!(
            !out.hints.iter().any(|h| h.starts_with("renamed:")),
            "no rename hint expected, got {:?}",
            out.hints
        );
    }
}

/// Verifier F08: both names already carry their own live facts and the
/// rename sentence is undated, so it returns a candidate, never a merge.
#[test]
fn f08_both_live_facts_undated_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Lantern runs on Python 3.13.",
        vec![project("Lantern"), typed("Python 3.13", "language")],
        vec![rel("Lantern", "runs on", "Python 3.13", true)],
        "2026-02-05",
    );
    let out = remember_at(
        &engine,
        "The Harbor pipeline was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "undated both-facts rename must not merge"
    );
    let reported = out.entities.iter().any(|e| !e.candidates.is_empty())
        || out.hints.iter().any(|h| h.contains("Lantern"));
    assert!(
        reported,
        "both-facts rename should report a candidate, got {:?} / {:?}",
        out.entities, out.hints
    );
}

/// Verifier F09: a bare shared first name ("Maya" for Maya Chen and Maya
/// Patel) never renames.
#[test]
fn f09_shared_first_name_old_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Maya Chen works at Acme.",
        vec![person("Maya Chen"), typed("Acme", "org")],
        vec![rel("Maya Chen", "works at", "Acme", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Maya Patel works at Brightline.",
        vec![person("Maya Patel"), typed("Brightline", "org")],
        vec![rel("Maya Patel", "works at", "Brightline", false)],
        "2026-01-06",
    );
    let out = remember_at(
        &engine,
        "Maya changed her name to Rio.",
        vec![person("Maya"), person("Rio")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Maya Chen"),
        card_id(&engine, "Maya Patel"),
        "seeds must stay separate"
    );
    let reported = out.entities.iter().any(|e| !e.candidates.is_empty())
        || out.hints.iter().any(|h| h.contains("Maya"));
    assert!(
        reported,
        "shared-first-name rename should report a candidate, got {:?} / {:?}",
        out.entities, out.hints
    );
    assert_ne!(
        card_id(&engine, "Maya Chen"),
        card_id(&engine, "Rio"),
        "bare shared first name must not have merged into Rio"
    );
    assert_ne!(
        card_id(&engine, "Maya Patel"),
        card_id(&engine, "Rio"),
        "bare shared first name must not have merged into Rio"
    );
}

/// Verifier F09b: the NEW name is the shared first name ("Maya").
#[test]
fn f09b_shared_first_name_new_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Kim Lee works at Acme.",
        vec![person("Kim Lee"), typed("Acme", "org")],
        vec![rel("Kim Lee", "works at", "Acme", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Maya Chen works at Brightline.",
        vec![person("Maya Chen"), typed("Brightline", "org")],
        vec![rel("Maya Chen", "works at", "Brightline", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "Maya Patel works at Initech.",
        vec![person("Maya Patel"), typed("Initech", "org")],
        vec![rel("Maya Patel", "works at", "Initech", false)],
        "2026-01-07",
    );
    let out = remember_at(
        &engine,
        "Kim Lee changed her name to Maya.",
        vec![person("Kim Lee"), person("Maya")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Kim Lee"),
        card_id(&engine, "Maya Chen"),
        "Kim Lee must not have merged into Maya Chen"
    );
    let reported = out.entities.iter().any(|e| !e.candidates.is_empty())
        || out.hints.iter().any(|h| h.contains("Maya"));
    assert!(
        reported,
        "shared-first-name new name should report a candidate, got {:?} / {:?}",
        out.entities, out.hints
    );
}

/// Verifier F11: a question is not a rename statement.
#[test]
fn f11_question_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Was Harbor renamed to Lantern?",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "a question must not merge"
    );
}

/// Verifier F12: a rename immediately reverted ("then back to Harbor") merges nothing.
#[test]
fn f12_reverted_rename_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Harbor was renamed to Lantern, then back to Harbor.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "a reverted rename must not merge"
    );
}

/// Verifier F13: a plan stated after the cue ("planned for Q3") merges nothing.
#[test]
fn f13_planned_after_cue_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor")],
        vec![],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "The rename of Harbor to Lantern is planned for Q3.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "a planned rename must not merge"
    );
}

/// Verifier F16: directory names are generic, never rename endpoints.
#[test]
fn f16_tests_directory_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Tests holds Fixtures.",
        vec![typed("Tests", "directory"), typed("Fixtures", "directory")],
        vec![rel("Tests", "holds", "Fixtures", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Specs holds Docs.",
        vec![typed("Specs", "directory"), typed("Docs", "directory")],
        vec![rel("Specs", "holds", "Docs", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "We renamed the tests directory to specs.",
        vec![typed("Tests", "directory"), typed("Specs", "directory")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Tests"),
        card_id(&engine, "Specs"),
        "tests->specs directory rename must not merge"
    );
}

/// Verifier F17: "the main branch" is a generic phrase.
#[test]
fn f17_main_branch_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Main tracks Origin.",
        vec![typed("Main", "branch"), typed("Origin", "branch")],
        vec![rel("Main", "tracks", "Origin", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Release tracks Stable.",
        vec![typed("Release", "branch"), typed("Stable", "branch")],
        vec![rel("Release", "tracks", "Stable", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "The main branch was renamed to release.",
        vec![typed("Main", "branch"), typed("Release", "branch")],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Main"),
        card_id(&engine, "Release"),
        "main branch -> release must not merge"
    );
}

/// Verifier F20: the same branch->main generic rename in agent voice.
#[test]
fn f20_team_bare_verb_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Branch tracks Origin.",
        vec![typed("Branch", "branch"), typed("Origin", "branch")],
        vec![rel("Branch", "tracks", "Origin", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "main tracks Release.",
        vec![typed("main", "branch"), typed("Release", "branch")],
        vec![rel("main", "tracks", "Release", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "The team renamed the branch to main.",
        vec![
            typed("Team", "team"),
            typed("Branch", "branch"),
            typed("main", "branch"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_ne!(
        card_id(&engine, "Branch"),
        card_id(&engine, "main"),
        "team-renamed branch->main must not merge"
    );
}

/// Verifier T01: "Harbor is now Lantern." is a rename.
#[test]
fn t01_is_now_name_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Harbor is now Lantern.",
        vec![project("Harbor"), project("Lantern")],
        vec![],
        "2026-06-01",
    );
    assert_eq!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "T01 'is now <name>' must merge"
    );
}

/// Verifier T02: parenthetical "(now Beacon)" is a rename.
#[test]
fn t02_parenthetical_now_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Atlas uses npm.",
        vec![project("Atlas"), typed("npm", "tool")],
        vec![rel("Atlas", "uses", "npm", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Atlas (now Beacon) uses Swift.",
        vec![
            project("Atlas"),
            project("Beacon"),
            typed("Swift", "language"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_eq!(
        card_id(&engine, "Atlas"),
        card_id(&engine, "Beacon"),
        "T02 '(now X)' must merge"
    );
}

/// Verifier T09: "Lantern (renamed from Harbor)" without "to" is a rename.
#[test]
fn t09_renamed_from_without_to_merges() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs on", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Lantern (renamed from Harbor) runs ingest.",
        vec![project("Lantern"), project("Harbor")],
        vec![],
        "2026-06-01",
    );
    assert_eq!(
        card_id(&engine, "Harbor"),
        card_id(&engine, "Lantern"),
        "T09 'renamed from' must merge"
    );
}

/// The CLI merge core now moves `relations.subj`: after `merge_entities`,
/// a later exclusive fact on the survivor ends the moved stale fact (the
/// merge command is unchanged-or-better).
#[test]
fn merge_command_moves_subj_and_ends_stale_exclusive_fact() {
    let engine = new_engine();
    let first = remember_at(
        &engine,
        "Atlas runs on Python 3.11.",
        vec![project("Atlas"), typed("Python 3.11", "language")],
        vec![rel("Atlas", "runs on", "Python 3.11", true)],
        "2026-01-09",
    );
    remember_at(
        &engine,
        "Beacon runs on Python 3.13.",
        vec![project("Beacon"), typed("Python 3.13", "language")],
        vec![],
        "2026-02-01",
    );
    // Merge via the command path (shares merge_entities_in with renames).
    engine.merge_entities("Beacon", "Atlas", false).unwrap();
    let survivor = card_id(&engine, "Beacon");
    // The moved 3.11 fact now keys on subj=survivor, so the stranded 3.13
    // value must end when a later exclusive fact lands.
    let third = remember_at(
        &engine,
        "Beacon runs on Python 3.14.",
        vec![project("Beacon"), typed("Python 3.14", "language")],
        vec![rel("Beacon", "runs on", "Python 3.14", true)],
        "2026-08-20",
    );
    assert!(
        !third.invalidated_relations.is_empty(),
        "merge must have moved subj so the exclusive slot ends stale facts"
    );
    let card = engine.entity("Beacon", 20).unwrap();
    assert_eq!(card.card.id, survivor);
    assert!(
        card.current_facts
            .iter()
            .any(|f| f.fact.contains("Python 3.14")),
        "current facts should hold Python 3.14, got {:?}",
        card.current_facts
    );
    assert!(
        !card
            .current_facts
            .iter()
            .any(|f| f.fact.contains("Python 3.11")),
        "Python 3.11 should not be current, got {:?}",
        card.current_facts
    );
    // first stays referenced for id readability.
    assert!(first.id > 0);
}

fn untyped(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        ..Default::default()
    }
}

// --- Fix round 2: written-form generic guard (verifier G/S/X) ---

/// Run one seed + one rename candidate and return the engine.
fn seed_rename(
    seed_subject: &str,
    seed_kind: &str,
    seed_pred: &str,
    seed_object: &str,
    seed_okind: &str,
    cand_content: &str,
    cand_entities: Vec<EntityIn>,
) -> Engine {
    let engine = new_engine();
    remember_at(
        &engine,
        &format!("{seed_subject} {seed_pred} {seed_object}."),
        vec![
            typed(seed_subject, seed_kind),
            typed(seed_object, seed_okind),
        ],
        vec![rel(seed_subject, seed_pred, seed_object, false)],
        "2026-01-05",
    );
    remember_at(&engine, cand_content, cand_entities, vec![], "2026-06-01");
    engine
}

fn assert_no_merge(engine: &Engine, a: &str, b: &str) {
    assert_ne!(
        card_id(engine, a),
        card_id(engine, b),
        "{a} and {b} must not have merged"
    );
}

fn assert_merged(engine: &Engine, a: &str, b: &str) {
    assert_eq!(
        card_id(engine, a),
        card_id(engine, b),
        "{a} and {b} should be one entity"
    );
}

#[test]
fn g01_repo_to_helios_merges_nothing() {
    let e = seed_rename(
        "Repo",
        "repo",
        "tracks",
        "Commits",
        "list",
        "The repo was renamed to Helios.",
        vec![typed("Repo", "repo"), untyped("Helios")],
    );
    assert_no_merge(&e, "Repo", "Helios");
}

#[test]
fn g02_channel_now_called_general_merges_nothing() {
    let e = seed_rename(
        "Channel",
        "channel",
        "carries",
        "Alerts",
        "list",
        "The channel is now called General.",
        vec![typed("Channel", "channel"), untyped("General")],
    );
    assert_no_merge(&e, "Channel", "General");
}

#[test]
fn g03_doc_to_playbook_merges_nothing() {
    let e = seed_rename(
        "Doc",
        "document",
        "lists",
        "Steps",
        "list",
        "The doc was renamed to Playbook.",
        vec![typed("Doc", "document"), untyped("Playbook")],
    );
    assert_no_merge(&e, "Doc", "Playbook");
}

#[test]
fn g04_ticket_to_aurora_merges_nothing() {
    let e = seed_rename(
        "Ticket",
        "issue",
        "tracks",
        "Bug 42",
        "issue",
        "The ticket was renamed to Aurora.",
        vec![typed("Ticket", "issue"), untyped("Aurora")],
    );
    assert_no_merge(&e, "Ticket", "Aurora");
}

#[test]
fn g05_sprint_to_cycle7_merges_nothing() {
    let e = seed_rename(
        "Sprint",
        "period",
        "ends",
        "May",
        "date",
        "The sprint was renamed to Cycle 7.",
        vec![typed("Sprint", "period"), untyped("Cycle 7")],
    );
    assert_no_merge(&e, "Sprint", "Cycle 7");
}

#[test]
fn g06_board_to_roadmap_merges_nothing() {
    let e = seed_rename(
        "Board",
        "doc",
        "lists",
        "Cards",
        "list",
        "The board was renamed to Roadmap.",
        vec![typed("Board", "doc"), untyped("Roadmap")],
    );
    assert_no_merge(&e, "Board", "Roadmap");
}

#[test]
fn g07_server_to_atlas_merges_nothing() {
    let e = seed_rename(
        "Server",
        "machine",
        "runs",
        "Debian",
        "os",
        "The server was renamed to Atlas.",
        vec![typed("Server", "machine"), untyped("Atlas")],
    );
    assert_no_merge(&e, "Server", "Atlas");
}

#[test]
fn g08_cluster_to_vega_merges_nothing() {
    let e = seed_rename(
        "Cluster",
        "infra",
        "runs",
        "Kubernetes",
        "tool",
        "The cluster was renamed to Vega.",
        vec![typed("Cluster", "infra"), untyped("Vega")],
    );
    assert_no_merge(&e, "Cluster", "Vega");
}

#[test]
fn g09_app_to_aurora_merges_nothing() {
    let e = seed_rename(
        "App",
        "tool",
        "uses",
        "React",
        "tool",
        "The app was renamed to Aurora.",
        vec![typed("App", "tool"), untyped("Aurora")],
    );
    assert_no_merge(&e, "App", "Aurora");
}

#[test]
fn g10_bucket_to_archive_merges_nothing() {
    let e = seed_rename(
        "Bucket",
        "storage",
        "holds",
        "Backups",
        "list",
        "The bucket was renamed to Archive.",
        vec![typed("Bucket", "storage"), untyped("Archive")],
    );
    assert_no_merge(&e, "Bucket", "Archive");
}

#[test]
fn g11_repo_to_main_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Repo tracks Commits.",
        vec![typed("Repo", "repo"), typed("Commits", "list")],
        vec![rel("Repo", "tracks", "Commits", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "main tracks Release.",
        vec![typed("main", "branch"), typed("Release", "branch")],
        vec![rel("main", "tracks", "Release", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "The repo was renamed to main.",
        vec![untyped("Repo"), untyped("main")],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Repo", "main");
}

#[test]
fn g12_channel_now_general_merges_nothing() {
    let e = seed_rename(
        "Channel",
        "channel",
        "carries",
        "Alerts",
        "list",
        "The channel is now general.",
        vec![untyped("Channel"), untyped("general")],
    );
    assert_no_merge(&e, "Channel", "general");
}

#[test]
fn s01_stable_merges_nothing() {
    let e = seed_rename(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor is now stable.",
        vec![project("Harbor"), untyped("Stable")],
    );
    assert_no_merge(&e, "Harbor", "Stable");
}

#[test]
fn s02_deprecated_merges_nothing() {
    let e = seed_rename(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor is now deprecated.",
        vec![project("Harbor"), untyped("Deprecated")],
    );
    assert_no_merge(&e, "Harbor", "Deprecated");
}

#[test]
fn s03_busy_merges_nothing() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Ana Silva works at Acme.",
        vec![person("Ana Silva"), typed("Acme", "org")],
        vec![rel("Ana Silva", "works at", "Acme", false)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Ana Silva is now busy.",
        vec![person("Ana Silva"), untyped("Busy")],
        vec![],
        "2026-06-01",
    );
    assert_no_merge(&engine, "Ana Silva", "Busy");
}

#[test]
fn s04_public_merges_nothing() {
    let e = seed_rename(
        "Atlas",
        "project",
        "uses",
        "npm",
        "tool",
        "Atlas is now public.",
        vec![project("Atlas"), untyped("Public")],
    );
    assert_no_merge(&e, "Atlas", "Public");
}

#[test]
fn s05_archived_merges_nothing() {
    let e = seed_rename(
        "Lantern",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Lantern is now archived.",
        vec![project("Lantern"), untyped("Archived")],
    );
    assert_no_merge(&e, "Lantern", "Archived");
}

#[test]
fn s06_in_berlin_merges_nothing() {
    let e = seed_rename(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor is now in Berlin.",
        vec![project("Harbor"), typed("Berlin", "city")],
    );
    assert_no_merge(&e, "Harbor", "Berlin");
}

#[test]
fn s07_now_based_merges_nothing() {
    let e = seed_rename(
        "Atlas",
        "project",
        "uses",
        "npm",
        "tool",
        "Atlas (now based in Berlin) runs the pipeline.",
        vec![project("Atlas"), typed("Berlin", "city")],
    );
    assert_no_merge(&e, "Atlas", "Berlin");
}

#[test]
fn x01_state_word_does_not_end_exclusive_fact() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Channel carries Alerts.",
        vec![typed("Channel", "channel"), typed("Alerts", "list")],
        vec![rel("Channel", "carries", "Alerts", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "The channel is now general.",
        vec![untyped("Channel"), untyped("general")],
        vec![],
        "2026-06-01",
    );
    remember_at(
        &engine,
        "General carries Pages.",
        vec![untyped("General"), typed("Pages", "list")],
        vec![rel("General", "carries", "Pages", true)],
        "2026-07-01",
    );
    assert_no_merge(&engine, "Channel", "General");
    let ch = engine.entity("Channel", 20).unwrap();
    assert!(
        ch.current_facts.iter().any(|f| f.fact.contains("Alerts")),
        "Channel Alerts should stay live, got {:?}",
        ch.current_facts
    );
    let gen = engine.entity("General", 20).unwrap();
    assert!(
        gen.current_facts.iter().any(|f| f.fact.contains("Pages")),
        "General Pages should be live, got {:?}",
        gen.current_facts
    );
}

#[test]
fn l01_true_plus_generic_second_cue() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Team owns Backlog.",
        vec![typed("Team", "team"), typed("Backlog", "list")],
        vec![rel("Team", "owns", "Backlog", false)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "Harbor was renamed to Lantern and the team is now called Platform.",
        vec![
            project("Harbor"),
            project("Lantern"),
            typed("Team", "team"),
            typed("Platform", "team"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_merged(&engine, "Harbor", "Lantern");
    assert_no_merge(&engine, "Team", "Platform");
}

#[test]
fn l02_two_true_renames_both_merge() {
    let engine = new_engine();
    remember_at(
        &engine,
        "Harbor runs on Python 3.11.",
        vec![project("Harbor"), typed("Python 3.11", "language")],
        vec![rel("Harbor", "runs", "Python 3.11", true)],
        "2026-01-05",
    );
    remember_at(
        &engine,
        "Atlas uses npm.",
        vec![project("Atlas"), typed("npm", "tool")],
        vec![rel("Atlas", "uses", "npm", true)],
        "2026-01-06",
    );
    remember_at(
        &engine,
        "Harbor was renamed to Lantern after Atlas was renamed to Beacon.",
        vec![
            project("Harbor"),
            project("Lantern"),
            project("Atlas"),
            project("Beacon"),
        ],
        vec![],
        "2026-06-01",
    );
    assert_merged(&engine, "Harbor", "Lantern");
    assert_merged(&engine, "Atlas", "Beacon");
}

#[test]
fn l03_deep_clause_merges() {
    let e = seed_rename(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "After the long migration the Harbor ingest project was renamed to Lantern.",
        vec![project("Harbor"), project("Lantern")],
    );
    assert_merged(&e, "Harbor", "Lantern");
}

#[test]
fn n01_zurich_hub_merges() {
    let e = seed_rename(
        "Zürich Hub",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "The Zürich Hub project was renamed to Helvetia.",
        vec![typed("Zürich Hub", "project"), typed("Helvetia", "project")],
    );
    assert_merged(&e, "Zürich Hub", "Helvetia");
}

#[test]
fn n02_sao_paulo_merges() {
    let e = seed_rename(
        "São Paulo Team",
        "team",
        "owns",
        "Backlog",
        "list",
        "The São Paulo Team was renamed to Latam Team.",
        vec![typed("São Paulo Team", "team"), typed("Latam Team", "team")],
    );
    assert_merged(&e, "São Paulo Team", "Latam Team");
}

#[test]
fn n03_cjk_merges() {
    let e = seed_rename(
        "東京",
        "project",
        "uses",
        "npm",
        "tool",
        "東京 is now Tokyo.",
        vec![typed("東京", "project"), typed("Tokyo", "project")],
    );
    assert_merged(&e, "東京", "Tokyo");
}

#[test]
fn m01_lower_old_upper_new_merges() {
    let e = seed_rename(
        "harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "harbor is now LANTERN.",
        vec![typed("harbor", "project"), typed("LANTERN", "project")],
    );
    assert_merged(&e, "harbor", "LANTERN");
}

#[test]
fn m02_upper_sentence_merges() {
    let e = seed_rename(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "HARBOR WAS RENAMED TO LANTERN.",
        vec![project("Harbor"), typed("lantern", "project")],
    );
    assert_merged(&e, "Harbor", "lantern");
}

#[test]
fn and_hedge_does_not_block_true_rename() {
    let e = seed_rename(
        "Harbor",
        "project",
        "runs on",
        "Python 3.11",
        "language",
        "Harbor was renamed to Lantern and we may expand it later.",
        vec![project("Harbor"), project("Lantern")],
    );
    assert_merged(&e, "Harbor", "Lantern");
}
