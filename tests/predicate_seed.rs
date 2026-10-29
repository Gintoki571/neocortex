//! Predicate seeding on open (Fix-up 2): a v4 database written by an older
//! build must gain the seeds and aliases added since. Seeding is idempotent,
//! never takes over a user-created predicate or alias, and a reopen with a
//! complete ontology writes nothing (Fix-up 3: and takes no write transaction,
//! so a busy or read-only database still opens).
use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, EntityIn, RecallInput, RelationIn, RememberInput};
use std::sync::Arc;
use std::time::Duration;

mod common;

use common::TempDir;

fn remember(
    engine: &Engine,
    content: &str,
    entities: &[(&str, &str)],
    relations: Vec<RelationIn>,
) -> i64 {
    engine
        .remember(RememberInput {
            content: content.into(),
            entities: entities
                .iter()
                .map(|(name, kind)| EntityIn {
                    name: (*name).into(),
                    kind: Some((*kind).into()),
                    ..Default::default()
                })
                .collect(),
            relations,
            ..Default::default()
        })
        .unwrap()
        .id
}

fn relation(source: &str, predicate: &str, target: &str) -> RelationIn {
    RelationIn {
        source: source.into(),
        predicate: predicate.into(),
        target: target.into(),
        ..Default::default()
    }
}

