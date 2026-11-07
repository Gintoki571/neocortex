//! Lane nc-h203: H2-03 negated-value questions (ended value + current for
//! contrast) and H3-08 text nickname aliases.

use neocortex::embed::HashEmbedder;
use neocortex::engine::{
    Config, Engine, EntityIn, RecallInput, RecallOutput, RelationIn, RememberInput,
};
use neocortex::paths;
use neocortex::temporal::{parse_instant, Clock};
use std::sync::Arc;

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

fn engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(clock())
}

fn remember(engine: &Engine, content: &str, setup: impl FnOnce(&mut RememberInput)) -> i64 {
    let mut input = RememberInput {
        content: content.into(),
        ..Default::default()
    };
    setup(&mut input);
    engine.remember(input).unwrap().id
}

fn person(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
        ..Default::default()
    }
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

#[test]
fn negated_value_frame_detects_brief_examples_but_not_discourse() {
    for q in [
        "what database does Lumen not use",
        "which tools did we stop using",
        "what does Ana no longer work on",
        "who is not on the team anymore",
        "what does Ana no longer work on?",
    ] {
        assert!(paths::negated_value_frame(q), "{q:?}");
    }
    for q in [
        "what are you not sure about for lumen",
        "why not use sqlite for lumen",
        "lumen uses sqlite, isn't it",
        "I don't remember what database lumen uses",
        "what database does lumen use",
        "what did we decide to use",
    ] {
        assert!(!paths::negated_value_frame(q), "{q:?}");
    }
}

#[test]
fn pn_ended_value_and_current_value_are_both_returned() {
    let e = engine();
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
    assert!(ids(&out).contains(&sqlite), "{:?}", out.memories);
    assert!(ids(&out).contains(&postgres), "{:?}", out.memories);
    assert!(
        out.facts
            .iter()
            .any(|f| f.target.contains("Postgres") && f.ended && f.valid_to.is_some()),
        "ended Postgres fact must be returned: {:?}",
        out.facts
    );
    assert!(
        out.facts
            .iter()
            .any(|f| f.target.contains("SQLite") && !f.ended),
        "current SQLite fact must stay for contrast: {:?}",
        out.facts
    );
}

