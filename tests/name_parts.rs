//! Connector-aware proper-noun spans and sentence-initial single names
//! (EX-05), and name-containment links between entities (ideas.md B5).

use neocortex::{
    embed::HashEmbedder,
    engine::{Engine, EntityIn, RememberInput, RememberOutput},
    extract, kg,
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

fn entity(name: &str, kind: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: (!kind.is_empty()).then(|| kind.to_string()),
        ..Default::default()
    }
}

fn remember(e: &Engine, text: &str, entities: Vec<EntityIn>) -> RememberOutput {
    e.remember(RememberInput {
        content: text.into(),
        entities,
        ..Default::default()
    })
    .unwrap()
}

fn names(text: &str) -> Vec<String> {
    extract::heuristic_entities(text)
        .into_iter()
        .map(|c| c.name)
        .collect()
}

/// (entity a, entity b, weight) rows, a < b.
fn parts(e: &Engine) -> Vec<(i64, i64, f32)> {
    let mut st =
        e.db.sql()
            .prepare("SELECT a,b,weight FROM name_part ORDER BY a,b")
            .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn id_of(e: &Engine, name: &str) -> i64 {
    e.db.sql()
        .query_row(
            "SELECT id FROM entities WHERE name=?1 ORDER BY id LIMIT 1",
            [name],
            |r| r.get(0),
        )
        .unwrap()
}

// ------------------------------------------------------- connector spans (B5)

#[test]
fn lower_case_connectors_join_proper_noun_spans() {
    for (text, span) in [
        (
            "We visited the Rua do Instituto Bacteriológico yesterday.",
            "Rua do Instituto Bacteriológico",
        ),
        (
            "She studies at Universidade da Beira Interior.",
            "Universidade da Beira Interior",
        ),
        ("He works with Maria dos Santos.", "Maria dos Santos"),
        ("The Casa das Flores is closed.", "Casa das Flores"),
        ("Marina di Carrara hosts the fair.", "Marina di Carrara"),
        ("Willem de Kooning painted it.", "Willem de Kooning"),
        (
            "The Ludwig von Mises Institute replied.",
            "Ludwig von Mises Institute",
        ),
        ("The Maestro della Scala conducted.", "Maestro della Scala"),
        ("Du Pont de Nemours filed.", "Du Pont de Nemours"),
    ] {
        let got = names(text);
        assert!(
            got.iter().any(|n| n == span),
            "{span:?} missing from {got:?} in {text:?}"
        );
    }
}

#[test]
fn generic_head_nouns_do_not_split_off_their_span() {
    let got = names("We visited the Rua do Instituto Bacteriológico yesterday.");
    for bad in [
        "Rua",
        "Instituto",
        "Instituto Bacteriológico",
        "Bacteriológico",
    ] {
        assert!(!got.iter().any(|n| n == bad), "{bad:?} leaked: {got:?}");
    }
    let got = names("She studies at Universidade da Beira Interior.");
    for bad in ["Universidade", "Beira", "Interior", "Beira Interior"] {
        assert!(!got.iter().any(|n| n == bad), "{bad:?} leaked: {got:?}");
    }
}

#[test]
fn english_of_names_keep_their_span() {
    let got = names("The Bank of America tower is open.");
    assert!(
        got.iter().any(|n| n == "Bank of America"),
        "Bank of America missing: {got:?}"
    );
    assert!(!got.iter().any(|n| n == "Bank"), "{got:?}");
    let got = names("United States of America");
    assert!(
        got.iter().any(|n| n == "United States of America"),
        "{got:?}"
    );
}

#[test]
fn a_hyphen_tail_does_not_start_a_span() {
    // "cryo-ET" is one hyphenated compound; "ET of Lon" is not a name.
    let got = names(
        "Prof. Kenji Watanabe from the Kyoto Institute of Technology visited for a week to plan cryo-ET of Lon filaments.",
    );
    assert!(
        got.iter().any(|n| n == "Kyoto Institute of Technology"),
        "{got:?}"
    );
    assert!(got.iter().any(|n| n == "Lon"), "{got:?}");
    assert!(
        !got.iter().any(|n| n.to_lowercase().starts_with("et of")),
        "a hyphen tail must not start a connector span: {got:?}"
    );
}

// ------------------------------------------------------ sentence-initial (EX-05)

#[test]
fn sentence_initial_name_before_a_verb_is_kept() {
    for (text, kept) in [
        ("Paris is the capital of France.", "Paris"),
        ("Cypher is the query language.", "Cypher"),
        ("Aveiro was quiet in winter.", "Aveiro"),
        ("Noor runs the lab.", "Noor"),
        ("Ingrid said the release slipped.", "Ingrid"),
        ("Tomas joined the team in May.", "Tomas"),
        ("Mira moved to Bergen.", "Mira"),
        ("Jonas lives in Oslo.", "Jonas"),
        ("Ada works on the parser.", "Ada"),
    ] {
        let got = names(text);
        assert!(
            got.iter().any(|n| n == kept),
            "{kept:?} missing from {got:?}"
        );
    }
    let got = names("Paris is the capital of France.");
    assert!(got.iter().any(|n| n == "France"), "{got:?}");
}

#[test]
fn sentence_initial_ordinary_words_stay_out() {
    for (text, rejected) in [
        ("The meeting was long.", &["The", "Meeting"][..]),
        ("Yesterday was busy.", &["Yesterday"]),
        ("Also, the build failed.", &["Also"]),
        ("PASS is not a name.", &["PASS"]),
        ("Merged is a status word.", &["Merged"]),
        ("Reports indicated starting was full.", &["Reports"]),
        ("Today has been long.", &["Today"]),
        ("March was cold.", &["March"]),
    ] {
        let got = names(text);
        for bad in rejected {
            assert!(!got.iter().any(|n| n == bad), "{bad:?} leaked: {got:?}");
        }
    }
}

#[test]
fn sentence_initial_stored_alias_is_kept_without_a_verb() {
    let e = engine();
    remember(
        &e,
        "Cascais is on the Atlantic.",
        vec![entity("Cascais", "place")],
    );
    // "hosts" is not in the verb-cue list: only the stored alias keeps it.
    let out = remember(&e, "Cascais hosts the regatta.", Vec::new());
    let got: Vec<&str> = out.entities.iter().map(|x| x.name.as_str()).collect();
    assert!(got.contains(&"Cascais"), "entities: {got:?}");
    // An unknown sentence-initial word before a non-verb stays out.
    let out = remember(&e, "Regatta week starts on Monday.", Vec::new());
    let got: Vec<&str> = out.entities.iter().map(|x| x.name.as_str()).collect();
    assert!(!got.contains(&"Regatta"), "entities: {got:?}");
}

#[test]
fn sentence_initial_name_resolves_to_the_stored_entity() {
    let e = engine();
    remember(&e, "Paris is a city.", vec![entity("Paris", "place")]);
    let out = remember(&e, "Paris is the capital of France.", Vec::new());
    let paris = out
        .entities
        .iter()
        .find(|x| x.name == "Paris")
        .unwrap_or_else(|| panic!("no Paris in {:?}", out.entities));
    assert_eq!(paris.id, id_of(&e, "Paris"));
}

// ------------------------------------------------------- containment edges (B5)

#[test]
fn a_longer_name_links_to_the_place_it_contains() {
    let e = engine();
    remember(
        &e,
        "Cascais marina is quiet in winter.",
        vec![
            entity("Cascais marina", "marina"),
            entity("Cascais", "place"),
        ],
    );
    let rows = parts(&e);
    let marina = id_of(&e, "Cascais marina");
    let cascais = id_of(&e, "Cascais");
    let expected = (marina.min(cascais), marina.max(cascais), 0.6);
    assert_eq!(rows, vec![expected]);
}

#[test]
fn containment_links_are_written_in_either_insertion_order() {
    // The place first, the compound second.
    let e = engine();
    remember(
        &e,
        "Aveiro is a city in Portugal.",
        vec![entity("Aveiro", "place")],
    );
    remember(
        &e,
        "Hackerspace Aveiro meets on Fridays.",
        vec![entity("Hackerspace Aveiro", "org")],
    );
    let rows = parts(&e);
    let aveiro = id_of(&e, "Aveiro");
    let space = id_of(&e, "Hackerspace Aveiro");
    assert_eq!(
        rows,
        vec![(aveiro.min(space), aveiro.max(space), 0.6)],
        "place-first order"
    );

    // The compound first, the place second.
    let e = engine();
    remember(
        &e,
        "Hackerspace Aveiro meets on Fridays.",
        vec![entity("Hackerspace Aveiro", "org")],
    );
    remember(
        &e,
        "Aveiro is a city in Portugal.",
        vec![entity("Aveiro", "place")],
    );
    let rows = parts(&e);
    let aveiro = id_of(&e, "Aveiro");
    let space = id_of(&e, "Hackerspace Aveiro");
    assert_eq!(
        rows,
        vec![(aveiro.min(space), aveiro.max(space), 0.6)],
        "compound-first order"
    );
}

#[test]
fn a_person_never_contains_a_place() {
    // "Paris Hilton" (person) is not part of "Paris" (place).
    let e = engine();
    remember(
        &e,
        "Paris Hilton arrived at the hotel.",
        vec![entity("Paris Hilton", "person"), entity("Paris", "place")],
    );
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));

    let e = engine();
    remember(&e, "Paris is a city.", vec![entity("Paris", "place")]);
    remember(
        &e,
        "Paris Hilton arrived at the hotel.",
        vec![entity("Paris Hilton", "person")],
    );
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
}

