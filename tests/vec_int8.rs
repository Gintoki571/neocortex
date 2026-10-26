//! int8 vector storage: KNN agreement with f32, the mode kept per database,
//! and `migrate-vectors --to int8` keeping recall for an f32 database.

use neocortex::db::{Db, VecMode};
use neocortex::embed::{Embedder, HashEmbedder};
use neocortex::engine::{Engine, RecallInput, RememberInput};
use neocortex::temporal::{parse_instant, Clock};
use std::sync::Arc;

mod common;
use common::TempDir;

fn random_unit(seed: &mut u64) -> Vec<f32> {
    let mut v: Vec<f32> = (0..768)
        .map(|_| {
            *seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (*seed >> 33) as f32 / (1u64 << 31) as f32 - 1.0
        })
        .collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

fn insert(db: &Db, id: i64, v: &[f32]) {
    db.sql()
        .execute(
            &format!(
                "INSERT INTO memories_vec(rowid,embedding,live) VALUES (?1,{},1)",
                db.vec_sql("?2")
            ),
            rusqlite::params![id, db.vec_blob(v)],
        )
        .unwrap();
}

fn knn1(db: &Db, q: &[f32]) -> Vec<i64> {
    let sql = format!(
        "SELECT rowid FROM memories_vec WHERE embedding MATCH {} AND k=1 AND live=1 ORDER BY distance",
        db.vec_sql("?1")
    );
    let mut st = db.sql().prepare(&sql).unwrap();
    st.query_map([db.vec_blob(q)], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

#[test]
fn int8_knn_top1_matches_f32_on_noisy_queries() {
    let f32_db = Db::open_in_memory_with_mode(VecMode::F32).unwrap();
    let int8_db = Db::open_in_memory_with_mode(VecMode::Int8).unwrap();
    let mut seed = 7u64;
    let docs: Vec<Vec<f32>> = (0..200).map(|_| random_unit(&mut seed)).collect();
    for (i, d) in docs.iter().enumerate() {
        insert(&f32_db, i as i64 + 1, d);
        insert(&int8_db, i as i64 + 1, d);
    }
    for (i, d) in docs.iter().enumerate() {
        let noise = random_unit(&mut seed);
        let q: Vec<f32> = d.iter().zip(&noise).map(|(a, b)| a + 0.3 * b).collect();
        let a = knn1(&f32_db, &q);
        let b = knn1(&int8_db, &q);
        assert_eq!(a, vec![i as i64 + 1], "f32 top-1 for query {i}");
        assert_eq!(b, a, "int8 top-1 disagrees with f32 for query {i}");
    }
}

#[test]
fn existing_database_keeps_its_mode() {
    let dir = TempDir::new("vec-mode");
    let int8_path = dir.join("int8.db");
    let f32_path = dir.join("f32.db");
    drop(Db::open_with_mode(&int8_path, VecMode::Int8).unwrap());
    drop(Db::open_with_mode(&f32_path, VecMode::F32).unwrap());
    // A reopen ignores the default for new files: the stored mode wins.
    assert_eq!(Db::open(&int8_path).unwrap().vec_mode(), VecMode::Int8);
    assert_eq!(Db::open(&f32_path).unwrap().vec_mode(), VecMode::F32);
}

#[test]
fn migrate_vectors_keeps_recall_for_four_queries() {
    let dir = TempDir::new("vec-migrate");
    let path = dir.join("m.db");
    let emb: Arc<dyn Embedder> = Arc::new(HashEmbedder);
    let clock = Clock {
        now: parse_instant(
            "2026-10-21T15:00:00Z",
            &Clock {
                now: 0,
                tz_offset_secs: 0,
            },
        )
        .unwrap(),
        tz_offset_secs: 0,
    };
    let open =
        |emb: &Arc<dyn Embedder>| Engine::open(&path, emb.clone()).unwrap().with_clock(clock);
    let memories = [
        "Ana works at the harbor clinic in Lisbon.",
        "Ben moved to Porto last spring.",
        "The release train leaves every second Tuesday.",
        "Carla prefers oat milk in her coffee.",
        "The database backup runs at 02:00 UTC.",
        "Dario is allergic to peanuts.",
        "The offsite meeting was moved to Coimbra.",
        "Eva bought a red bicycle in May.",
    ];
    {
        let e = Engine::open_with_mode(&path, emb.clone(), VecMode::F32)
            .unwrap()
            .with_clock(clock);
        assert_eq!(e.db.vec_mode(), VecMode::F32);
        for content in memories {
            e.remember(RememberInput {
                content: content.into(),
                ..Default::default()
            })
            .unwrap();
        }
    }
    let queries = [
        "where does Ana work",
        "where did Ben move",
        "when does the release train leave",
        "what milk does Carla take",
    ];
    let ids = |e: &Engine| -> Vec<Vec<i64>> {
        queries
            .iter()
            .map(|q| {
                e.recall(&RecallInput {
                    query: (*q).into(),
                    ..Default::default()
                })
                .unwrap()
                .memories
                .iter()
                .map(|m| m.id)
                .collect()
            })
            .collect()
    };
    let before = ids(&open(&emb));
    {
        let mut e = open(&emb);
        e.db.migrate_vectors(VecMode::Int8).unwrap();
        assert_eq!(e.db.vec_mode(), VecMode::Int8);
    }
    let reopened = open(&emb);
    assert_eq!(reopened.db.vec_mode(), VecMode::Int8, "mode is persisted");
    assert_eq!(ids(&reopened), before);
}