fn count(engine: &Engine, sql: &str) -> i64 {
    engine.db.sql().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn predicate_id(engine: &Engine, canonical: &str) -> i64 {
    engine
        .db
        .sql()
        .query_row(
            "SELECT id FROM predicates WHERE canonical=?1",
            [canonical],
            |r| r.get(0),
        )
        .unwrap()
}

fn alias_owner(engine: &Engine, alias: &str) -> Option<i64> {
    let norm = neocortex::text::norm_predicate(alias);
    engine
        .db
        .sql()
        .query_row(
            "SELECT predicate_id FROM predicate_aliases WHERE alias_norm=?1",
            [norm],
            |r| r.get(0),
        )
        .ok()
}

/// The seed set before H2-14: the `mentor` family and the `accountable to`
/// alias of `report to` did not exist. (`is accountable to` and `reporting to`
/// fold to the same `alias_norm`s as `accountable to` and `report to`, so they
/// leave no separate row.) The causal family did exist - that is exactly what
/// the previous build checked before it re-seeded, so it never noticed the
/// missing rows.
fn rewind_to_the_old_seed_set(engine: &Engine) {
    engine
        .db
        .sql()
        .execute_batch(
            "DELETE FROM predicates WHERE canonical IN ('mentor','mentored by');
             DELETE FROM predicate_aliases WHERE alias_norm='accountable to';",
        )
        .unwrap();
}

#[test]
fn an_old_seed_set_gains_the_new_seeds_and_aliases_on_reopen() {
    let dir = TempDir::new("seed-old-set");
    let path = dir.join("seed.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &e,
            "Noah Tansen works at Acme.",
            &[("Noah Tansen", "person"), ("Acme", "org")],
            vec![relation("Noah Tansen", "works at", "Acme")],
        );
        rewind_to_the_old_seed_set(&e);
        assert_eq!(
            count(
                &e,
                "SELECT count(*) FROM predicates WHERE canonical IN ('mentor','mentored by')"
            ),
            0,
            "the database must look like the older build's"
        );
        assert_eq!(
            count(
                &e,
                "SELECT count(*) FROM predicate_aliases WHERE alias_norm='accountable to'"
            ),
            0
        );
        assert_eq!(
            count(
                &e,
                "SELECT count(*) FROM predicates WHERE canonical='cause'"
            ),
            1,
            "the old build already seeded the causal family: presence of 'cause' is not a seed-set check"
        );
    }

    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    // The mentor pair is back, seeded, and linked as one family both ways.
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM predicates WHERE canonical IN ('mentor','mentored by') AND seeded=1"
        ),
        2,
        "the missing mentor pair must be seeded on open"
    );
    let mentor = predicate_id(&e, "mentor");
    let mentored_by = predicate_id(&e, "mentored by");
    let (mentor_group, mentored_group, mentor_inverse, mentored_inverse): (
        i64,
        i64,
        Option<i64>,
        Option<i64>,
    ) =
        e.db.sql()
            .query_row(
                "SELECT (SELECT group_id FROM predicates WHERE id=?1),
                    (SELECT group_id FROM predicates WHERE id=?2),
                    (SELECT inverse_of FROM predicates WHERE id=?1),
                    (SELECT inverse_of FROM predicates WHERE id=?2)",
                [mentor, mentored_by],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
    assert_eq!(mentor_group, mentored_group, "one mentor family");
    assert_eq!(mentor_group, mentor, "the canonical direction anchors it");
    assert_eq!(mentor_inverse, Some(mentored_by));
    assert_eq!(mentored_inverse, Some(mentor));

    // The aliases added with H2-14 reach `report to`. A question normalises the
    // surface, so `accountable to` and `is accountable to` both key on the same
    // `alias_norm`, and `reporting to` folds onto the canonical `report to`.
    let report_to = predicate_id(&e, "report to");
    for alias in ["accountable to", "is accountable to", "reporting to"] {
        assert_eq!(
            alias_owner(&e, alias),
            Some(report_to),
            "'{alias}' must resolve to the seeded 'report to'"
        );
    }

    // A write through the restored alias lands in the seeded family (stored in
    // the canonical `manage` direction, so the fact is Noah -> Mira), and the
    // ask renders it back from Mira's side.
    remember(
        &e,
        "Mira Shah is accountable to Noah Tansen.",
        &[("Noah Tansen", "person"), ("Mira Shah", "person")],
        vec![relation("Mira Shah", "accountable to", "Noah Tansen")],
    );
    let (src, canonical, dst, group): (String, String, String, i64) =
        e.db.sql()
            .query_row(
                "SELECT s.norm, p.canonical, d.norm, COALESCE(NULLIF(p.group_id,0),p.id)
             FROM relations r JOIN predicates p ON p.id=r.predicate_id
             JOIN entities s ON s.id=r.src JOIN entities d ON d.id=r.dst
             ORDER BY r.id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
    assert_eq!(
        (src.as_str(), canonical.as_str(), dst.as_str()),
        ("noah tansen", "manage", "mira shah"),
        "the alias writes the canonical manage direction"
    );
    assert_eq!(
        group,
        predicate_id(&e, "manage"),
        "the write joins the manage family"
    );

    let out = e
        .recall(&RecallInput {
            query: "Who is Mira Shah accountable to?".into(),
            ..Default::default()
        })
        .unwrap();
    let fact = out
        .facts
        .iter()
        .find(|f| !f.memory && f.source == "Mira Shah" && f.target == "Noah Tansen")
        .unwrap_or_else(|| {
            panic!(
                "the accountability must render from Mira's side: {:?}",
                out.facts
            )
        });
    assert!(
        fact.predicate.contains("report") || fact.predicate.contains("accountable"),
        "unexpected predicate {:?}",
        fact.predicate
    );
    assert!(
        !out.facts
            .iter()
            .any(|f| !f.memory && f.source == "Noah Tansen" && f.target == "Mira Shah"),
        "Noah does not report to Mira: {:?}",
        out.facts
    );
}

/// The user wrote `accountable to` before the seed knew the surface, so the
/// alias is theirs. A later seed must not move it to `report to`: the user row
/// wins and only the surface nobody uses arrives.
#[test]
fn a_user_alias_is_not_hijacked_by_a_new_seed() {
    let dir = TempDir::new("seed-user-alias");
    let path = dir.join("seed.db");
    type Row = (String, String, i64, i64, i64, Option<i64>);
    let select = |e: &Engine| -> Row {
        e.db.sql()
            .query_row(
                "SELECT canonical,display,seeded,exclusive,group_id,inverse_of FROM predicates
             WHERE canonical='accountable to'",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .unwrap()
    };
    let before;
    let user_id;
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        rewind_to_the_old_seed_set(&e);
        remember(
            &e,
            "Mira Shah is accountable to Noah Tansen.",
            &[("Noah Tansen", "person"), ("Mira Shah", "person")],
            vec![relation("Mira Shah", "accountable to", "Noah Tansen")],
        );
        user_id = alias_owner(&e, "accountable to").expect("the user write owns its alias");
        before = select(&e);
        assert_eq!(before.0, "accountable to");
        assert_eq!(before.2, 0, "a user write is not a seeded row");
        assert_eq!(
            predicate_id(&e, "accountable to"),
            user_id,
            "the user predicate owns the surface"
        );
    }

    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(
        alias_owner(&e, "accountable to"),
        Some(user_id),
        "an existing alias keeps its owner"
    );
    assert_eq!(
        select(&e),
        before,
        "the seed left the user row exactly as it was"
    );
    assert_eq!(
        alias_owner(&e, "reporting to"),
        Some(predicate_id(&e, "report to")),
        "a seed surface still reaches its own row"
    );
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM predicates WHERE canonical='report to' AND seeded=1"
        ),
        1,
        "the seed row arrived even though one of its surfaces was taken"
    );
}