#[test]
fn no_longer_ended_and_current_are_both_returned() {
    let e = engine();
    remember(&e, "Ana works on Acme.", |i| {
        i.entities = vec![person("Ana"), entity("Acme")];
        i.relations = vec![RelationIn {
            valid_from: Some("2026-01-01".into()),
            valid_to: Some("2026-06-01".into()),
            ..relation("Ana", "works on", "Acme", "Ana works on Acme")
        }];
    });
    remember(&e, "Ana works on Globex.", |i| {
        i.entities = vec![person("Ana"), entity("Globex")];
        i.relations = vec![relation("Ana", "works on", "Globex", "Ana works on Globex")];
    });
    let out = e
        .recall(&RecallInput {
            query: "what does Ana no longer work on".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts
            .iter()
            .any(|f| f.target.contains("Acme") && f.ended),
        "ended Acme fact must be returned: {:?}",
        out.facts
    );
    assert!(
        out.facts
            .iter()
            .any(|f| f.target.contains("Globex") && !f.ended),
        "current Globex fact must be returned for contrast: {:?}",
        out.facts
    );
}

#[test]
fn who_is_not_on_the_team_anymore_returns_the_ended_person() {
    let e = engine();
    remember(&e, "Ana works on Core team.", |i| {
        i.entities = vec![person("Ana"), entity("Core team")];
        i.relations = vec![RelationIn {
            valid_from: Some("2026-01-01".into()),
            valid_to: Some("2026-06-01".into()),
            ..relation("Ana", "works on", "Core team", "Ana works on Core team")
        }];
    });
    remember(&e, "Bob works on Core team.", |i| {
        i.entities = vec![person("Bob"), entity("Core team")];
        i.relations = vec![relation(
            "Bob",
            "works on",
            "Core team",
            "Bob works on Core team",
        )];
    });
    let out = e
        .recall(&RecallInput {
            query: "who is not on the Core team anymore".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts
            .iter()
            .any(|f| f.source.contains("Ana") && f.ended),
        "ended Ana membership must be returned: {:?}",
        out.facts
    );
    assert!(
        out.facts
            .iter()
            .any(|f| f.source.contains("Bob") && !f.ended),
        "current Bob membership must stay for contrast: {:?}",
        out.facts
    );
}

#[test]
fn a_negated_relation_row_is_returned_with_the_current_value() {
    let e = engine();
    remember(&e, "Lumen uses SQLite.", |i| {
        i.entities = vec![entity("Lumen")];
        i.relations = vec![relation("Lumen", "uses", "SQLite", "Lumen uses SQLite")];
    });
    remember(&e, "Lumen does not use Postgres.", |i| {
        i.entities = vec![entity("Lumen")];
        i.relations = vec![RelationIn {
            negated: Some(true),
            ..relation("Lumen", "uses", "Postgres", "Lumen does not use Postgres")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "what database does lumen not use".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts
            .iter()
            .any(|f| f.target.contains("Postgres") && f.polarity == 0),
        "negated Postgres row must be returned: {:?}",
        out.facts
    );
    assert!(
        out.facts
            .iter()
            .any(|f| f.target.contains("SQLite") && f.polarity == 1 && !f.ended),
        "current SQLite value must stay for contrast: {:?}",
        out.facts
    );
}

#[test]
fn discourse_negation_does_not_trigger_ended_injection() {
    let e = engine();
    remember(&e, "Lumen uses SQLite.", |i| {
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
    for q in [
        "what are you not sure about for lumen",
        "why not use sqlite for lumen",
        "lumen uses sqlite, isn't it",
    ] {
        let out = e
            .recall(&RecallInput {
                query: q.into(),
                ..Default::default()
            })
            .unwrap();
        assert!(
            !out.facts
                .iter()
                .any(|f| f.target.contains("Postgres") && f.ended),
            "{q:?} must not inject the ended fact: {:?}",
            out.facts
        );
    }
}

#[test]
fn text_who_goes_by_becomes_an_alias() {
    let e = engine();
    remember(
        &e,
        "Elizabeth Ward, who goes by Liz, is a product designer on the onboarding squad.",
        |i| {
            i.entities = vec![person("Elizabeth Ward"), entity("onboarding squad")];
            i.relations = vec![relation(
                "Elizabeth Ward",
                "works on",
                "onboarding squad",
                "Elizabeth Ward works on onboarding squad",
            )];
        },
    );
    let out = e.entity("Liz", 5).unwrap();
    assert_eq!(out.card.name, "Elizabeth Ward", "{out:?}");
    assert!(
        out.card
            .aliases
            .iter()
            .any(|a| neocortex::text::norm_name(a) == "liz"),
        "{:?}",
        out.card
    );
}

#[test]
fn text_parenthetical_and_known_as_and_called_become_aliases() {
    for (content, full, nick) in [
        (
            "Dmitri Volkov (Dima) is a backend engineer.",
            "Dmitri Volkov",
            "Dima",
        ),
        (
            "Robert Kessler, known as Bobby, runs the vendor desk.",
            "Robert Kessler",
            "Bobby",
        ),
        (
            "Jorge Fuentes, called Jo by the team, owns the schema registry.",
            "Jorge Fuentes",
            "Jo",
        ),
        (
            "Dmitri Volkov, who goes by Dima, is a backend engineer.",
            "Dmitri Volkov",
            "Dima",
        ),
    ] {
        let e = engine();
        remember(&e, content, |i| {
            i.entities = vec![person(full), entity("Onboarding squad")];
            i.relations = vec![relation(full, "works on", "Onboarding squad", "")];
        });
        let out = e.entity(nick, 5).unwrap();
        assert_eq!(out.card.name, full, "{content:?} -> {out:?}");
    }
}

#[test]
fn text_nickname_is_never_created_for_a_place_or_org() {
    let e = engine();
    remember(&e, "Paris, known as Paname, is the capital.", |i| {
        i.entities = vec![EntityIn {
            name: "Paris".into(),
            kind: Some("place".into()),
            ..Default::default()
        }];
    });
    let err = e.entity("Paname", 5).unwrap_err().to_string();
    assert!(err.contains("Paname"), "{err:?}");

    let e2 = engine();
    remember(&e2, "Novak Labs, known as NL, is a research lab.", |i| {
        i.entities = vec![EntityIn {
            name: "Novak Labs".into(),
            kind: Some("org".into()),
            ..Default::default()
        }];
    });
    let err = e2.entity("NL", 5).unwrap_err().to_string();
    assert!(err.contains("NL"), "{err:?}");
}

#[test]
fn text_nickname_colliding_with_another_entity_is_not_added() {
    let e = engine();
    remember(&e, "Liz Zhang is a designer.", |i| {
        i.entities = vec![person("Liz Zhang")];
    });
    let written = e
        .remember(RememberInput {
            content: "Elizabeth Ward, who goes by Liz, is a product designer.".into(),
            entities: vec![person("Elizabeth Ward")],
            ..Default::default()
        })
        .unwrap();
    let card = e.entity("Elizabeth Ward", 5).unwrap();
    assert!(
        !card
            .card
            .aliases
            .iter()
            .any(|a| neocortex::text::norm_name(a) == "liz"),
        "Liz already names Liz Zhang and must not become Elizabeth's alias: {:?}",
        card
    );
    let liz = e.entity("Liz", 5).unwrap();
    assert_eq!(liz.card.name, "Liz Zhang", "{liz:?}");
    assert!(
        written
            .hints
            .iter()
            .any(|h| h.contains("Liz") && (h.contains("Liz Zhang") || h.contains("already"))),
        "colliding nickname must be reported: {:?}",
        written.hints
    );
}

#[test]
fn text_nickname_without_a_pattern_creates_no_alias() {
    let e = engine();
    remember(&e, "Elizabeth Ward is a product designer.", |i| {
        i.entities = vec![person("Elizabeth Ward")];
    });
    let err = e.entity("Liz", 5).unwrap_err().to_string();
    assert!(err.contains("Liz"), "{err:?}");
}

#[test]
fn config_helper_stays_constructible() {
    // Guard against accidentally requiring new config keys for this lane.
    let _cfg = Config::default();
}

// ---------------------------------------------------------------------------
// Fix round 1: A12 subject scoping, A5 frame over-trigger
// ---------------------------------------------------------------------------

#[test]
fn a5_frame_does_not_fire_for_adjective_state() {
    // "not flaky" describes a property/state, not an excluded/ended value.
    assert!(!paths::negated_value_frame("which tests are not flaky"));
    // Controls that must still fire.
    assert!(paths::negated_value_frame(
        "what database does Lumen not use"
    ));
    assert!(paths::negated_value_frame("who is not on the team anymore"));
    assert!(paths::negated_value_frame(
        "what does Ana no longer work on"
    ));
}

#[test]
fn a12_different_subject_same_predicate_leaks_nothing() {
    // Mia ended Emacs; Noa never stopped using anything. Asking about Noa must
    // not return Mia's ended/current values.
    let e = engine();
    remember(&e, "Mia uses Emacs.", |i| {
        i.entities = vec![person("Mia"), entity("Emacs")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Mia", "uses", "Emacs", "Mia uses Emacs")
        }];
    });
    remember(&e, "Mia uses VS Code.", |i| {
        i.entities = vec![person("Mia"), entity("VS Code")];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Mia", "uses", "VS Code", "Mia uses VS Code")
        }];
    });
    remember(&e, "Noa uses Neovim.", |i| {
        i.entities = vec![person("Noa"), entity("Neovim")];
        i.relations = vec![relation("Noa", "uses", "Neovim", "Noa uses Neovim")];
    });
    let out = e
        .recall(&RecallInput {
            query: "what editor does Noa no longer use".into(),
            ..Default::default()
        })
        .unwrap();
    let facts = out
        .facts
        .iter()
        .map(|f| f.fact.as_str())
        .collect::<Vec<_>>();
    assert!(
        !facts
            .iter()
            .any(|f| f.contains("Mia") || f.contains("Emacs") || f.contains("VS Code")),
        "Noa query must not leak Mia's values: {facts:?}"
    );
    assert!(
        !out.facts.iter().any(|f| f.source.contains("Mia")),
        "Noa query must not leak Mia by source: {:?}",
        out.facts
    );
}

#[test]
fn a12b_two_subjects_each_question_gets_only_its_own() {
    let e = engine();
    remember(&e, "Ana uses Emacs.", |i| {
        i.entities = vec![person("Ana"), entity("Emacs")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Ana", "uses", "Emacs", "Ana uses Emacs")
        }];
    });
    remember(&e, "Ana uses Vim.", |i| {
        i.entities = vec![person("Ana"), entity("Vim")];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Ana", "uses", "Vim", "Ana uses Vim")
        }];
    });
    remember(&e, "Bo uses Nano.", |i| {
        i.entities = vec![person("Bo"), entity("Nano")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Bo", "uses", "Nano", "Bo uses Nano")
        }];
    });
    remember(&e, "Bo uses Helix.", |i| {
        i.entities = vec![person("Bo"), entity("Helix")];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Bo", "uses", "Helix", "Bo uses Helix")
        }];
    });
    let ana = e
        .recall(&RecallInput {
            query: "what editor does Ana no longer use".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        ana.facts
            .iter()
            .any(|f| f.source.contains("Ana") && f.target.contains("Emacs") && f.ended),
        "Ana's ended Emacs missing: {:?}",
        ana.facts
    );
    assert!(
        !ana.facts.iter().any(|f| f.source.contains("Bo")
            || f.fact.contains("Nano")
            || f.fact.contains("Helix")),
        "Ana query leaked Bo: {:?}",
        ana.facts
    );
    let bo = e
        .recall(&RecallInput {
            query: "what editor does Bo no longer use".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        bo.facts
            .iter()
            .any(|f| f.source.contains("Bo") && f.target.contains("Nano") && f.ended),
        "Bo's ended Nano missing: {:?}",
        bo.facts
    );
    assert!(
        !bo.facts.iter().any(|f| f.source.contains("Ana")
            || f.fact.contains("Emacs")
            || f.fact.contains("Vim")),
        "Bo query leaked Ana: {:?}",
        bo.facts
    );
}

fn ended_editor_setup(engine: &Engine, who: &str, old: &str, new: &str) {
    remember(engine, &format!("{who} uses {old}."), |i| {
        i.entities = vec![person(who), entity(old)];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation(who, "uses", old, &format!("{who} uses {old}"))
        }];
    });
    remember(engine, &format!("{who} uses {new}."), |i| {
        i.entities = vec![person(who), entity(new)];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation(who, "uses", new, &format!("{who} uses {new}"))
        }];
    });
}