#[test]
fn org_never_contains_org() {
    // Two organisations sharing words are not containers: "Novak Labs" is
    // not part of "Novak Lab Supplies" (no singular/plural match either).
    let e = engine();
    remember(
        &e,
        "Novak Labs opened in Brno.",
        vec![entity("Novak Labs", "org")],
    );
    remember(
        &e,
        "Novak Lab Supplies sells pipettes.",
        vec![entity("Novak Lab Supplies", "org")],
    );
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
}

#[test]
fn contained_side_must_be_a_typed_place() {
    // "Azul" is a common word; only a place may be the contained side.
    let e = engine();
    remember(
        &e,
        "Ana joined the Clube de Leitura Mar Azul.",
        vec![entity("Clube de Leitura Mar Azul", "org")],
    );
    remember(&e, "Azul is a nice word.", vec![entity("Azul", "org")]);
    assert!(parts(&e).is_empty(), "org inside org: {:?}", parts(&e));
    let e = engine();
    remember(
        &e,
        "Ana joined the Clube de Leitura Mar Azul.",
        vec![entity("Clube de Leitura Mar Azul", "org")],
    );
    remember(&e, "Azul is a nice word.", vec![entity("Azul", "")]);
    assert!(parts(&e).is_empty(), "untyped contained: {:?}", parts(&e));
}