/// A user predicate can own a seed's canonical, not just an alias. The seed
/// must not claim that row (its display, exclusivity and `seeded` marker stay
/// the user's), and the sibling seed still joins the family the user's row
/// anchors.
#[test]
fn a_user_predicate_owning_a_seed_canonical_keeps_its_own_row() {
    let dir = TempDir::new("seed-user-canonical");
    let path = dir.join("seed.db");
    let user_id;
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        rewind_to_the_old_seed_set(&e);
        remember(
            &e,
            "Marta Lima mentors Rui Costa.",
            &[("Marta Lima", "person"), ("Rui Costa", "person")],
            vec![relation("Marta Lima", "mentors", "Rui Costa")],
        );
        user_id = predicate_id(&e, "mentor");
        let (seeded, display, exclusive): (i64, String, i64) =
            e.db.sql()
                .query_row(
                    "SELECT seeded,display,exclusive FROM predicates WHERE id=?1",
                    [user_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
        assert_eq!((seeded, exclusive), (0, 0), "before the reopen");
        assert_eq!(display, "mentors");
    }

    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    let (seeded, display, exclusive): (i64, String, i64) =
        e.db.sql()
            .query_row(
                "SELECT seeded,display,exclusive FROM predicates WHERE id=?1",
                [user_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(
        (seeded, exclusive),
        (0, 0),
        "the user row is not adopted into the seed set"
    );
    assert_eq!(display, "mentors", "and keeps its own display");
    // The sibling canonical was free, so it arrives seeded and joins the family
    // the user's row anchors.
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM predicates WHERE canonical='mentored by' AND seeded=1"
        ),
        1
    );
    let (group, inverse): (i64, Option<i64>) =
        e.db.sql()
            .query_row(
                "SELECT group_id,inverse_of FROM predicates WHERE canonical='mentored by'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(group, user_id);
    assert_eq!(inverse, Some(user_id));
}

/// A seeded row that lost its family links is repaired on reopen. Exclusivity
/// is policy, but a v4 open only re-asserts it for the `mentored by`
/// correction: no other seeded row's flag is rewritten.
#[test]
fn a_seeded_row_with_stale_links_is_repaired_on_reopen() {
    let dir = TempDir::new("seed-stale-links");
    let path = dir.join("seed.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        e.db.sql()
            .execute_batch(
                "UPDATE predicates SET group_id=0, inverse_of=NULL WHERE canonical='report to';
                 UPDATE predicates SET exclusive=0 WHERE canonical='married to';
                 UPDATE predicates SET exclusive=1 WHERE canonical='mentored by';",
            )
            .unwrap();
    }

    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    let manage = predicate_id(&e, "manage");
    let (group, inverse): (i64, Option<i64>) =
        e.db.sql()
            .query_row(
                "SELECT group_id,inverse_of FROM predicates WHERE canonical='report to'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(group, manage, "the family link is repaired");
    assert_eq!(inverse, Some(manage), "the inverse link is repaired");

    let exclusive = |canonical: &str| -> i64 {
        e.db.sql()
            .query_row(
                "SELECT exclusive FROM predicates WHERE canonical=?1",
                [canonical],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(
        exclusive("mentored by"),
        0,
        "the one seeded exclusivity a v4 open corrects"
    );
    assert_eq!(
        exclusive("married to"),
        0,
        "every other seeded flag is left as it is"
    );
}

/// The first reopen restores the missing seeds; the second must find everything
/// present and write no row at all (`total_changes` counts inserts, updates
/// and deletes on the connection).
#[test]
fn a_second_reopen_writes_nothing() {
    let dir = TempDir::new("seed-no-writes");
    let path = dir.join("seed.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &e,
            "Noah Tansen manages Mira Shah.",
            &[("Noah Tansen", "person"), ("Mira Shah", "person")],
            vec![relation("Noah Tansen", "manages", "Mira Shah")],
        );
        rewind_to_the_old_seed_set(&e);
    }
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        assert!(
            e.db.sql().total_changes() > 0,
            "the first reopen restores the missing seeds"
        );
    }
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(
        e.db.sql().total_changes(),
        0,
        "a reopen with a complete ontology must not write a row"
    );
    assert_eq!(
        count(&e, "SELECT count(*) FROM predicates WHERE seeded=1"),
        23,
        "the seed set is neither doubled nor pruned"
    );
    assert_eq!(
        count(&e, "SELECT count(*) FROM predicate_aliases"),
        count(
            &e,
            "SELECT count(DISTINCT alias_norm) FROM predicate_aliases"
        ),
        "no duplicate alias rows"
    );
}

/// A complete ontology must not take the write lock on open: another
/// connection holding `BEGIN IMMEDIATE` must not be able to fail the open (it
/// did before: `BEGIN IMMEDIATE` waited out the timeout and errored), and the
/// open must not move the change counter. Wall time is not the signal here -
/// the graphqlite extension contends for the same lock on every open.
#[test]
fn a_complete_ontology_opens_without_a_write_transaction() {
    let dir = TempDir::new("seed-no-write-lock");
    let path = dir.join("seed.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &e,
            "Noah Tansen manages Mira Shah.",
            &[("Noah Tansen", "person"), ("Mira Shah", "person")],
            vec![relation("Noah Tansen", "manages", "Mira Shah")],
        );
    }
    let holder = rusqlite::Connection::open(&path).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    holder.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        e.db.sql().total_changes(),
        0,
        "a complete ontology writes no row"
    );
    let out = e
        .recall(&RecallInput {
            query: "Who manages Mira Shah?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(!out.memories.is_empty(), "the open database still answers");
}

/// A read-only database file must open and answer: `stats`, `recall` and
/// `validate` only read, so the reconcile cannot be a hard requirement.
#[cfg(unix)]
#[test]
fn a_read_only_database_opens_and_recalls() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new("seed-read-only");
    let path = dir.join("seed.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &e,
            "Noah Tansen manages Mira Shah.",
            &[("Noah Tansen", "person"), ("Mira Shah", "person")],
            vec![relation("Noah Tansen", "manages", "Mira Shah")],
        );
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(
        count(&e, "SELECT count(*) FROM predicates WHERE seeded=1"),
        23,
        "the ontology is already complete, so nothing has to be written"
    );
    let out = e
        .recall(&RecallInput {
            query: "Who manages Mira Shah?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        out.facts
            .iter()
            .any(|f| !f.memory && f.source == "Noah Tansen" && f.target == "Mira Shah"),
        "a read-only database still answers: {:?}",
        out.facts
    );
}

/// A read-only database that needs the seeds must still open: the reconcile is
/// skipped (a read-only file accepts `BEGIN IMMEDIATE` and fails at the first
/// statement, so it is decided before the transaction) and the next writable
/// open repairs the seeds.
#[cfg(unix)]
#[test]
fn a_read_only_database_that_needs_the_seeds_opens_anyway() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new("seed-read-only-repair");
    let path = dir.join("seed.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &e,
            "Noah Tansen manages Mira Shah.",
            &[("Noah Tansen", "person"), ("Mira Shah", "person")],
            vec![relation("Noah Tansen", "manages", "Mira Shah")],
        );
        rewind_to_the_old_seed_set(&e);
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM predicates WHERE canonical IN ('mentor','mentored by')"
        ),
        0,
        "nothing can be written to a read-only file"
    );
    let out = e
        .recall(&RecallInput {
            query: "Who manages Mira Shah?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(!out.memories.is_empty(), "recall works read-only");
    drop(e);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM predicates WHERE canonical IN ('mentor','mentored by') AND seeded=1"
        ),
        2,
        "the next writable open repairs the seeds"
    );
}

