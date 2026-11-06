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

/// Busy timeout of an open: another process may be mid-transaction, and a
/// command must wait for it rather than fail.
pub(crate) const OPEN_BUSY: std::time::Duration = std::time::Duration::from_secs(5);

/// Busy timeout of a write transaction. A writer only ever waits behind
/// another writer's transaction, which is milliseconds of work, so waiting is
/// always better than losing the write. The open budget (5 s) is not enough on
/// a machine whose CPU or disk stalls the lock holder: the concurrent-writer
/// tests still failed with "database is locked" during another process's IO
/// storm (OPS-FLAKE), while 8 writers pass this budget under pure CPU load at
/// load avg 90. Reads keep the short budget, so a recall never pays this wait
/// (`recall` records access with [`Db::write_within`] and `OPEN_BUSY`).
const WRITE_BUSY: std::time::Duration = std::time::Duration::from_secs(30);

/// Busy timeout while loading the bundled graphqlite extension into an
/// already-initialised database. The load replays the extension's schema DDL,
/// and a no-op `CREATE INDEX IF NOT EXISTS` takes the write lock, so a busy
/// writer would otherwise stall every open for its whole busy timeout (twice,
/// measured 10 s) and the concurrent-writer tests flaked (OPS-FLAKE). The
/// extension tolerates a blocked DDL (the tables already exist); the full wait
/// is kept for a fresh file, whose graph schema still has to be created.
const LOAD_BUSY: std::time::Duration = std::time::Duration::from_millis(100);

/// The seeded-ontology reconcile is best-effort (see [`Db::seed_predicates`]),
/// so it waits far less than an open may: a database another process is
/// writing, or one opened read-only, opens at once and reconciles on the next
/// writable open.
const SEED_BUSY: std::time::Duration = std::time::Duration::from_secs(1);

/// Lock wait of the bit reconcile on a recall: a busy writer makes the recall
/// fall back to the exact scan at once, and the next recall retries.
const BITS_BUSY: std::time::Duration = std::time::Duration::from_millis(50);

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

/// Binary prefilter over memory vectors (lane nc-binvec): one sign bit per
/// dimension, packed LSB-first, plus the `live` filter column of
/// `memories_vec`. Derived from the stored vectors, so it is created on demand
/// (never by `create_schema`) and only used when `Config.vec_prefilter = "bit"`.
pub const BITS_TABLE: &str = "memories_bits";

/// Rows per backfill batch: 1024 x 3 KiB keeps a batch small on the low-RAM target.
const BITS_BATCH: i64 = 1024;

/// Sign bits of `v`, LSB-first (96 bytes for 768 dims). Hamming distance over
/// two of these estimates the angle between the vectors (d/768 ~ theta/pi), so
/// the table only generates candidates; the exact rerank orders them.
pub fn sign_bits(v: &[f32]) -> Vec<u8> {
    let mut out = vec![0u8; DIMS.div_ceil(8)];
    for (i, x) in v.iter().take(DIMS).enumerate() {
        if *x > 0.0 {
            out[i / 8] |= 1 << (i % 8);
        }
    }
    out
}