#[test]
fn containment_container_must_be_typed() {
    // An untyped longer name is not evidence that it is a place or org.
    let e = engine();
    remember(&e, "Aveiro is a city.", vec![entity("Aveiro", "place")]);
    remember(
        &e,
        "Hackerspace Aveiro meets on Fridays.",
        vec![entity("Hackerspace Aveiro", "")],
    );
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
}

#[test]
fn possessive_ends_a_connector_span() {
    // "Nova's" ends the span; the head after it is not part of the name.
    let got = names("She studied at Universidade Nova's Porto campus.");
    assert!(
        got.iter().any(|n| n == "Universidade Nova"),
        "missing Universidade Nova: {got:?}"
    );
    assert!(
        !got.iter().any(|n| n.contains('\'')),
        "possessive leaked into a name: {got:?}"
    );
}

#[test]
fn a_name_typed_after_its_first_mention_still_links() {
    // The first mention is heuristic (untyped); the later typed mention
    // fills the type in place, and the containment link must follow.
    let e = engine();
    remember(
        &e,
        "Trondheim is cold in winter.",
        vec![entity("Trondheim", "place")],
    );
    remember(&e, "Ingrid rejoined the Trondheim Rowing Club.", Vec::new());
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
    remember(
        &e,
        "Ingrid rows at the Trondheim Rowing Club.",
        vec![entity("Trondheim Rowing Club", "organization")],
    );
    let rows = parts(&e);
    let town = id_of(&e, "Trondheim");
    let club = id_of(&e, "Trondheim Rowing Club");
    assert_eq!(rows, vec![(town.min(club), town.max(club), 0.6)]);
}

#[test]
fn untyped_names_do_not_link() {
    // Without a place/org type there is no evidence the shorter name is a
    // container, so two bare names stay apart.
    let e = engine();
    remember(
        &e,
        "Alice Smith called.",
        vec![entity("Alice Smith", ""), entity("Alice", "")],
    );
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
    // Two people stay apart even with types.
    let e = engine();
    remember(
        &e,
        "Maria Silva called.",
        vec![entity("Maria Silva", "person"), entity("Maria", "person")],
    );
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
}