/// A database that needs the seeds but is being written by another process
/// must still open: the reconcile is skipped with one stderr line and the next
/// writable open repairs the seeds.
#[test]
fn an_open_skips_the_reconcile_while_another_writer_holds_the_lock() {
    let dir = TempDir::new("seed-busy-open");
    let path = dir.join("seed.db");
    {
        let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
        remember(
            &e,
            "Mira Shah is accountable to Noah Tansen.",
            &[("Noah Tansen", "person"), ("Mira Shah", "person")],
            vec![relation("Mira Shah", "accountable to", "Noah Tansen")],
        );
        rewind_to_the_old_seed_set(&e);
    }
    let holder = rusqlite::Connection::open(&path).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM predicates WHERE canonical IN ('mentor','mentored by')"
        ),
        0,
        "the reconcile is skipped while another writer holds the lock"
    );
    // The read path records access in one write; a short timeout keeps the
    // recall from waiting for the holder (the write is best-effort anyway).
    e.db.sql().busy_timeout(Duration::from_millis(50)).unwrap();
    let out = e
        .recall(&RecallInput {
            query: "Who is Mira Shah accountable to?".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(
        !out.memories.is_empty(),
        "recall works while another writer holds the lock"
    );
    drop(e);
    holder.execute_batch("ROLLBACK").unwrap();
    drop(holder);
    let e = Engine::open(&path, Arc::new(HashEmbedder)).unwrap();
    assert_eq!(
        count(
            &e,
            "SELECT count(*) FROM predicates WHERE canonical IN ('mentor','mentored by') AND seeded=1"
        ),
        2,
        "the next writable open reconciles the seeds"
    );
}
