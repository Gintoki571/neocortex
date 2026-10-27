//! Lane nc-f4 regression tests for the 2026-10-23 resolution failures:
//! ER-07 typo/transliteration dedup with hard blockers, Q-08 typo'd query
//! entity resolution, and REL-19 typo'd exclusive targets (tests are named
//! after the items in briefs/open_items.md).

use neocortex::{
    embed::{Embedder, HashEmbedder, Mode},
    engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput, RememberOutput},
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

fn person(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("person".into()),
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

fn place(name: &str) -> EntityIn {
    EntityIn {
        name: name.into(),
        kind: Some("place".into()),
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

/// Fixed-vectors embedder for the cosine-agreement tests: texts are matched
/// by their prefix against the first key that applies, everything else falls
/// back to the hash embedder.
struct MapEmbedder {
    map: Vec<(String, Vec<f32>)>,
}

impl Embedder for MapEmbedder {
    fn embed(&self, texts: &[&str], mode: Mode) -> neocortex::Result<Vec<Vec<f32>>> {
        let fallback = HashEmbedder.embed(texts, mode)?;
        Ok(texts
            .iter()
            .zip(fallback)
            .map(|(text, fallback)| {
                self.map
                    .iter()
                    .find(|(key, _)| text.starts_with(key.as_str()))
                    .map(|(_, vector)| vector.clone())
                    .unwrap_or(fallback)
            })
            .collect())
    }

    fn name(&self) -> &str {
        "map"
    }
}

fn axis(i: usize) -> Vec<f32> {
    let mut v = vec![0.0; 768];
    v[i] = 1.0;
    v
}

fn map_engine(map: Vec<(&str, Vec<f32>)>) -> Engine {
    let map = map.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    Engine::open_in_memory(Arc::new(MapEmbedder { map }))
        .unwrap()
        .with_clock(Clock {
            now: NOW,
            tz_offset_secs: 0,
        })
}

fn sql_i64(e: &Engine, sql: &str) -> Option<i64> {
    e.db.sql().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn resolution<'a>(out: &'a RememberOutput, name: &str) -> &'a neocortex::engine::EntityResolution {
    out.entities
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no resolution for {name:?} in {:#?}", out.entities))
}

pub fn remember_named(
    e: &Engine,
    content: &str,
    entities: Vec<EntityIn>,
    relations: Vec<RelationIn>,
) -> RememberOutput {
    e.remember(RememberInput {
        content: content.into(),
        entities,
        relations,
        ..Default::default()
    })
    .unwrap()
}

// ------------------------------------------------------------------ ER-07

#[test]
fn er07_typo_becomes_the_same_entity() {
    let e = engine();
    // ER-08: the typo pair also names the same employer, the corroborating
    // evidence an automatic near-name merge needs.
    let first = remember_named(
        &e,
        "Jonathan Smith called Acme about the servers.",
        vec![person("Jonathan Smith"), org("Acme")],
        vec![],
    );
    let smith_id = resolution(&first, "Jonathan Smith").id;
    let second = remember_named(
        &e,
        "Jonathon Smith called Acme about the servers.",
        vec![person("Jonathon Smith"), org("Acme")],
        vec![],
    );
    let res = resolution(&second, "Jonathon Smith");
    assert_eq!(res.id, smith_id, "typo must resolve to the stored entity");
    assert_eq!(res.status, "near");
}

#[test]
fn er07_transliteration_becomes_the_same_entity() {
    let e = engine();
    let first = remember_named(
        &e,
        "Mueller joined the audit.",
        vec![person("Müller")],
        vec![],
    );
    let muller_id = resolution(&first, "Müller").id;
    let second = remember_named(
        &e,
        "Mueller joined the audit review.",
        vec![person("Mueller")],
        vec![],
    );
    assert_eq!(resolution(&second, "Mueller").id, muller_id);
}

#[test]
fn er07_digit_siblings_stay_separate_and_are_reported() {
    let e = engine();
    let first = remember_named(&e, "Room 101 was checked.", vec![org("Room 101")], vec![]);
    let room_id = resolution(&first, "Room 101").id;
    let second = remember_named(&e, "Room 102 was swept.", vec![org("Room 102")], vec![]);
    let res = resolution(&second, "Room 102");
    assert_ne!(res.id, room_id, "digit siblings must not merge");
    assert_eq!(res.status, "ambiguous", "blocked pair is never silent");
    let cands = &res.candidates;
    assert!(cands.iter().any(|c| c.id == room_id), "{cands:?}");
}

#[test]
fn er07_person_tokens_block_the_automatic_merge() {
    let e = engine();
    let first = remember_named(
        &e,
        "Teresa Mota signed the contract.",
        vec![person("Teresa Mota")],
        vec![],
    );
    let mota_id = resolution(&first, "Teresa Mota").id;
    let second = remember_named(
        &e,
        "Teresa Moura signed the contract too.",
        vec![person("Teresa Moura")],
        vec![],
    );
    let res = resolution(&second, "Teresa Moura");
    assert_ne!(res.id, mota_id, "Teresa Mota/Moura stay separate");
    assert_eq!(res.status, "ambiguous");
    assert!(res.candidates.iter().any(|c| c.id == mota_id));
}

// ER-07 layered dedup, near band: `name_near <= JW < name_merge` pairs and
// over-allowance merges surface as ambiguous candidates (reported, never
// silent), and subset siblings are blocked the same way.
#[test]
fn er07_near_band_and_subset_report_ambiguous_candidates() {
    let e = engine();
    // JW(Nowak, Nowata) = 0.893 - inside the near band, not blocked.
    let first = remember_named(&e, "Nowak opened the case.", vec![person("Nowak")], vec![]);
    let nowak_id = resolution(&first, "Nowak").id;
    let second = remember_named(
        &e,
        "Nowata opened the case too.",
        vec![person("Nowata")],
        vec![],
    );
    let res = resolution(&second, "Nowata");
    assert_ne!(res.id, nowak_id, "near-band pairs never auto-merge");
    assert_eq!(res.status, "ambiguous", "near pair is reported, not silent");
    assert!(res.candidates.iter().any(|c| c.id == nowak_id), "{res:?}");

    // Subset sibling: blocked before the typo gate, same reporting shape.
    let third = remember_named(
        &e,
        "Novak Group runs the case.",
        vec![org("Novak Group")],
        vec![],
    );
    let group_id = resolution(&third, "Novak Group").id;
    let fourth = remember_named(
        &e,
        "Novak Group North runs the case too.",
        vec![org("Novak Group North")],
        vec![],
    );
    let res = resolution(&fourth, "Novak Group North");
    assert_ne!(res.id, group_id, "subset siblings never auto-merge");
    assert_eq!(res.status, "ambiguous", "blocked sibling is reported");
    assert!(res.candidates.iter().any(|c| c.id == group_id), "{res:?}");
}

#[test]
fn er07_cosine_disagreement_keeps_the_pair_separate() {
    let e = map_engine(vec![
        ("Bergen", axis(0)),
        ("Bergn", axis(2)),
        // Memory texts feed the context signal; keep them orthogonal as well.
        ("Dana lives in Bergen", axis(1)),
        ("Dana moves to Bergn", axis(3)),
    ]);
    let first = remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let bergen_id = resolution(&first, "Bergen").id;
    let second = remember_named(
        &e,
        "Dana moves to Bergn.",
        vec![person("Dana"), place("Bergn")],
        vec![rel("Dana", "moves to", "Bergn", true)],
    );
    let res = resolution(&second, "Bergn");
    assert_ne!(res.id, bergen_id, "cosine-disagreeing pair stays separate");
    assert_eq!(res.status, "ambiguous");
    assert!(res.candidates.iter().any(|c| c.id == bergen_id));
}

// Model-gated reviewer repro (fix round): with the real UltraBERT embedder
// the entity-cosine gate AGREES for near-name siblings (same topic, not the
// same thing), so auto-merge must hold on the name layer alone. Covers the
// reviewer table: subset siblings and over-allowance edits are never merges;
// a typo-sized edit still is. Skip pattern of the other model-gated tests in
// tests/ (no model dir -> not run).
fn real_model_engine() -> Option<Engine> {
    let dir = match std::env::var_os("NEOCORTEX_MODEL_DIR") {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let home = std::env::var_os("HOME")?;
            std::path::Path::new(&home).join(".neocortex/models/UltraBERT-Embed-150M")
        }
    };
    if !dir.is_dir() {
        return None;
    }
    let emb = neocortex::embed::load(Some(&dir)).unwrap();
    Some(
        Engine::open_in_memory(Arc::from(emb) as Arc<dyn Embedder>)
            .unwrap()
            .with_clock(Clock {
                now: NOW,
                tz_offset_secs: 0,
            }),
    )
}

#[test]
fn er07_real_model_near_names_keep_their_names() {
    let Some(e) = real_model_engine() else {
        eprintln!("skipping: UltraBERT model directory absent");
        return;
    };
    fn note(name: String) -> String {
        format!("{name} handled the booking.")
    }

    // Rows 1-4: near-name siblings must NOT auto-merge on topic agreement.
    for (first, second, kind) in [
        ("Bergen Airport", "Bergen Airport South", "place"),
        ("Novak Group", "Novak Group North", "org"),
        ("Jan Novak", "Jan Nováček", "person"),
        ("Anna Tomczak", "Anna Tomczuka", "person"),
    ] {
        let kind = Some(kind.into());
        let entity = EntityIn {
            name: first.into(),
            kind: kind.clone(),
            ..Default::default()
        };
        let out = remember_named(&e, &note(first.into()), vec![entity], vec![]);
        let first_id = out.entities[0].id;
        let entity = EntityIn {
            name: second.into(),
            kind: kind.clone(),
            ..Default::default()
        };
        let out = remember_named(&e, &note(second.into()), vec![entity], vec![]);
        let res = resolution(&out, second);
        assert!(
            res.id != first_id,
            "{first:?}/{second:?} must not auto-merge: {res:?}"
        );
        let status = res.status.as_str();
        assert!(
            status == "new" || status == "ambiguous",
            "{first:?}/{second:?} must be new or ambiguous, got {status:?}"
        );
    }

    // Row 5: a typo-sized edit still merges when the writes corroborate it
    // (both name the same employer; ER-08).
    let out = remember_named(
        &e,
        "Jonathan Smith called Acme about the servers.",
        vec![person("Jonathan Smith"), org("Acme")],
        vec![],
    );
    let smith_id = out.entities[0].id;
    let out = remember_named(
        &e,
        "Jonathon Smith called Acme about the servers again.",
        vec![person("Jonathon Smith"), org("Acme")],
        vec![],
    );
    let res = resolution(&out, "Jonathon Smith");
    assert_eq!(res.id, smith_id, "one edit must still merge: {res:?}");
    assert_eq!(res.status, "near");
}

// H2-12 investigation: the write path has no email/company field, so the only
// non-name identity evidence is the entity description (folded into the card
// vector) and the memory context. When descriptions are supplied the existing
// `entity_conflict` veto keeps a near surname pair apart (measured card 0.639,
// context 0.597, both below 0.73); with no descriptions the card vector is the
// bare name again and the pair merges on the name layer alone - the documented
// limitation, not pinned here.
#[test]
fn h2_12_supplied_descriptions_keep_a_near_surname_pair_apart() {
    let Some(e) = real_model_engine() else {
        eprintln!("skipping: UltraBERT model directory absent");
        return;
    };
    let described = |name: &str, description: &str| EntityIn {
        name: name.into(),
        kind: Some("person".into()),
        description: Some(description.into()),
        ..Default::default()
    };
    let first = remember_named(
        &e,
        "Jonas Webber interviewed with us on 2026-09-30 but withdrew.",
        vec![described(
            "Jonas Webber",
            "interviewed with us on 2026-09-30 but withdrew",
        )],
        vec![],
    );
    let webber_id = first.entities[0].id;
    let second = remember_named(
        &e,
        "Jonas Weber joined as frontend engineer on 2026-10-05.",
        vec![described(
            "Jonas Weber",
            "joined as frontend engineer on 2026-10-05",
        )],
        vec![],
    );
    let res = resolution(&second, "Jonas Weber");
    assert_ne!(
        res.id, webber_id,
        "different people must not merge: {res:?}"
    );
    assert!(
        res.status == "new" || res.status == "ambiguous",
        "expected new or ambiguous, got {:?}",
        res.status
    );
}

// ER-08: a typo-sized near-name merge needs corroborating evidence. Two
// different people one Damerau-Levenshtein edit apart, each with their own
// residence, must stay two entities with both facts live; the second mention
// is reported as ambiguous with the candidate instead of silently merging.
#[test]
fn er08_different_people_one_edit_apart_stay_separate() {
    let e = engine();
    let first = remember_named(
        &e,
        "Anouk Kridhar lives in Pune.",
        vec![person("Anouk Kridhar"), place("Pune")],
        vec![rel("Anouk Kridhar", "lives in", "Pune", true)],
    );
    let kridhar_id = resolution(&first, "Anouk Kridhar").id;
    let second = remember_named(
        &e,
        "Anouk Tridhar lives in Hyderabad.",
        vec![person("Anouk Tridhar"), place("Hyderabad")],
        vec![rel("Anouk Tridhar", "lives in", "Hyderabad", true)],
    );
    let res = resolution(&second, "Anouk Tridhar");
    assert_ne!(
        res.id, kridhar_id,
        "different people must not merge: {res:?}"
    );
    assert_eq!(res.status, "ambiguous", "the candidate is reported");
    assert!(res.candidates.iter().any(|c| c.id == kridhar_id), "{res:?}");
    assert!(
        second.invalidated_relations.is_empty(),
        "person B's write must not end person A's fact: {:?}",
        second.invalidated_relations
    );

    // Alternating writes: each person keeps their own entity and fact.
    let third = remember_named(
        &e,
        "Anouk Kridhar still lives in Pune.",
        vec![person("Anouk Kridhar"), place("Pune")],
        vec![rel("Anouk Kridhar", "lives in", "Pune", true)],
    );
    assert_eq!(resolution(&third, "Anouk Kridhar").id, kridhar_id);
    let fourth = remember_named(
        &e,
        "Anouk Tridhar still lives in Hyderabad.",
        vec![person("Anouk Tridhar"), place("Hyderabad")],
        vec![rel("Anouk Tridhar", "lives in", "Hyderabad", true)],
    );
    assert_eq!(resolution(&fourth, "Anouk Tridhar").id, res.id);
    assert!(third.invalidated_relations.is_empty());
    assert!(fourth.invalidated_relations.is_empty());

    for rid in [first.relations[0].id, second.relations[0].id] {
        let live = sql_i64(
            &e,
            &format!("SELECT EXISTS(SELECT 1 FROM relations WHERE id={rid} AND valid_to IS NULL AND expired_at IS NULL)"),
        );
        assert_eq!(live, Some(1), "relation {rid} must stay live");
    }
}

// A shared place is not identity evidence: two people with alike surnames in
// the same city are still two people.
#[test]
fn er08_shared_city_alone_does_not_merge() {
    let e = engine();
    let first = remember_named(
        &e,
        "Anouk Kridhar lives in Pune.",
        vec![person("Anouk Kridhar"), place("Pune")],
        vec![rel("Anouk Kridhar", "lives in", "Pune", true)],
    );
    let kridhar_id = resolution(&first, "Anouk Kridhar").id;
    let second = remember_named(
        &e,
        "Anouk Tridhar lives in Pune too.",
        vec![person("Anouk Tridhar"), place("Pune")],
        vec![rel("Anouk Tridhar", "lives in", "Pune", true)],
    );
    let res = resolution(&second, "Anouk Tridhar");
    assert_ne!(res.id, kridhar_id, "a shared city is no identity: {res:?}");
    assert_eq!(res.status, "ambiguous");
    assert!(
        second.invalidated_relations.is_empty(),
        "{:?}",
        second.invalidated_relations
    );
}

// A typo-sized near-name merge stays automatic when the write shares a
// non-place neighbour (the employer) with the candidate's stored memories.
#[test]
fn er08_corroborated_typo_still_merges() {
    let e = engine();
    let first = remember_named(
        &e,
        "Jonathan Smith called Acme about the servers.",
        vec![person("Jonathan Smith"), org("Acme")],
        vec![],
    );
    let smith_id = resolution(&first, "Jonathan Smith").id;
    let second = remember_named(
        &e,
        "Jonathon Smith called Acme about the servers again.",
        vec![person("Jonathon Smith"), org("Acme")],
        vec![],
    );
    let res = resolution(&second, "Jonathon Smith");
    assert_eq!(res.id, smith_id, "shared employer corroborates: {res:?}");
    assert_eq!(res.status, "near");
}

// ------------------------------------------------------------------- Q-08

#[test]
fn q08_typoed_query_resolves_the_entity() {
    let e = engine();
    remember_named(
        &e,
        "Moritz Kanold works at Nordwind.",
        vec![person("Moritz Kanold"), org("Nordwind")],
        vec![rel("Moritz Kanold", "works at", "Nordwind", true)],
    );
    let out = e
        .recall(&RecallInput {
            query: "who is Moritz Kanoldt".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.entities.iter().any(|x| x.name == "Moritz Kanold"),
        "typo'd query must seed the entity: {:?}",
        out.entities
    );
}

#[test]
fn q08_fuzzy_lookup_resolves_bergen_from_bergn() {
    let e = engine();
    remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let out = e.entity("Bergn", 10).unwrap();
    assert_eq!(out.card.name, "Bergen");
}

// ------------------------------------------------------------------ REL-19

#[test]
fn rel19_typo_target_resolves_to_the_same_value() {
    let e = engine();
    let first = remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let rid = first.relations[0].id;
    let bergen_id = resolution(&first, "Bergen").id;
    let second = remember_named(
        &e,
        "Dana lives in Bergn.",
        vec![person("Dana"), place("Bergn")],
        vec![rel("Dana", "lives in", "Bergn", true)],
    );
    assert!(
        second.invalidated_relations.is_empty(),
        "typo'd target must not end the correct fact: {:?}",
        second.invalidated_relations
    );
    assert_eq!(resolution(&second, "Bergn").id, bergen_id);
    let live = sql_i64(
        &e,
        &format!("SELECT EXISTS(SELECT 1 FROM relations WHERE id={rid} AND valid_to IS NULL AND expired_at IS NULL)"),
    );
    assert_eq!(live, Some(1), "Bergen fact must stay live");
}

#[test]
fn rel19_blocked_near_target_ends_nothing_and_reports() {
    let e = engine();
    let first = remember_named(
        &e,
        "Dana reports to Teresa Mota.",
        vec![person("Dana"), person("Teresa Mota")],
        vec![rel("Dana", "reports to", "Teresa Mota", true)],
    );
    let rid = first.relations[0].id;
    let second = remember_named(
        &e,
        "Dana reports to Teresa Moura.",
        vec![person("Dana"), person("Teresa Moura")],
        vec![rel("Dana", "reports to", "Teresa Moura", true)],
    );
    assert!(second.invalidated_relations.is_empty());
    assert!(
        second.conflicts.iter().any(|c| c.relation_id == rid),
        "blocked near target must report a conflict candidate: {:?}",
        second.conflicts
    );
    let live = sql_i64(
        &e,
        &format!("SELECT EXISTS(SELECT 1 FROM relations WHERE id={rid} AND valid_to IS NULL AND expired_at IS NULL)"),
    );
    assert_eq!(live, Some(1));
}

#[test]
fn rel19_true_move_to_another_city_still_ends_the_fact() {
    let e = engine();
    let first = remember_named(
        &e,
        "Dana lives in Bergen.",
        vec![person("Dana"), place("Bergen")],
        vec![rel("Dana", "lives in", "Bergen", true)],
    );
    let rid = first.relations[0].id;
    let second = remember_named(
        &e,
        "Dana lives in Oslo.",
        vec![person("Dana"), place("Oslo")],
        vec![rel("Dana", "lives in", "Oslo", true)],
    );
    assert!(
        second.invalidated_relations.contains(&rid),
        "a genuinely different city must end the old fact: {:?}",
        second.invalidated_relations
    );
    assert!(second.conflicts.is_empty(), "{:?}", second.conflicts);
}