#[test]
fn equal_and_unrelated_names_do_not_link() {
    let e = engine();
    remember(&e, "Aveiro is a city.", vec![entity("Aveiro", "place")]);
    remember(&e, "Aveiro is a city.", vec![entity("Aveiro", "place")]);
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
    remember(&e, "Lisbon is a city.", vec![entity("Lisbon", "place")]);
    assert!(parts(&e).is_empty(), "{:?}", parts(&e));
}

#[test]
fn containment_edge_reaches_the_graph_projection() {
    // The write path links a new entity before its node exists, so the edge
    // must be projected once the node is: both insertion orders.
    fn projected(e: &Engine) {
        let rows = e
            .cypher("MATCH (a)-[r:CONTAINS]->(b) RETURN a.name AS a, b.name AS b")
            .unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        let mut names = vec![
            rows[0]["a"].as_str().unwrap().to_string(),
            rows[0]["b"].as_str().unwrap().to_string(),
        ];
        names.sort();
        assert_eq!(
            names,
            vec!["Cascais".to_string(), "Cascais marina".to_string()]
        );
    }

    let e = engine();
    remember(
        &e,
        "Cascais marina is quiet in winter.",
        vec![entity("Cascais marina", "marina")],
    );
    remember(
        &e,
        "Cascais is on the Atlantic.",
        vec![entity("Cascais", "place")],
    );
    projected(&e);

    let e = engine();
    remember(
        &e,
        "Cascais is on the Atlantic.",
        vec![entity("Cascais", "place")],
    );
    remember(
        &e,
        "Cascais marina is quiet in winter.",
        vec![entity("Cascais marina", "marina")],
    );
    projected(&e);
}

#[test]
fn containment_edge_reaches_the_ppr_snapshot() {
    let e = engine();
    remember(
        &e,
        "Cascais marina is quiet in winter.",
        vec![
            entity("Cascais marina", "marina"),
            entity("Cascais", "place"),
        ],
    );
    let snap = kg::snapshot(&e.db, None, false).unwrap();
    let marina = snap.ent_node[&id_of(&e, "Cascais marina")] as usize;
    let cascais = snap.ent_node[&id_of(&e, "Cascais")] as usize;
    let out = snap.csr.offsets[marina]..snap.csr.offsets[marina + 1];
    assert!(
        snap.csr.targets[out.clone()].contains(&(cascais as u32)),
        "Cascais marina has no edge to Cascais"
    );
    let back = snap.csr.offsets[cascais]..snap.csr.offsets[cascais + 1];
    assert!(
        snap.csr.targets[back].contains(&(marina as u32)),
        "the containment edge is not undirected"
    );
}

#[test]
fn merging_moves_the_containment_link() {
    // Merging a linked entity into another keeps the link on the survivor and
    // never leaves a row pointing at a deleted entity.
    let e = engine();
    remember(
        &e,
        "Cascais marina is quiet in winter.",
        vec![
            entity("Cascais marina", "marina"),
            entity("Cascais", "place"),
        ],
    );
    remember(
        &e,
        "Lisbon is the capital.",
        vec![entity("Lisbon", "place")],
    );
    let marina = id_of(&e, "Cascais marina");
    let cascais = id_of(&e, "Cascais");
    let lisbon = id_of(&e, "Lisbon");
    e.merge_entities(&format!("e{lisbon}"), &format!("e{cascais}"), false)
        .unwrap();
    assert_eq!(
        parts(&e),
        vec![(marina.min(lisbon), marina.max(lisbon), 0.6)],
        "the link should follow the survivor"
    );
}

#[test]
fn reindex_rebuilds_the_containment_edge() {
    let e = engine();
    remember(
        &e,
        "Cascais marina is quiet in winter.",
        vec![
            entity("Cascais marina", "marina"),
            entity("Cascais", "place"),
        ],
    );
    let marina = id_of(&e, "Cascais marina");
    let cascais = id_of(&e, "Cascais");
    e.db.graph()
        .cypher("MATCH ()-[r:CONTAINS]->() DELETE r")
        .unwrap();
    kg::rebuild_all(&e.db).unwrap();
    let snap = kg::snapshot(&e.db, None, false).unwrap();
    let m = snap.ent_node[&marina] as usize;
    let c = snap.ent_node[&cascais] as usize;
    let range = snap.csr.offsets[m]..snap.csr.offsets[m + 1];
    assert!(snap.csr.targets[range].contains(&(c as u32)));
}
