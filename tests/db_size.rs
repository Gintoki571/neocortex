//! Fresh-database storage behaviour: sqlite-vec pre-allocates a whole chunk per
//! vector table on the first insert, so a new database must not reserve a
//! 1024-vector chunk (3 MiB x 4 tables) for a handful of memories.

use neocortex::db::Db;

fn unit(i: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; 768];
    v[i % 768] = 1.0;
    v
}

#[test]
fn fresh_vector_tables_use_small_chunks() {
    let db = Db::open_in_memory().unwrap();
    for id in 1..=2i64 {
        db.sql()
            .execute(
                "INSERT INTO memories(id,content,kind,created_at,valid_from,content_hash)
                 VALUES (?1,'x','fact',0,0,?2)",
                rusqlite::params![id, format!("h{id}")],
            )
            .unwrap();
        db.sql()
            .execute(
                &format!(
                    "INSERT INTO memories_vec(rowid,embedding,live) VALUES (?1,{},1)",
                    db.vec_sql("?2")
                ),
                rusqlite::params![id, db.vec_blob(&unit(id as usize))],
            )
            .unwrap();
    }
    let bytes: i64 = db
        .sql()
        .query_row(
            "SELECT COALESCE(SUM(pgsize),0) FROM dbstat WHERE name='memories_vec_vector_chunks00'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        bytes > 0 && bytes < 512 * 1024,
        "first memories_vec chunk is {bytes} bytes; expected the smaller chunk_size"
    );
    // Every vec table (including ones not written yet) carries the same option.
    let declared: i64 = db
        .sql()
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE name IN ('memories_vec','entities_vec','predicates_vec','relations_vec')
               AND sql LIKE '%chunk_size=%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(declared, 4);
}

#[test]
fn knn_still_works_with_small_chunks() {
    let db = Db::open_in_memory().unwrap();
    for id in 1..=100i64 {
        db.sql()
            .execute(
                "INSERT INTO memories(id,content,kind,created_at,valid_from,content_hash)
                 VALUES (?1,'x','fact',0,0,?2)",
                rusqlite::params![id, format!("h{id}")],
            )
            .unwrap();
        db.sql()
            .execute(
                &format!(
                    "INSERT INTO memories_vec(rowid,embedding,live) VALUES (?1,{},1)",
                    db.vec_sql("?2")
                ),
                rusqlite::params![id, db.vec_blob(&unit(id as usize))],
            )
            .unwrap();
    }
    let mut st = db
        .sql()
        .prepare(&format!(
            "SELECT rowid FROM memories_vec WHERE embedding MATCH {} AND k=3 ORDER BY distance",
            db.vec_sql("?1")
        ))
        .unwrap();
    let rows: Vec<i64> = st
        .query_map([db.vec_blob(&unit(42))], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows.first(), Some(&42), "{rows:?}");
}
