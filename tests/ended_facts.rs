//! Lane nc-ended: a text-only "no longer" retraction ends the live fact with a
//! hyphenated or multi-word object (H3-09), and a yes/no "still" question
//! shows the matching ended fact with its end date (H3-11).

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput},
    render,
    temporal::Clock,
};
use std::sync::Arc;

/// 2026-10-23 12:00 UTC, a Friday.
const NOW: i64 = 1_792_756_800;

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn person(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
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

fn project(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("project".into()),
        ..Default::default()
    }
}

fn place(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("place".into()),
        ..Default::default()
    }
}

fn org(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("org".into()),
        ..Default::default()
    }
}

fn group(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("group".into()),
        ..Default::default()
    }
}

fn rel(src: &str, predicate: &str, dst: &str, exclusive: bool) -> RelationIn {
    RelationIn {
        source: src.into(),
        predicate: predicate.into(),
        target: dst.into(),
        exclusive: Some(exclusive),
        ..Default::default()
    }
}

fn recall(e: &Engine, query: &str) -> neocortex::engine::RecallOutput {
    e.recall(&RecallInput {
        query: query.into(),
        ..Default::default()
    })
    .unwrap()
}

fn text(e: &Engine, out: &neocortex::engine::RecallOutput) -> String {
    render::render_recall(out, &render::clock_fmt(&e.clock()))
}

/// The relation's end, if any: `valid_to` or the system retirement time.
fn relation_end(e: &Engine, rid: i64) -> Option<i64> {
    e.db.sql()
        .query_row(
            "SELECT COALESCE(valid_to, expired_at) FROM relations WHERE id=?1",
            [rid],
            |r| r.get(0),
        )
        .unwrap()
}

/// Ingest a relation-bearing memory and return its relation id.
fn tomasz_reviews_payment(e: &Engine) -> i64 {
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak is the security reviewer for all payment-adjacent code.".into(),
            valid_from: Some("2026-02-04".into()),
            entities: vec![person("Tomasz Nowak")],
            relations: vec![rel(
                "Tomasz Nowak",
                "reviews",
                "payment-adjacent code",
                false,
            )],
            ..Default::default()
        })
        .unwrap();
    out.relations[0].id
}

fn retract(e: &Engine, content: &str) -> neocortex::engine::RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        entities: vec![person("Tomasz Nowak"), person("Ines Park")],
        ..Default::default()
    })
    .unwrap()
}

/// Ingest a "leads the team" relation and return its id.
fn tomasz_leads_team(e: &Engine) -> i64 {
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak leads the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            relations: vec![rel("Tomasz Nowak", "leads", "team", false)],
            ..Default::default()
        })
        .unwrap();
    out.relations[0].id
}

// ------------------------------------------------------------- H3-09

#[test]
fn h3_09_no_longer_with_a_hyphenated_object_ends_the_fact() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer reviews payment-adjacent code.");
    assert!(
        out.invalidated_relations.contains(&rid),
        "hyphenated object not matched: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_09_a_trailing_date_phrase_does_not_hide_the_object() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(
        &e,
        "Tomasz Nowak no longer reviews payment-adjacent code since August 2026; Ines Park took over the security reviews.",
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "'since August 2026' tail kept the fact live: {out:?}"
    );
}

