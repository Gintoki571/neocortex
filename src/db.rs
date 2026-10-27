//! SQLite connection with sqlite-vec + graphqlite loaded, schema applied.

use crate::embed::DIMS;
use crate::error::Result;
use rusqlite::OptionalExtension;
use std::path::Path;
use std::sync::Once;

/// v4: typed entities (norm,type,homonym), predicate groups/inverses and the
/// agent's original predicate text. Migrations are an explicit ladder below.
///
/// The version lives in `schema_meta`, not `PRAGMA user_version`: graphqlite
/// owns that pragma (it stamps 1 for its own schema and its cypher executor
/// refuses to start on any other value).
pub const SCHEMA_VERSION: i64 = 4;

/// Storage mode of the four vec0 tables. `F32` is 768 x f32 (3,072 B per
/// vector); `Int8` is 768 x i8 (768 B), see [`INT8_LEVELS`]. A database
/// records its mode in `schema_meta.vec_mode` and keeps it on every open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VecMode {
    F32,
    Int8,
}

/// Mode of a new database when `NEOCORTEX_VEC` is unset.
const NEW_DB_MODE: VecMode = VecMode::F32;

/// Int8 levels: each vector is scaled so its largest element maps to 127. Cosine
/// is scale-invariant, so sqlite-vec and the readers need no stored scale; the
/// decoder renormalises to unit length (embedders return unit vectors). A fixed
/// [-1, 1] scale wasted about 4x of the range and cost 0.004 holdout MRR.
const INT8_LEVELS: f32 = 127.0;

impl VecMode {
    pub fn name(self) -> &'static str {
        match self {
            VecMode::F32 => "f32",
            VecMode::Int8 => "int8",
        }
    }

    pub fn parse(s: &str) -> Result<VecMode> {
        match s {
            "f32" => Ok(VecMode::F32),
            "int8" => Ok(VecMode::Int8),
            _ => Err(crate::err!("unknown vector mode '{s}' (use f32 or int8)")),
        }
    }

    /// `NEOCORTEX_VEC` for a new database; unset or empty means [`NEW_DB_MODE`].
    pub fn from_env() -> Result<VecMode> {
        match std::env::var("NEOCORTEX_VEC") {
            Ok(s) if !s.is_empty() => VecMode::parse(&s),
            _ => Ok(NEW_DB_MODE),
        }
    }

    /// Blob for a vec0 column of this mode.
    pub fn encode(self, v: &[f32]) -> Vec<u8> {
        match self {
            VecMode::F32 => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            VecMode::Int8 => {
                let max = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                let k = if max > 0.0 { INT8_LEVELS / max } else { 0.0 };
                v.iter().map(|x| ((x * k).round() as i8) as u8).collect()
            }
        }
    }

    /// Inverse of [`VecMode::encode`]; None unless the blob is one DIMS-long vector.
    pub fn decode(self, blob: &[u8]) -> Option<Vec<f32>> {
        match self {
            VecMode::F32 if blob.len() == DIMS * 4 => Some(
                blob.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|x| f32::from_le_bytes(*x))
                    .collect(),
            ),
            VecMode::Int8 if blob.len() == DIMS => {
                let mut v: Vec<f32> = blob.iter().map(|b| f32::from(*b as i8)).collect();
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    v.iter_mut().for_each(|x| *x /= norm);
                }
                Some(v)
            }
            _ => None,
        }
    }

    /// SQL expression that passes a blob bound to `arg` as a vector of this
    /// mode: sqlite-vec rejects an int8 vector without its subtype.
    pub fn bind_sql(self, arg: &str) -> String {
        match self {
            VecMode::F32 => arg.to_string(),
            VecMode::Int8 => format!("vec_int8({arg})"),
        }
    }
}

/// The four vec0 tables, in rewrite order for `migrate_vectors`.
const VEC_TABLES: [&str; 4] = [
    "memories_vec",
    "entities_vec",
    "predicates_vec",
    "relations_vec",
];

/// One connection for everything (relational, FTS5, vec0, Cypher), so a
/// write is one transaction.
pub struct Db {
    g: graphqlite::Connection,
    vec: VecMode,
}

fn register_vec() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        // sqlite-vec registers itself on every connection opened after this.
        #[allow(clippy::missing_transmute_annotations)]
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

impl Db {
    /// Open or create. A new file takes its vector mode from `NEOCORTEX_VEC`.
    pub fn open(path: &Path) -> Result<Db> {
        Self::open_as(path, None)
    }

    /// Open or create; a new file gets `mode` (an existing one keeps its own).
    pub fn open_with_mode(path: &Path, mode: VecMode) -> Result<Db> {
        Self::open_as(path, Some(mode))
    }