/// One connection for everything (relational, FTS5, vec0, Cypher), so a
/// write is one transaction.
pub struct Db {
    g: graphqlite::Connection,
    vec: VecMode,
    /// Set once this connection reconciled `BITS_TABLE` (see [`Db::ensure_bits`]).
    bits_reconciled: std::cell::Cell<bool>,
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
        conn.busy_timeout(OPEN_BUSY)?;
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
        // Loading the bundled graphqlite extension runs its schema DDL, and a
        // no-op `CREATE INDEX IF NOT EXISTS` takes the write lock (OPS-FLAKE):
        // with a writer active, every open then waited its whole busy timeout
        // twice and the concurrent-writer tests flaked. When the graph schema
        // is already there the DDL has nothing to create, so the load may fail
        // fast: open stays a read, and an uncontended open repairs anything a
        // skip left behind. A brand-new file keeps the full wait, because its
        // graph tables still have to be created.
        let graph_ready: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='nodes')",
            [],
            |r| r.get(0),
        )?;
        if graph_ready {
            conn.busy_timeout(LOAD_BUSY)?;
        }
        let g = graphqlite::Connection::from_rusqlite(conn)?;
        if graph_ready {
            g.sqlite_connection().busy_timeout(OPEN_BUSY)?;
        }
        let mut db = Db {
            g,
            vec: VecMode::F32,
            bits_reconciled: std::cell::Cell::new(false),
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
        // Opt-in binary prefilter (`NEOCORTEX_VEC_PREFILTER=bit`): fill the
        // derived table on the first open that asks for it, so writes keep it
        // in sync from then on. Best-effort: a read-only database opens without
        // it, and the read path ensures it again (or falls back to the scan).
        if matches!(
            std::env::var("NEOCORTEX_VEC_PREFILTER").as_deref(),
            Ok("bit")
        ) {
            let _ = db.ensure_bits();
        }
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
        // Probe with reads and run only the DDL that is missing. A no-op
        // `CREATE INDEX IF NOT EXISTS` still takes the write lock in SQLite,
        // so the old unconditional batch made every open - `recall` included -
        // queue behind writers and fail at the busy timeout under load
        // (OPS-FLAKE). In steady state an open now runs no write at all.
        let tables = [
            (
                "memory_links",
                "CREATE TABLE IF NOT EXISTS memory_links (
                   a INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
                   b INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
                   weight REAL NOT NULL,
                   PRIMARY KEY (a, b),
                   CHECK (a < b)
                 );",
            ),
            (
                "memory_access",
                "CREATE TABLE IF NOT EXISTS memory_access (
                   memory_id INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
                   at INTEGER NOT NULL,
                   PRIMARY KEY (memory_id, at)
                 );",
            ),
            (
                "name_tokens",
                "CREATE TABLE IF NOT EXISTS name_tokens (
                   token TEXT NOT NULL,
                   entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
                   PRIMARY KEY (token, entity_id)
                 );",
            ),
            (
                "name_part",
                "CREATE TABLE IF NOT EXISTS name_part (
                   a INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
                   b INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
                   weight REAL NOT NULL,
                   PRIMARY KEY (a, b),
                   CHECK (a < b)
                 );",
            ),
        ];
        let indexes = [
            (
                "idx_memory_links_b",
                "CREATE INDEX IF NOT EXISTS idx_memory_links_b ON memory_links(b);",
            ),
            (
                "idx_memory_access_at",
                "CREATE INDEX IF NOT EXISTS idx_memory_access_at ON memory_access(at);",
            ),
            (
                "idx_name_tokens_entity",
                "CREATE INDEX IF NOT EXISTS idx_name_tokens_entity ON name_tokens(entity_id);",
            ),
            (
                "idx_name_part_b",
                "CREATE INDEX IF NOT EXISTS idx_name_part_b ON name_part(b);",
            ),
            (
                "idx_rel_subj",
                "CREATE INDEX IF NOT EXISTS idx_rel_subj ON relations(subj, predicate_id);",
            ),
        ];
        let mut ddl = String::new();
        for (table, sql) in tables {
            if !self.table_exists(table)? {
                ddl.push_str(sql);
                ddl.push('\n');
            }
        }
        for (index, sql) in indexes {
            if !self.index_exists(index)? {
                ddl.push_str(sql);
                ddl.push('\n');
            }
        }
        if !ddl.is_empty() {
            self.sql().execute_batch(&ddl)?;
        }
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
        // The seeded ontology is reconciled on every v4 open: a database first
        // written by an older build gains the seeds and aliases added since
        // (the `mentor` family, `accountable to`). A presence probe cannot tell:
        // the causal family made an old database look complete.
        self.seed_predicates(false)?;
        self.rekey_symbol_names()?;
        Ok(())
    }

    /// Entities whose stored key is not `norm_name(name)`: (id, name, stored
    /// norm, current norm). A name never changes, so a difference means the key
    /// was written by a build that folded `#`/`+` away (TOOL-NAMES) or dropped
    /// the `s` of an all-caps abbreviation's plural (EX-06b, "DBs" = "DB"). Only
    /// names holding one of those shapes are read, so this is one cheap scan.
    pub fn stale_symbol_keys(&self) -> Result<Vec<(i64, String, String, String)>> {
        let mut st = self.sql().prepare(
            "SELECT id,name,norm FROM entities WHERE instr(name,'#')>0 OR instr(name,'+')>0 OR name GLOB '*[A-Z][A-Z]s*'",
        )?;
        let rows = st.query_map([], |r| {
            Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, norm) = row?;
            let now = crate::text::norm_name(&name);
            if now != norm && !now.is_empty() {
                out.push((id, name, norm, now));
            }
        }
        Ok(out)
    }

    /// Re-key entities written before TOOL-NAMES or EX-06b: `entities.norm`, the `name`
    /// alias and `name_tokens` move to the new key and the stale alias goes
    /// (its raw text is not stored, so only the name can derive a new one).
    /// Never merges: an entity whose new key is taken by a live row of the same
    /// type is left as is and `validate` reports it. Best-effort like the seed
    /// reconcile: a busy or read-only file opens and repairs on a later open.
    fn rekey_symbol_names(&self) -> Result<()> {
        let stale = self.stale_symbol_keys()?;
        if stale.is_empty() {
            return Ok(());
        }
        let own_txn = self.sql().is_autocommit();
        if own_txn && self.sql().is_readonly("main")? {
            return Ok(());
        }
        if own_txn {
            self.sql().busy_timeout(SEED_BUSY)?;
        }
        let begun = if own_txn {
            self.sql().execute_batch("BEGIN IMMEDIATE")
        } else {
            Ok(())
        };
        if own_txn {
            self.sql().busy_timeout(OPEN_BUSY)?;
        }
        match begun {
            Ok(()) => {}
            Err(e) if own_txn && is_busy_or_readonly(&e) => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        let written = (|| -> Result<()> {
            for (id, _, old, new) in &stale {
                let moved = self.sql().execute(
                    "UPDATE OR IGNORE entities SET norm=?1 WHERE id=?2",
                    rusqlite::params![new, id],
                )?;
                if moved == 0 {
                    continue;
                }
                self.sql().execute(
                    "INSERT OR IGNORE INTO entity_aliases(alias_norm,entity_id,origin) VALUES (?1,?2,'name')",
                    rusqlite::params![new, id],
                )?;
                self.sql().execute(
                    "DELETE FROM entity_aliases WHERE entity_id=?1 AND alias_norm=?2",
                    rusqlite::params![id, old],
                )?;
                self.sql()
                    .execute("DELETE FROM name_tokens WHERE entity_id=?1", [id])?;
                for token in crate::text::name_tokens(new) {
                    self.sql().execute(
                        "INSERT OR IGNORE INTO name_tokens(token,entity_id) VALUES (?1,?2)",
                        rusqlite::params![token, id],
                    )?;
                }
                crate::resolve::refresh_entity_fts(self, *id)?;
            }
            Ok(())
        })();
        if own_txn {
            if written.is_ok() {
                if let Err(e) = self.sql().execute_batch("COMMIT") {
                    let _ = self.sql().execute_batch("ROLLBACK");
                    return Err(e.into());
                }
            } else {
                let _ = self.sql().execute_batch("ROLLBACK");
            }
        }
        written
    }

    fn table_exists(&self, name: &str) -> Result<bool> {
        let n: i64 = self.sql().query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Read-only probe before running `CREATE INDEX IF NOT EXISTS`, which takes
    /// the write lock even when the index already exists.
    fn index_exists(&self, name: &str) -> Result<bool> {
        let n: i64 = self.sql().query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name=?1",
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
        self.seed_predicates(true)?;
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

    /// Sign bits of every stored memory vector, `BITS_TABLE` created if needed.
    /// The first call on a connection reconciles the table with `memories_vec`
    /// (count and largest rowid; `meta.vec_bits` alone cannot see rows an older
    /// binary added or removed). Later calls only check that the table exists,
    /// so a recall costs one catalog probe. The reconcile waits at most
    /// [`BITS_BUSY`] for a writer and then fails, so the caller falls back to
    /// the exact scan; a failed reconcile is retried by the next call. On a
    /// read-only database the write fails the same way.
    pub fn ensure_bits(&self) -> Result<()> {
        let created = !self.table_exists(BITS_TABLE)?;
        if !created && self.bits_reconciled.get() {
            return Ok(());
        }
        self.sql().busy_timeout(BITS_BUSY)?;
        let done = self.reconcile_bits(created);
        self.sql().busy_timeout(OPEN_BUSY)?;
        done?;
        self.bits_reconciled.set(true);
        Ok(())
    }

    fn reconcile_bits(&self, created: bool) -> Result<()> {
        if created {
            self.sql().execute_batch(&format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS {BITS_TABLE} \
                 USING vec0(embedding bit[{DIMS}], live integer, chunk_size=1024)"
            ))?;
        } else {
            let done: bool = self.sql().query_row(
                "SELECT EXISTS(SELECT 1 FROM meta WHERE key='vec_bits')",
                [],
                |r| r.get(0),
            )?;
            if done && self.bits_in_sync()? {
                return Ok(());
            }
        }
        self.backfill_bits()?;
        self.write_within(BITS_BUSY, |db| {
            db.sql().execute(
                "INSERT OR REPLACE INTO meta(key,value) VALUES('vec_bits','1')",
                [],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// Row count and largest rowid of `BITS_TABLE` equal those of
    /// `memories_vec`. Cheap shortcut: a swap of one missing and one orphan
    /// row with the same count and maximum passes, and the next write that
    /// changes the maximum or the count repairs it.
    fn bits_in_sync(&self) -> Result<bool> {
        let stat = |t: &str| -> rusqlite::Result<(i64, i64)> {
            self.sql().query_row(
                &format!("SELECT count(*), coalesce(max(rowid), 0) FROM {t}"),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        };
        Ok(stat(BITS_TABLE)? == stat("memories_vec")?)
    }

    /// Fill `BITS_TABLE` from `memories_vec` in rowid batches (never holds more
    /// than one batch of vectors); rows already present are skipped, so a
    /// partial table is completed rather than trusted. Bit rows whose vector
    /// is gone are deleted first, so they cannot take candidate slots. Every
    /// batch takes the short [`BITS_BUSY`] budget explicitly: `Db::write`
    /// would set the patient write budget and a recall waiting on a writer
    /// must fall back to the exact scan instead of blocking
    /// (`reconcile_does_not_wait_out_a_busy_writer`).
    fn backfill_bits(&self) -> Result<()> {
        self.write_within(BITS_BUSY, |db| {
            db.sql().execute(
                &format!(
                    "DELETE FROM {BITS_TABLE} WHERE rowid NOT IN (SELECT rowid FROM memories_vec)"
                ),
                [],
            )?;
            Ok(())
        })?;
        let have: std::collections::HashSet<i64> = {
            let mut st = self
                .sql()
                .prepare(&format!("SELECT rowid FROM {BITS_TABLE}"))?;
            let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut last = 0i64;
        loop {
            let rows: Vec<(i64, Vec<u8>, i64)> = {
                let mut st = self.sql().prepare(&format!(
                    "SELECT rowid, embedding, live FROM memories_vec \
                     WHERE rowid > ?1 ORDER BY rowid LIMIT {BITS_BATCH}"
                ))?;
                let rows = st.query_map([last], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            if rows.is_empty() {
                return Ok(());
            }
            last = rows.last().map(|r| r.0).unwrap_or(last);
            self.write_within(BITS_BUSY, |db| {
                let mut st = db.sql().prepare(&format!(
                    "INSERT INTO {BITS_TABLE}(rowid,embedding,live) VALUES (?1,vec_bit(?2),?3)"
                ))?;
                for (id, blob, live) in &rows {
                    if have.contains(id) {
                        continue;
                    }
                    let v = db
                        .vec
                        .decode(blob)
                        .ok_or_else(|| crate::err!("memories_vec row {id} is not a vector"))?;
                    st.execute(rusqlite::params![id, sign_bits(&v), live])?;
                }
                Ok(())
            })?;
        }
    }

    /// Whether the bit table exists (never creates it). Probed on every call,
    /// so a table another process creates or drops is seen by the next write.
    pub fn has_bits(&self) -> bool {
        self.table_exists(BITS_TABLE).unwrap_or(false)
    }

    /// Ids of the `k` memories nearest `query` by Hamming distance over the
    /// sign bits, `live_only` applying the `live=1` filter (history queries
    /// keep expired rows). Empty when the table does not exist.
    pub fn bits_knn(&self, query: &[f32], k: usize, live_only: bool) -> Result<Vec<i64>> {
        match self.bits_scan(query, k, live_only) {
            Ok(ids) => Ok(ids),
            // A shadow layout this build does not know (a different
            // sqlite-vec) still answers through the module's own KNN.
            Err(_) => self.bits_knn_vec0(query, k, live_only),
        }
    }

    /// Top-`k` by Hamming distance read straight from the vec0 chunk shadow
    /// tables: one validity, rowid and vector blob per 1024-row chunk, one
    /// popcount loop in Rust. The module's KNN is O(rows x k) in its top-k
    /// selection (50 ms for k=1000 on 32k rows), as expensive as the exact
    /// cosine scan it replaces; this scan is ~6 ms / 32k rows and ~21 ms /
    /// 108k. sqlite-vec is pinned to 0.1.9, whose shadow layout is
    /// `_chunks(validity,rowids)` + `_vector_chunks00(vectors)` +
    /// `_metadatachunks00` (i64 per row for `live`); any other shape errors and
    /// the caller falls back to [`Db::bits_knn_vec0`].
    fn bits_scan(&self, query: &[f32], k: usize, live_only: bool) -> Result<Vec<i64>> {
        let stride = DIMS / 8;
        let sql = if live_only {
            "SELECT c.validity, c.rowids, v.vectors, m.data
               FROM memories_bits_chunks c
               JOIN memories_bits_vector_chunks00 v ON v.rowid = c.chunk_id
               JOIN memories_bits_metadatachunks00 m ON m.rowid = c.chunk_id
              ORDER BY c.chunk_id"
        } else {
            "SELECT c.validity, c.rowids, v.vectors, NULL
               FROM memories_bits_chunks c
               JOIN memories_bits_vector_chunks00 v ON v.rowid = c.chunk_id
              ORDER BY c.chunk_id"
        };
        let qb = sign_bits(query);
        let mut st = self.sql().prepare(sql)?;
        let mut heap: std::collections::BinaryHeap<(u32, i64)> =
            std::collections::BinaryHeap::new();
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })?;
        for row in rows {
            let (validity, rowids, vectors, live) = row?;
            let count = rowids.len() / 8;
            if rowids.len() % 8 != 0 || vectors.len() != count * stride {
                return Err(crate::err!("memories_bits chunk layout changed"));
            }
            for i in 0..count {
                if validity.get(i / 8).is_none_or(|b| b >> (i % 8) & 1 == 0) {
                    continue;
                }
                if live_only {
                    let Some(l) = &live else {
                        return Err(crate::err!("memories_bits has no live metadata"));
                    };
                    let Some(bytes) = l.get(i * 8..i * 8 + 8) else {
                        return Err(crate::err!("memories_bits live metadata is short"));
                    };
                    if i64::from_le_bytes(bytes.try_into().expect("8 bytes")) != 1 {
                        continue;
                    }
                }
                let Some(id) = rowids.get(i * 8..i * 8 + 8) else {
                    continue;
                };
                let id = i64::from_le_bytes(id.try_into().expect("8 bytes"));
                let d = qb
                    .iter()
                    .zip(&vectors[i * stride..(i + 1) * stride])
                    .map(|(a, b)| (a ^ b).count_ones())
                    .sum::<u32>();
                if heap.len() < k {
                    heap.push((d, id));
                } else if heap.peek().is_some_and(|top| (d, id) < *top) {
                    heap.pop();
                    heap.push((d, id));
                }
            }
        }
        let mut out: Vec<(u32, i64)> = heap.into_vec();
        out.sort_unstable();
        Ok(out.into_iter().map(|(_, id)| id).collect())
    }

    /// The vec0 KNN fallback of [`Db::bits_knn`] (`ORDER BY distance`).
    fn bits_knn_vec0(&self, query: &[f32], k: usize, live_only: bool) -> Result<Vec<i64>> {
        let live = if live_only { " AND live=1" } else { "" };
        let sql = format!(
            "SELECT rowid FROM {BITS_TABLE} WHERE embedding MATCH vec_bit(?1) AND k=?2{live} ORDER BY distance"
        );
        let mut st = self.sql().prepare(&sql)?;
        let ids = st
            .query_map(rusqlite::params![sign_bits(query), k as i64], |r| {
                r.get::<_, i64>(0)
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// Store a memory vector (rowid = memory id, live = 1) and its sign bits
    /// when the prefilter table exists.
    pub fn insert_memory_vec(&self, id: i64, v: &[f32]) -> Result<()> {
        self.sql().execute(
            &format!(
                "INSERT INTO memories_vec(rowid,embedding,live) VALUES (?1,{},1)",
                self.vec_sql("?2")
            ),
            rusqlite::params![id, self.vec_blob(v)],
        )?;
        if self.has_bits() {
            self.sql().execute(
                &format!(
                    "INSERT INTO {BITS_TABLE}(rowid,embedding,live) VALUES (?1,vec_bit(?2),1)"
                ),
                rusqlite::params![id, sign_bits(v)],
            )?;
        }
        Ok(())
    }

    /// Flip the `live` flag of a memory vector (soft forget, supersede).
    pub fn set_memory_vec_live(&self, id: i64, live: i64) -> Result<()> {
        self.sql().execute(
            "UPDATE memories_vec SET live=?1 WHERE rowid=?2",
            rusqlite::params![live, id],
        )?;
        if self.has_bits() {
            self.sql().execute(
                &format!("UPDATE {BITS_TABLE} SET live=?1 WHERE rowid=?2"),
                rusqlite::params![live, id],
            )?;
        }
        Ok(())
    }

    /// Drop a memory vector (hard forget).
    pub fn delete_memory_vec(&self, id: i64) -> Result<()> {
        self.sql()
            .execute("DELETE FROM memories_vec WHERE rowid=?1", [id])?;
        if self.has_bits() {
            self.sql()
                .execute(&format!("DELETE FROM {BITS_TABLE} WHERE rowid=?1"), [id])?;
        }
        Ok(())
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

    /// Runs on every v4 open ([`Db::ensure_additive_tables`]) and must stay
    /// idempotent: the current rows and aliases are read once with plain
    /// SELECTs, the missing work is collected, and only then is a write
    /// transaction opened - a complete ontology takes none, so an open never
    /// needs the write lock. `adopt` is the v3 -> v4 migration, where rows
    /// predate the `seeded` marker and are taken into the seed set (the seed
    /// re-applies its display/exclusivity/symmetric policy there). On a v4
    /// open (`adopt = false`) a canonical or alias owned by a user-created row
    /// is left alone: the user row wins and the seed is skipped.
    ///
    /// The write is best-effort on a v4 open: `stats`, `recall` and `validate`
    /// only read, so a database another process is writing, or one opened
    /// read-only, must still open. A busy or read-only reconcile is skipped
    /// with one stderr line and the next writable open repairs the seeds; a
    /// migration's seed step (and every other migration) keeps its error.
    fn seed_predicates(&self, adopt: bool) -> Result<()> {
        struct Row {
            id: i64,
            display: String,
            exclusive: bool,
            symmetric: bool,
            group_id: i64,
            inverse_of: Option<i64>,
            seeded: bool,
        }
        struct Seed {
            canonical: &'static str,
            display: &'static str,
            aliases: &'static [&'static str],
            exclusive: bool,
            symmetric: bool,
            group: &'static str,
            inverse: Option<&'static str>,
        }
        /// One pending write. Collected from the SELECTs below, so a complete
        /// ontology returns before any transaction is opened.
        enum Work<'a> {
            /// The canonical is absent: insert it and link it.
            Insert(&'a Seed),
            /// A pre-v4 row joins the seed set (`seeded = 1`).
            Adopt(i64),
            /// A seeded row lost its display.
            Display(i64, &'a Seed),
            /// A seeded row's `exclusive` policy differs (v3 -> v4 only).
            Exclusive(i64, &'a Seed),
            /// A seeded row lost `symmetric` (v3 -> v4 only).
            Symmetric(i64),
            /// A seeded row's `group_id`/`inverse_of` drifted.
            Links(i64, &'a Seed),
            /// Alias norm to insert, owned by the seed's canonical.
            Alias(String, &'a str),
            /// A `mentored by` row still marked exclusive.
            MentoredExclusive(i64),
        }
        // Hand-written predicate ontology. Small on purpose: a wrong seed
        // merges real predicates, so only measured or unambiguous families are
        // here. `group` is the canonical predicate a row belongs to
        // (exclusivity and conflicts key on it); `inverse` names the predicate
        // that states the same relation from the other side. Seeded
        // exclusivity is policy and is not cleared by `exclusive: false`
        // writes.
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
                aliases: &[
                    "reports to",
                    "report to",
                    "reported to",
                    "reporting to",
                    "accountable to",
                    "is accountable to",
                ],
                exclusive: true,
                symmetric: false,
                group: "manage",
                inverse: Some("manage"),
            },
            // E4b-pattern pair: `mentor` is the canonical direction (mentor ->
            // mentee), `mentored by` the same relation from the mentee side.
            // Both are non-exclusive: a mentee can have several mentors
            // ("reports to" is the exclusive one, like employment).
            Seed {
                canonical: "mentor",
                display: "mentors",
                aliases: &["mentors", "mentor", "mentored", "mentoring"],
                exclusive: false,
                symmetric: false,
                group: "mentor",
                inverse: Some("mentored by"),
            },
            Seed {
                canonical: "mentored by",
                display: "mentored by",
                aliases: &["mentored by", "is mentored by"],
                exclusive: false,
                symmetric: false,
                group: "mentor",
                inverse: Some("mentor"),
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
        // ---- read phase: plain SELECTs, no transaction ----
        // Current ontology, read once: tens of rows, so a normal open does no
        // SQL beyond these two scans and the map lookups below.
        let mut rows: std::collections::HashMap<String, Row> = std::collections::HashMap::new();
        {
            let mut st = self.sql().prepare(
                "SELECT id,canonical,display,exclusive,symmetric,group_id,inverse_of,seeded
                 FROM predicates",
            )?;
            let it = st.query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    Row {
                        id: r.get(0)?,
                        display: r.get(2)?,
                        exclusive: r.get::<_, i64>(3)? != 0,
                        symmetric: r.get::<_, i64>(4)? != 0,
                        group_id: r.get(5)?,
                        inverse_of: r.get(6)?,
                        seeded: r.get::<_, i64>(7)? != 0,
                    },
                ))
            })?;
            for row in it {
                let (canonical, row) = row?;
                rows.insert(canonical, row);
            }
        }
        let mut alias_norms: std::collections::HashSet<String> = std::collections::HashSet::new();
        {
            let mut st = self
                .sql()
                .prepare("SELECT alias_norm FROM predicate_aliases")?;
            let it = st.query_map([], |r| r.get::<_, String>(0))?;
            for norm in it {
                alias_norms.insert(norm?);
            }
        }
        // A row whose family links do not name the seed's group/inverse has
        // drifted (a family row that is itself missing counts: the link cannot
        // point at it yet).
        let links_drift = |seed: &Seed, row: &Row| -> bool {
            let group_ok = rows.get(seed.group).is_some_and(|g| g.id == row.group_id);
            let inverse_ok = match seed.inverse {
                Some(canonical) => rows
                    .get(canonical)
                    .is_some_and(|i| row.inverse_of == Some(i.id)),
                None => true,
            };
            !(group_ok && inverse_ok)
        };
        let mut work: Vec<Work> = Vec::new();
        for seed in SEEDS {
            // Seeds this database owns. A canonical already taken by another
            // row on a v4 open is the user's: it is skipped whole (no marker,
            // no links, no aliases), so a seed never claims a predicate the
            // agent created.
            let owned = match rows.get(seed.canonical) {
                None => {
                    work.push(Work::Insert(seed));
                    true
                }
                Some(row) if row.seeded || adopt => {
                    if !row.seeded {
                        // Pre-v4 rows carry no marker: the migration adopts
                        // them into the seed set.
                        work.push(Work::Adopt(row.id));
                    }
                    // A blank display is repaired on any open; the rest of the
                    // hand-written policy (exclusivity, symmetry) is re-applied
                    // only by the v3 -> v4 migration, which takes the rows into
                    // the seed set.
                    if row.display.is_empty() && !seed.display.is_empty() {
                        work.push(Work::Display(row.id, seed));
                    }
                    if adopt && row.exclusive != seed.exclusive {
                        work.push(Work::Exclusive(row.id, seed));
                    }
                    if adopt && seed.symmetric && !row.symmetric {
                        work.push(Work::Symmetric(row.id));
                    }
                    if links_drift(seed, row) {
                        work.push(Work::Links(row.id, seed));
                    }
                    true
                }
                // The canonical belongs to an agent-created row: the user's row
                // wins and this seed is skipped whole.
                Some(_) => false,
            };
            if !owned {
                continue;
            }
            // An alias already owned by another predicate is the user's: it is
            // never moved to a seed row.
            for alias in seed
                .aliases
                .iter()
                .copied()
                .chain(std::iter::once(seed.canonical))
            {
                let norm = crate::text::norm_predicate(alias);
                if alias_norms.insert(norm.clone()) {
                    work.push(Work::Alias(norm, seed.canonical));
                }
            }
        }
        // A database built while `mentored by` was seeded exclusive carries the
        // wrong policy: a mentee can have several mentors. The seed value owns
        // seeded exclusivity (it is not agent-settable), so every open corrects
        // stale rows this cheaply and idempotently (no row is touched once the
        // value is right). This is the one exclusivity correction a v4 open
        // applies to a seeded row.
        if let Some(row) = rows.get("mentored by") {
            if row.seeded && row.exclusive {
                work.push(Work::MentoredExclusive(row.id));
            }
        }
        if work.is_empty() {
            return Ok(());
        }
        // ---- write phase: one transaction, and only when something is missing ----
        let own_txn = self.sql().is_autocommit();
        let best_effort = !adopt && own_txn;
        // A read-only file accepts `BEGIN IMMEDIATE` (in WAL mode it only takes
        // the shared-memory write lock) and fails at the first statement, so it
        // is decided here, before the transaction.
        if best_effort && self.sql().is_readonly("main")? {
            eprintln!(
                "neocortex: seeded predicate ontology not reconciled (database is read-only); \
                 the next writable open will repair it"
            );
            return Ok(());
        }
        // The transaction is opened by hand: `Db::write` turns SQLITE_BUSY and
        // SQLITE_READONLY into a message, and an open has to tell "another
        // writer has it for now" from a real failure. The body below joins this
        // transaction through `Db::write`.
        if best_effort {
            self.sql().busy_timeout(SEED_BUSY)?;
        }
        let begun = if own_txn {
            self.sql().execute_batch("BEGIN IMMEDIATE")
        } else {
            Ok(())
        };
        if best_effort {
            self.sql().busy_timeout(OPEN_BUSY)?;
        }
        match begun {
            Ok(()) => {}
            Err(e) if best_effort && is_busy_or_readonly(&e) => {
                eprintln!(
                    "neocortex: seeded predicate ontology not reconciled ({e}); \
                     the next writable open will repair it"
                );
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        }
        let written = self.write(|db| {
            // Inserts first, so a seed whose family arrives in this pass links
            // to the new rows below. OR IGNORE because another writer may have
            // created the same canonical between the read and this transaction.
            for item in &work {
                if let Work::Insert(seed) = item {
                    db.sql().execute(
                        "INSERT OR IGNORE INTO predicates(canonical,display,exclusive,symmetric,seeded)
                         VALUES (?1,?2,?3,?4,1)",
                        rusqlite::params![
                            seed.canonical,
                            seed.display,
                            i64::from(seed.exclusive),
                            i64::from(seed.symmetric)
                        ],
                    )?;
                }
            }
            // Canonical -> (id, seeded), read once after the inserts.
            let mut ids: std::collections::HashMap<String, (i64, bool)> =
                std::collections::HashMap::new();
            {
                let mut st = db
                    .sql()
                    .prepare("SELECT canonical,id,seeded FROM predicates")?;
                let it = st.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        (r.get::<_, i64>(1)?, r.get::<_, i64>(2)? != 0),
                    ))
                })?;
                for row in it {
                    let (canonical, id) = row?;
                    ids.insert(canonical, id);
                }
            }
            let id_of = |canonical: &str| -> Result<i64> {
                ids.get(canonical)
                    .map(|(id, _)| *id)
                    .ok_or_else(|| crate::err!("seed predicate '{canonical}' is missing"))
            };
            for item in &work {
                match item {
                    Work::Insert(seed) => {
                        let (id, ours) =
                            ids.get(seed.canonical).copied().ok_or_else(|| {
                                crate::err!("seed predicate '{}' is missing", seed.canonical)
                            })?;
                        if !ours {
                            // Another writer created it as a user row first.
                            continue;
                        }
                        db.sql().execute(
                            "UPDATE predicates SET group_id=?1 WHERE id=?2",
                            rusqlite::params![id_of(seed.group)?, id],
                        )?;
                        if let Some(inverse) = seed.inverse {
                            db.sql().execute(
                                "UPDATE predicates SET inverse_of=?1 WHERE id=?2",
                                rusqlite::params![id_of(inverse)?, id],
                            )?;
                        }
                    }
                    Work::Adopt(id) => {
                        db.sql()
                            .execute("UPDATE predicates SET seeded=1 WHERE id=?1", [id])?;
                    }
                    Work::Display(id, seed) => {
                        db.sql().execute(
                            "UPDATE predicates SET display=?1 WHERE id=?2",
                            rusqlite::params![seed.display, id],
                        )?;
                    }
                    Work::Exclusive(id, seed) => {
                        db.sql().execute(
                            "UPDATE predicates SET exclusive=?1 WHERE id=?2",
                            rusqlite::params![i64::from(seed.exclusive), id],
                        )?;
                    }
                    Work::Symmetric(id) => {
                        db.sql()
                            .execute("UPDATE predicates SET symmetric=1 WHERE id=?1", [id])?;
                    }
                    Work::Links(id, seed) => {
                        db.sql().execute(
                            "UPDATE predicates SET group_id=?1 WHERE id=?2",
                            rusqlite::params![id_of(seed.group)?, id],
                        )?;
                        if let Some(inverse) = seed.inverse {
                            db.sql().execute(
                                "UPDATE predicates SET inverse_of=?1 WHERE id=?2",
                                rusqlite::params![id_of(inverse)?, id],
                            )?;
                        }
                    }
                    Work::Alias(norm, canonical) => {
                        db.sql().execute(
                            "INSERT OR IGNORE INTO predicate_aliases(alias_norm,predicate_id) VALUES (?1,?2)",
                            rusqlite::params![norm, id_of(canonical)?],
                        )?;
                    }
                    Work::MentoredExclusive(id) => {
                        db.sql()
                            .execute("UPDATE predicates SET exclusive=0 WHERE id=?1", [id])?;
                    }
                }
            }
            Ok(())
        });
        if own_txn {
            if written.is_ok() {
                if let Err(e) = self.sql().execute_batch("COMMIT") {
                    let _ = self.sql().execute_batch("ROLLBACK");
                    return Err(e.into());
                }
            } else {
                let _ = self.sql().execute_batch("ROLLBACK");
            }
        }
        written
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
            db.seed_predicates(true)?;
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
    /// importer can wrap many writes in one atomic unit. The write waits up to
    /// [`WRITE_BUSY`], not the shorter open budget.
    pub fn write<T>(&self, f: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
        self.write_within(WRITE_BUSY, f)
    }

    /// [`Db::write`] with an explicit lock budget. A read path that records a
    /// best-effort write (access history) passes [`OPEN_BUSY`]: the caller's
    /// answer must not wait long for a writer it does not need; the bit-table
    /// reconcile passes its own [`BITS_BUSY`] the same way. The connection
    /// returns to the [`OPEN_BUSY`] default afterwards, so a caller that wants
    /// a shorter wait for the whole region sets it again around its own
    /// statements. A call already inside a transaction joins it and leaves the
    /// budget alone.
    pub fn write_within<T>(
        &self,
        budget: std::time::Duration,
        f: impl FnOnce(&Db) -> Result<T>,
    ) -> Result<T> {
        if !self.sql().is_autocommit() {
            return f(self);
        }
        self.sql().busy_timeout(budget)?;
        let out = self.write_locked(f);
        self.sql().busy_timeout(OPEN_BUSY)?;
        out
    }

    fn write_locked<T>(&self, f: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
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

/// True when a write failed only because another connection holds the write
/// lock or the database file is read-only, i.e. the write would succeed later
/// (or needs a writable file). Used by the best-effort seed reconcile.
fn is_busy_or_readonly(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if matches!(
                f.code,
                rusqlite::ErrorCode::DatabaseBusy
                    | rusqlite::ErrorCode::DatabaseLocked
                    | rusqlite::ErrorCode::ReadOnly
            )
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