#[test]
fn a12_adv1_pronoun_subject_does_not_leak_either_woman() {
    let e = engine();
    ended_editor_setup(&e, "Ana", "Emacs", "Vim");
    ended_editor_setup(&e, "Bea", "Nano", "Helix");
    let out = e
        .recall(&RecallInput {
            query: "what does she no longer use".into(),
            ..Default::default()
        })
        .unwrap();
    // "She" is ambiguous: must not confidently inject either woman's ended value.
    assert!(
        !out.facts
            .iter()
            .any(|f| f.ended && (f.source.contains("Ana") || f.fact.contains("Emacs"))),
        "pronoun query leaked Ana: {:?}",
        out.facts
    );
    assert!(
        !out.facts
            .iter()
            .any(|f| f.ended && (f.source.contains("Bea") || f.fact.contains("Nano"))),
        "pronoun query leaked Bea: {:?}",
        out.facts
    );
}

#[test]
fn a12_adv2_team_subjects_scoped() {
    let e = engine();
    remember(&e, "Platform team uses Jenkins.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Platform team".into(),
                kind: Some("org".into()),
                ..Default::default()
            },
            entity("Jenkins"),
        ];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation(
                "Platform team",
                "uses",
                "Jenkins",
                "Platform team uses Jenkins",
            )
        }];
    });
    remember(&e, "Platform team uses GitHub Actions.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Platform team".into(),
                kind: Some("org".into()),
                ..Default::default()
            },
            entity("GitHub Actions"),
        ];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation(
                "Platform team",
                "uses",
                "GitHub Actions",
                "Platform team uses GitHub Actions",
            )
        }];
    });
    remember(&e, "Data team uses Airflow.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Data team".into(),
                kind: Some("org".into()),
                ..Default::default()
            },
            entity("Airflow"),
        ];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Data team", "uses", "Airflow", "Data team uses Airflow")
        }];
    });
    remember(&e, "Data team uses Dagster.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Data team".into(),
                kind: Some("org".into()),
                ..Default::default()
            },
            entity("Dagster"),
        ];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Data team", "uses", "Dagster", "Data team uses Dagster")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "what tool does the Platform team no longer use".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts
            .iter()
            .any(|f| f.source.contains("Platform") && f.target.contains("Jenkins") && f.ended),
        "Platform's ended Jenkins missing: {:?}",
        out.facts
    );
    assert!(
        !out.facts.iter().any(|f| f.source.contains("Data")
            || f.fact.contains("Airflow")
            || f.fact.contains("Dagster")),
        "Platform query leaked Data team: {:?}",
        out.facts
    );
}