#[test]
fn h3_09_since_a_noun_is_not_a_time_tail() {
    // "since the migration" continues the clause, so the object is not whole.
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(
        &e,
        "Tomasz Nowak no longer reviews payment-adjacent code since the migration.",
    );
    assert!(
        !out.invalidated_relations.contains(&rid),
        "'since the migration' ended the fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_09_a_different_object_does_not_end_the_fact() {
    // "payment code" is not "payment-adjacent code": the live fact stays.
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer reviews payment code.");
    assert!(
        !out.invalidated_relations.contains(&rid),
        "a different object ended the fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

// ------------------------------------------------------------- H3-13

fn trial_handover(e: &Engine) {
    e.remember(RememberInput {
        content: "Tunde Kalu led the Harbor trial.".into(),
        valid_from: Some("2026-01-10".into()),
        entities: vec![person("Tunde Kalu"), project("Harbor trial")],
        relations: vec![rel("Tunde Kalu", "led", "Harbor trial", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Amara Nwosu now leads the Harbor trial.".into(),
        valid_from: Some("2026-02-10".into()),
        entities: vec![person("Amara Nwosu"), project("Harbor trial")],
        relations: vec![rel("Amara Nwosu", "leads", "Harbor trial", true)],
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn h3_13_past_slot_questions_retrieve_ended_memory_but_current_does_not() {
    let e = engine();
    trial_handover(&e);
    for query in [
        "which trial did Tunde Kalu lead",
        "the trial that Tunde Kalu led",
        "who used to lead the Harbor trial",
        "who is the former lead of the Harbor trial",
    ] {
        let out = recall(&e, query);
        assert_eq!(
            out.memories.first().map(|m| m.id),
            Some(1),
            "{query}: {:?}",
            out.memories
        );
        assert!(
            out.facts.iter().any(|fact| fact.id == 1 && fact.ended),
            "{query}: {:?}",
            out.facts
        );
    }
    let current = recall(&e, "who leads the Harbor trial");
    assert_eq!(current.memories.first().map(|m| m.id), Some(2));
    assert!(!current.facts.iter().any(|fact| fact.id == 1));
}

#[test]
fn h3_13_past_state_boost_requires_the_asked_predicate() {
    let e = engine();
    trial_handover(&e);
    let out = recall(&e, "which tool did Tunde Kalu use");
    let old = out.memories.iter().find(|m| m.id == 1).unwrap();
    assert!(
        old.score < 2.0,
        "unrelated ended predicate was boosted: {}",
        old.score
    );
}

#[test]
fn h3_13_negated_retraction_is_not_resurrected_as_live() {
    let e = engine();
    trial_handover(&e);
    let out = recall(&e, "Tunde Kalu did not lead the Harbor trial");
    assert!(!out.facts.iter().any(|fact| fact.id == 1 && !fact.ended));
}

// ------------------------------------------------------------- H3-13b

fn setup_lead_handover_with_competitors(e: &Engine) {
    e.remember(RememberInput {
        content: "Mira Sol led the Birch launch.".into(),
        valid_from: Some("2026-01-10".into()),
        entities: vec![person("Mira Sol"), project("Birch launch")],
        relations: vec![rel("Mira Sol", "leads", "Birch launch", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Noah Imani took over as lead of the Birch launch.".into(),
        valid_from: Some("2026-02-10".into()),
        entities: vec![person("Noah Imani"), project("Birch launch")],
        relations: vec![rel("Noah Imani", "leads", "Birch launch", true)],
        ..Default::default()
    })
    .unwrap();
    for i in 0..30 {
        let colleague = format!("Colleague {i:02}");
        e.remember(RememberInput {
            content: format!(
                "{colleague} reviewed Birch launch planning notes during meeting {i:02}."
            ),
            valid_from: Some(format!("2026-01-0{}", (i % 9) + 1)),
            entities: vec![person(&colleague), project("Birch launch")],
            relations: vec![rel(&colleague, "reviewed notes for", "Birch launch", false)],
            ..Default::default()
        })
        .unwrap();
    }
}

fn setup_tool_handover_with_competitors(e: &Engine) {
    e.remember(RememberInput {
        content: "Cobalt uses pnpm for package management.".into(),
        valid_from: Some("2026-01-10".into()),
        entities: vec![project("Cobalt"), tool("pnpm")],
        relations: vec![rel("Cobalt", "uses", "pnpm", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Cobalt switched from pnpm to yarn.".into(),
        valid_from: Some("2026-02-10".into()),
        entities: vec![project("Cobalt"), tool("pnpm"), tool("yarn")],
        relations: vec![rel("Cobalt", "uses", "yarn", true)],
        ..Default::default()
    })
    .unwrap();
    for i in 0..30 {
        let colleague = format!("Colleague {i:02}");
        e.remember(RememberInput {
            content: format!("{colleague} reviewed pnpm planning notes during meeting {i:02}."),
            valid_from: Some(format!("2026-01-0{}", (i % 9) + 1)),
            entities: vec![person(&colleague), tool("pnpm")],
            relations: vec![rel(&colleague, "reviewed notes for", "pnpm", false)],
            ..Default::default()
        })
        .unwrap();
    }
}

fn setup_owner_handover_with_competitors(e: &Engine) {
    e.remember(RememberInput {
        content: "Lena Ortiz owns the Orchid archive.".into(),
        valid_from: Some("2026-01-10".into()),
        entities: vec![person("Lena Ortiz"), project("Orchid archive")],
        relations: vec![rel("Lena Ortiz", "owns", "Orchid archive", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Pavel Duran took over ownership of the Orchid archive.".into(),
        valid_from: Some("2026-02-10".into()),
        entities: vec![person("Pavel Duran"), project("Orchid archive")],
        relations: vec![rel("Pavel Duran", "owns", "Orchid archive", true)],
        ..Default::default()
    })
    .unwrap();
    for i in 0..30 {
        let colleague = format!("Colleague {i:02}");
        e.remember(RememberInput {
            content: format!(
                "{colleague} reviewed Orchid archive planning notes during meeting {i:02}."
            ),
            valid_from: Some(format!("2026-01-0{}", (i % 9) + 1)),
            entities: vec![person(&colleague), project("Orchid archive")],
            relations: vec![rel(
                &colleague,
                "reviewed notes for",
                "Orchid archive",
                false,
            )],
            ..Default::default()
        })
        .unwrap();
    }
}

#[test]
fn h3_13b_relative_clause_and_before_successor_lead_ranks_ended_memory_in_top_3() {
    let e = engine();
    setup_lead_handover_with_competitors(&e);
    for query in [
        "the Birch launch that Mira Sol led",
        "the Birch launch that Mira Sol lead",
        "who led Birch launch before Noah Imani",
        "who lead Birch launch before Noah Imani",
    ] {
        let out = recall(&e, query);
        let rank = out.memories.iter().position(|m| m.id == 1).map(|i| i + 1);
        assert!(
            rank.is_some_and(|r| r <= 3),
            "{query}: ended memory rank is {rank:?}, expected <= 3; top memories: {:?}",
            out.memories
                .iter()
                .take(5)
                .map(|m| (m.id, &m.content, m.score))
                .collect::<Vec<_>>()
        );
    }
    // Controls:
    // "who leads Birch launch" -> live memory (m2) is #1, m1 is not #1
    let cur = recall(&e, "who leads Birch launch");
    assert_eq!(cur.memories.first().map(|m| m.id), Some(2));
    // "what did Mira Sol do before lunch" -> m1 is not boosted
    let lunch = recall(&e, "what did Mira Sol do before lunch");
    let m1_lunch = lunch.memories.iter().find(|m| m.id == 1);
    assert!(m1_lunch.is_none() || m1_lunch.unwrap().score < 2.0);
    // passive "led by" -> live memory is #1
    let passive = recall(&e, "the Birch launch led by Noah Imani");
    assert_eq!(passive.memories.first().map(|m| m.id), Some(2));
}

#[test]
fn h3_13b_relative_clause_and_before_successor_tool_ranks_ended_memory_in_top_3() {
    let e = engine();
    setup_tool_handover_with_competitors(&e);
    for query in [
        "the pnpm that Cobalt used",
        "the pnpm that Cobalt use",
        "who used pnpm before yarn",
        "who use pnpm before yarn",
    ] {
        let out = recall(&e, query);
        let rank = out.memories.iter().position(|m| m.id == 1).map(|i| i + 1);
        assert!(
            rank.is_some_and(|r| r <= 3),
            "{query}: ended memory rank is {rank:?}, expected <= 3; top memories: {:?}",
            out.memories
                .iter()
                .take(5)
                .map(|m| (m.id, &m.content, m.score))
                .collect::<Vec<_>>()
        );
    }
    // Controls:
    let cur = recall(&e, "which package manager does Cobalt use");
    assert_eq!(cur.memories.first().map(|m| m.id), Some(2));
    let lunch = recall(&e, "what did Cobalt do before lunch");
    let m1_lunch = lunch.memories.iter().find(|m| m.id == 1);
    assert!(m1_lunch.is_none() || m1_lunch.unwrap().score < 2.0);
}

#[test]
fn h3_13b_relative_clause_and_before_successor_ownership_ranks_ended_memory_in_top_3() {
    let e = engine();
    setup_owner_handover_with_competitors(&e);
    for query in [
        "the Orchid archive that Lena Ortiz owned",
        "the Orchid archive that Lena Ortiz own",
        "who owned Orchid archive before Pavel Duran",
        "who own Orchid archive before Pavel Duran",
    ] {
        let out = recall(&e, query);
        let rank = out.memories.iter().position(|m| m.id == 1).map(|i| i + 1);
        assert!(
            rank.is_some_and(|r| r <= 3),
            "{query}: ended memory rank is {rank:?}, expected <= 3; top memories: {:?}",
            out.memories
                .iter()
                .take(5)
                .map(|m| (m.id, &m.content, m.score))
                .collect::<Vec<_>>()
        );
    }
    // Controls:
    let cur = recall(&e, "who owns Orchid archive");
    assert_eq!(cur.memories.first().map(|m| m.id), Some(2));
    let lunch = recall(&e, "what did Lena Ortiz do before lunch");
    let m1_lunch = lunch.memories.iter().find(|m| m.id == 1);
    assert!(m1_lunch.is_none() || m1_lunch.unwrap().score < 2.0);
}

/// The verifier's ordering corpus (nc-h313b verify1 section 3b): three
/// projects Tessa Nakamura led, oldest first, then a handover that ends the
/// newest one. An ordering question that also carries a past-tense cue must
/// still answer with the chronologically first project.
fn tessa_ordering_corpus(e: &Engine) {
    for (project_name, day) in [
        ("Elm rollout", "2023-02-01"),
        ("Aspen refresh", "2023-09-01"),
        ("Rowan migration", "2024-01-05"),
    ] {
        e.remember(RememberInput {
            content: format!("Tessa Nakamura led the {project_name}."),
            valid_from: Some(day.into()),
            entities: vec![person("Tessa Nakamura"), project(project_name)],
            relations: vec![rel("Tessa Nakamura", "leads", project_name, true)],
            ..Default::default()
        })
        .unwrap();
    }
    e.remember(RememberInput {
        content: "Emil Sorensen took over as lead of the Rowan migration.".into(),
        valid_from: Some("2025-01-05".into()),
        entities: vec![person("Emil Sorensen"), project("Rowan migration")],
        relations: vec![rel("Emil Sorensen", "leads", "Rowan migration", true)],
        ..Default::default()
    })
    .unwrap();
}

/// The answer to an ordering question: the chronologically first project,
/// "Elm rollout" (m1). The ended Rowan fact (m3) must not outrank it just
/// because the question carries a past-tense verb.
#[test]
fn h3_13b_ordering_question_with_past_tense_cue_keeps_date_order() {
    let e = engine();
    tessa_ordering_corpus(&e);
    for query in [
        // R1, R3, R4, R5, R9, R11 from verify1 section 3b.
        "which project did Tessa Nakamura first lead",
        "the first project that Tessa Nakamura led",
        "which project did Tessa Nakamura lead after 2023-03-01",
        "which project did Tessa Nakamura lead before May",
        "which project did Tessa Nakamura lead before Aspen refresh",
        "which project did Tessa Nakamura lead first, Elm or Rowan",
    ] {
        let out = recall(&e, query);
        assert_eq!(
            out.memories.first().map(|m| m.id),
            Some(1),
            "{query}: expected the earliest project first, got {:?}",
            out.memories
                .iter()
                .take(5)
                .map(|m| (m.id, &m.content, m.score))
                .collect::<Vec<_>>()
        );
    }
    // R10: "who did Tessa lead before Emil" names the old holder, but its
    // object ("who") is the asked side and the named subject is Tessa, so it
    // stays an ordering question. It must not put the ended fact first.
    let out = recall(&e, "who did Tessa Nakamura lead before Emil Sorensen");
    assert!(
        !out.memories.iter().take(1).any(|m| m.id == 3),
        "R10 put the ended Rowan fact first: {:?}",
        out.memories
            .iter()
            .take(5)
            .map(|m| (m.id, &m.content, m.score))
            .collect::<Vec<_>>()
    );
    // Controls: the two target frames still put the ended memory in the top 3,
    // and the current-state question still answers with the successor.
    for query in [
        "the Rowan migration that Tessa Nakamura led",
        "who led the Rowan migration before Emil Sorensen",
    ] {
        let out = recall(&e, query);
        let rank = out.memories.iter().position(|m| m.id == 3).map(|i| i + 1);
        assert!(
            rank.is_some_and(|r| r <= 3),
            "{query}: ended memory rank is {rank:?}, expected <= 3"
        );
    }
    let cur = recall(&e, "who leads the Rowan migration");
    assert_eq!(cur.memories.first().map(|m| m.id), Some(4));
}

// ------------------------------------------------------------- H3-11

/// Atlas used npm, then switched to pnpm: the npm fact is ended. Eight other
/// Atlas tools fill the fact channels so the ended row competes for a slot.
fn atlas_switched_to_pnpm(e: &Engine) {
    for (i, tool_name) in [
        "Vitest",
        "Playwright",
        "Docker",
        "Terraform",
        "Redis",
        "Postgres",
        "Sentry",
        "Renovate",
    ]
    .iter()
    .enumerate()
    {
        e.remember(RememberInput {
            content: format!("Atlas depends on {tool_name} in its build pipeline."),
            valid_from: Some(format!("2025-0{}-01", i % 9 + 1)),
            entities: vec![project("Atlas"), tool(tool_name)],
            relations: vec![rel("Atlas", "depends on", tool_name, false)],
            ..Default::default()
        })
        .unwrap();
    }
    e.remember(RememberInput {
        content: "The Atlas repo uses npm for package management.".into(),
        valid_from: Some("2025-03-01".into()),
        entities: vec![project("Atlas"), tool("npm")],
        relations: vec![rel("Atlas", "uses", "npm", true)],
        ..Default::default()
    })
    .unwrap();
    e.remember(RememberInput {
        content: "Decision on 2026-02-10: switched Atlas from npm to pnpm because installs were slow. npm is no longer used in Atlas.".into(),
        valid_from: Some("2026-02-10".into()),
        entities: vec![project("Atlas"), tool("pnpm")],
        relations: vec![rel("Atlas", "uses", "pnpm", true)],
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn h3_11_still_question_shows_the_ended_object_with_its_end_date() {
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is npm still used in atlas");
    let shown = text(&e, &out);
    let npm_line = shown
        .lines()
        .find(|l| l.contains("→ npm"))
        .unwrap_or_else(|| panic!("no npm fact in the answer:\n{shown}"));
    assert!(
        npm_line.contains("ended"),
        "npm fact not marked ended: {npm_line}"
    );
    assert!(
        npm_line.contains("2026-02-10"),
        "end date missing: {npm_line}"
    );
}

#[test]
fn h3_11_still_question_never_shows_a_live_fact_as_ended() {
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is pnpm still used in atlas");
    let shown = text(&e, &out);
    let pnpm_line = shown
        .lines()
        .find(|l| l.contains("→ pnpm"))
        .unwrap_or_else(|| panic!("no pnpm fact in the answer:\n{shown}"));
    assert!(
        !pnpm_line.contains("ended"),
        "live pnpm fact shown as ended: {pnpm_line}"
    );
}

#[test]
fn h3_11_question_naming_only_the_subject_is_unchanged() {
    // The channel needs both ends: "is npm still used" names no subject.
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is npm still used");
    let shown = text(&e, &out);
    assert!(
        !shown
            .lines()
            .any(|l| l.contains("→ npm") && l.contains("ended")),
        "a subject-only question showed the ended fact:\n{shown}"
    );
}

#[test]
fn h3_11_question_without_still_is_unchanged() {
    let e = engine();
    atlas_switched_to_pnpm(&e);
    let out = recall(&e, "is npm used in atlas");
    let shown = text(&e, &out);
    assert!(
        !shown
            .lines()
            .any(|l| l.contains("→ npm") && l.contains("ended")),
        "a question without a status cue showed the ended fact:\n{shown}"
    );
}

// ------------------------------------------------------------- H3-12

#[test]
fn h3_12_approves_does_not_end_reviews() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer approves payment-adjacent code.");
    assert!(
        !out.invalidated_relations.contains(&rid),
        "approves ended reviews fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_no_longer_only_ends_nothing() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(
        &e,
        "Tomasz Nowak no longer only reviews payment-adjacent code.",
    );
    assert!(
        !out.invalidated_relations.contains(&rid),
        "no longer only ended fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_same_verb_reviews_ends_fact() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer reviews payment-adjacent code.");
    assert!(
        out.invalidated_relations.contains(&rid),
        "same verb reviews failed to end fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_stopped_using_npm_ends_uses_npm() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Atlas uses npm for building packages.".into(),
            entities: vec![project("Atlas"), tool("npm")],
            relations: vec![rel("Atlas", "uses", "npm", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Atlas stopped using npm.".into(),
            entities: vec![project("Atlas"), tool("npm")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "stopped using did not end uses: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_left_acme_ends_works_at_acme() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Ivan left Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "left did not end works at: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_no_longer_lives_in_pune_ends_lives_in_pune() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Carol lives in Pune.".into(),
            entities: vec![person("Carol"), place("Pune")],
            relations: vec![rel("Carol", "lives in", "Pune", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Carol no longer lives in Pune.".into(),
            entities: vec![person("Carol"), place("Pune")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "no longer lives in did not end lives in: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_no_longer_works_with_does_not_end_works_at() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;

    let out2 = e
        .remember(RememberInput {
            content: "Ivan no longer works with Ana.".into(),
            entities: vec![person("Ivan"), person("Ana")],
            ..Default::default()
        })
        .unwrap();
    assert!(!out2.invalidated_relations.contains(&rid));
    assert_eq!(relation_end(&e, rid), None);

    let out3 = e
        .remember(RememberInput {
            content: "Ivan no longer works with Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        !out3.invalidated_relations.contains(&rid),
        "works with ended works at: {out3:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_retraction_with_relations_passed_by_caller_is_unchanged() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid1 = out1.relations[0].id;

    let out2 = e
        .remember(RememberInput {
            content: "Ivan joined Globex; he left Acme.".into(),
            entities: vec![person("Ivan"), org("Globex"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Globex", true)],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid1),
        "old relation not ended: {out2:?}"
    );
    let rid2 = out2.relations[0].id;
    assert!(
        !out2.invalidated_relations.contains(&rid2),
        "new relation ended itself: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid1), Some(NOW));
    assert_eq!(relation_end(&e, rid2), None);
}

/// S01: "heads" is outside the predicate ontology (it is also a role noun), so
/// the retraction matcher carries the heads/leads synonym pair explicitly.
#[test]
fn h3_12_no_longer_heads_ends_leads() {
    let e = engine();
    let rid = tomasz_leads_team(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer heads the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "heads did not end leads: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// R02: the same synonym in the past tense.
#[test]
fn h3_12_no_longer_headed_ends_leads() {
    let e = engine();
    let rid = tomasz_leads_team(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer headed the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "headed did not end leads: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// R01: the same verb still ends, so the synonym path did not replace it.
#[test]
fn h3_12_no_longer_leads_ends_leads() {
    let e = engine();
    let rid = tomasz_leads_team(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer leads the team.".into(),
            entities: vec![person("Tomasz Nowak"), group("team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "leads did not end leads: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// X01: an -ly adverb between the cue and the verb is not part of the verb.
#[test]
fn h3_12_adverb_between_cue_and_verb_ends() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer actively reviews payment-adjacent code.".into(),
            entities: vec![person("Tomasz Nowak")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.invalidated_relations.contains(&rid),
        "adverb hid the verb: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// X02: the adverb is stripped, the preposition still tells "works at" apart.
#[test]
fn h3_12_no_longer_works_remotely_ends_works_at() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Ivan works at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            relations: vec![rel("Ivan", "works at", "Acme", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Ivan no longer works remotely at Acme.".into(),
            entities: vec![person("Ivan"), org("Acme")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "remotely hid works at: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// N02: "no longer just X" negates exclusivity, like "no longer only X".
#[test]
fn h3_12_no_longer_just_does_not_end() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer just reviews payment-adjacent code.".into(),
            entities: vec![person("Tomasz Nowak")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        !out.invalidated_relations.contains(&rid),
        "no longer just ended the fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

/// The run/manage synonym pair: a second entry in the table, and its negative
/// side, so the table cannot silently over-match.
#[test]
fn h3_12_no_longer_runs_ends_manages() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Tomasz Nowak manages the platform team.".into(),
            entities: vec![person("Tomasz Nowak"), group("platform team")],
            relations: vec![rel("Tomasz Nowak", "manages", "platform team", false)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Tomasz Nowak no longer runs the platform team.".into(),
            entities: vec![person("Tomasz Nowak"), group("platform team")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "runs did not end manages: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// A synonym pair never matches outside itself: "oversees" is paired with
/// "supervises", so it must not end "reviews".
#[test]
fn h3_12_oversees_does_not_end_reviews() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract(&e, "Tomasz Nowak no longer oversees payment-adjacent code.");
    assert!(
        !out.invalidated_relations.contains(&rid),
        "oversees ended reviews: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

/// S02/S05 (round-2 defect 2): "stopped <verb>ing" is a retraction cue too.
#[test]
fn h3_12_stopped_working_at_ends_works_at() {
    let e = engine();
    let rid = tomasz_works_at_acme(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak stopped working at Acme.",
        vec![person("Tomasz Nowak"), org("Acme")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "stopped working at did not end works at: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

#[test]
fn h3_12_has_stopped_reviewing_ends_reviews() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak has stopped reviewing payment-adjacent code.",
        vec![person("Tomasz Nowak")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "has stopped reviewing did not end reviews: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// The generic "stopped" cue still respects the verb: another verb ends nothing.
#[test]
fn h3_12_stopped_approving_does_not_end_reviews() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak stopped approving payment-adjacent code.",
        vec![person("Tomasz Nowak")],
    );
    assert!(
        !out.invalidated_relations.contains(&rid),
        "stopped approving ended reviews: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

/// Y01-Y09, P5: a trailing `, only ...` / `, just ...` clause states the new
/// state, so the retraction still ends the fact (main behaviour; round 1
/// regressed here). The scope check applies between the cue and the object and
/// immediately before the cue, never after the object.
fn tomasz_works_at_acme(e: &Engine) -> i64 {
    let out = e
        .remember(RememberInput {
            content: "Tomasz Nowak works at Acme.".into(),
            valid_from: Some("2026-01-04".into()),
            entities: vec![person("Tomasz Nowak"), org("Acme"), org("Brightline")],
            relations: vec![rel("Tomasz Nowak", "works at", "Acme", false)],
            ..Default::default()
        })
        .unwrap();
    out.relations[0].id
}

fn retract_with(
    e: &Engine,
    content: &str,
    entities: Vec<EntityIn>,
) -> neocortex::engine::RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        entities,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn h3_12_trailing_scope_clause_still_ends_works_at() {
    for text in [
        "Tomasz Nowak no longer works at Acme, only at Brightline.",
        "Tomasz Nowak no longer works at Acme, just at Brightline.",
        "Tomasz Nowak no longer works at Acme; only Brightline remains.",
        "Tomasz Nowak no longer works at Acme, only there.",
        "Tomasz Nowak no longer works at Acme, just not there.",
    ] {
        let e = engine();
        let rid = tomasz_works_at_acme(&e);
        let out = retract_with(
            &e,
            text,
            vec![person("Tomasz Nowak"), org("Acme"), org("Brightline")],
        );
        assert!(
            out.invalidated_relations.contains(&rid),
            "{text}: trailing clause kept the fact live: {out:?}"
        );
        assert_eq!(relation_end(&e, rid), Some(NOW), "{text}");
    }
}

/// Y05: synonym verb plus a trailing scope clause.
#[test]
fn h3_12_trailing_scope_clause_still_ends_synonym_verb() {
    let e = engine();
    let rid = tomasz_leads_team(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak no longer heads the team, only advises it.",
        vec![person("Tomasz Nowak"), group("team")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "trailing clause kept the synonym retraction live: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// Y07: trailing clause naming a different object ("the docs").
#[test]
fn h3_12_trailing_scope_clause_still_ends_reviews() {
    let e = engine();
    let rid = tomasz_reviews_payment(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak no longer reviews payment-adjacent code, only the docs.",
        vec![person("Tomasz Nowak")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "trailing clause kept reviews live: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// Y12 control: a scope word between the cue and the object still ends nothing.
#[test]
fn h3_12_no_longer_only_with_trailing_contrast_ends_nothing() {
    let e = engine();
    let rid = tomasz_works_at_acme(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak no longer only works at Acme, but also at Brightline.",
        vec![person("Tomasz Nowak"), org("Acme"), org("Brightline")],
    );
    assert!(
        !out.invalidated_relations.contains(&rid),
        "no longer only ended the fact: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

#[test]
fn h3_12_coordinated_pair_retraction_unchanged() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Acme Corp and Northwind are partners since 2023.".into(),
            entities: vec![org("Acme Corp"), org("Northwind")],
            relations: vec![rel("Acme Corp", "partners with", "Northwind", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Acme Corp and Northwind are no longer partners.".into(),
            entities: vec![org("Acme Corp"), org("Northwind")],
            ..Default::default()
        })
        .unwrap();
    assert!(
        out2.invalidated_relations.contains(&rid),
        "pair partners not ended: {out2:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// B10/B12 (round-3 defect 1): a negating word before the cue keeps the fact
/// live, including contracted auxiliaries ("hasn't", straight or curly
/// apostrophe) and the spaced "has not" / "did not" forms.
#[test]
fn h3_12_negated_auxiliary_before_stopped_keeps_works_at_live() {
    for text in [
        "Tomasz Nowak hasn't stopped working at Acme.",
        "Tomasz Nowak hasn\u{2019}t stopped working at Acme.",
        "Tomasz Nowak has not stopped working at Acme.",
        "Tomasz Nowak never stopped working at Acme.",
        "Tomasz Nowak did not stop working at Acme.",
        "Tomasz Nowak didn't stop working at Acme.",
    ] {
        let e = engine();
        let rid = tomasz_works_at_acme(&e);
        let out = retract_with(
            &e,
            text,
            vec![person("Tomasz Nowak"), org("Acme"), org("Brightline")],
        );
        assert!(
            !out.invalidated_relations.contains(&rid),
            "{text}: double negative ended works at: {out:?}"
        );
        assert_eq!(relation_end(&e, rid), None, "{text}");
    }
}

#[test]
fn h3_12_negated_auxiliary_before_stopped_keeps_reviews_live() {
    for text in [
        "Tomasz Nowak hasn't stopped reviewing payment-adjacent code.",
        "Tomasz Nowak hasn\u{2019}t stopped reviewing payment-adjacent code.",
        "Tomasz Nowak hasn't stopped approving payment-adjacent code.",
        "Tomasz Nowak has not stopped reviewing payment-adjacent code.",
    ] {
        let e = engine();
        let rid = tomasz_reviews_payment(&e);
        let out = retract_with(&e, text, vec![person("Tomasz Nowak")]);
        assert!(
            !out.invalidated_relations.contains(&rid),
            "{text}: double negative ended reviews: {out:?}"
        );
        assert_eq!(relation_end(&e, rid), None, "{text}");
    }
}

/// B13: the same guard on the "stopped using" cue.
#[test]
fn h3_12_hasnt_stopped_using_keeps_uses_live() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Atlas uses npm for building packages.".into(),
            entities: vec![project("Atlas"), tool("npm")],
            relations: vec![rel("Atlas", "uses", "npm", true)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    for text in [
        "Atlas hasn't stopped using npm.",
        "Atlas hasn't stopped using npm for builds.",
    ] {
        let out2 = retract_with(&e, text, vec![project("Atlas"), tool("npm")]);
        assert!(
            !out2.invalidated_relations.contains(&rid),
            "{text}: double negative ended uses: {out2:?}"
        );
        assert_eq!(relation_end(&e, rid), None, "{text}");
    }
}

// ============================================================ nc-h312r remainder

/// "X no longer works there" resolves "there" to X's single live work place.
#[test]
fn h3_12r_no_longer_works_there_ends_single_work_place() {
    let e = engine();
    let rid = tomasz_works_at_acme(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak no longer works there.",
        vec![person("Tomasz Nowak")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "works there did not end: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// Adverb between verb and "there": "works remotely there".
#[test]
fn h3_12r_no_longer_works_remotely_there_ends_single_work_place() {
    let e = engine();
    let rid = tomasz_works_at_acme(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak no longer works remotely there.",
        vec![person("Tomasz Nowak")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "works remotely there did not end: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// "on Fridays" is a scope clause: the general fact stays live.
#[test]
fn h3_12r_works_there_on_fridays_scope_ends_nothing() {
    let e = engine();
    let rid = tomasz_works_at_acme(&e);
    let out = retract_with(
        &e,
        "Tomasz Nowak no longer works there on Fridays.",
        vec![person("Tomasz Nowak")],
    );
    assert!(
        !out.invalidated_relations.contains(&rid),
        "scope clause ended fact: {out:?}"
    );
    assert!(
        out.conflicts.is_empty(),
        "scope clause produced conflicts: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), None);
}

/// No live work place for the subject: nothing ends, no conflict.
#[test]
fn h3_12r_works_there_with_no_work_place_ends_nothing() {
    let e = engine();
    let out = retract_with(
        &e,
        "Ana Silva no longer works there.",
        vec![person("Ana Silva")],
    );
    assert!(
        out.invalidated_relations.is_empty(),
        "ended without a place: {out:?}"
    );
    assert!(
        out.conflicts.is_empty(),
        "conflicted without a place: {out:?}"
    );
}

/// Two live work places: conflict candidates, end nothing.
#[test]
fn h3_12r_works_there_two_live_work_places_is_conflict() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Tomasz Nowak works at Acme.".into(),
            valid_from: Some("2026-01-04".into()),
            entities: vec![person("Tomasz Nowak"), org("Acme")],
            relations: vec![rel("Tomasz Nowak", "works at", "Acme", false)],
            ..Default::default()
        })
        .unwrap();
    let rid1 = out1.relations[0].id;
    let out2 = e
        .remember(RememberInput {
            content: "Tomasz Nowak works at Brightline.".into(),
            valid_from: Some("2026-01-05".into()),
            entities: vec![person("Tomasz Nowak"), org("Brightline")],
            relations: vec![rel("Tomasz Nowak", "works at", "Brightline", false)],
            ..Default::default()
        })
        .unwrap();
    let rid2 = out2.relations[0].id;
    let out = retract_with(
        &e,
        "Tomasz Nowak no longer works there.",
        vec![person("Tomasz Nowak")],
    );
    assert!(
        !out.invalidated_relations.contains(&rid1),
        "ended rid1 with two places: {out:?}"
    );
    assert!(
        !out.invalidated_relations.contains(&rid2),
        "ended rid2 with two places: {out:?}"
    );
    assert!(
        !out.conflicts.is_empty(),
        "two places did not conflict: {out:?}"
    );
    assert_eq!(relation_end(&e, rid1), None);
    assert_eq!(relation_end(&e, rid2), None);
}

/// "verb + object noun + preposition + target": "supplies parts to Globex".
#[test]
fn h3_12r_supplies_parts_to_ends_fact() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Acme supplies parts to Globex.".into(),
            valid_from: Some("2026-01-04".into()),
            entities: vec![org("Acme"), org("Globex")],
            relations: vec![rel("Acme", "supplies parts to", "Globex", false)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let canon: String = e
        .db
        .sql()
        .query_row(
            "SELECT p.canonical FROM relations r JOIN predicates p ON p.id=r.predicate_id WHERE r.id=?1",
            [rid],
            |r| r.get(0),
        )
        .unwrap();
    let out = retract_with(
        &e,
        "Acme no longer supplies parts to Globex.",
        vec![org("Acme"), org("Globex")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "supplies parts to not ended (canon={canon}): {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}

/// Retraction verb "supplies parts to" also ends a stored "supplies" fact.
#[test]
fn h3_12r_supplies_parts_to_ends_simple_supplies() {
    let e = engine();
    let out1 = e
        .remember(RememberInput {
            content: "Acme supplies Globex.".into(),
            valid_from: Some("2026-01-04".into()),
            entities: vec![org("Acme"), org("Globex")],
            relations: vec![rel("Acme", "supplies", "Globex", false)],
            ..Default::default()
        })
        .unwrap();
    let rid = out1.relations[0].id;
    let out = retract_with(
        &e,
        "Acme no longer supplies parts to Globex.",
        vec![org("Acme"), org("Globex")],
    );
    assert!(
        out.invalidated_relations.contains(&rid),
        "supplies parts to did not end supplies: {out:?}"
    );
    assert_eq!(relation_end(&e, rid), Some(NOW));
}