    fn open_as(path: &Path, new_mode: Option<VecMode>) -> Result<Db> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            // Never chmod a directory this process did not create: `--db /tmp/x.db`
            // must not change /tmp, and `--db ./x.db` must not change the checkout.
            let created = !dir.exists();
            std::fs::create_dir_all(dir)?;
            #[cfg(unix)]
            if created {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
        }
        register_vec();
        let conn = rusqlite::Connection::open(path)?;
        let db = Self::init(conn, new_mode)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            // SQLite creates -wal/-shm 0644; they hold full memory text.
            for suffix in ["-wal", "-shm"] {
                let mut side = path.as_os_str().to_os_string();
                side.push(suffix);
                let side = std::path::PathBuf::from(side);
                if side.exists() {
                    let _ = std::fs::set_permissions(side, std::fs::Permissions::from_mode(0o600));
                }
            }
        }
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Db> {
        Self::open_in_memory_as(None)
    }

    pub fn open_in_memory_with_mode(mode: VecMode) -> Result<Db> {
        Self::open_in_memory_as(Some(mode))
    }

    fn open_in_memory_as(new_mode: Option<VecMode>) -> Result<Db> {
        register_vec();
        Self::init(rusqlite::Connection::open_in_memory()?, new_mode)
    }

    /// `new_mode` is the mode of a fresh database; None reads `NEOCORTEX_VEC`.
    fn init(conn: rusqlite::Connection, new_mode: Option<VecMode>) -> Result<Db> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // journal_mode returns a row, so it cannot go through execute_batch.
        // Switching a brand-new file to WAL ignores busy_timeout, so two
        // processes opening it at once need an explicit retry.
        let mut tries = 0;
        loop {
            match conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0)) {
                Err(rusqlite::Error::SqliteFailure(e, _))
                    if e.code == rusqlite::ErrorCode::DatabaseBusy && tries < 100 =>
                {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                r => {
                    r?;
                    break;
                }
            }
        }
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")?;
        let g = graphqlite::Connection::from_rusqlite(conn)?;
        let mut db = Db {
            g,
            vec: VecMode::F32,
        };
        // graphqlite stamps `user_version=1` when it creates its own schema in
        // a fresh file, so presence of our tables, not the number alone, says
        // whether this database was ever initialised.
        if !db.table_exists("memories")? {
            let mode = match new_mode {
                Some(m) => m,
                None => VecMode::from_env()?,
            };
            db.create_schema(mode)?;
            db.vec = mode;
            return Ok(db);
        }
        let v = db.schema_version()?;
        if v > SCHEMA_VERSION {
            return Err(crate::err!(
                "database schema v{v} is newer than this neocortex (v{SCHEMA_VERSION})"
            ));
        }
        // 0 = a v1 database (no schema_meta row yet); 1 = stamped v1.
        if v < 2 {
            db.migrate_v1_to_v2()?;
        }
        if v < 3 {
            db.migrate_v2_to_v3()?;
        }
        if v < 4 {
            db.migrate_v3_to_v4()?;
        }
        // Additive tables (E4a links, E5 access history) do not change the
        // relational shape the ladder guards, so they are ensured for every
        // v4 database instead of renumbering the schema. Idempotent by design.
        db.ensure_additive_tables()?;
        db.vec = db.stored_vec_mode()?;
        Ok(db)
    }

    fn stored_vec_mode(&self) -> Result<VecMode> {
        let s: String =
            self.sql()
                .query_row("SELECT vec_mode FROM schema_meta LIMIT 1", [], |r| r.get(0))?;
        VecMode::parse(&s)
    }

    fn ensure_additive_tables(&self) -> Result<()> {
        // Databases before the vector mode existed hold f32 vectors.
        if !self.column_exists("schema_meta", "vec_mode")? {
            self.sql().execute_batch(
                "ALTER TABLE schema_meta ADD COLUMN vec_mode TEXT NOT NULL DEFAULT 'f32'",
            )?;
        }
        self.sql().execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_links (
               a INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
               b INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
               weight REAL NOT NULL,
               PRIMARY KEY (a, b),
               CHECK (a < b)
             );
             CREATE INDEX IF NOT EXISTS idx_memory_links_b ON memory_links(b);
             CREATE TABLE IF NOT EXISTS memory_access (
               memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
               at INTEGER NOT NULL,
               PRIMARY KEY (memory_id, at)
             );
             CREATE INDEX IF NOT EXISTS idx_memory_access_at ON memory_access(at);
             CREATE TABLE IF NOT EXISTS name_tokens (
               token TEXT NOT NULL,
               entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
               PRIMARY KEY (token, entity_id)
             );
             CREATE INDEX IF NOT EXISTS idx_name_tokens_entity ON name_tokens(entity_id);
             CREATE TABLE IF NOT EXISTS name_part (
               a INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
               b INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
               weight REAL NOT NULL,
               PRIMARY KEY (a, b),
               CHECK (a < b)
             );
             CREATE INDEX IF NOT EXISTS idx_name_part_b ON name_part(b);
             CREATE INDEX IF NOT EXISTS idx_rel_subj ON relations(subj, predicate_id);",
        )?;
        // B5 name tokens are derived from entities.norm; a database written
        // before the table existed is backfilled once (the empty-table probe
        // is a primary-key lookup, so a normal open writes nothing).
        let empty: i64 = self.sql().query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM name_tokens LIMIT 1)",
            [],
            |r| r.get(0),
        )?;
        if empty == 1 {
            self.backfill_name_tokens()?;
        }
        // New seeded families (the causal group) must reach existing v4
        // databases too. Presence of the family anchor is the check: when it
        // exists the whole current seed set does, so a normal open writes
        // nothing.
        let anchored: i64 = self.sql().query_row(
            "SELECT EXISTS(SELECT 1 FROM predicates WHERE canonical='cause')",
            [],
            |r| r.get(0),
        )?;
        if anchored == 0 {
            self.seed_predicates()?;
        }
        Ok(())
    }

    fn table_exists(&self, name: &str) -> Result<bool> {
        let n: i64 = self.sql().query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Fill `name_tokens` for every entity that predates the table (B5).
    fn backfill_name_tokens(&self) -> Result<()> {
        let rows: Vec<(i64, String)> = {
            let mut st = self.sql().prepare("SELECT id,norm FROM entities")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        if rows.is_empty() {
            return Ok(());
        }
        let mut st = self
            .sql()
            .prepare("INSERT OR IGNORE INTO name_tokens(token,entity_id) VALUES (?1,?2)")?;
        for (id, norm) in rows {
            for token in crate::text::name_tokens(&norm) {
                st.execute(rusqlite::params![token, id])?;
            }
        }
        Ok(())
    }

    /// Fresh database: create every table, then stamp the version and mode.
    fn create_schema(&self, mode: VecMode) -> Result<()> {
        self.sql().execute_batch(include_str!("schema.sql"))?;
        self.create_vec_tables(mode)?;
        self.seed_predicates()?;
        self.set_version(SCHEMA_VERSION)?;
        self.sql()
            .execute("UPDATE schema_meta SET vec_mode=?1", [mode.name()])?;
        Ok(())
    }

    /// chunk_size=64: the 1024 default reserves 12.6 MiB on the first insert
    /// (a whole chunk per table). Chunk bytes scale with the mode, not the count.
    fn create_vec_tables(&self, mode: VecMode) -> Result<()> {
        for name in VEC_TABLES {
            self.create_vec_tables_one(name, mode)?;
        }
        Ok(())
    }

    fn create_vec_tables_one(&self, name: &str, mode: VecMode) -> Result<()> {
        let ty = match mode {
            VecMode::F32 => "float",
            VecMode::Int8 => "int8",
        };
        // memories_vec carries the `live` filter column.
        let live = if name == "memories_vec" {
            ", live integer"
        } else {
            ""
        };
        self.sql().execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {name} USING vec0(embedding {ty}[768] distance_metric=cosine{live}, chunk_size=64)"
        ))?;
        Ok(())
    }

    /// Storage mode of the vec0 tables (`schema_meta.vec_mode`).
    pub fn vec_mode(&self) -> VecMode {
        self.vec
    }

    /// Blob to bind for a vec0 column (write or KNN query), in this database's mode.
    pub fn vec_blob(&self, v: &[f32]) -> Vec<u8> {
        self.vec.encode(v)
    }

    /// Vector read back from a vec0 column, dequantized to f32 in int8 mode.
    pub fn blob_to_vec(&self, blob: &[u8]) -> Option<Vec<f32>> {
        self.vec.decode(blob)
    }

    /// SQL argument for a vec0 vector bound to `arg` (see [`VecMode::bind_sql`]).
    pub fn vec_sql(&self, arg: &str) -> String {
        self.vec.bind_sql(arg)
    }

    /// Rewrite the vector tables into `to` in one transaction, then VACUUM.
    /// Only f32 -> int8 exists. Rows are read into memory first, since the
    /// vec0 tables are dropped and recreated in place.
    pub fn migrate_vectors(&mut self, to: VecMode) -> Result<()> {
        if to == self.vec {
            return Ok(());
        }
        if to != VecMode::Int8 {
            return Err(crate::err!("only migration to int8 is supported"));
        }
        self.write(|db| {
            for name in VEC_TABLES {
                let has_live = name == "memories_vec";
                let live_col = if has_live { "live" } else { "NULL" };
                let rows: Vec<(i64, Vec<f32>, Option<i64>)> = {
                    let mut st = db
                        .sql()
                        .prepare(&format!("SELECT rowid, embedding, {live_col} FROM {name}"))?;
                    let rows = st.query_map([], |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?, r.get(2)?))
                    })?;
                    let mut out = Vec::new();
                    for row in rows {
                        let (id, blob, live) = row?;
                        let v = db
                            .vec
                            .decode(&blob)
                            .ok_or_else(|| crate::err!("{name} row {id} is not a f32 vector"))?;
                        out.push((id, v, live));
                    }
                    out
                };
                db.sql().execute_batch(&format!("DROP TABLE {name}"))?;
                db.create_vec_tables_one(name, to)?;
                let bind = to.bind_sql("?2");
                let insert = if has_live {
                    format!("INSERT INTO {name}(rowid,embedding,live) VALUES (?1,{bind},?3)")
                } else {
                    format!("INSERT INTO {name}(rowid,embedding) VALUES (?1,{bind})")
                };
                for (id, v, live) in rows {
                    let blob = to.encode(&v);
                    if has_live {
                        db.sql()
                            .execute(&insert, rusqlite::params![id, blob, live])?;
                    } else {
                        db.sql().execute(&insert, rusqlite::params![id, blob])?;
                    }
                }
            }
            db.sql()
                .execute("UPDATE schema_meta SET vec_mode=?1", [to.name()])?;
            Ok(())
        })?;
        self.sql().execute_batch("VACUUM")?;
        self.vec = to;
        Ok(())
    }

    /// Hand-written predicate ontology. Small on purpose: a wrong seed merges
    /// real predicates, so only measured or unambiguous families are here.
    ///
    /// `group` is the canonical predicate a row belongs to (exclusivity and
    /// conflicts key on it); `inverse` names the predicate that states the same
    /// relation from the other side. Seeded exclusivity is policy and is not
    /// cleared by `exclusive: false` writes.
    fn seed_predicates(&self) -> Result<()> {
        struct Seed {
            canonical: &'static str,
            display: &'static str,
            aliases: &'static [&'static str],
            exclusive: bool,
            symmetric: bool,
            group: &'static str,
            inverse: Option<&'static str>,
        }
        const SEEDS: &[Seed] = &[
            Seed {
                canonical: "live in",
                display: "lives in",
                aliases: &["lives in", "live in", "lived in", "living in"],
                exclusive: true,
                symmetric: false,
                group: "live in",
                inverse: None,
            },
            Seed {
                canonical: "reside in",
                display: "resides in",
                aliases: &["resides in", "reside in", "resided in"],
                exclusive: true,
                symmetric: false,
                group: "live in",
                inverse: None,
            },
            Seed {
                canonical: "base in",
                display: "based in",
                aliases: &["based in", "is based in", "base in"],
                exclusive: true,
                symmetric: false,
                group: "live in",
                inverse: None,
            },
            Seed {
                canonical: "work at",
                display: "works at",
                aliases: &["works at", "work at", "worked at", "employed at"],
                exclusive: true,
                symmetric: false,
                group: "work at",
                inverse: None,
            },
            Seed {
                canonical: "work for",
                display: "works for",
                aliases: &["works for", "work for", "worked for"],
                exclusive: true,
                symmetric: false,
                group: "work at",
                inverse: None,
            },
            Seed {
                canonical: "employ by",
                display: "employed by",
                aliases: &["employed by", "is employed by", "employ by"],
                exclusive: true,
                symmetric: false,
                group: "work at",
                inverse: None,
            },
            Seed {
                canonical: "married to",
                display: "married to",
                aliases: &["married to", "is married to", "marries"],
                exclusive: true,
                symmetric: true,
                group: "married to",
                inverse: None,
            },
            Seed {
                canonical: "manage",
                display: "manages",
                aliases: &["manages", "manage", "managed", "managing"],
                exclusive: false,
                symmetric: false,
                group: "manage",
                inverse: Some("report to"),
            },
            Seed {
                canonical: "report to",
                display: "reports to",
                aliases: &["reports to", "report to", "reported to"],
                exclusive: true,
                symmetric: false,
                group: "manage",
                inverse: Some("manage"),
            },
            Seed {
                canonical: "parent of",
                display: "parent of",
                aliases: &["parent of", "is parent of"],
                exclusive: false,
                symmetric: false,
                group: "parent of",
                inverse: Some("child of"),
            },
            Seed {
                canonical: "child of",
                display: "child of",
                aliases: &["child of", "is child of"],
                exclusive: false,
                symmetric: false,
                group: "parent of",
                inverse: Some("parent of"),
            },
            Seed {
                canonical: "friend with",
                display: "friends with",
                aliases: &["friends with", "friend with", "is friends with"],
                exclusive: false,
                symmetric: true,
                group: "friend with",
                inverse: None,
            },
            Seed {
                canonical: "sibling of",
                display: "sibling of",
                aliases: &["sibling of", "is sibling of"],
                exclusive: false,
                symmetric: true,
                group: "sibling of",
                inverse: None,
            },
            Seed {
                canonical: "born in",
                display: "born in",
                aliases: &["born in", "was born in"],
                exclusive: false,
                symmetric: false,
                group: "born in",
                inverse: None,
            },
            Seed {
                canonical: "born on",
                display: "born on",
                aliases: &["born on", "was born on"],
                exclusive: false,
                symmetric: false,
                group: "born on",
                inverse: None,
            },
            Seed {
                canonical: "headquartered in",
                display: "headquartered in",
                aliases: &["headquartered in", "is headquartered in"],
                exclusive: true,
                symmetric: false,
                group: "headquartered in",
                inverse: None,
            },
            // E4b: one causal family. "caused by" and "because of" state the
            // same relation from the effect side, so they are inverses and a
            // write in either direction lands in the canonical cause direction.
            Seed {
                canonical: "cause",
                display: "causes",
                aliases: &["causes", "cause", "caused", "causing"],
                exclusive: false,
                symmetric: false,
                group: "cause",
                inverse: Some("caus by"),
            },
            Seed {
                canonical: "led to",
                display: "led to",
                aliases: &["led to", "leads to", "lead to"],
                exclusive: false,
                symmetric: false,
                group: "cause",
                inverse: None,
            },
            Seed {
                canonical: "result in",
                display: "resulted in",
                aliases: &["resulted in", "results in", "result in"],
                exclusive: false,
                symmetric: false,
                group: "cause",
                inverse: None,
            },
            Seed {
                canonical: "caus by",
                display: "caused by",
                aliases: &["caused by", "is caused by"],
                exclusive: false,
                symmetric: false,
                group: "cause",
                inverse: Some("cause"),
            },
            Seed {
                canonical: "because of",
                display: "because of",
                aliases: &["because of", "is because of", "because"],
                exclusive: false,
                symmetric: false,
                group: "cause",
                inverse: Some("cause"),
            },
        ];
        for seed in SEEDS {
            self.sql().execute(
                "INSERT INTO predicates(canonical,display,exclusive,symmetric,seeded)
                 VALUES (?1,?2,?3,?4,1)
                 ON CONFLICT(canonical) DO UPDATE SET
                   display=CASE WHEN predicates.display='' THEN excluded.display ELSE predicates.display END,
                   symmetric=MAX(predicates.symmetric,excluded.symmetric),
                   -- Migration re-applies the hand-written policy; a seeded
                   -- predicate's exclusivity is not a user-settable flag.
                   exclusive=excluded.exclusive,
                   seeded=1",
                rusqlite::params![
                    seed.canonical,
                    seed.display,
                    i64::from(seed.exclusive),
                    i64::from(seed.symmetric)
                ],
            )?;
            let id: i64 = self.sql().query_row(
                "SELECT id FROM predicates WHERE canonical=?1",
                [seed.canonical],
                |r| r.get(0),
            )?;
            for alias in seed.aliases {
                let norm = crate::text::norm_predicate(alias);
                self.sql().execute(
                    "INSERT OR IGNORE INTO predicate_aliases(alias_norm,predicate_id) VALUES (?1,?2)",
                    rusqlite::params![norm, id],
                )?;
            }
            // An alias whose canonical is different from the seed surface must
            // still reach the seed row ("employed at" -> "work at").
            let own_norm = crate::text::norm_predicate(seed.canonical);
            self.sql().execute(
                "INSERT OR IGNORE INTO predicate_aliases(alias_norm,predicate_id) VALUES (?1,?2)",
                rusqlite::params![own_norm, id],
            )?;
        }
        for seed in SEEDS {
            let id: i64 = self.sql().query_row(
                "SELECT id FROM predicates WHERE canonical=?1",
                [seed.canonical],
                |r| r.get(0),
            )?;
            let group: i64 = self.sql().query_row(
                "SELECT id FROM predicates WHERE canonical=?1",
                [seed.group],
                |r| r.get(0),
            )?;
            self.sql()
                .execute("UPDATE predicates SET group_id=?1 WHERE id=?2", [group, id])?;
            if let Some(inverse) = seed.inverse {
                let inverse_id: i64 = self.sql().query_row(
                    "SELECT id FROM predicates WHERE canonical=?1",
                    [inverse],
                    |r| r.get(0),
                )?;
                self.sql().execute(
                    "UPDATE predicates SET inverse_of=?1 WHERE id=?2",
                    [inverse_id, id],
                )?;
            }
        }
        Ok(())
    }

    /// v2 -> v3: the baseline releases added polarity and ended_by to
    /// relations. Predicate ontology (display/group_id/inverse_of/symmetric/"
    /// seeded) belongs to v3 -> v4, which handles every pre-f2 predicate
    /// shape.
    fn migrate_v2_to_v3(&self) -> Result<()> {
        self.write(|db| {
            if !db.column_exists("relations", "polarity")? {
                db.sql().execute_batch(
                    "ALTER TABLE relations ADD COLUMN polarity INTEGER NOT NULL DEFAULT 1;",
                )?;
            }
            if !db.column_exists("relations", "ended_by")? {
                db.sql()
                    .execute_batch("ALTER TABLE relations ADD COLUMN ended_by INTEGER;")?;
            }
            db.set_version(3)
        })
    }

    /// v3 -> v4: entities become unique on (norm, type, homonym) with a
    /// lower-cased type; predicates gain the group/inverse/symmetric ontology
    /// and the agent's original predicate text moves onto each relation.
    ///
    /// The entities table is rebuilt (SQLite cannot drop a column-level UNIQUE
    /// constraint). Foreign keys are off for the rebuild only: dropping the old
    /// table with them on would cascade into mentions/relations.
    fn migrate_v3_to_v4(&self) -> Result<()> {
        self.sql().execute_batch("PRAGMA foreign_keys=OFF")?;
        let result = self.write(|db| {
            if !db.column_exists("entities", "homonym")? {
                db.sql().execute_batch(
                    "CREATE TABLE entities_v4 (
                       id INTEGER PRIMARY KEY,
                       name TEXT NOT NULL,
                       norm TEXT NOT NULL,
                       type TEXT NOT NULL DEFAULT '',
                       homonym INTEGER NOT NULL DEFAULT 0,
                       description TEXT NOT NULL DEFAULT '',
                       created_at INTEGER NOT NULL,
                       updated_at INTEGER NOT NULL,
                       mention_count INTEGER NOT NULL DEFAULT 0
                     );
                     INSERT INTO entities_v4(id,name,norm,type,homonym,description,created_at,updated_at,mention_count)
                       SELECT id,name,norm,lower(type),0,description,created_at,updated_at,mention_count FROM entities;
                     DROP TABLE entities;
                     ALTER TABLE entities_v4 RENAME TO entities;
                     CREATE UNIQUE INDEX IF NOT EXISTS idx_entities_norm_type
                       ON entities(norm, type, homonym);
                     CREATE INDEX IF NOT EXISTS idx_entities_norm ON entities(norm);",
                )?;
            }
            for (table, column, ddl) in [
                (
                    "predicates",
                    "display",
                    "ALTER TABLE predicates ADD COLUMN display TEXT NOT NULL DEFAULT '';",
                ),
                (
                    "predicates",
                    "group_id",
                    "ALTER TABLE predicates ADD COLUMN group_id INTEGER NOT NULL DEFAULT 0;",
                ),
                (
                    "predicates",
                    "inverse_of",
                    "ALTER TABLE predicates ADD COLUMN inverse_of INTEGER;",
                ),
                (
                    "predicates",
                    "symmetric",
                    "ALTER TABLE predicates ADD COLUMN symmetric INTEGER NOT NULL DEFAULT 0;",
                ),
                (
                    "predicates",
                    "seeded",
                    "ALTER TABLE predicates ADD COLUMN seeded INTEGER NOT NULL DEFAULT 0;",
                ),
                (
                    "relations",
                    "predicate_text",
                    "ALTER TABLE relations ADD COLUMN predicate_text TEXT NOT NULL DEFAULT '';",
                ),
                (
                    "relations",
                    "subj",
                    "ALTER TABLE relations ADD COLUMN subj INTEGER NOT NULL DEFAULT 0;",
                ),
            ] {
                if !db.column_exists(table, column)? {
                    db.sql().execute_batch(ddl)?;
                }
            }
            db.sql()
                .execute("UPDATE relations SET subj=src WHERE subj=0", [])?;
            // v3 stored fact-sentence vectors; the phrase is the right signal.
            db.sql().execute("DELETE FROM predicates_vec", [])?;
            db.seed_predicates()?;
            db.sql().execute(
                "UPDATE relations SET predicate_text=COALESCE(
                     (SELECT NULLIF(display,'') FROM predicates p WHERE p.id=relations.predicate_id),
                     (SELECT canonical FROM predicates p WHERE p.id=relations.predicate_id),
                     '')
                 WHERE predicate_text=''",
                [],
            )?;
            db.set_version(4)
        });
        let _ = self.sql().execute_batch("PRAGMA foreign_keys=ON");
        result
    }

    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        Ok(self.sql().query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name=?2)",
            rusqlite::params![table, column],
            |r| r.get(0),
        )?)
    }

    fn schema_version(&self) -> Result<i64> {
        if !self.table_exists("schema_meta")? {
            return Ok(0);
        }
        let v: Option<i64> = self
            .sql()
            .query_row("SELECT version FROM schema_meta LIMIT 1", [], |r| r.get(0))
            .optional()?;
        Ok(v.unwrap_or(0))
    }

    fn set_version(&self, v: i64) -> Result<()> {
        self.sql()
            .execute_batch("CREATE TABLE IF NOT EXISTS schema_meta (version INTEGER NOT NULL)")?;
        // UPDATE keeps the row's vec_mode column.
        if self
            .sql()
            .execute("UPDATE schema_meta SET version=?1", [v])?
            == 0
        {
            self.sql()
                .execute("INSERT INTO schema_meta(version) VALUES(?1)", [v])?;
        }
        Ok(())
    }

    /// v1 -> v2: collapse live duplicate content hashes (keeping the oldest,
    /// retiring the rest as superseded) and add the unique live-hash index.
    /// One transaction: a crash leaves a v1 database, never a half-migrated one.
    fn migrate_v1_to_v2(&self) -> Result<()> {
        let now = crate::temporal::Clock::system().now;
        self.write(|db| {
            // Same statement predicate twice: first the vec `live` flags, then
            // the rows themselves (the MIN stays the oldest id either way).
            let dup_sql = "SELECT id FROM memories WHERE expired_at IS NULL
                 AND id <> (SELECT MIN(m2.id) FROM memories m2
                            WHERE m2.content_hash = memories.content_hash
                              AND m2.expired_at IS NULL)";
            let dups: Vec<i64> = {
                let mut st = db.sql().prepare(dup_sql)?;
                let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            for id in &dups {
                db.sql()
                    .execute("UPDATE memories_vec SET live=0 WHERE rowid=?1", [id])?;
            }
            db.sql().execute(
                "UPDATE memories SET expired_at=?1, superseded_by=(
                     SELECT MIN(m2.id) FROM memories m2
                     WHERE m2.content_hash = memories.content_hash
                       AND m2.expired_at IS NULL)
                 WHERE expired_at IS NULL
                   AND id <> (SELECT MIN(m2.id) FROM memories m2
                              WHERE m2.content_hash = memories.content_hash
                                AND m2.expired_at IS NULL)",
                [now],
            )?;
            db.sql()
                .execute_batch("DROP INDEX IF EXISTS idx_memories_hash")?;
            db.sql().execute_batch(
                "CREATE UNIQUE INDEX idx_memories_hash ON memories(content_hash)
                 WHERE expired_at IS NULL",
            )?;
            db.sql().execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_meta (version INTEGER NOT NULL)",
            )?;
            db.set_version(2)
        })
    }

    /// Plain SQL access (same connection as Cypher).
    pub fn sql(&self) -> &rusqlite::Connection {
        self.g.sqlite_connection()
    }

    /// Cypher access (graphqlite) on the same connection.
    pub fn graph(&self) -> &graphqlite::Connection {
        &self.g
    }

    /// Run `f` inside BEGIN IMMEDIATE .. COMMIT; rolls back on error (including
    /// a failed COMMIT). A call already inside a transaction joins it, so an
    /// importer can wrap many writes in one atomic unit.
    pub fn write<T>(&self, f: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
        if !self.sql().is_autocommit() {
            return f(self);
        }
        self.sql().execute_batch("BEGIN IMMEDIATE")?;
        match f(self) {
            Ok(v) => match self.sql().execute_batch("COMMIT") {
                Ok(()) => Ok(v),
                Err(e) => {
                    let _ = self.sql().execute_batch("ROLLBACK");
                    Err(e.into())
                }
            },
            Err(e) => {
                let _ = self.sql().execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}

/// True for a UNIQUE constraint violation (not any other constraint).
pub fn is_unique_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(i: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; 768];
        v[i] = 1.0;
        v
    }

    #[test]
    fn vec0_live_filter_and_update() {
        let db = Db::open_in_memory().unwrap();
        let c = db.sql();
        for (id, i, live) in [(1, 0, 1), (2, 0, 0), (3, 1, 1)] {
            c.execute(
                &format!(
                    "INSERT INTO memories_vec(rowid, embedding, live) VALUES (?1, {}, ?3)",
                    db.vec_sql("?2")
                ),
                rusqlite::params![id, db.vec_blob(&unit(i)), live],
            )
            .unwrap();
        }
        let knn = |sql: &str| -> Vec<i64> {
            let mut st = c.prepare(sql).unwrap();
            st.query_map([db.vec_blob(&unit(0))], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        let q = format!("SELECT rowid FROM memories_vec WHERE embedding MATCH {} AND k = 3 AND live = 1 ORDER BY distance", db.vec_sql("?1"));
        let q = q.as_str();
        assert_eq!(knn(q), vec![1, 3]);
        c.execute("UPDATE memories_vec SET live = 0 WHERE rowid = 1", [])
            .unwrap();
        assert_eq!(knn(q), vec![3]);
    }

    #[test]
    fn cypher_inside_transaction_rolls_back() {
        let db = Db::open_in_memory().unwrap();
        let r: Result<()> = db.write(|db| {
            db.graph()
                .cypher("CREATE (:Entity {id: 'e:1', name: 'x'})")?;
            Err(crate::err!("boom"))
        });
        assert!(r.is_err());
        let n = db
            .graph()
            .cypher("MATCH (n:Entity) RETURN count(n) AS c")
            .unwrap();
        assert_eq!(n.iter().next().unwrap().get::<i64>("c").unwrap(), 0);
        db.write(|db| {
            db.graph()
                .cypher("CREATE (:Entity {id: 'e:1', name: 'x'})")?;
            Ok(())
        })
        .unwrap();
        let n = db
            .graph()
            .cypher("MATCH (n:Entity) RETURN count(n) AS c")
            .unwrap();
        assert_eq!(n.iter().next().unwrap().get::<i64>("c").unwrap(), 1);
    }

    #[test]
    fn fts_triggers_index_memories() {
        let db = Db::open_in_memory().unwrap();
        let c = db.sql();
        c.execute("INSERT INTO memories(content, kind, created_at, valid_from, content_hash) VALUES ('production db runs Postgres', 'fact', 0, 0, 'h')", []).unwrap();
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'postgres'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn a_failed_commit_rolls_back_so_the_next_write_works() {
        let db = Db::open_in_memory().unwrap();
        // A deferred foreign-key violation only surfaces at COMMIT.
        let r: Result<()> = db.write(|db| {
            db.sql().execute_batch("PRAGMA defer_foreign_keys=ON")?;
            db.sql().execute(
                "INSERT INTO mentions(memory_id, entity_id) VALUES (999, 999)",
                [],
            )?;
            Ok(())
        });
        assert!(r.is_err(), "the commit must fail on the dangling mention");
        assert!(db.sql().is_autocommit(), "the transaction must be closed");
        db.write(|db| {
            db.sql()
                .execute("INSERT INTO meta(key,value) VALUES ('k','v')", [])?;
            Ok(())
        })
        .expect("the next write must not hit 'within a transaction'");
    }

    #[test]
    fn unique_live_hash_is_enforced() {
        let db = Db::open_in_memory().unwrap();
        let insert = |hash: &str| {
            db.sql().execute(
                "INSERT INTO memories(content,kind,created_at,valid_from,content_hash)
                 VALUES ('x','note',0,0,?1)",
                [hash],
            )
        };
        insert("h").unwrap();
        let e = insert("h").unwrap_err();
        assert!(is_unique_violation(&e), "{e}");
        // Retired rows do not occupy the hash.
        db.sql()
            .execute(
                "UPDATE memories SET expired_at=1 WHERE content_hash='h'",
                [],
            )
            .unwrap();
        insert("h").unwrap();
    }

    #[test]
    fn f32_round_trips_and_rejects_wrong_lengths() {
        let v = unit(3);
        assert_eq!(VecMode::F32.decode(&VecMode::F32.encode(&v)), Some(v));
        assert_eq!(VecMode::F32.decode(&[0u8; 4]), None);
        assert_eq!(VecMode::Int8.decode(&[0u8; 4]), None);
    }

    #[test]
    fn int8_round_trip_error_is_within_one_level() {
        // Deterministic unit vectors with elements spread over [-1, 1].
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..50 {
            let mut v: Vec<f32> = (0..768)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed % 2001) as f32 / 1000.0 - 1.0
                })
                .collect();
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.iter_mut().for_each(|x| *x /= norm);
            let back = VecMode::Int8.decode(&VecMode::Int8.encode(&v)).unwrap();
            let worst = v
                .iter()
                .zip(&back)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            let max = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(worst <= max / 127.0, "round trip error {worst} (max {max})");
        }
        // The largest element maps to 127; -64 is 192 as a byte.
        assert_eq!(
            VecMode::Int8.encode(&[1.0, -1.0, 2.0, 0.0]),
            vec![64, 192, 127, 0]
        );
    }

    #[test]
    fn vec_mode_parses_names() {
        assert_eq!(VecMode::parse("int8").unwrap(), VecMode::Int8);
        assert_eq!(VecMode::parse("f32").unwrap(), VecMode::F32);
        assert!(VecMode::parse("i8").is_err());
    }
}