#[test]
fn a12_adv3_nicknamed_subject_scoped() {
    let e = engine();
    remember(&e, "Elizabeth Ward, who goes by Liz, uses Emacs.", |i| {
        i.entities = vec![person("Elizabeth Ward"), entity("Emacs")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation(
                "Elizabeth Ward",
                "uses",
                "Emacs",
                "Elizabeth Ward uses Emacs",
            )
        }];
    });
    remember(&e, "Elizabeth Ward uses Vim.", |i| {
        i.entities = vec![person("Elizabeth Ward"), entity("Vim")];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Elizabeth Ward", "uses", "Vim", "Elizabeth Ward uses Vim")
        }];
    });
    ended_editor_setup(&e, "Mia", "Nano", "Helix");
    let out = e
        .recall(&RecallInput {
            query: "what does Liz no longer use".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts.iter().any(
            |f| (f.source.contains("Elizabeth") || f.source.contains("Liz"))
                && f.target.contains("Emacs")
                && f.ended
        ),
        "Liz/Elizabeth's ended Emacs missing: {:?}",
        out.facts
    );
    assert!(
        !out.facts
            .iter()
            .any(|f| f.source.contains("Mia") || f.fact.contains("Nano")),
        "Liz query leaked Mia: {:?}",
        out.facts
    );
}

#[test]
fn a12_adv4_who_not_on_team_scoped_to_named_team() {
    // "who is not on the Core team anymore" names Core team (object/anchor) -
    // must return Ana's ended membership in Core, not leaks from Design team.
    let e = engine();
    remember(&e, "Ana works on Core team.", |i| {
        i.entities = vec![person("Ana"), entity("Core team")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Ana", "works on", "Core team", "Ana works on Core team")
        }];
    });
    remember(&e, "Ana works on Design team.", |i| {
        i.entities = vec![person("Ana"), entity("Design team")];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Ana", "works on", "Design team", "Ana works on Design team")
        }];
    });
    remember(&e, "Bo works on Ops team.", |i| {
        i.entities = vec![person("Bo"), entity("Ops team")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Bo", "works on", "Ops team", "Bo works on Ops team")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "who is not on the Core team anymore".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts
            .iter()
            .any(|f| f.source.contains("Ana") && f.target.contains("Core") && f.ended),
        "Ana's ended Core membership missing: {:?}",
        out.facts
    );
    assert!(
        !out.facts
            .iter()
            .any(|f| f.source.contains("Bo") || f.fact.contains("Ops team")),
        "Core query leaked Bo/Ops: {:?}",
        out.facts
    );
}

#[test]
fn a12_adv5_passive_like_object_named_no_leak() {
    // "what database is Lumen no longer using" style - Lumen named, must not
    // leak Quarry's ended database.
    let e = engine();
    remember(&e, "Lumen uses Postgres.", |i| {
        i.entities = vec![entity("Lumen"), entity("Postgres")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Lumen", "uses", "Postgres", "Lumen uses Postgres")
        }];
    });
    remember(&e, "Lumen uses SQLite.", |i| {
        i.entities = vec![entity("Lumen"), entity("SQLite")];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Lumen", "uses", "SQLite", "Lumen uses SQLite")
        }];
    });
    remember(&e, "Quarry uses MySQL.", |i| {
        i.entities = vec![entity("Quarry"), entity("MySQL")];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Quarry", "uses", "MySQL", "Quarry uses MySQL")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "what database is Lumen no longer using".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts
            .iter()
            .any(|f| f.source.contains("Lumen") && f.target.contains("Postgres") && f.ended),
        "Lumen's ended Postgres missing: {:?}",
        out.facts
    );
    assert!(
        !out.facts
            .iter()
            .any(|f| f.source.contains("Quarry") || f.fact.contains("MySQL")),
        "Lumen query leaked Quarry: {:?}",
        out.facts
    );
}

#[test]
fn a12_adv6_org_subject_no_cross_leak() {
    let e = engine();
    remember(&e, "Acme employs Dana.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Acme".into(),
                kind: Some("org".into()),
                ..Default::default()
            },
            person("Dana"),
        ];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Acme", "employs", "Dana", "Acme employs Dana")
        }];
    });
    remember(&e, "Acme employs Evan.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Acme".into(),
                kind: Some("org".into()),
                ..Default::default()
            },
            person("Evan"),
        ];
        i.relations = vec![RelationIn {
            valid_from: Some("2025-01-01".into()),
            ..relation("Acme", "employs", "Evan", "Acme employs Evan")
        }];
    });
    remember(&e, "Globex employs Faye.", |i| {
        i.entities = vec![
            EntityIn {
                name: "Globex".into(),
                kind: Some("org".into()),
                ..Default::default()
            },
            person("Faye"),
        ];
        i.relations = vec![RelationIn {
            valid_from: Some("2024-01-01".into()),
            valid_to: Some("2025-01-01".into()),
            ..relation("Globex", "employs", "Faye", "Globex employs Faye")
        }];
    });
    let out = e
        .recall(&RecallInput {
            query: "who does Acme no longer employ".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts.iter().any(|f| f.fact.contains("Dana") && f.ended),
        "Acme's ended Dana missing: {:?}",
        out.facts
    );
    assert!(
        !out.facts
            .iter()
            .any(|f| f.fact.contains("Globex") || f.fact.contains("Faye")),
        "Acme query leaked Globex/Faye: {:?}",
        out.facts
    );
}
