use crate::db::Db;
use crate::embed::Mode;
use crate::engine::Engine;
use crate::engine::{
    Community, CommunityRef, EntityCandidate, EntityCard, EntityOutput, FactHit, MemoryHit,
    NextHop, OverviewOutput, RecallInput, RecallOutput,
};
use crate::{extract, graph, kg, paths, rank, rerank, temporal, text, Result};
use rusqlite::{params, params_from_iter, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const MEMORY_COLUMNS: &str =
    "id,content,kind,tags,importance,scope,source,session,created_at,valid_from,valid_to,expired_at,superseded_by";

/// Weight of the query-directed path channel in the RRF fusion.
const PATH_WEIGHT: f64 = 2.5;
/// Weight of the exclusion channel: the memories of the typed items that lack
/// the property a "which X is not Y" question asks about (H2-02).
const EXCLUSION_WEIGHT: f64 = 2.5;
/// Score multiplier for memories of the typed items that do have the
/// excluded property: they are the wrong answer to that question.
const EXCLUSION_PENALTY: f64 = 0.5;
/// Weight of the status channel: bug reports no later memory fixes answer a
/// "what is still open" question (H2-15).
const STATUS_WEIGHT: f64 = 4.0;
/// Score multiplier for a bug a later memory fixes: still shown, ranked after
/// the open ones.
const STATUS_CLOSED_PENALTY: f64 = 0.5;
/// Score multiplier for a memory that reports no defect at all on a status
/// question: "Lumen is written in Rust" is not an open bug.
const STATUS_NEUTRAL_PENALTY: f64 = 0.7;
/// How many first-pass answers seed the cause follow (H2-16).
const CAUSE_SEEDS: usize = 2;
/// Weight of the followed cause memory: it explains the answer, it is not the
/// answer (the memory that names the fix stays first).
const CAUSE_WEIGHT: f64 = 1.5;
/// Most "is a later memory a fix" lookups one status question may run.
const STATUS_LOOKUPS: usize = 20;
/// How many FTS hits one "is a later memory a fix" lookup reads: enough to
/// find the live candidates for a bug from any point in the timeline.
const STATUS_FTS_HITS: usize = 200;
/// How many live candidates one lookup examines, in bm25 order.
const STATUS_FIX_CANDIDATES: usize = 30;
/// The status lookup's candidate query. The FTS table is the only source, so
/// the MATCH runs once per lookup; a join that starts from `memories` makes
/// SQLite evaluate it once per stored memory.
const STATUS_FTS_SQL: &str = "SELECT rowid FROM memories_fts WHERE memories_fts MATCH ?1
     ORDER BY bm25(memories_fts) LIMIT ?2";
/// Top dense-retrieval cosine below which a query with no lexical or entity
/// support is treated as "nothing relevant is stored".
const ABSTAIN_COSINE: f64 = 0.18;
/// Most `next` hints one recall may carry.
const HINT_MAX: usize = 2;
/// Words whose frame names what the question asks for.
const WH_WORDS: [&str; 9] = [
    "how", "what", "when", "where", "which", "who", "whom", "whose", "why",
];
/// Prepositions that belong to a final verb group ("grow up", "based in").
const PARTICLES: [&str; 12] = [
    "about", "at", "by", "for", "from", "in", "into", "of", "on", "to", "up", "with",
];

fn generation(db: &Db) -> Result<i64> {
    Ok(db.sql().query_row(
        "SELECT COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key='gen'),0)",
        [],
        |r| r.get(0),
    )?)
}

fn snapshot(engine: &Engine, as_of: Option<i64>, history: bool) -> Result<Arc<kg::Snapshot>> {
    // The snapshot cache is cleared by eval when a knob changes.
    let gen = generation(&engine.db)?;
    if let Some((g, a, h, snap)) = engine.snapshot_cache.lock().unwrap().as_ref() {
        if *g == gen && *a == as_of && *h == history {
            return Ok(Arc::clone(snap));
        }
    }
    let name_parts = paths::name_hop_on(engine.cfg.name_part_hop);
    let snap = Arc::new(kg::snapshot_with(&engine.db, as_of, history, name_parts)?);
    *engine.snapshot_cache.lock().unwrap() = Some((gen, as_of, history, Arc::clone(&snap)));
    Ok(snap)
}

type MemoryRow = (
    i64,
    String,
    String,
    String,
    f64,
    String,
    String,
    String,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

fn read_row(r: &rusqlite::Row) -> rusqlite::Result<MemoryRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
        r.get(12)?,
    ))
}

/// Fetch rows only for the ids a channel actually produced.
fn fetch_rows(db: &Db, ids: &[i64]) -> Result<HashMap<i64, MemoryRow>> {
    let mut out = HashMap::new();
    for chunk in ids.chunks(400) {
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT {MEMORY_COLUMNS} FROM memories WHERE id IN ({marks})");
        let mut st = db.sql().prepare(&sql)?;
        let mut rows = st.query(params_from_iter(chunk.iter()))?;
        while let Some(r) = rows.next()? {
            let row = read_row(r)?;
            out.insert(row.0, row);
        }
    }
    Ok(out)
}

/// Memories that mention any of the entities, deduplicated and in id order.
fn memories_mentioning(db: &Db, entities: &[i64]) -> Result<Vec<i64>> {
    let mut out = Vec::new();
    for chunk in entities.chunks(400) {
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT DISTINCT memory_id FROM mentions WHERE entity_id IN ({marks})");
        let mut st = db.sql().prepare(&sql)?;
        for id in st.query_map(params_from_iter(chunk.iter()), |r| r.get::<_, i64>(0))? {
            out.push(id?);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// E4a: expand the best dense hits through temporal links. A memory one day
/// away from the anchor is a weak neighbour; a memory minutes away is nearly
/// as relevant as the anchor itself.
fn temporal_link_ids(db: &Db, seeds: &[(i64, f64)], keep: usize, depth: usize) -> Result<Vec<i64>> {
    let seeds: Vec<(i64, f64)> = seeds.iter().take(keep).copied().collect();
    if seeds.is_empty() || depth == 0 {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = seeds.iter().map(|(id, _)| *id).collect();
    let rank: HashMap<i64, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i + 1)).collect();
    let cosine: HashMap<i64, f64> = seeds.iter().copied().collect();
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for chunk in ids.chunks(400) {
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql =
            format!("SELECT a,b,weight FROM memory_links WHERE a IN ({marks}) OR b IN ({marks})");
        let mut st = db.sql().prepare(&sql)?;
        let mut p: Vec<Box<dyn rusqlite::ToSql>> = chunk
            .iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>)
            .collect();
        p.extend(
            chunk
                .iter()
                .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>),
        );
        for row in st.query_map(
            rusqlite::params_from_iter(p.iter().map(|x| x.as_ref())),
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            },
        )? {
            let (a, b, weight) = row?;
            let (anchor, other) = if rank.contains_key(&a) {
                (a, b)
            } else {
                (b, a)
            };
            let strength = cosine.get(&anchor).copied().unwrap_or(0.0).max(0.05)
                / rank.get(&anchor).copied().unwrap_or(1) as f64;
            let entry = scores.entry(other).or_insert(0.0);
            *entry = entry.max(weight * strength);
        }
    }
    let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(depth);
    Ok(ranked.into_iter().map(|(id, _)| id).collect())
}

/// Memory KNN over `memories_vec`: the exact vec0 scan, or — when
/// `Config.vec_prefilter = "bit"` — the `memories_bits` Hamming scan followed
/// by an exact cosine rerank of the candidates. Both return `(id, cosine)` in
/// descending order (ties on id) truncated to `k`. The bit table is derived:
/// the first use creates and backfills it, and a database that cannot be
/// written (read-only, no table) falls back to the exact scan.
fn memory_knn(engine: &Engine, query: &[f32], k: usize, history: bool) -> Result<Vec<(i64, f32)>> {
    if engine.cfg.vec_prefilter == "bit" && engine.db.ensure_bits().is_ok() {
        let n = engine
            .cfg
            .vec_prefilter_n
            .max(1)
            .saturating_mul(k)
            .clamp(k, 5000);
        let ids = engine.db.bits_knn(query, n, !history)?;
        let vectors = fetch_vectors(&engine.db, &ids)?;
        let mut scored: Vec<(i64, f32)> = ids
            .iter()
            .filter_map(|id| vectors.get(id).map(|v| (*id, rank::cosine(query, v))))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.truncate(k);
        return Ok(scored);
    }
    let knn_sql = if history {
        "SELECT rowid,1-distance FROM memories_vec
         WHERE embedding MATCH {} AND k=?2 ORDER BY distance"
    } else {
        "SELECT rowid,1-distance FROM memories_vec
         WHERE embedding MATCH {} AND k=?2 AND live=1 ORDER BY distance"
    };
    let mut st = engine
        .db
        .sql()
        .prepare(&knn_sql.replace("{}", &engine.db.vec_sql("?1")))?;
    // At least SEM_FACTS neighbours: the semantic beam uses them as prose hops.
    let knn = st
        .query_map(params![engine.db.vec_blob(query), k as i64], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, f32>(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(knn)
}

fn fetch_vectors(db: &Db, ids: &[i64]) -> Result<HashMap<i64, Vec<f32>>> {
    // vec0 serves `rowid = ?` as a point lookup but `rowid IN (...)` as a full
    // scan of the table, so fetch one row at a time.
    let mut out = HashMap::new();
    let mut st = db
        .sql()
        .prepare_cached("SELECT embedding FROM memories_vec WHERE rowid = ?1")?;
    for &id in ids {
        if let Some(v) = st
            .query_row([id], |r| r.get::<_, Vec<u8>>(0))
            .optional()?
            .and_then(|blob| db.blob_to_vec(&blob))
        {
            out.insert(id, v);
        }
    }
    Ok(out)
}

/// The `keep` memory ids closest to the query, read in bounded chunks so an
/// exclusion question over a whole type never holds that type's vectors in RAM
/// (low-resource target). Ties break on id, like a stable sort by cosine.
fn top_by_cosine(db: &Db, ids: &[i64], query_vec: &[f32], keep: usize) -> Result<Vec<i64>> {
    let chunk = 1024;
    let mut scored: Vec<(f32, i64)> = Vec::new();
    for part in ids.chunks(chunk) {
        let vectors = fetch_vectors(db, part)?;
        for id in part {
            let cosine = vectors
                .get(id)
                .map(|v| rank::cosine(query_vec, v))
                .unwrap_or(0.0);
            scored.push((cosine, *id));
        }
        if scored.len() >= keep * 2 {
            scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            scored.truncate(keep);
        }
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.truncate(keep);
    Ok(scored.into_iter().map(|(_, id)| id).collect())
}

/// H2-15: the bug reports among the candidates, split into the ones no later
/// memory fixes (the answer to "what is still open") and the ones a later
/// memory fixes. A report that carries a fix cue itself is already closed
/// unless the cue is negated ("not fixed yet").
///
/// A question that names a defect ("is the login bug still open") only ranks
/// the reports about that defect; a general question ("what bug is still
/// open") ranks every open report. Reports are ranked by query cosine before
/// the lookup budget, so which ones are classified never depends on the order
/// they were stored in.
fn status_channels(
    engine: &Engine,
    want: &HashSet<String>,
    query_vec: &[f32],
    rows: &HashMap<i64, MemoryRow>,
    now: i64,
) -> Result<(Vec<i64>, HashSet<i64>)> {
    let mut closed = HashSet::new();
    let mut bugs = Vec::new();
    for (id, row) in rows {
        if !paths::is_bug_memory(&row.1) {
            continue;
        }
        if paths::has_fix_cue(&row.1) {
            closed.insert(*id);
        } else if want.is_empty()
            || paths::status_terms(&row.1)
                .iter()
                .any(|term| want.contains(term))
        {
            bugs.push(*id);
        }
    }
    let ranked = top_by_cosine(&engine.db, &bugs, query_vec, STATUS_LOOKUPS)?;
    let mut open = Vec::new();
    for id in ranked {
        let Some(row) = rows.get(&id) else { continue };
        if fixed_later(&engine.db, id, &row.1, row.9, now)? {
            closed.insert(id);
        } else {
            open.push(id);
        }
    }
    let open = top_by_cosine(&engine.db, &open, query_vec, 30)?;
    Ok((open, closed))
}

/// A memory written at or after `from` and still live at `now` that fixes the
/// defect `content` reports: it carries a fix cue and shares a date and a
/// content word, or two content words.
///
/// The lookup is driven by the FTS index (`STATUS_FTS_SQL`) and filters the
/// hits in bm25 order, so a status question costs one MATCH per lookup, not
/// one per stored memory.
fn fixed_later(db: &Db, id: i64, content: &str, from: i64, now: i64) -> Result<bool> {
    let mut terms: Vec<String> = paths::status_terms(content).into_iter().collect();
    terms.sort();
    terms.truncate(12);
    let Some(q) = text::fts_query(&terms.join(" "), &[]) else {
        return Ok(false);
    };
    let hits: Vec<i64> = db
        .sql()
        .prepare_cached(STATUS_FTS_SQL)?
        .query_map(params![q, STATUS_FTS_HITS as i64], |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<_>>()?;
    let rows = fetch_rows(db, &hits)?;
    let mut live = 0;
    for other in hits {
        if other == id {
            continue;
        }
        let Some(row) = rows.get(&other) else {
            continue;
        };
        if row.9 < from || is_ended(row, now) {
            continue;
        }
        live += 1;
        if paths::has_fix_cue(&row.1) && paths::same_bug(content, &row.1) {
            return Ok(true);
        }
        if live >= STATUS_FIX_CANDIDATES {
            break;
        }
    }
    Ok(false)
}

/// World-time end of a memory (valid_to reached) or system retirement.
fn is_ended(row: &MemoryRow, cutoff: i64) -> bool {
    row.10.is_some_and(|x| x <= cutoff) || row.11.is_some()
}

/// True when the memory states its own interval and that interval misses `w`
/// (half-open overlap, the same rule as `since`/`until`). A row whose
/// valid_from is only its ingest time has no stated interval: it is never
/// outside a window, because nobody said when it happened. An event without a
/// stored end still has one when its text states it ("from X to Y"); otherwise
/// it happens at its start, so it belongs to the window that contains that
/// start. Anything else without an end is still true and overlaps later
/// windows.
fn dated_outside(row: &MemoryRow, w: &temporal::Window, clock: &temporal::Clock) -> bool {
    let stated = row.10.is_some() || row.9 != row.8;
    if !stated {
        return false;
    }
    let end = match row.10 {
        Some(to) => Some(to),
        None if row.2 == "event" => stated_end(row, clock),
        None => None,
    };
    match end {
        Some(end) => row.9 >= w.end || end <= w.start,
        None => row.2 == "event" && !(w.start <= row.9 && row.9 < w.end),
    }
}

/// The exclusive end an event's own text states, when the writer stored only a
/// start: a text date stores just its start (remember.rs), so "Sprint 27 runs
/// from 2026-10-26 to 2026-11-06" keeps its end only in the wording. The same
/// parser that dated the row reads it back; a text whose first window is not
/// the stored start says nothing about this interval.
fn stated_end(row: &MemoryRow, clock: &temporal::Clock) -> Option<i64> {
    let text = temporal::find_window(&row.1, clock)?;
    (text.start == row.9 && text.end > text.start).then_some(text.end)
}

/// A window filters only when the question scopes itself to it ("in August
/// 2026", "during 2025", "on 2026-08-01"). A bare month that modifies a noun
/// ("the December conference", "the December 2026 conference") dates the
/// subject, not the search, so it stays a ranking boost.
fn window_scope(w: &temporal::Window) -> bool {
    let matched = text::fold(&w.matched);
    let lead = matched.split_whitespace().next().unwrap_or("");
    matches!(
        lead,
        "in" | "during"
            | "over"
            | "throughout"
            | "within"
            | "on"
            | "since"
            | "before"
            | "after"
            | "between"
            | "from"
    )
}

/// A planned row is shown for a future-event question only when it is about
/// the same thing: at least two non-stopword query tokens (3+ chars) appear in
/// its text.
fn planned_matches_query(content: &str, query: &str) -> bool {
    let want: Vec<String> = text::tokens(query)
        .into_iter()
        .filter(|t| t.chars().count() >= 3 && !text::is_stopword(t))
        .map(|t| text::singularize(&t))
        .collect();
    if want.is_empty() {
        return false;
    }
    let have: HashSet<String> = text::tokens(content)
        .into_iter()
        .map(|t| text::singularize(&t))
        .collect();
    want.iter().filter(|t| have.contains(*t)).count() >= 2
}

#[allow(clippy::too_many_arguments)]
fn eligible(
    row: &MemoryRow,
    as_of: Option<i64>,
    history: bool,
    input: &RecallInput,
    since: Option<i64>,
    until: Option<i64>,
    now: i64,
    planned_ok: bool,
) -> bool {
    let (_, _, kind, tags, _, scope, source, session, _, from, to, expired, superseded) = row;
    // Forgotten memories (expired without a successor) are never returned.
    if expired.is_some() && superseded.is_none() {
        return false;
    }
    if expired.is_some() && !history {
        return false;
    }
    // A plan or a future-dated memory has not happened yet: it is not current
    // truth unless the question asks about plans, takes an as-of snapshot, or
    // asks about that specific future event's date or state.
    if as_of.is_none() && !planned_ok {
        let future = *from > now || tags.split_whitespace().any(|tag| tag == "planned");
        if future
            && !(temporal::future_event_intent(&input.query)
                && planned_matches_query(&row.1, &input.query))
        {
            return false;
        }
    }
    if let Some(t) = as_of {
        if *from > t || to.is_some_and(|x| t >= x) {
            return false;
        }
    }
    if !input.kinds.is_empty() && !input.kinds.iter().any(|x| x == kind) {
        return false;
    }
    if !input.tags.is_empty() {
        let set: HashSet<String> = tags.split_whitespace().map(str::to_string).collect();
        if !input
            .tags
            .iter()
            .map(|x| text::fold(x))
            .any(|x| set.contains(&x))
        {
            return false;
        }
    }
    if input.scope.as_deref().is_some_and(|x| x != scope)
        || input.source.as_deref().is_some_and(|x| x != source)
        || input.session.as_deref().is_some_and(|x| x != session)
    {
        return false;
    }
    // Overlap, not containment: [from, to) must meet [since, until).
    if until.is_some_and(|x| *from >= x) || since.is_some_and(|x| to.is_some_and(|t| t <= x)) {
        return false;
    }
    true
}

fn parse_bound(s: Option<&str>, clock: &temporal::Clock) -> Result<Option<i64>> {
    s.map(|x| temporal::parse_instant(x, clock).ok_or_else(|| crate::err!("invalid time '{x}'")))
        .transpose()
}

fn structured_filters(
    input: &RecallInput,
    since: Option<i64>,
    until: Option<i64>,
    as_of: Option<i64>,
) -> bool {
    !input.kinds.is_empty()
        || !input.tags.is_empty()
        || input.scope.is_some()
        || input.source.is_some()
        || input.session.is_some()
        || since.is_some()
        || until.is_some()
        || as_of.is_some()
}

/// Every entity an alias names (an alias may belong to several entities).
fn alias_entities(db: &Db, names: &[String]) -> Result<Vec<i64>> {
    let mut out = HashSet::new();
    for chunk in names.chunks(400) {
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql =
            format!("SELECT DISTINCT entity_id FROM entity_aliases WHERE alias_norm IN ({marks})");
        let mut st = db.sql().prepare(&sql)?;
        let rows = st.query_map(params_from_iter(chunk.iter()), |r| r.get::<_, i64>(0))?;
        for row in rows {
            out.insert(row?);
        }
    }
    let mut ids: Vec<i64> = out.into_iter().collect();
    ids.sort_unstable();
    Ok(ids)
}

fn relation_visible(
    valid_from: i64,
    valid_to: Option<i64>,
    expired: Option<i64>,
    now: i64,
    history: bool,
    as_of: Option<i64>,
) -> bool {
    let t = as_of.unwrap_or(now);
    if valid_from > t {
        return false;
    }
    if as_of.is_some() {
        return valid_to.is_none_or(|v| t < v);
    }
    if history {
        return true;
    }
    valid_to.is_none_or(|v| now < v) && expired.is_none()
}

/// Relation-fact seeds: `relations_fts` hits and `relations_vec` KNN, both
/// endpoints get half of the relation's score.
type SeedPairs = Vec<(i64, f64)>;

struct RelationSeedOptions<'a> {
    query_entities: &'a HashSet<i64>,
    fact_entity_boost: f64,
    fact_memory_seed: f64,
    /// Restrict the relation-vector arm to relations incident to the question
    /// anchors (`Config.rel_prefilter`).
    prefilter: bool,
}

/// Anchors whose incident relations the prefilter may read, and the upper
/// bound on that relation set (a hub anchor would otherwise cost more than the
/// global KNN).
const REL_PREFILTER_SEEDS: usize = 20;
const REL_PREFILTER_CAP: usize = 800;

#[allow(clippy::too_many_arguments)]
fn relation_seeds_with_config(
    db: &Db,
    query: &str,
    query_vec: &[f32],
    now: i64,
    as_of: Option<i64>,
    history: bool,
    prefix_min: usize,
    options: RelationSeedOptions<'_>,
) -> Result<(SeedPairs, SeedPairs)> {
    let mut entities = Vec::new();
    let mut memories = Vec::new();
    let mut add = |relation_id: i64, src: i64, dst: i64, score: f64| -> Result<()> {
        let matched_endpoint =
            options.query_entities.contains(&src) || options.query_entities.contains(&dst);
        for id in [src, dst] {
            let weight = if options.query_entities.contains(&id) {
                score * options.fact_entity_boost
            } else {
                score
            };
            entities.push((id, weight));
        }
        if matched_endpoint && options.fact_memory_seed > 0.0 {
            let mut st = db
                .sql()
                .prepare("SELECT memory_id FROM relation_evidence WHERE relation_id=?1")?;
            for memory_id in st.query_map([relation_id], |r| r.get::<_, i64>(0))? {
                memories.push((memory_id?, score * options.fact_memory_seed));
            }
        }
        Ok(())
    };
    if let Some(q) = text::fts_query_with(query, &[], prefix_min) {
        let mut st = db.sql().prepare(
            "SELECT r.id,r.src,r.dst,r.valid_from,r.valid_to,r.expired_at FROM relations_fts JOIN relations r ON r.id=relations_fts.rowid
             WHERE relations_fts MATCH ?1
               AND r.valid_from<=COALESCE(?4,?2)
               AND ((?4 IS NOT NULL AND (r.valid_to IS NULL OR r.valid_to>?4) AND r.expired_at IS NULL)
                    OR (?4 IS NULL AND (?3=1 OR ((r.valid_to IS NULL OR r.valid_to>?2) AND r.expired_at IS NULL))))
             ORDER BY bm25(relations_fts,1.0,1.0) LIMIT 10",
        )?;
        let hits: Vec<(i64, i64, i64)> = st
            .query_map(params![q, now, i64::from(history), as_of], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        for (rank, (relation_id, src, dst)) in hits.iter().enumerate() {
            let score = 0.5 / (1.0 + rank as f64);
            add(*relation_id, *src, *dst, score)?;
        }
    }
    if options.prefilter && !options.query_entities.is_empty() {
        // The question names entities, so the relation-fact seeds live on
        // their incident relations. Point-read the vector of each (bounded)
        // incident relation instead of scanning the whole `relations_vec`.
        let mut seeds: Vec<i64> = options.query_entities.iter().copied().collect();
        seeds.sort_unstable();
        seeds.truncate(REL_PREFILTER_SEEDS);
        let marks = std::iter::repeat_n("?", seeds.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id,src,dst,valid_from,valid_to,expired_at FROM relations
             WHERE (src IN ({marks}) OR dst IN ({marks}))
             ORDER BY valid_from DESC, id DESC LIMIT {REL_PREFILTER_CAP}"
        );
        let mut st = db.sql().prepare(&sql)?;
        let mut rows = st.query(params_from_iter(seeds.iter().chain(seeds.iter())))?;
        let mut vector_st = db
            .sql()
            .prepare_cached("SELECT embedding FROM relations_vec WHERE rowid=?1")?;
        let mut scored: Vec<(i64, i64, i64, f32)> = Vec::new();
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let src: i64 = r.get(1)?;
            let dst: i64 = r.get(2)?;
            let vf: i64 = r.get(3)?;
            let vt: Option<i64> = r.get(4)?;
            let expired: Option<i64> = r.get(5)?;
            if !relation_visible(vf, vt, expired, now, history, as_of) {
                continue;
            }
            let Some(v) = vector_st
                .query_row([id], |r| r.get::<_, Vec<u8>>(0))
                .optional()?
                .and_then(|blob| db.blob_to_vec(&blob))
            else {
                continue;
            };
            let cosine = rank::cosine(query_vec, &v);
            if cosine >= 0.5 {
                scored.push((id, src, dst, cosine));
            }
        }
        scored.sort_by(|a, b| b.3.total_cmp(&a.3).then(a.0.cmp(&b.0)));
        scored.truncate(10);
        for (id, src, dst, cosine) in scored {
            add(id, src, dst, f64::from(cosine) * 0.5)?;
        }
    } else {
        let mut st = db.sql().prepare(&format!(
            "SELECT rowid,1-distance FROM relations_vec
             WHERE embedding MATCH {} AND k=10 ORDER BY distance",
            db.vec_sql("?1")
        ))?;
        let hits: Vec<(i64, f32)> = st
            .query_map([db.vec_blob(query_vec)], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let ids: Vec<i64> = hits
            .iter()
            .filter(|(_, cosine)| *cosine >= 0.5)
            .map(|(id, _)| *id)
            .collect();
        if !ids.is_empty() {
            let marks = std::iter::repeat_n("?", ids.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT id,src,dst,valid_from,valid_to,expired_at FROM relations WHERE id IN ({marks})"
            );
            let mut st = db.sql().prepare(&sql)?;
            let mut rows = st.query(params_from_iter(ids.iter()))?;
            let mut visible = HashMap::new();
            while let Some(r) = rows.next()? {
                let id: i64 = r.get(0)?;
                let src: i64 = r.get(1)?;
                let dst: i64 = r.get(2)?;
                let vf: i64 = r.get(3)?;
                let vt: Option<i64> = r.get(4)?;
                let expired: Option<i64> = r.get(5)?;
                if relation_visible(vf, vt, expired, now, history, as_of) {
                    visible.insert(id, (src, dst));
                }
            }
            for (id, cosine) in hits {
                if let Some(&(src, dst)) = visible.get(&id) {
                    add(id, src, dst, f64::from(cosine) * 0.5)?;
                }
            }
        }
    }
    Ok((entities, memories))
}

/// The wh-frame of a question: what `recall` could not finish.
struct WhFrame {
    /// Relation or question words for `follow.predicate`.
    want: String,
    /// Answer-side words, compared against a hop's predicate vector.
    frame: String,
    /// The single word that decides whether the context already answers it.
    guard: String,
}

fn word_forms(word: &str) -> HashSet<String> {
    let bare = word.trim_end_matches("'s");
    let mut out = HashSet::new();
    for form in [
        bare.to_string(),
        text::singularize(bare),
        text::norm_predicate(bare),
    ] {
        if form.chars().count() > 1 {
            out.insert(form);
        }
    }
    out
}

/// Every spelling of an entity name a question may use, so the name is not
/// mistaken for a relation word.
fn bridge_words(engine: &Engine, ids: &[i64]) -> Result<HashSet<String>> {
    let mut out = HashSet::new();
    let mut names: Vec<String> = Vec::new();
    for chunk in ids.chunks(400) {
        if chunk.is_empty() {
            continue;
        }
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT name FROM entities WHERE id IN ({marks})");
        let mut st = engine.db.sql().prepare(&sql)?;
        for name in st.query_map(params_from_iter(chunk.iter()), |r| r.get::<_, String>(0))? {
            names.push(name?);
        }
    }
    for name in names {
        for token in text::tokens(&name) {
            let bare = token.trim_end_matches("'s").to_string();
            let singular = text::singularize(&bare);
            out.insert(bare);
            out.insert(singular);
        }
    }
    Ok(out)
}

fn wh_frame(query: &str, anchors: &HashSet<String>) -> Option<WhFrame> {
    const LIGHT_VERBS: [&str; 11] = [
        "be", "been", "did", "do", "does", "gave", "get", "got", "have", "made", "took",
    ];
    let words = text::tokens(query);
    let at = words.iter().position(|w| WH_WORDS.contains(&w.as_str()))?;
    let wh = words[at].clone();
    let content = |w: &str| {
        let bare = w.trim_end_matches("'s");
        w.chars().count() > 1
            && !text::is_stopword(w)
            && !WH_WORDS.contains(&w)
            && !anchors.contains(w)
            && !anchors.contains(bare)
            && !anchors.contains(&text::singularize(bare))
    };
    let mut want = vec![wh.clone()];
    for w in words.iter().skip(at + 1) {
        if content(w) && !want.iter().any(|x| x == w) {
            want.push(w.clone());
            if want.len() == 4 {
                break;
            }
        }
    }
    let mut frame = vec![wh.clone()];
    if matches!(wh.as_str(), "which" | "what" | "whose" | "who" | "whom") {
        if let Some(head) = words.iter().skip(at + 1).find(|w| !text::is_stopword(w)) {
            if content(head) {
                frame.push(head.clone());
            }
        }
    }
    // The final verb group ("get schooled", "grow up", "based in"): the last
    // content word plus the nearest particle and a light verb right before it.
    let mut tail: Vec<String> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for (i, w) in words.iter().enumerate().rev() {
        if !content(w) && !PARTICLES.contains(&w.as_str()) {
            continue;
        }
        if PARTICLES.contains(&w.as_str()) {
            pending.push(w.clone());
            continue;
        }
        if i > 0 && LIGHT_VERBS.contains(&words[i - 1].as_str()) && content(&words[i - 1]) {
            tail.push(words[i - 1].clone());
        }
        tail.push(w.clone());
        if let Some(nearest) = pending.last() {
            tail.push(nearest.clone());
        }
        break;
    }
    for w in tail {
        if !frame.iter().any(|x| x == &w) {
            frame.push(w);
        }
    }
    let guard = if matches!(wh.as_str(), "which" | "what" | "whose") && frame.len() > 1 {
        frame[1].clone()
    } else {
        frame.last()?.clone()
    };
    if want.len() == 1 && frame.len() == 1 {
        return None;
    }
    Some(WhFrame {
        want: want.join(" "),
        frame: frame.join(" "),
        guard,
    })
}

fn bridge_answer_type(query: &str) -> Option<&'static str> {
    let words = text::tokens(query);
    if words.iter().any(|w| w == "who" || w == "whom") {
        return Some("person");
    }
    if words.iter().any(|w| {
        matches!(
            w.as_str(),
            "where" | "city" | "town" | "country" | "place" | "location"
        )
    }) {
        return Some("place");
    }
    if words
        .iter()
        .any(|w| matches!(w.as_str(), "company" | "organization" | "org"))
    {
        return Some("org");
    }
    if words
        .iter()
        .any(|w| matches!(w.as_str(), "language" | "tool" | "framework" | "database"))
    {
        return Some("tool");
    }
    None
}

/// Answer types a question's wh-frame asks for. Only the head noun decides
/// the type: "which practice" asks for an organization, "what species" for an
/// animal, "where" for a place (an organization is a valid place answer: "where
/// did she get schooled" is answered by a school). A frame with no known head
/// ("which one", "what happened") asks for no type, so the reverse hop stays
/// off and an unreadable question costs one token scan.
fn answer_frame_types(query: &str) -> Option<Vec<&'static str>> {
    const PERSON: &[&str] = &["person"];
    const PLACE: &[&str] = &["place", "organization"];
    let words = text::tokens(query);
    let at = words.iter().position(|w| WH_WORDS.contains(&w.as_str()))?;
    match words[at].as_str() {
        "who" | "whom" => return Some(PERSON.to_vec()),
        "where" => return Some(PLACE.to_vec()),
        _ => {}
    }
    let head = words.iter().skip(at + 1).find(|w| {
        !text::is_stopword(w) && !matches!(w.as_str(), "do" | "did" | "does" | "is" | "was")
    })?;
    let types: &[&str] = match head.as_str() {
        "company" | "firm" | "organization" | "org" | "studio" | "school" | "academy"
        | "society" | "practice" | "investor" | "lab" | "laboratory" | "supplier"
        | "consultancy" | "employer" | "client" | "university" | "institute" | "agency"
        | "bank" | "club" | "gym" | "hospital" | "clinic" | "committee" | "council" | "charity"
        | "startup" => &["organization"],
        "city" | "town" | "village" | "country" | "place" | "location" | "region" => PLACE,
        "instrument" | "breed" | "allergen" | "dish" | "colour" | "color" | "book" => &["object"],
        "species" | "pet" | "animal" => &["animal", "object"],
        "language" | "tool" | "framework" | "database" => &["tool"],
        _ => return None,
    };
    Some(types.to_vec())
}

/// Most bridge entities the reverse hop starts from.
const REV_HOP_BRIDGES: usize = 3;
/// Most facts the reverse hop merges (the whole fact list is capped at 8).
const REV_HOP_FACTS: usize = 2;
/// A bridge with more live relations than this is a hub: its neighbourhood
/// says nothing about the question and would flood the fact list.
const REV_HOP_HUB: i64 = 200;

/// Ordering key of one reverse-hop candidate: a predicate the question shares
/// a word with first, then a seeded group, then the earlier (more grounded)
/// bridge, then overlap size, recency and id.
fn rank_key(
    candidate: &(bool, bool, usize, usize, i64, i64),
) -> (
    bool,
    bool,
    std::cmp::Reverse<usize>,
    usize,
    i64,
    std::cmp::Reverse<i64>,
) {
    (
        candidate.0,
        candidate.1,
        std::cmp::Reverse(candidate.2),
        candidate.3,
        candidate.4,
        std::cmp::Reverse(candidate.5),
    )
}

/// Reverse-hop answer-type pull: the answer frame names a type, so look
/// backward from the bridge entities retrieval already returned for a live
/// relation whose far end has that type. The bridge is grounded by a returned
/// memory, while the terminal relation may be paraphrased far below the
/// relation-KNN cutoff, so this bypasses the relation seed set entirely.
/// Bounded: <= 3 bridges, one relation per (bridge, predicate group), <= 2
/// merged facts, hubs skipped.
#[allow(clippy::too_many_arguments)]
fn rev_hop_facts(
    engine: &Engine,
    query: &str,
    chains: &[paths::PathHit],
    memories: &[MemoryHit],
    anchors: &HashSet<i64>,
    cutoff: i64,
    existing: &HashSet<i64>,
) -> Result<(Vec<FactHit>, Vec<i64>)> {
    let Some(types) = answer_frame_types(query) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut bridges: Vec<i64> = Vec::new();
    for memory in memories.iter().take(5) {
        let mut st = engine
            .db
            .sql()
            .prepare("SELECT entity_id FROM mentions WHERE memory_id=?1")?;
        for id in st.query_map([memory.id], |r| r.get::<_, i64>(0))? {
            let id = id?;
            if !anchors.contains(&id) && !bridges.contains(&id) {
                bridges.push(id);
            }
        }
    }
    for chain in chains {
        if let Some(step) = chain.steps.last() {
            let id = step_end(step);
            if id > 0 && !anchors.contains(&id) && !bridges.contains(&id) {
                bridges.push(id);
            }
        }
    }
    bridges.truncate(REV_HOP_BRIDGES);
    if bridges.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut degree = engine.db.sql().prepare_cached(
        "SELECT COUNT(*) FROM relations
         WHERE (src=?1 OR dst=?1) AND expired_at IS NULL
           AND (valid_to IS NULL OR valid_to>?2)",
    )?;
    let marks = std::iter::repeat_n("?", types.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT r.id,r.src,r.dst,p.group_id,p.seeded,
                COALESCE(NULLIF(r.predicate_text,''),NULLIF(p.display,''),p.canonical),
                r.valid_from
         FROM relations r
         JOIN predicates p ON p.id=r.predicate_id
         JOIN entities s ON s.id=r.src
         JOIN entities d ON d.id=r.dst
         WHERE r.polarity=1 AND r.expired_at IS NULL AND r.valid_from<=?2
           AND (r.valid_to IS NULL OR r.valid_to>?2)
           AND ((r.src=?1 AND d.type IN ({marks})) OR (r.dst=?1 AND s.type IN ({marks})))
         ORDER BY r.valid_from DESC, r.id DESC LIMIT 40"
    );
    let mut rel_st = engine.db.sql().prepare(&sql)?;
    let query_forms: HashSet<String> = text::tokens(query)
        .iter()
        .flat_map(|token| word_forms(token))
        .collect();
    let mut ranked: Vec<(bool, bool, usize, usize, i64, i64)> = Vec::new();
    for (bridge_at, bridge) in bridges.iter().enumerate() {
        let live: i64 = degree.query_row(params![bridge, cutoff], |r| r.get(0))?;
        if live > REV_HOP_HUB {
            continue;
        }
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(*bridge), Box::new(cutoff)];
        params.extend(
            types
                .iter()
                .map(|t| Box::new(t.to_string()) as Box<dyn rusqlite::ToSql>),
        );
        params.extend(
            types
                .iter()
                .map(|t| Box::new(t.to_string()) as Box<dyn rusqlite::ToSql>),
        );
        let mut rows = rel_st.query(params_from_iter(params.iter().map(|p| p.as_ref())))?;
        let mut best: HashMap<i64, (bool, bool, usize, usize, i64, i64)> = HashMap::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let src: i64 = row.get(1)?;
            let dst: i64 = row.get(2)?;
            let group: i64 = row.get(3)?;
            let seeded: i64 = row.get(4)?;
            let predicate: String = row.get(5)?;
            let valid_from: i64 = row.get(6)?;
            if existing.contains(&id) {
                continue;
            }
            let far = if src == *bridge { dst } else { src };
            if anchors.contains(&far) {
                continue;
            }
            let overlap = text::tokens(&predicate)
                .iter()
                .filter(|token| !word_forms(token).is_disjoint(&query_forms))
                .count();
            let candidate = (overlap > 0, seeded == 1, bridge_at, overlap, valid_from, id);
            let better = best
                .get(&group)
                .is_none_or(|old| rank_key(&candidate) > rank_key(old));
            if better {
                best.insert(group, candidate);
            }
        }
        ranked.extend(best.into_values());
    }
    ranked.sort_by_key(|a| std::cmp::Reverse(rank_key(a)));
    let ids: Vec<i64> = ranked
        .into_iter()
        .take(REV_HOP_FACTS)
        .map(|row| row.5)
        .collect();
    if ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let facts = paths::facts_by_ids(&engine.db, &ids, cutoff, REV_HOP_FACTS)?;
    let mut evidence = Vec::new();
    for id in &ids {
        if let Some(memory_id) = engine
            .db
            .sql()
            .query_row(
                "SELECT memory_id FROM relation_evidence WHERE relation_id=?1
                 ORDER BY memory_id DESC LIMIT 1",
                [id],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
        {
            evidence.push(memory_id);
        }
    }
    if std::env::var_os("NEOCORTEX_DEBUG_FACTS").is_some() {
        eprintln!(
            "rev_hop bridges={} pulled={} kept={}",
            bridges.len(),
            ids.len(),
            facts.len()
        );
    }
    Ok((facts, evidence))
}

/// Pull the answer-typed edge of a small set of returned bridge entities.
/// This deliberately bypasses relation KNN: the bridge is already grounded
/// by a returned memory, while the terminal predicate may be paraphrased.
#[allow(clippy::too_many_arguments)]
fn bridge_pull_facts(
    engine: &Engine,
    query: &str,
    query_vec: &[f32],
    chains: &[paths::PathHit],
    memories: &[MemoryHit],
    anchors: &HashSet<i64>,
    cutoff: i64,
    existing: &HashSet<i64>,
) -> Result<(Vec<FactHit>, Vec<i64>)> {
    let Some(answer_type) = bridge_answer_type(query) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut bridges = Vec::new();
    for chain in chains {
        if let Some(step) = chain.steps.last() {
            let id = step_end(step);
            if !anchors.contains(&id) && !bridges.contains(&id) {
                bridges.push(id);
            }
        }
    }
    for memory in memories.iter().take(5) {
        let mut st = engine
            .db
            .sql()
            .prepare("SELECT entity_id FROM mentions WHERE memory_id=?1")?;
        for id in st.query_map([memory.id], |r| r.get::<_, i64>(0))? {
            let id = id?;
            if !anchors.contains(&id) && !bridges.contains(&id) {
                bridges.push(id);
            }
        }
    }
    bridges.truncate(3);
    if bridges.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let mut degree = Vec::new();
    let mut st = engine.db.sql().prepare(
        "SELECT entity_id,COUNT(*) FROM (
             SELECT src AS entity_id FROM relations
             WHERE expired_at IS NULL AND (valid_to IS NULL OR valid_to>?1)
             UNION ALL
             SELECT dst AS entity_id FROM relations
             WHERE expired_at IS NULL AND (valid_to IS NULL OR valid_to>?1)
         ) GROUP BY entity_id",
    )?;
    for row in st.query_map([cutoff], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as usize))
    })? {
        degree.push(row?);
    }
    degree.sort_by_key(|(_, count)| *count);
    let p99 = degree
        .get((degree.len() * 99).div_ceil(100).saturating_sub(1))
        .map(|(_, count)| *count)
        .unwrap_or(usize::MAX);

    let query_words: HashSet<String> = text::tokens(query).into_iter().collect();
    let mut ranked: Vec<(usize, f64, i64, i64)> = Vec::new();
    let mut already_present = 0;
    for bridge in bridges {
        let count = degree
            .iter()
            .find_map(|(id, count)| (*id == bridge).then_some(*count))
            .unwrap_or(0);
        if count > p99 {
            continue;
        }
        let mut st = engine.db.sql().prepare(
            "SELECT r.id,
                    COALESCE(NULLIF(r.predicate_text,''),NULLIF(p.display,''),p.canonical),
                    r.valid_from
             FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             JOIN entities s ON s.id=r.src
             JOIN entities d ON d.id=r.dst
             WHERE r.polarity=1 AND r.expired_at IS NULL
               AND (r.valid_to IS NULL OR r.valid_to>?1) AND r.valid_from<=?1
               AND ((r.src=?2 AND d.type=?3) OR (r.dst=?2 AND s.type=?3))
             ORDER BY r.valid_from DESC LIMIT 2",
        )?;
        for row in st.query_map(params![cutoff, bridge, answer_type], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })? {
            let (id, predicate, valid_from) = row?;
            if existing.contains(&id) {
                already_present += 1;
                continue;
            }
            let overlap = text::tokens(&predicate)
                .into_iter()
                .filter(|word| query_words.contains(word))
                .count();
            let cosine = engine
                .db
                .sql()
                .query_row(
                    "SELECT embedding FROM relations_vec WHERE rowid=?1",
                    [id],
                    |r| r.get::<_, Vec<u8>>(0),
                )
                .ok()
                .and_then(|blob| engine.db.blob_to_vec(&blob))
                .map(|vec| f64::from(rank::cosine(query_vec, &vec)))
                .unwrap_or(0.0);
            ranked.push((overlap, cosine, valid_from, id));
        }
    }
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.total_cmp(&a.1))
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| b.3.cmp(&a.3))
    });
    let ids: Vec<i64> = ranked.into_iter().take(2).map(|(_, _, _, id)| id).collect();
    let facts = paths::facts_by_ids(&engine.db, &ids, cutoff, 2)?;
    let mut evidence = Vec::new();
    for id in &ids {
        if let Some(memory_id) = engine
            .db
            .sql()
            .query_row(
                "SELECT memory_id FROM relation_evidence WHERE relation_id=?1
                 ORDER BY memory_id DESC LIMIT 1",
                [id],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
        {
            evidence.push(memory_id);
        }
    }
    if std::env::var_os("NEOCORTEX_DEBUG_FACTS").is_some() {
        eprintln!(
            "bridge_pull pulled={} kept={} already-present={}",
            ids.len(),
            facts.len(),
            already_present
        );
    }
    Ok((facts, evidence))
}

fn step_end(step: &paths::Step) -> i64 {
    if step.memory.is_some() {
        if step.dst < 1 {
            step.src
        } else {
            step.dst
        }
    } else if step.reverse {
        step.src
    } else {
        step.dst
    }
}

/// True when the entity has a live relation `follow` would accept for the
/// question's relation words: a hint that names a hop no relation can walk
/// only costs the caller a call.
fn relation_matches_frame(
    engine: &Engine,
    id: i64,
    frame_forms: &HashSet<String>,
    exclude: Option<i64>,
    now: i64,
) -> Result<bool> {
    let mut st = engine.db.sql().prepare_cached(
        "SELECT r.id,p.canonical,r.fact FROM relations r
           JOIN predicates p ON p.id=r.predicate_id
          WHERE (r.src=?1 OR r.dst=?2) AND r.expired_at IS NULL
            AND (r.valid_to IS NULL OR r.valid_to>?3)",
    )?;
    let rows = st.query_map(params![id, id, now], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (relation_id, predicate, fact) = row?;
        if Some(relation_id) == exclude {
            continue;
        }
        let forms = format!("{predicate} {fact}");
        if text::tokens(&forms)
            .iter()
            .any(|token| !word_forms(token).is_disjoint(frame_forms))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `follow`-ready hints for the hop retrieval could not finish: name the bridge
/// entity (the end of the best partial chain, or a fuzzy seed) and the words to
/// ask it for. Empty when the returned context already covers the wh-frame.
#[allow(clippy::too_many_arguments)]
fn next_hops(
    engine: &Engine,
    query: &str,
    anchors: &HashSet<i64>,
    words_of: &[i64],
    chains: &[&paths::PathHit],
    seeds: &[(i64, f64)],
    facts: &[FactHit],
    memories: &[MemoryHit],
    memory_vectors: &HashMap<i64, Vec<f32>>,
    now: i64,
    tau: f32,
) -> Result<Vec<NextHop>> {
    if tau <= 0.0 {
        return Ok(Vec::new());
    }
    let anchor_words = bridge_words(engine, words_of)?;
    let Some(wh) = wh_frame(query, &anchor_words) else {
        return Ok(Vec::new());
    };
    // Cheap guard: a returned fact or memory already says one of the
    // question's relation words, so the question is probably answered. Checked
    // before the frame is embedded, which is the only added cost of hints.
    let mut cue_forms = HashSet::new();
    for token in text::tokens(&wh.frame) {
        let bare = token.trim_end_matches("'s");
        if WH_WORDS.contains(&token.as_str())
            || text::is_stopword(&token)
            || PARTICLES.contains(&bare)
        {
            continue;
        }
        cue_forms.extend(word_forms(&token));
    }
    if cue_forms.is_empty() {
        cue_forms = word_forms(&wh.guard);
    }
    let seen = |value: &str| {
        text::tokens(value)
            .iter()
            .any(|token| !word_forms(token).is_disjoint(&cue_forms))
    };
    if facts.iter().any(|f| seen(&f.predicate) || seen(&f.fact))
        || memories.iter().any(|m| seen(&m.content))
    {
        return Ok(Vec::new());
    }
    let mut frame_forms: HashSet<String> = HashSet::new();
    for token in text::tokens(&wh.frame) {
        frame_forms.extend(word_forms(&token));
    }
    // A hint is only worth its tokens when `follow` can actually walk the hop,
    // so find the followable bridges first and skip the frame embedding when
    // there are none.
    let mut bridge_ids: Vec<i64> = Vec::new();
    let mut seed_bridges: Vec<i64> = Vec::new();
    for chain in chains {
        let Some(step) = chain.steps.last() else {
            continue;
        };
        let end = step_end(step);
        if end < 1 || anchors.contains(&end) || bridge_ids.contains(&end) {
            continue;
        }
        let exclude = if step.memory.is_some() {
            None
        } else {
            Some(step.relation_id)
        };
        if !relation_matches_frame(engine, end, &frame_forms, exclude, now)? {
            continue;
        }
        bridge_ids.push(end);
        if bridge_ids.len() >= HINT_MAX {
            break;
        }
    }
    if bridge_ids.is_empty() {
        for (id, _) in seeds {
            if bridge_ids.contains(id)
                || !relation_matches_frame(engine, *id, &frame_forms, None, now)?
            {
                continue;
            }
            bridge_ids.push(*id);
            seed_bridges.push(*id);
            if bridge_ids.len() >= HINT_MAX {
                break;
            }
        }
    }
    if bridge_ids.is_empty() {
        return Ok(Vec::new());
    }
    let frame_vec = engine
        .emb
        .embed(&[wh.frame.as_str()], Mode::Query)?
        .remove(0);
    // The answer may already be in the context without saying the ask word:
    // a returned fact or memory that reads like the wh-frame ends the search.
    let mut relation_st = engine
        .db
        .sql()
        .prepare_cached("SELECT embedding FROM relations_vec WHERE rowid=?1")?;
    let mut memory_st = engine
        .db
        .sql()
        .prepare_cached("SELECT embedding FROM memories_vec WHERE rowid=?1")?;
    let cosine = |blob: Vec<u8>| -> Option<f32> {
        engine
            .db
            .blob_to_vec(&blob)
            .map(|v| rank::cosine(&frame_vec, &v))
    };
    let mut best = 0.0f32;
    for fact in facts {
        if let Some(c) = relation_st
            .query_row([fact.id], |r| r.get::<_, Vec<u8>>(0))
            .optional()?
            .and_then(cosine)
        {
            best = best.max(c);
        }
    }
    for memory in memories {
        if let Some(v) = memory_vectors.get(&memory.id) {
            best = best.max(rank::cosine(&frame_vec, v));
        }
    }
    if best >= tau {
        return Ok(Vec::new());
    }
    for chain in chains {
        let Some(step) = chain.steps.last() else {
            continue;
        };
        let vector = if let Some(memory_id) = step.memory {
            memory_st
                .query_row([memory_id], |r| r.get::<_, Vec<u8>>(0))
                .optional()?
                .and_then(|b| engine.db.blob_to_vec(&b))
        } else {
            relation_st
                .query_row([step.relation_id], |r| r.get::<_, Vec<u8>>(0))
                .optional()?
                .and_then(|b| engine.db.blob_to_vec(&b))
        };
        if vector.is_some_and(|v| rank::cosine(&frame_vec, &v) >= tau) {
            // This chain's last hop already answers the wh-frame.
            return Ok(Vec::new());
        }
    }
    let mut out = Vec::new();
    for id in bridge_ids {
        let (name, kind): (String, String) =
            engine
                .db
                .sql()
                .query_row("SELECT name,type FROM entities WHERE id=?1", [id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
        let why = if seed_bridges.contains(&id) {
            "no chain covers the question".to_string()
        } else if kind.is_empty() {
            "best chain ends here".to_string()
        } else {
            format!("chain ends at a {kind}")
        };
        out.push(NextHop {
            entity: format!("e{id}"),
            name,
            want: wh.want.clone(),
            why,
        });
    }
    Ok(out)
}

fn memory_has_ended_anchor_relation(
    engine: &Engine,
    memory_id: i64,
    cutoff: i64,
    anchors: &HashSet<i64>,
    query: &str,
) -> Result<bool> {
    if anchors.is_empty() {
        return Ok(false);
    }
    let placeholders = std::iter::repeat_n("?", anchors.len())
        .collect::<Vec<_>>()
        .join(",");
    let predicate_param = 3 + 2 * anchors.len();
    let sql = format!(
        "SELECT EXISTS(
             SELECT 1 FROM relation_evidence e JOIN relations r ON r.id=e.relation_id
             JOIN predicates p ON p.id=r.predicate_id
             WHERE e.memory_id=?1 AND (r.expired_at IS NOT NULL OR r.valid_to IS NOT NULL AND r.valid_to<=?2)
               AND (r.src IN ({placeholders}) OR r.dst IN ({placeholders}))
               AND (instr(?{predicate_param}, lower(r.predicate_text)) > 0 OR instr(?{predicate_param}, lower(p.canonical)) > 0 OR instr(?{predicate_param}, lower(p.display)) > 0)
         )"
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(memory_id), Box::new(cutoff)];
    params.extend(
        anchors
            .iter()
            .copied()
            .map(|id| Box::new(id) as Box<dyn rusqlite::ToSql>),
    );
    params.extend(
        anchors
            .iter()
            .copied()
            .map(|id| Box::new(id) as Box<dyn rusqlite::ToSql>),
    );
    params.push(Box::new(text::norm_predicate(query)));
    Ok(engine.db.sql().query_row(
        &sql,
        params_from_iter(params.iter().map(|p| p.as_ref())),
        |r| r.get(0),
    )?)
}

pub fn recall(engine: &Engine, input: &RecallInput) -> Result<RecallOutput> {
    engine.check_embedder()?;
    if input.query.trim().is_empty() {
        return Err(crate::err!("query must not be empty"));
    }
    let clock = engine.clock();
    let now = clock.now;
    let as_of = parse_bound(input.as_of.as_deref(), &clock)?;
    let since = parse_bound(input.since.as_deref(), &clock)?;
    let until = parse_bound(input.until.as_deref(), &clock)?;
    let window = temporal::find_window(&input.query, &clock);
    let history_query = temporal::history_intent(&input.query);
    let past_state_query = temporal::past_state_intent(&input.query);
    // Planned memories stay out of current answers unless the question is
    // about plans, and an ordering/duration question is a history question:
    // it needs the ended intervals to compute an answer.
    let planned_ok = temporal::plan_intent(&input.query, &clock)
        || temporal::wants_plans(&input.query)
        || window.as_ref().is_some_and(|w| w.start >= now);
    let ordering = temporal::ordering_intent(&input.query);
    let duration_query = temporal::duration_intent(&input.query);
    let ordered = ordering || duration_query;
    let history = input.include_history || history_query || as_of.is_some() || ordered;
    let k = input.k.unwrap_or(8).clamp(1, 50);
    let budget = input.budget.unwrap_or(1200).min(20_000);
    let negated = paths::negation_intent(&input.query);
    // H2-02: "which service is not in Rust" names a type and a property; the
    // answer is the item that lacks it, not a memory containing a negation.
    let exclusion = if negated {
        None
    } else {
        paths::exclusion_entities(&engine.db, &input.query, now)?
    };
    let causal_query = paths::causal_intent(&input.query);
    let status_query = engine.cfg.status_rank && paths::status_intent(&input.query);
    let query_text = input
        .query
        .split_whitespace()
        .take(64)
        .collect::<Vec<_>>()
        .join(" ");
    let query_vec = engine.emb.embed(&[&query_text], Mode::Query)?.remove(0);
    let depth = (5 * k).max(50);
    // Filtered channels need headroom: the KNN/BM25 depth is applied before
    // eligibility, so oversample when structured filters are present.
    let fetch_depth = if structured_filters(input, since, until, as_of) {
        (depth * 4).min(2000)
    } else {
        depth
    };
    let mut rows: HashMap<i64, MemoryRow> = HashMap::new();

    // ------------------------------------------------------------ bm25
    let mut bm25_scored: Vec<(i64, f64)> = Vec::new();
    if let Some(q) = text::fts_query_with(&query_text, &input.keywords, engine.cfg.fts_prefix_min) {
        let mut st = engine.db.sql().prepare(
            "SELECT rowid,bm25(memories_fts,1.0,2.0) FROM memories_fts
             WHERE memories_fts MATCH ?1 ORDER BY bm25(memories_fts,1.0,2.0) LIMIT ?2",
        )?;
        for row in st.query_map(params![q, fetch_depth as i64], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
        })? {
            bm25_scored.push(row?);
        }
    }
    let missing: Vec<i64> = bm25_scored
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    let bm25_scored: Vec<(i64, f64)> = bm25_scored
        .into_iter()
        .filter(|(id, _)| {
            rows.get(id).is_some_and(|row| {
                eligible(row, as_of, history, input, since, until, now, planned_ok)
            })
        })
        .collect();
    let best_bm25 = bm25_scored
        .iter()
        .map(|(_, score)| score.abs())
        .fold(0.0f64, f64::max);
    let bm25: Vec<i64> = bm25_scored
        .into_iter()
        .filter(|(_, score)| score.abs() >= engine.cfg.bm25_rel * best_bm25)
        .map(|(id, _)| id)
        .take(depth)
        .collect();

    // ---------------------------------------------------------- vector
    let mut knn = memory_knn(
        engine,
        &query_vec,
        fetch_depth.max(paths::SEM_FACTS),
        history,
    )?;
    let nearest_memories: Vec<i64> = knn.iter().map(|(id, _)| *id).collect();
    knn.truncate(fetch_depth);
    let knn_ids: Vec<i64> = knn.iter().map(|(id, _)| *id).collect();
    let missing: Vec<i64> = knn_ids
        .iter()
        .copied()
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    let mut vec_ranked: Vec<(i64, f64)> = knn
        .into_iter()
        .filter(|(id, _)| {
            rows.get(id).is_some_and(|row| {
                eligible(row, as_of, history, input, since, until, now, planned_ok)
            })
        })
        .map(|(id, cosine)| (id, f64::from(cosine)))
        .collect();
    vec_ranked.truncate(depth);
    let vec_ids: Vec<i64> = vec_ranked.iter().map(|(id, _)| *id).collect();

    // ------------------------------------------------------- temporal
    let mut time_ids: Vec<i64> = Vec::new();
    let mut window_empty = false;
    if let Some(w) = &window {
        let overlapping: i64 = engine.db.sql().query_row(
            "SELECT count(*) FROM memories WHERE expired_at IS NULL
             AND valid_from < ?2 AND (valid_to IS NULL OR valid_to > ?1)",
            params![w.start, w.end],
            |r| r.get(0),
        )?;
        window_empty = overlapping == 0;
        let count: i64 = engine.db.sql().query_row(
            "SELECT count(*) FROM memories WHERE valid_from >= ?1 AND valid_from < ?2",
            params![w.start, w.end],
            |r| r.get(0),
        )?;
        let sql = if count <= 5000 {
            "SELECT id FROM memories WHERE valid_from >= ?1 AND valid_from < ?2 ORDER BY valid_from DESC"
        } else {
            "SELECT id FROM memories WHERE valid_from >= ?1 AND valid_from < ?2 ORDER BY valid_from DESC LIMIT 400"
        };
        let mut st = engine.db.sql().prepare(sql)?;
        let mut candidates: Vec<i64> = st
            .query_map(params![w.start, w.end], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<_>>()?;
        let seen: HashSet<i64> = candidates.iter().copied().collect();
        candidates.extend(vec_ids.iter().copied().filter(|id| {
            !seen.contains(id)
                && rows
                    .get(id)
                    .is_some_and(|row| row.9 >= w.start && row.9 < w.end)
        }));
        let missing: Vec<i64> = candidates
            .iter()
            .copied()
            .filter(|id| !rows.contains_key(id))
            .collect();
        rows.extend(fetch_rows(&engine.db, &missing)?);
        let in_window: Vec<i64> = candidates
            .into_iter()
            .filter(|id| {
                rows.get(id).is_some_and(|row| {
                    eligible(row, as_of, history, input, since, until, now, planned_ok)
                })
            })
            .collect();
        let vectors = fetch_vectors(&engine.db, &in_window)?;
        let mut scored: Vec<(i64, f64)> = in_window
            .into_iter()
            .filter_map(|id| {
                vectors
                    .get(&id)
                    .map(|v| (id, f64::from(rank::cosine(&query_vec, v))))
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(depth);
        time_ids = scored.into_iter().map(|(id, _)| id).collect();
    }

    // ---------------------------------------------------------- graph
    let snap = snapshot(engine, as_of, history)?;
    let mut ent_seeds: HashMap<i64, f64> = HashMap::new();
    let mut seed_entity = |id: i64, weight: f64| {
        let entry = ent_seeds.entry(id).or_insert(0.0);
        if weight > *entry {
            *entry = weight;
        }
    };
    // (a) exact alias hits in the query and in agent-supplied entities.
    let mut names: Vec<String> = extract::ngrams(&input.query, 4)
        .into_iter()
        .map(|x| x.0)
        .collect();
    names.extend(input.entities.iter().map(|x| text::norm_name(x)));
    names.sort();
    names.dedup();
    // Strong anchors block pass-through and enable prose hops; weak ones
    // (anchor_tiers) only seed and start beams.
    let anchors = crate::anchor::query_anchors(
        &engine.db,
        &input.query,
        &names,
        &engine.cfg.anchor_tiers,
        now,
    )?;
    let query_alias_entities: HashSet<i64> = anchors.all().into_iter().collect();
    for id in &query_alias_entities {
        seed_entity(*id, 1.0);
    }
    // (b) entity KNN against the query vector.
    {
        let mut st = engine.db.sql().prepare(&format!(
            "SELECT rowid,1-distance FROM entities_vec
             WHERE embedding MATCH {} AND k=10 ORDER BY distance",
            engine.db.vec_sql("?1")
        ))?;
        let hits: Vec<(i64, f32)> = st
            .query_map([engine.db.vec_blob(&query_vec)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        for (id, cosine) in hits {
            if cosine >= engine.cfg.entity_seed {
                seed_entity(id, f64::from(cosine));
            }
        }
    }
    // (c) relation-fact hits.
    let (relation_entities, relation_memories) = relation_seeds_with_config(
        &engine.db,
        &query_text,
        &query_vec,
        now,
        as_of,
        history,
        engine.cfg.fts_prefix_min,
        RelationSeedOptions {
            query_entities: &query_alias_entities,
            fact_entity_boost: engine.cfg.fact_entity_boost,
            fact_memory_seed: engine.cfg.fact_memory_seed,
            prefilter: engine.cfg.rel_prefilter,
        },
    )?;
    let relation_support = !relation_entities.is_empty() || !relation_memories.is_empty();
    for (id, weight) in relation_entities {
        seed_entity(id, weight);
    }
    // Specificity: one GROUP BY, not one query per seed.
    let seed_ids: Vec<i64> = ent_seeds.keys().copied().collect();
    let mut specificity: HashMap<i64, i64> = HashMap::new();
    for chunk in seed_ids.chunks(400) {
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql =
            format!("SELECT entity_id,COUNT(*) FROM mentions WHERE entity_id IN ({marks}) GROUP BY entity_id");
        let mut st = engine.db.sql().prepare(&sql)?;
        let mut rows = st.query(params_from_iter(chunk.iter()))?;
        while let Some(r) = rows.next()? {
            specificity.insert(r.get(0)?, r.get(1)?);
        }
    }
    for (id, weight) in ent_seeds.iter_mut() {
        let mentions = specificity.get(id).copied().unwrap_or(0);
        *weight /= mentions.max(1) as f64;
    }
    let mut mem_seed_weights: HashMap<i64, f64> = vec_ranked
        .iter()
        .take(engine.cfg.mem_seed_top)
        .map(|(id, cosine)| (*id, cosine.max(0.0)))
        .collect();
    for (id, weight) in relation_memories {
        mem_seed_weights
            .entry(id)
            .and_modify(|current| *current = current.max(weight))
            .or_insert(weight);
    }
    let mem_seeds: Vec<(i64, f64)> = mem_seed_weights.into_iter().collect();
    let ent_total: f64 = ent_seeds.values().sum();
    let mem_total: f64 = mem_seeds.iter().map(|(_, w)| w).sum();
    let mut seeds: Vec<(u32, f64)> = Vec::new();
    if ent_total > 0.0 {
        for (id, weight) in &ent_seeds {
            if let Some(&node) = snap.ent_node.get(id) {
                seeds.push((node, weight * (1.0 - engine.cfg.mem_seed_share) / ent_total));
            }
        }
    }
    if mem_total > 0.0 {
        let target = if ent_total > 0.0 {
            engine.cfg.mem_seed_share
        } else {
            1.0
        };
        for (id, weight) in &mem_seeds {
            if let Some(&node) = snap.mem_node.get(id) {
                seeds.push((node, weight * target / mem_total));
            }
        }
    }
    let ppr = graph::ppr_push(&snap.csr, &seeds, engine.cfg.alpha, engine.cfg.eps, 200_000);
    let mut graph_ids: Vec<i64> = Vec::new();
    let mut ppr_score: HashMap<i64, f64> = HashMap::new();
    for (node, score) in ppr {
        match snap.node_kind.get(node as usize) {
            Some(('m', id)) => graph_ids.push(*id),
            Some(('e', id)) => {
                ppr_score.insert(*id, score);
            }
            _ => {}
        }
    }
    graph_ids.truncate(fetch_depth);
    let missing: Vec<i64> = graph_ids
        .iter()
        .copied()
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    let graph_ids: Vec<i64> = graph_ids
        .into_iter()
        .filter(|id| {
            rows.get(id).is_some_and(|row| {
                eligible(row, as_of, history, input, since, until, now, planned_ok)
            })
        })
        .take(depth)
        .collect();

    // ------------------------------------------------------------ path
    // Query-directed beam search over relations. This is what answers "where
    // does the manager of X live": the destination memory shares no words with
    // the question, so no lexical or vector channel can find it. Seeds are the
    // exact query anchors only; fuzzy seeds would start paths elsewhere.
    let mut path_starts: Vec<i64> = anchors.all();
    path_starts.truncate(4);
    let blocking: Vec<i64> = path_starts
        .iter()
        .copied()
        .filter(|id| anchors.block.contains(id))
        .collect();
    // "What did X drink *before* Y" asks for the older state; a path from the
    // query anchors lands on the current one and buries the answer.
    // The semantic beam only walks facts the question recognises, so it can
    // also start from fuzzy seeds ("Noor's" -> Noor Haddad).
    let mut sem_starts = path_starts.clone();
    let mut fuzzy: Vec<(i64, f64)> = ent_seeds.iter().map(|(id, w)| (*id, *w)).collect();
    fuzzy.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    for (id, _) in fuzzy {
        if sem_starts.len() >= 4 {
            break;
        }
        if !sem_starts.contains(&id) {
            sem_starts.push(id);
        }
    }
    let vis = paths::Visibility {
        now,
        as_of,
        history,
    };
    // History questions ("whoever used to lead X") still need the second hop:
    // only the lexical beam (current-state term matching) stays off for them.
    let (lexical_hits, sem_hits) = if negated {
        (Vec::new(), Vec::new())
    } else {
        let lexical = if path_starts.is_empty() || history_query {
            Vec::new()
        } else {
            paths::search(&engine.db, &query_text, &path_starts, vis)?
        };
        let semantic = if engine.cfg.w_sempath > 0.0 {
            paths::search_semantic(
                &engine.db,
                &query_text,
                &query_vec,
                &sem_starts,
                &blocking,
                anchors.prose,
                &nearest_memories,
                vis,
                engine.cfg.hop_scorer == "on",
                engine.cfg.name_part_hop,
            )?
        } else {
            Vec::new()
        };
        (lexical, semantic)
    };
    let mut path_ids = paths::evidence(&engine.db, &lexical_hits)?;
    let mut sem_ids = paths::evidence(&engine.db, &sem_hits)?;
    let missing: Vec<i64> = path_ids
        .iter()
        .chain(&sem_ids)
        .copied()
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    for ids in [&mut path_ids, &mut sem_ids] {
        ids.retain(|id| {
            rows.get(id).is_some_and(|row| {
                eligible(row, as_of, history, input, since, until, now, planned_ok)
            })
        });
    }
    sem_ids.truncate(depth);
    let top_chains = paths::top_chains(&lexical_hits, &sem_hits);
    path_ids.truncate(depth);

    // ---------------------------------------------------------- causal
    // E4b: a why-question asks for the cause, so the evidence of a causal
    // relation is a first-class channel, not just another fact in the list.
    let mut causal_ids = if engine.cfg.w_causal > 0.0 && causal_query {
        paths::causal_memories(&engine.db, &query_vec, &query_text, vis, 12)?
    } else {
        Vec::new()
    };
    let missing: Vec<i64> = causal_ids
        .iter()
        .copied()
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    causal_ids.retain(|id| {
        rows.get(id)
            .is_some_and(|row| eligible(row, as_of, history, input, since, until, now, planned_ok))
    });
    causal_ids.truncate(depth);

    // ----------------------------------------------------------- tlink
    // E4a: the dense hits are anchors; memories that happened around the same
    // entity within a day are candidates even when they share no query words.
    let mut tlink_ids = if engine.cfg.w_tlink > 0.0 {
        temporal_link_ids(&engine.db, &vec_ranked, engine.cfg.mem_seed_top, depth)?
    } else {
        Vec::new()
    };
    let missing: Vec<i64> = tlink_ids
        .iter()
        .copied()
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    tlink_ids.retain(|id| {
        rows.get(id)
            .is_some_and(|row| eligible(row, as_of, history, input, since, until, now, planned_ok))
    });
    tlink_ids.truncate(depth);

    // ------------------------------------------------------- exclusion
    // The typed items without the property are the answer ("report-service is
    // written in Python" answers "which service is not in Rust"), so their
    // memories form a channel of their own, ranked by the query vector. Items
    // that do have the property are demoted below.
    let mut excl_ids: Vec<i64> = Vec::new();
    let mut blocked_ids: HashSet<i64> = HashSet::new();
    if let Some(ex) = &exclusion {
        let lacking_all = memories_mentioning(&engine.db, &ex.lacking)?;
        excl_ids = top_by_cosine(
            &engine.db,
            &lacking_all,
            &query_vec,
            depth.saturating_mul(4),
        )?;
        let lacking: HashSet<i64> = lacking_all.into_iter().collect();
        blocked_ids = memories_mentioning(&engine.db, &ex.with_property)?
            .into_iter()
            .filter(|id| !lacking.contains(id))
            .collect();
    }
    let missing: Vec<i64> = excl_ids
        .iter()
        .copied()
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    excl_ids.retain(|id| {
        rows.get(id)
            .is_some_and(|row| eligible(row, as_of, history, input, since, until, now, planned_ok))
    });
    excl_ids.truncate(depth);

    // ---------------------------------------------------------- status
    // H2-15: "what is still open" asks for the defects no later memory fixes.
    // Only the candidate rows are classified; the rule re-ranks, never hides.
    // A question that names a defect (`want` is not empty) is about that
    // defect: only its reports are ranked, and its own fix is not demoted.
    let status_want: HashSet<String> = if status_query {
        paths::status_question_terms(&input.query)
    } else {
        HashSet::new()
    };
    let status_specific = !status_want.is_empty();
    let (mut status_ids, status_closed) = if status_query {
        status_channels(engine, &status_want, &query_vec, &rows, now)?
    } else {
        (Vec::new(), HashSet::new())
    };
    status_ids.retain(|id| {
        rows.get(id)
            .is_some_and(|row| eligible(row, as_of, history, input, since, until, now, planned_ok))
    });

    let mut channels = vec![
        rank::Channel {
            name: "bm25",
            weight: engine.cfg.w_bm25,
            ids: &bm25,
        },
        rank::Channel {
            name: "vec",
            weight: engine.cfg.w_vector,
            ids: &vec_ids,
        },
        rank::Channel {
            name: "graph",
            weight: engine.cfg.w_graph,
            ids: &graph_ids,
        },
        rank::Channel {
            name: "time",
            weight: engine.cfg.w_temporal,
            ids: &time_ids,
        },
        rank::Channel {
            name: "path",
            weight: PATH_WEIGHT,
            ids: &path_ids,
        },
        rank::Channel {
            name: "sem",
            weight: engine.cfg.w_sempath,
            ids: &sem_ids,
        },
        rank::Channel {
            name: "causal",
            weight: engine.cfg.w_causal,
            ids: &causal_ids,
        },
        rank::Channel {
            name: "tlink",
            weight: engine.cfg.w_tlink,
            ids: &tlink_ids,
        },
        rank::Channel {
            name: "excl",
            weight: EXCLUSION_WEIGHT,
            ids: &excl_ids,
        },
        rank::Channel {
            name: "status",
            weight: STATUS_WEIGHT,
            ids: &status_ids,
        },
    ];
    // H2-16: a why-question asks for the cause of what answered the first
    // pass, so the seeds are the first-pass head, not the query.
    let (mut cause_ids, cause_relations) = if engine.cfg.cause_follow && causal_query {
        let seeds: Vec<i64> = rank::rrf(&channels, engine.cfg.rrf_k)
            .into_iter()
            .take(CAUSE_SEEDS)
            .map(|f| f.id)
            .collect();
        paths::cause_follow(&engine.db, &seeds, &anchors.all(), vis, 6)?
    } else {
        (Vec::new(), Vec::new())
    };
    let missing: Vec<i64> = cause_ids
        .iter()
        .copied()
        .filter(|id| !rows.contains_key(id))
        .collect();
    rows.extend(fetch_rows(&engine.db, &missing)?);
    cause_ids.retain(|id| {
        rows.get(id)
            .is_some_and(|row| eligible(row, as_of, history, input, since, until, now, planned_ok))
    });
    if !cause_ids.is_empty() {
        channels.push(rank::Channel {
            name: "cause",
            weight: CAUSE_WEIGHT,
            ids: &cause_ids,
        });
    }
    let fused = rank::rrf(&channels, engine.cfg.rrf_k);
    // E5: retention only nudges scores when the knob is on. Reads are batched
    // once for the whole candidate set; a zero weight never queries the table.
    let retention_scores: HashMap<i64, f64> = if engine.cfg.w_retention != 0.0 {
        let ids: Vec<i64> = fused.iter().map(|f| f.id).collect();
        let mut times: HashMap<i64, Vec<i64>> = HashMap::new();
        for chunk in ids.chunks(400) {
            let marks = std::iter::repeat_n("?", chunk.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql =
                format!("SELECT memory_id,at FROM memory_access WHERE memory_id IN ({marks})");
            let mut st = engine.db.sql().prepare(&sql)?;
            for row in st.query_map(params_from_iter(chunk.iter()), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            })? {
                let (id, at) = row?;
                times.entry(id).or_default().push(at);
            }
        }
        fused
            .iter()
            .filter_map(|f| {
                let row = rows.get(&f.id)?;
                let empty = Vec::new();
                let access = times.get(&f.id).unwrap_or(&empty);
                Some((f.id, rank::retention(row.4, row.8, access, now)))
            })
            .collect()
    } else {
        HashMap::new()
    };
    let cutoff = as_of.unwrap_or(now);
    // H2-15: a memory that matches the question better than every defect
    // report is an answer, not a near miss, and is never demoted.
    let best_bug = if status_query {
        fused
            .iter()
            .filter(|f| {
                rows.get(&f.id)
                    .is_some_and(|row| paths::is_bug_memory(&row.1))
            })
            .map(|f| f.score)
            .fold(0.0, f64::max)
    } else {
        0.0
    };
    let mut candidates = Vec::new();
    for f in fused {
        let Some(row) = rows.get(&f.id) else { continue };
        // H2-10: a calendar window in the question is a filter, not only a
        // boost. A row that states its own interval and misses the window is
        // not an answer to "what happened in August 2026"; a row dated only by
        // its ingest time has no stated interval and stays, ranked as before.
        if as_of.is_none()
            && window
                .as_ref()
                .is_some_and(|w| window_scope(w) && dated_outside(row, w, &clock))
        {
            continue;
        }
        let mut score = f.score;
        if is_ended(row, cutoff) {
            if !history {
                score *= engine.cfg.expired_penalty;
            } else if as_of.is_none() {
                let ended_pair = past_state_query
                    && memory_has_ended_anchor_relation(
                        engine,
                        f.id,
                        cutoff,
                        &query_alias_entities,
                        &input.query,
                    )?;
                score *= if past_state_query {
                    if ended_pair {
                        engine.cfg.history_boost.max(4.0)
                    } else {
                        1.0
                    }
                } else {
                    engine.cfg.history_boost
                };
            }
        }
        if window
            .as_ref()
            .is_some_and(|w| row.9 >= w.start && row.9 < w.end)
        {
            score *= 1.25;
        }
        let age = (now - row.9).max(0) as f64 / 86_400.0;
        score *= 0.9 + 0.1 * 2f64.powf(-age / engine.cfg.half_life_days);
        score *= 0.9 + 0.2 * row.4;
        if let Some(retention) = retention_scores.get(&f.id) {
            score *= 1.0 + engine.cfg.w_retention * retention;
        }
        if blocked_ids.contains(&f.id) {
            score *= EXCLUSION_PENALTY;
        }
        if status_query {
            if paths::is_bug_memory(&row.1) {
                // A general "what is still open" ranks fixed reports after the
                // open ones. A question that names one defect keeps its report
                // and its fix where the ordinary channels put them: halving
                // them would let unrelated open bugs answer for it.
                if !status_specific && status_closed.contains(&f.id) {
                    score *= STATUS_CLOSED_PENALTY;
                }
            } else if f.score < best_bug {
                // Only the memories that no defect report outranks: a plain
                // answer the question matches better keeps its place.
                score *= STATUS_NEUTRAL_PENALTY;
            }
        }
        candidates.push((f.id, score, f.ranks));
    }
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    // A question about what is *not* the case must not be answered with the
    // positive facts: keep only memories that state a negative themselves,
    // in their wording or as evidence of a negative relation.
    if negated {
        let mut st = engine.db.sql().prepare_cached(
            "SELECT 1 FROM relation_evidence e JOIN relations r ON r.id = e.relation_id
             WHERE e.memory_id = ?1 AND r.polarity = 0 LIMIT 1",
        )?;
        let mut keep = Vec::with_capacity(candidates.len());
        for c in candidates {
            let cue = rows
                .get(&c.0)
                .is_some_and(|row| paths::has_negation_cue(&row.1));
            if cue || st.exists([c.0])? {
                keep.push(c);
            }
        }
        candidates = keep;
    }
    // Abstention: no channel found anything to stand on, so say so instead of
    // returning the nearest unrelated memories. Dense similarity is the only
    // signal left, and on UltraBERT it barely separates "paraphrased but
    // stored" (>= 0.185 on the holdout) from "not stored" (<= 0.180 on
    // off-domain probes), so the threshold sits between them.
    let best_vector = vec_ranked.first().map(|(_, cosine)| *cosine).unwrap_or(0.0);
    let supported = !bm25.is_empty()
        || !ent_seeds.is_empty()
        || relation_support
        || !path_ids.is_empty()
        || !causal_ids.is_empty()
        || window.is_some()
        || best_vector >= ABSTAIN_COSINE;
    if !supported || candidates.is_empty() {
        return Ok(RecallOutput {
            memories: Vec::new(),
            facts: Vec::new(),
            entities: Vec::new(),
            next: Vec::new(),
            window: window.map(|w| (w.start, w.end, w.matched)),
            candidates: 0,
            tokens_used: 0,
            tokens_budget: budget,
            omitted: 0,
            // A window that filtered every candidate out is as empty as one
            // that overlapped nothing.
            window_empty,
            over_budget: false,
        });
    }
    // Cross-encoder: score (query, memory) jointly for the head of the fused
    // list and blend with the fused score. It reads text only, not validity,
    // so history / as-of questions (where the ended memory is the answer) keep
    // the fused order. A missing or broken model leaves the order untouched.
    if engine.cfg.rerank_top > 0 && !history && candidates.len() > 1 {
        let head = engine.cfg.rerank_top.min(candidates.len());
        let texts: Vec<&str> = candidates[..head]
            .iter()
            .map(|c| rows[&c.0].1.as_str())
            .collect();
        if let Ok(logits) = engine.reranker.score(&query_text, &texts) {
            candidates = rerank::blend(candidates, &logits, engine.cfg.w_rerank);
        }
    }
    let mmr_ids: Vec<i64> = candidates.iter().map(|x| x.0).take(3 * k).collect();
    let vecs = fetch_vectors(&engine.db, &mmr_ids)?;
    let mmr_input: Vec<(i64, f64)> = candidates.iter().map(|x| (x.0, x.1)).take(3 * k).collect();
    let selected = rank::mmr(&mmr_input, &vecs, engine.cfg.mmr_lambda, k);
    let costs: Vec<usize> = selected
        .iter()
        .map(|id| {
            rows.get(id)
                .map(|r| (r.1.len() + 12).div_ceil(4))
                .unwrap_or(0)
        })
        .collect();
    let kept = rank::pack(&costs, budget);
    let kept_ids: HashSet<i64> = kept.iter().map(|&i| selected[i]).collect();
    let mut memories = Vec::new();
    for id in &selected {
        let Some(row) = rows.get(id) else { continue };
        let ranks = candidates
            .iter()
            .find(|x| x.0 == *id)
            .map(|x| x.2.clone())
            .unwrap_or_default();
        let mut reasons: Vec<String> = ranks.into_iter().map(|(n, r)| format!("{n}#{r}")).collect();
        if window
            .as_ref()
            .is_some_and(|w| row.9 >= w.start && row.9 < w.end)
        {
            reasons.push("time".into());
        }
        if kept_ids.contains(id) {
            memories.push(MemoryHit {
                id: *id,
                kind: row.2.clone(),
                content: row.1.clone(),
                valid_from: row.9,
                valid_to: row.10,
                ended: is_ended(row, cutoff),
                superseded_by: row.12,
                tags: row.3.split_whitespace().map(str::to_string).collect(),
                score: candidates
                    .iter()
                    .find(|x| x.0 == *id)
                    .map(|x| x.1)
                    .unwrap_or(0.0),
                reasons,
                duration: duration_query.then(|| match row.10 {
                    Some(to) => temporal::duration(row.9, to, &clock),
                    None => format!("ongoing since {}", temporal::fmt_date(row.9, &clock)),
                }),
                maybe_superseded_by: None,
            });
        }
    }
    let tokens_used: usize = kept.iter().map(|&i| costs[i]).sum();
    let over_budget = tokens_used > budget;
    // E5: access is recorded for memories the caller actually receives
    // (`memories`, after packing), in one write transaction. The second
    // statement keeps only the most recent `access_keep` timestamps.
    let returned_memory_ids: Vec<i64> = memories.iter().map(|memory| memory.id).collect();
    if !returned_memory_ids.is_empty() {
        let _ = engine.db.write(|db| {
            let marks = std::iter::repeat_n("?", returned_memory_ids.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "UPDATE memories SET access_count=access_count+1,last_access=?1 WHERE id IN ({marks})"
            );
            let mut p: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
            p.extend(
                returned_memory_ids
                    .iter()
                    .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>),
            );
            db.sql().execute(
                &sql,
                rusqlite::params_from_iter(p.iter().map(|x| x.as_ref())),
            )?;
            let mut insert = db
                .sql()
                .prepare_cached("INSERT OR IGNORE INTO memory_access(memory_id,at) VALUES (?1,?2)")?;
            let mut trim = db.sql().prepare_cached(
                "DELETE FROM memory_access WHERE memory_id=?1 AND at <
                 (SELECT at FROM memory_access WHERE memory_id=?1
                  ORDER BY at DESC LIMIT 1 OFFSET ?2)",
            )?;
            for id in &returned_memory_ids {
                insert.execute(params![id, now])?;
                if engine.cfg.access_keep == 0 {
                    db.sql().execute(
                        "DELETE FROM memory_access WHERE memory_id=?1",
                        [id],
                    )?;
                } else {
                    trim.execute(params![id, engine.cfg.access_keep as i64 - 1])?;
                }
            }
            Ok(())
        });
    }
    let mut relevant_entities: HashSet<i64> = ent_seeds.keys().copied().collect();
    for chunk in returned_memory_ids.chunks(400) {
        if chunk.is_empty() {
            continue;
        }
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT DISTINCT entity_id FROM mentions WHERE memory_id IN ({marks})");
        let mut st = engine.db.sql().prepare(&sql)?;
        for entity_id in st.query_map(params_from_iter(chunk.iter()), |r| r.get::<_, i64>(0))? {
            relevant_entities.insert(entity_id?);
        }
    }
    let top_entities: Vec<i64> = ppr_score
        .iter()
        .filter(|(id, _)| relevant_entities.contains(id) && snap.ent_node.contains_key(id))
        .map(|(id, s)| (*id, *s))
        .sorted_by_score()
        .into_iter()
        .take(10)
        .map(|x| x.0)
        .collect();
    // A time window asks about past states too, so ended facts stay in view.
    let facts = facts_for(
        engine,
        &top_entities,
        history || since.is_some() || until.is_some(),
        cutoff,
        (since, until),
        8,
        Some(&ppr_score),
        Some(&query_vec),
    )?;
    // The last hop of the best paths is often the only place the answer
    // appears ("Liam Vijappa lives in Denver"), and that relation belongs to an
    // entity the neighbourhood search never reached. Render it explicitly.
    let existing_facts: HashSet<i64> = facts.iter().map(|fact| fact.id).collect();
    let (pulled, evidence_ids) = if engine.cfg.bridge_pull && !negated {
        bridge_pull_facts(
            engine,
            &query_text,
            &query_vec,
            &top_chains,
            &memories,
            &query_alias_entities,
            cutoff,
            &existing_facts,
        )?
    } else {
        (Vec::new(), Vec::new())
    };
    // The reverse hop reads the answer frame and walks back from the returned
    // bridges, so its facts lead: the terminal relation it finds is the one
    // the forward channels could not rank.
    let (rev_pulled, rev_evidence) = if engine.cfg.rev_hop && !negated {
        rev_hop_facts(
            engine,
            &query_text,
            &top_chains,
            &memories,
            &query_alias_entities,
            cutoff,
            &existing_facts,
        )?
    } else {
        (Vec::new(), Vec::new())
    };
    // The evidence memory of a pulled fact is a candidate in its own right
    // (same rule as `bridge_pull`): the fact line carries the answer, the
    // memory carries the context around it.
    if !rev_evidence.is_empty() {
        let rows_to_add = fetch_rows(&engine.db, &rev_evidence)?;
        for id in rev_evidence {
            if let Some(memory) = memories.iter_mut().find(|memory| memory.id == id) {
                if !memory.reasons.iter().any(|reason| reason == "revhop#1") {
                    memory.reasons.push("revhop#1".into());
                }
                continue;
            }
            let Some(row) = rows_to_add.get(&id) else {
                continue;
            };
            if !eligible(row, as_of, history, input, since, until, now, planned_ok) {
                continue;
            }
            memories.push(MemoryHit {
                id,
                kind: row.2.clone(),
                content: row.1.clone(),
                valid_from: row.9,
                valid_to: row.10,
                ended: is_ended(row, cutoff),
                superseded_by: row.12,
                tags: row.3.split_whitespace().map(str::to_string).collect(),
                score: 0.0,
                reasons: vec!["revhop#1".into()],
                duration: None,
                maybe_superseded_by: None,
            });
        }
    }
    if !evidence_ids.is_empty() {
        let rows_to_add = fetch_rows(&engine.db, &evidence_ids)?;
        for id in evidence_ids {
            if let Some(memory) = memories.iter_mut().find(|memory| memory.id == id) {
                if !memory.reasons.iter().any(|reason| reason == "bridge#1") {
                    memory.reasons.push("bridge#1".into());
                }
                continue;
            }
            let Some(row) = rows_to_add.get(&id) else {
                continue;
            };
            if !eligible(row, as_of, history, input, since, until, now, planned_ok) {
                continue;
            }
            memories.push(MemoryHit {
                id,
                kind: row.2.clone(),
                content: row.1.clone(),
                valid_from: row.9,
                valid_to: row.10,
                ended: is_ended(row, cutoff),
                superseded_by: row.12,
                tags: row.3.split_whitespace().map(str::to_string).collect(),
                score: 0.0,
                reasons: vec!["bridge#1".into()],
                duration: None,
                maybe_superseded_by: None,
            });
        }
    }
    let mut facts = if negated {
        facts
            .into_iter()
            .filter(|fact| fact.polarity == 0 || paths::has_negation_cue(&fact.fact))
            .collect()
    } else {
        let mut merged = pulled;
        if !top_chains.is_empty() {
            let prose = paths::prose_facts(&engine.db, &sem_hits, engine.cfg.prose_margin, cutoff)?
                .into_iter()
                .filter(|fact| {
                    rows.get(&fact.id).is_some_and(|row| {
                        eligible(row, as_of, history, input, since, until, now, planned_ok)
                    })
                });
            merged.extend(prose);
            for fact in paths::facts(&engine.db, &top_chains, cutoff, 8)? {
                if !merged.iter().any(|existing| existing.id == fact.id) {
                    merged.push(fact);
                }
            }
        }
        for fact in facts {
            if !merged.iter().any(|x| x.id == fact.id && !x.memory) {
                merged.push(fact);
            }
        }
        // The reverse hop is an addition, not a replacement: the forward list
        // keeps its own 8-fact budget (a chain fact is grounded by a returned
        // memory and a guess must never evict it), and the hop appends at most
        // `REV_HOP_FACTS` lines after it.
        merged.truncate(8);
        let fresh: Vec<FactHit> = rev_pulled
            .into_iter()
            .filter(|fact| !merged.iter().any(|existing| existing.id == fact.id))
            .take(REV_HOP_FACTS)
            .collect();
        merged.extend(fresh);
        merged
    };
    // H2-16: the cause the first-pass answer points at leads the facts. A
    // relation already rendered by a chain stays where it is, once.
    if !cause_relations.is_empty() {
        let known: HashSet<i64> = facts.iter().map(|fact| fact.id).collect();
        let mut extra: Vec<FactHit> = paths::facts_by_ids(&engine.db, &cause_relations, cutoff, 2)?
            .into_iter()
            .filter(|fact| (history || !fact.ended) && !known.contains(&fact.id))
            .collect();
        if !extra.is_empty() {
            extra.extend(facts);
            extra.truncate(8);
            facts = extra;
        }
    }
    // The facts that give the excluded property to a typed item are the wrong
    // answers ("auth-service is written in Rust" for "which service is not in
    // Rust"): keep them, but after the facts that answer the question.
    if let Some(ex) = &exclusion {
        if !ex.facts.is_empty() {
            let wrong: HashSet<i64> = ex.facts.iter().copied().collect();
            facts.sort_by_key(|fact| wrong.contains(&fact.id));
        }
    }
    // H2-14: the asked family adds (or re-renders in place) the rows the
    // question is phrased from, so an inverse question never loses the fact to
    // a canonical rendering. A negation question is left alone.
    if !negated {
        let asked = asked_family_facts(
            engine,
            &query_text,
            &query_vec,
            &query_alias_entities,
            history,
            cutoff,
            (since, until),
            2,
        )?;
        let mut fresh = Vec::new();
        for fact in asked {
            match facts.iter().position(|f| f.id == fact.id && !f.memory) {
                // Keep the position and the counted fields; only the side of
                // the sentence changes.
                Some(pos) => {
                    facts[pos].source = fact.source;
                    facts[pos].predicate = fact.predicate;
                    facts[pos].target = fact.target;
                }
                None if !facts.iter().any(|f| f.id == fact.id && f.memory) => fresh.push(fact),
                None => {}
            }
        }
        if !fresh.is_empty() {
            fresh.extend(facts);
            fresh.truncate(8);
            facts = fresh;
        }
    }
    // H3-11: a yes/no "still" question that names both ends of a relation gets
    // the ended relation between them, marked ended with its end date, so the
    // answer can say "no, ended <date>". Live rows never come from this path.
    if !negated {
        let ended = ended_pair_facts(engine, &query_text, &query_alias_entities, cutoff)?;
        let fresh: Vec<FactHit> = ended
            .into_iter()
            .filter(|f| !facts.iter().any(|x| x.id == f.id && !x.memory))
            .collect();
        if !fresh.is_empty() {
            let mut merged = fresh;
            merged.extend(facts);
            merged.truncate(8);
            facts = merged;
        }
    }
    // Ordering and duration questions want the intervals themselves, oldest
    // first, with the elapsed time spelled out. Other queries keep relevance
    // order.
    if duration_query {
        for fact in &mut facts {
            fact.duration = if fact.retracted {
                None
            } else {
                Some(match fact.valid_to {
                    Some(to) => temporal::duration(fact.valid_from, to, &clock),
                    None => format!(
                        "ongoing since {}",
                        temporal::fmt_date(fact.valid_from, &clock)
                    ),
                })
            };
        }
    }
    // An ordering question wants the intervals themselves, oldest first.
    // A duration question asks about one interval ("how long was the offsite"),
    // so it keeps relevance order: sorting it by date put unrelated older
    // memories above the answer.
    if ordering {
        memories.sort_by_key(|m| (m.valid_from, m.id));
        facts.sort_by_key(|f| (f.valid_from, f.id));
    }
    let entities = top_entities
        .iter()
        .take(3)
        .filter_map(|id| {
            let keep = engine
                .db
                .sql()
                .query_row(
                    "SELECT type,description FROM entities WHERE id=?1",
                    [id],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .ok()
                .is_some_and(|(kind, description)| {
                    relevant_entities.contains(id) && (!kind.is_empty() || !description.is_empty())
                });
            if keep || ent_seeds.contains_key(id) {
                Some(card(engine, *id))
            } else {
                None
            }
        })
        .collect::<Result<Vec<_>>>()?;
    // A chain the beams could not finish is handed to the caller as one
    // `follow` call: the bridge entity plus the relation the question wants.
    let next = if negated || history_query || as_of.is_some() || engine.cfg.hint_tau <= 0.0 {
        Vec::new()
    } else {
        let chains: Vec<&paths::PathHit> = sem_hits.iter().chain(&lexical_hits).collect();
        let mut seeds: Vec<(i64, f64)> = ent_seeds.iter().map(|(id, w)| (*id, *w)).collect();
        seeds.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        next_hops(
            engine,
            &query_text,
            &query_alias_entities,
            &sem_starts,
            &chains,
            &seeds,
            &facts,
            &memories,
            &vecs,
            now,
            engine.cfg.hint_tau,
        )?
    };
    if engine.cfg.correction_hints {
        for m in memories.iter_mut().filter(|m| !m.ended) {
            m.maybe_superseded_by =
                crate::correction::later_correction(&engine.db, m.id, &m.content, m.valid_from)?;
        }
    }
    Ok(RecallOutput {
        memories,
        facts,
        entities,
        next,
        window_empty,
        window: window.map(|w| (w.start, w.end, w.matched)),
        candidates: candidates.len(),
        tokens_used,
        tokens_budget: budget,
        omitted: selected.len().saturating_sub(kept.len()),
        over_budget,
    })
}

trait ScoreOrder {
    fn sorted_by_score(self) -> Vec<(i64, f64)>;
}
impl<I: Iterator<Item = (i64, f64)>> ScoreOrder for I {
    fn sorted_by_score(self) -> Vec<(i64, f64)> {
        let mut v: Vec<_> = self.collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        v
    }
}

fn card(engine: &Engine, id: i64) -> Result<EntityCard> {
    let (name, kind, desc): (String, String, String) = engine.db.sql().query_row(
        "SELECT name,type,description FROM entities WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let norm = text::norm_name(&name);
    let mut st = engine
        .db
        .sql()
        .prepare("SELECT alias_norm FROM entity_aliases WHERE entity_id=?1 AND alias_norm!=?2")?;
    let aliases = st
        .query_map(params![id, norm], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(EntityCard {
        id,
        name,
        kind,
        aliases,
        description: desc,
        community: None,
    })
}

/// "Alice, Bob, Carol +9": up to three names, then how many more there are.
fn label_of(names: &[String], total: usize) -> String {
    let shown = names.iter().take(3).cloned().collect::<Vec<_>>();
    let mut label = shown.join(", ");
    let rest = total.saturating_sub(shown.len());
    if rest > 0 {
        label.push_str(&format!(" +{rest}"));
    }
    label
}

/// The best-connected entities of a community: graph degree is mentions plus
/// live relations in and out, the same edges `kg::snapshot` projects.
fn community_names(engine: &Engine, community: i64) -> Result<Vec<String>> {
    let now = engine.clock().now;
    let mut st = engine.db.sql().prepare(
        "SELECT e.name
         FROM communities c JOIN entities e ON e.id=c.entity_id
         WHERE c.community=?1
         ORDER BY (SELECT COUNT(*) FROM mentions m WHERE m.entity_id=e.id)
                + (SELECT COUNT(*) FROM relations r
                    WHERE r.expired_at IS NULL AND (r.valid_to IS NULL OR r.valid_to>?2)
                      AND r.src=e.id)
                + (SELECT COUNT(*) FROM relations r
                    WHERE r.expired_at IS NULL AND (r.valid_to IS NULL OR r.valid_to>?2)
                      AND r.dst=e.id) DESC,
                  e.id
         LIMIT 3",
    )?;
    let rows = st
        .query_map(params![community, now], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(rows)
}

/// The community an entity belongs to, as shown on its card.
fn community_ref(engine: &Engine, id: i64) -> Result<Option<CommunityRef>> {
    let comm = cached_communities(engine)?;
    let Some(&community) = comm.get(&id) else {
        return Ok(None);
    };
    let names = community_names(engine, community)?;
    let size: i64 = engine.db.sql().query_row(
        "SELECT COUNT(*) FROM communities WHERE community=?1",
        [community],
        |r| r.get(0),
    )?;
    Ok(Some(CommunityRef {
        id: community,
        label: label_of(&names, size as usize),
        size: size as usize,
    }))
}

/// The community's most common live positive predicates.
fn community_predicates(engine: &Engine, community: i64) -> Result<Vec<String>> {
    let now = engine.clock().now;
    let mut st = engine.db.sql().prepare(
        "SELECT COALESCE(NULLIF(p.display,''),p.canonical) AS pred, COUNT(*) AS n
         FROM relations r
         JOIN communities cs ON cs.entity_id=r.src AND cs.community=?1
         JOIN communities cd ON cd.entity_id=r.dst AND cd.community=?1
         JOIN predicates p ON p.id=r.predicate_id
         WHERE r.expired_at IS NULL AND (r.valid_to IS NULL OR r.valid_to>?2)
           AND r.polarity=1
         GROUP BY pred ORDER BY n DESC, pred LIMIT 3",
    )?;
    let rows = st
        .query_map(params![community, now], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(rows)
}

/// Oldest memory start and youngest end (or start) covered by the community.
fn community_span(engine: &Engine, community: i64) -> Result<Option<(i64, i64)>> {
    let (start, end): (Option<i64>, Option<i64>) = engine.db.sql().query_row(
        "SELECT MIN(m.valid_from), MAX(COALESCE(m.valid_to,m.valid_from))
         FROM memories m
         JOIN mentions mn ON mn.memory_id=m.id
         JOIN communities c ON c.entity_id=mn.entity_id AND c.community=?1
         WHERE m.expired_at IS NULL",
        [community],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(start.zip(end))
}

#[allow(clippy::too_many_arguments)]
fn facts_for(
    engine: &Engine,
    ids: &[i64],
    history: bool,
    cutoff: i64,
    (since, until): (Option<i64>, Option<i64>),
    limit: usize,
    ppr: Option<&HashMap<i64, f64>>,
    query_vec: Option<&[f32]>,
) -> Result<Vec<FactHit>> {
    facts_for_view(
        engine,
        ids,
        history,
        cutoff,
        (since, until),
        limit,
        ppr,
        query_vec,
        None,
    )
}

/// Same as `facts_for` with a per-side view: `viewer` is the entity whose card
/// or profile the facts are rendered for, so inverse relations ("A manages B"
/// stored after "B reports to A") and symmetric relations (married to, stored
/// once in (min,max) order) are rendered with the viewer as subject.
#[allow(clippy::too_many_arguments)]
fn facts_for_view(
    engine: &Engine,
    ids: &[i64],
    history: bool,
    cutoff: i64,
    (since, until): (Option<i64>, Option<i64>),
    limit: usize,
    ppr: Option<&HashMap<i64, f64>>,
    query_vec: Option<&[f32]>,
    viewer: Option<i64>,
) -> Result<Vec<FactHit>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let marks = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut st = engine.db.sql().prepare(&format!(
        "SELECT r.id,r.src,r.dst,
            COALESCE(NULLIF(r.predicate_text,''),NULLIF(p.display,''),p.canonical),
            (SELECT COALESCE(NULLIF(ip.display,''),ip.canonical)
             FROM predicates ip WHERE ip.id=p.inverse_of),
            p.symmetric,r.fact,r.valid_from,r.valid_to,r.expired_at,e.name,d.name,
            r.polarity,COALESCE(NULLIF(p.group_id,0),p.id),r.ended_by,r.subj,
            (SELECT nb.fact FROM relations nb WHERE nb.id=r.ended_by)
         FROM relations r JOIN predicates p ON p.id=r.predicate_id
         JOIN entities e ON e.id=r.src JOIN entities d ON d.id=r.dst
         WHERE (r.src IN ({marks}) OR r.dst IN ({marks})) AND r.src != r.dst"
    ))?;
    let mut out = Vec::new();
    let mut sql_params: Vec<Box<dyn rusqlite::ToSql>> = ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>)
        .collect();
    sql_params.extend(
        ids.iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>),
    );
    for x in st.query_map(
        rusqlite::params_from_iter(sql_params.iter().map(|x| x.as_ref())),
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<i64>>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, i64>(12)?,
                r.get::<_, i64>(13)?,
                r.get::<_, Option<i64>>(14)?,
                r.get::<_, i64>(15)?,
                r.get::<_, Option<String>>(16)?,
            ))
        },
    )? {
        let (
            id,
            src,
            dst,
            pred,
            inverse_pred,
            symmetric,
            fact,
            vf,
            vt,
            expired,
            source,
            target,
            polarity,
            group,
            ended_by,
            subj,
            by_fact,
        ) = x?;
        if vf > cutoff {
            continue;
        }
        if !history && (expired.is_some() || vt.is_some_and(|v| cutoff >= v)) {
            continue;
        }
        if until.is_some_and(|u| vf >= u) || since.is_some_and(|s| vt.is_some_and(|t| t <= s)) {
            continue;
        }
        // From the other side of an inverse or symmetric relation the fact is
        // rendered with the viewer as subject and the inverse phrase.
        let other_side = viewer == Some(dst) && (inverse_pred.is_some() || symmetric != 0);
        let (source, target, predicate) = if other_side {
            (
                target.clone(),
                source.clone(),
                inverse_pred.clone().unwrap_or(pred.clone()),
            )
        } else {
            (source.clone(), target.clone(), pred.clone())
        };
        let cosine = if let Some(query) = query_vec {
            engine
                .db
                .sql()
                .query_row(
                    "SELECT embedding FROM relations_vec WHERE rowid=?1",
                    [id],
                    |r| r.get::<_, Vec<u8>>(0),
                )
                .optional()?
                .and_then(|blob| engine.db.blob_to_vec(&blob))
                .map(|v| rank::cosine(query, &v))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        // The count is only needed for the rows that survive the limit below.
        let counted = polarity == 1 && expired.is_none() && vt.is_none_or(|v| v > cutoff);
        out.push((
            ppr.map_or(0.0, |scores| {
                scores.get(&src).copied().unwrap_or(0.0) + scores.get(&dst).copied().unwrap_or(0.0)
            }) + f64::from(cosine),
            FactHit {
                id,
                source,
                predicate,
                target,
                fact,
                valid_from: vf,
                valid_to: vt,
                ended: expired.is_some() || vt.is_some_and(|v| v <= cutoff),
                retracted: expired.is_some() && vt.is_none(),
                polarity,
                memory: false,
                current_values: None,
                replaced_by: ended_by.zip(by_fact),
                duration: None,
            },
            counted.then_some((if subj == 0 { src } else { subj }, group)),
        ));
    }
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.truncate(limit);
    out.into_iter()
        .map(|(_, mut fact, key)| {
            if let Some((subj, group)) = key {
                // Several current values are only worth a marker when the
                // predicate is single-valued (seeded exclusive, or a group an
                // agent flagged): on "depends on" or "uses" two live values
                // are ordinary and the marker read like a conflict warning.
                let (n, single): (i64, i64) = engine.db.sql().query_row(
                    "SELECT count(*),
                            (SELECT COALESCE(MAX(exclusive),0) FROM predicates
                             WHERE id=?2 OR group_id=?2)
                     FROM relations WHERE subj=?1 AND predicate_id IN
                         (SELECT id FROM predicates WHERE group_id=?2) AND polarity=1
                     AND expired_at IS NULL AND valid_from<=?3 AND (valid_to IS NULL OR valid_to>?3)",
                    params![subj, group, cutoff],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                fact.current_values = (n >= 2 && single != 0).then_some(n as usize);
            }
            Ok(fact)
        })
        .collect()
}

// ------------------------------------------------------- asked predicate
//
// The seed channels above never consult `predicate_aliases`, so a question
// phrased from the inverse side of a stored row ("who does Mira report to",
// "who is Mira's manager") surfaced no fact when the only row was "Noah
// manages Mira" (H2-14). This channel reads the asked family from the
// question and renders its live rows from the named person's side.

/// Seeded predicate groups a question can ask about by role or by phrasing.
const ASKED_GROUPS: &[&str] = &["manage", "mentor", "work at"];

/// Single-word role nouns and the seeded group they ask about; the `bool` is
/// true when that role is the canonically stored source ("manager" is stored
/// as `manager manages report`). Read-side only: a bare role noun is
/// write-direction-ambiguous ("Liam is Maria's manager" vs "Noah's manager is
/// Mira"), so it never joins `predicate_aliases`.
const ROLE_SURFACES: &[(&str, &str, bool)] = &[
    ("manager", "manage", true),
    ("boss", "manage", true),
    ("supervisor", "manage", true),
    ("mentor", "mentor", true),
    ("mentee", "mentor", false),
];

/// A question's asked family: which predicate group, which phrase named it and
/// how that phrase states the relation.
struct AskedRelation {
    group: i64,
    /// Byte span of the phrase that named the family.
    surface: (usize, usize),
    /// The phrase's grammatical subject is the canonical src (`manages`) rather
    /// than the canonical dst (`reports to`, `mentored by`).
    active: bool,
    /// A role noun: the canonical side of the role itself (the answer side).
    role: Option<bool>,
}

#[derive(Clone, Copy, PartialEq)]
enum NamedSide {
    Src,
    Dst,
}

fn query_word_spans(query: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in query.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                out.push((s, i));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s, query.len()));
    }
    out
}

/// The folded word before `start`, skipping punctuation-only gaps.
fn preceding_word(query: &str, start: usize) -> Option<String> {
    let head = query[..start].trim_end();
    let word = head
        .rsplit(|c: char| c.is_whitespace() || matches!(c, '?' | '!' | ',' | '.' | ';' | ':'))
        .find(|w| !w.is_empty())?;
    Some(text::fold(word))
}

/// Folded word prefixes that can name an asked family: the role nouns plus
/// every seeded verb surface of `manage`, `mentor` and `work at`. A question
/// without one cannot match, so it costs no SQL at all.
const ASKED_CUE_PREFIXES: &[&str] = &[
    "manage",
    "boss",
    "supervis",
    "report",
    "accountab",
    "mentor",
    "mentee",
    "work",
    "employ",
];

fn has_asked_cue(query: &str) -> bool {
    text::tokens(query)
        .iter()
        .any(|word| ASKED_CUE_PREFIXES.iter().any(|cue| word.starts_with(cue)))
}

fn asked_relation(engine: &Engine, query: &str) -> Result<Option<AskedRelation>> {
    if !has_asked_cue(query) {
        return Ok(None);
    }
    let words = query_word_spans(query);
    if words.is_empty() {
        return Ok(None);
    }
    // A role noun is checked first: "Rui's mentor" also matches the `mentor`
    // verb alias, and only the role reading makes the named person the mentee.
    for (s, e) in &words {
        let Some(word) = text::tokens(&query[*s..*e]).into_iter().next() else {
            continue;
        };
        let word = text::singularize(&word);
        let Some(&(_, group_name, answer_src)) =
            ROLE_SURFACES.iter().find(|(role, _, _)| *role == word)
        else {
            continue;
        };
        let group: Option<i64> = engine
            .db
            .sql()
            .prepare_cached("SELECT id FROM predicates WHERE canonical=?1")?
            .query_row([group_name], |r| r.get(0))
            .optional()?;
        if let Some(group) = group {
            return Ok(Some(AskedRelation {
                group,
                surface: (*s, *e),
                active: true,
                role: Some(answer_src),
            }));
        }
    }
    // Then a seeded predicate phrase: "report(s) to", "accountable to",
    // "works at", "mentored by". Every 1-4-word window goes into one lookup,
    // longest first so "accountable to" beats a bare "to".
    let mut windows: Vec<((usize, usize), String)> = Vec::new();
    for n in (1..=4.min(words.len())).rev() {
        for w in 0..=words.len() - n {
            let span = (words[w].0, words[w + n - 1].1);
            let norm = text::norm_predicate(&query[span.0..span.1]);
            if !norm.is_empty() {
                windows.push((span, norm));
            }
        }
    }
    if windows.is_empty() {
        return Ok(None);
    }
    let mut norms: Vec<&str> = windows.iter().map(|(_, norm)| norm.as_str()).collect();
    norms.sort_unstable();
    norms.dedup();
    let marks = std::iter::repeat_n("?", norms.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT a.alias_norm, COALESCE(NULLIF(p.group_id,0),p.id),
                COALESCE(g.canonical,p.canonical), p.inverse_of, p.id
         FROM predicate_aliases a
         JOIN predicates p ON p.id=a.predicate_id
         LEFT JOIN predicates g ON g.id=p.group_id
         WHERE a.alias_norm IN ({marks}) AND p.seeded=1"
    );
    let mut st = engine.db.sql().prepare_cached(&sql)?;
    let mut seeded: HashMap<String, (i64, String, Option<i64>, i64)> = HashMap::new();
    for row in st.query_map(params_from_iter(norms.iter()), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, i64>(4)?,
        ))
    })? {
        let (norm, group, group_name, inverse, id) = row?;
        seeded.insert(norm, (group, group_name, inverse, id));
    }
    for (span, norm) in windows {
        let Some((group, group_name, inverse, id)) = seeded.get(&norm) else {
            continue;
        };
        if !ASKED_GROUPS.contains(&group_name.as_str()) {
            continue;
        }
        // A write of this phrase is stored with its source as dst when the
        // inverse row has the lower id; then the phrase's grammatical subject
        // is the canonical dst.
        let active = inverse.is_none_or(|inverse| inverse > *id);
        return Ok(Some(AskedRelation {
            group: *group,
            surface: span,
            active,
            role: None,
        }));
    }
    Ok(None)
}

/// The query entities named by an exact alias, with the longest mention's byte
/// span. A role anchor resolved through a relation (`anchor_tiers` "2") is not
/// named in the text and is dropped here.
fn anchor_mentions(
    engine: &Engine,
    query: &str,
    anchors: &HashSet<i64>,
) -> Result<Vec<(i64, usize, usize)>> {
    if anchors.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = anchors.iter().copied().take(64).collect();
    let marks = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut st = engine.db.sql().prepare(&format!(
        "SELECT alias_norm,entity_id FROM entity_aliases WHERE entity_id IN ({marks})"
    ))?;
    let mut by_name: HashMap<String, Vec<i64>> = HashMap::new();
    for row in st.query_map(params_from_iter(ids.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })? {
        let (name, id) = row?;
        by_name.entry(name).or_default().push(id);
    }
    let mut best: HashMap<i64, (usize, usize)> = HashMap::new();
    for (norm, start, end) in extract::ngrams(query, 4) {
        let Some(found) = by_name.get(&norm) else {
            continue;
        };
        for id in found {
            let better = match best.get(id) {
                Some(&(bs, be)) => {
                    (end - start) > (be - bs) || ((end - start) == (be - bs) && start < bs)
                }
                None => true,
            };
            if better {
                best.insert(*id, (start, end));
            }
        }
    }
    let mut out: Vec<(i64, usize, usize)> = best
        .into_iter()
        .map(|(id, (start, end))| (id, start, end))
        .collect();
    out.sort_by_key(|(id, start, end)| (*start, *end, *id));
    Ok(out)
}

/// The person a bare or possessive first name names ("Mira's manager") when
/// no exact alias matched: the one multi-word entity a single token names
/// (the anchor-tier 1 rule). Bounded to words next to the asked phrase.
fn asked_mention_fallback(
    engine: &Engine,
    query: &str,
    asked: &AskedRelation,
) -> Result<Option<(i64, usize, usize)>> {
    let words = query_word_spans(query);
    let Some(at) = words
        .iter()
        .position(|(start, end)| *start <= asked.surface.0 && asked.surface.0 < *end)
    else {
        return Ok(None);
    };
    let end_at = words
        .iter()
        .position(|(start, end)| *start < asked.surface.1 && asked.surface.1 <= *end)
        .unwrap_or(at);
    for distance in 1..=3 {
        let mut candidates = Vec::new();
        if at >= distance {
            candidates.push(at - distance);
        }
        if end_at + distance < words.len() {
            candidates.push(end_at + distance);
        }
        for index in candidates {
            let (start, end) = words[index];
            let token = text::norm_name(&query[start..end]);
            if token.chars().count() < 3 || text::is_stopword(&token) || filler(&token) {
                continue;
            }
            if let Some(id) = crate::anchor::unique_name_token(&engine.db, &token)? {
                return Ok(Some((id, start, end)));
            }
        }
    }
    Ok(None)
}

/// Words that may sit between a person and the role word that belongs to them
/// ("Mira's manager", "the mentor of Mira") without breaking the phrase.
fn filler(word: &str) -> bool {
    matches!(
        word,
        "of" | "the"
            | "a"
            | "an"
            | "is"
            | "was"
            | "are"
            | "were"
            | "and"
            | "&"
            | "to"
            | "for"
            | "at"
            | "by"
            | "with"
            | "my"
            | "his"
            | "her"
            | "their"
            | "our"
            | "your"
            | "its"
    )
}

/// Which canonical side of the asked family a mention sits on, and whether the
/// fact should be rendered from the mention's side (`before`: the mention is
/// the phrase's subject, so it leads the sentence).
fn named_side(
    asked: &AskedRelation,
    query: &str,
    start: usize,
    end: usize,
) -> Option<(NamedSide, bool)> {
    let before = end <= asked.surface.0;
    let after = start >= asked.surface.1;
    if let Some(answer_src) = asked.role {
        // Only the mention the role word belongs to ("Rui's mentor", "the
        // mentor of Rui"); a role word in another clause is not this question.
        let gap = if before {
            &query[end..asked.surface.0]
        } else if after {
            &query[asked.surface.1..start]
        } else {
            return None;
        };
        if text::tokens(gap).iter().any(|word| !filler(word)) {
            return None;
        }
        // A bare role word is a verb when an auxiliary makes the named person
        // its subject ("who does Marta mentor").
        let aux = preceding_word(query, start)
            .is_some_and(|word| matches!(word.as_str(), "do" | "does" | "did"));
        let side = if aux == answer_src {
            NamedSide::Src
        } else {
            NamedSide::Dst
        };
        return Some((side, before));
    }
    if !(before || after) {
        return None;
    }
    let side = match (asked.active, before) {
        (true, true) | (false, false) => NamedSide::Src,
        (true, false) | (false, true) => NamedSide::Dst,
    };
    Some((side, before))
}

/// A yes/no question about the present ("is npm still used", "are they
/// currently on call"): the status cue the ended-pair channel needs.
fn status_yes_no(query: &str) -> bool {
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .collect();
    matches!(
        words.first().map(String::as_str),
        Some("is" | "are" | "was" | "were" | "does" | "do" | "did" | "has" | "have")
    ) && words
        .iter()
        .any(|w| matches!(w.as_str(), "still" | "anymore" | "currently"))
}

/// H3-11: the ended relations between two entities the question names, most
/// recent first (at most two). Live rows are excluded by the end condition.
fn ended_pair_facts(
    engine: &Engine,
    query: &str,
    anchors: &HashSet<i64>,
    cutoff: i64,
) -> Result<Vec<FactHit>> {
    if !status_yes_no(query) {
        return Ok(Vec::new());
    }
    let mut ids: Vec<i64> = anchor_mentions(engine, query, anchors)?
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() < 2 {
        return Ok(Vec::new());
    }
    let marks = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT r.id FROM relations r
         WHERE r.polarity=1 AND r.src IN ({marks}) AND r.dst IN ({marks})
           AND (r.expired_at IS NOT NULL OR (r.valid_to IS NOT NULL AND r.valid_to<=?))
         ORDER BY r.id DESC LIMIT 2"
    );
    let mut bind: Vec<i64> = ids.clone();
    bind.extend(ids.iter().copied());
    bind.push(cutoff);
    let mut st = engine.db.sql().prepare(&sql)?;
    let rel_ids: Vec<i64> = st
        .query_map(params_from_iter(bind.iter()), |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    paths::facts_by_ids(&engine.db, &rel_ids, cutoff, 2)
}

/// Live rows of the asked family touching a named entity on the side the
/// question asks about, rendered from that entity's side. At most `limit`.
#[allow(clippy::too_many_arguments)]
fn asked_family_facts(
    engine: &Engine,
    query: &str,
    query_vec: &[f32],
    anchors: &HashSet<i64>,
    history: bool,
    cutoff: i64,
    (since, until): (Option<i64>, Option<i64>),
    limit: usize,
) -> Result<Vec<FactHit>> {
    let Some(asked) = asked_relation(engine, query)? else {
        return Ok(Vec::new());
    };
    let mut mentions = anchor_mentions(engine, query, anchors)?;
    // A bare or possessive first name ("Mira's manager") is not an exact alias,
    // so the single-entity token rule adds it; another anchor elsewhere in the
    // question must not hide it. Rendering still needs the mention to sit on
    // the asked side of a live row, so an extra mention cannot invent a fact.
    if let Some((id, start, end)) = asked_mention_fallback(engine, query, &asked)? {
        if !mentions.iter().any(|(known, _, _)| *known == id) {
            mentions.push((id, start, end));
        }
    }
    if mentions.is_empty() {
        return Ok(Vec::new());
    }
    let mut ids: Vec<i64> = mentions.iter().map(|(id, _, _)| *id).collect();
    ids.sort_unstable();
    ids.dedup();
    let marks = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT r.id,r.src,r.dst,
            COALESCE(NULLIF(r.predicate_text,''),NULLIF(p.display,''),p.canonical),
            (SELECT COALESCE(NULLIF(ip.display,''),ip.canonical)
             FROM predicates ip WHERE ip.id=p.inverse_of),
            p.symmetric,r.fact,r.valid_from,r.valid_to,r.expired_at,e.name,d.name,
            r.polarity,r.ended_by,
            (SELECT nb.fact FROM relations nb WHERE nb.id=r.ended_by)
         FROM relations r JOIN predicates p ON p.id=r.predicate_id
         JOIN entities e ON e.id=r.src JOIN entities d ON d.id=r.dst
         WHERE COALESCE(NULLIF(p.group_id,0),p.id)=?1 AND r.polarity=1
           AND (r.src IN ({marks}) OR r.dst IN ({marks}))"
    );
    let mut st = engine.db.sql().prepare(&sql)?;
    let mut sql_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(asked.group)];
    for id in ids.iter().chain(ids.iter()) {
        sql_params.push(Box::new(*id));
    }
    let mut candidates = Vec::new();
    for row in st.query_map(
        rusqlite::params_from_iter(sql_params.iter().map(|x| x.as_ref())),
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<i64>>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, i64>(12)?,
                r.get::<_, Option<i64>>(13)?,
                r.get::<_, Option<String>>(14)?,
            ))
        },
    )? {
        candidates.push(row?);
    }
    let mut ranked: Vec<(f64, FactHit)> = Vec::new();
    for (
        id,
        src,
        dst,
        pred,
        inverse_pred,
        symmetric,
        fact,
        vf,
        vt,
        expired,
        source,
        target,
        polarity,
        ended_by,
        by_fact,
    ) in candidates
    {
        if vf > cutoff {
            continue;
        }
        if !history && (expired.is_some() || vt.is_some_and(|v| cutoff >= v)) {
            continue;
        }
        if until.is_some_and(|u| vf >= u) || since.is_some_and(|s| vt.is_some_and(|t| t <= s)) {
            continue;
        }
        // Prefer the mention that is the phrase's subject (it leads the
        // rendered sentence); any on-side mention will do otherwise.
        let mut viewer: Option<i64> = None;
        let mut viewer_before = false;
        for (anchor, start, end) in &mentions {
            let Some((side, before)) = named_side(&asked, query, *start, *end) else {
                continue;
            };
            let on_side = match side {
                NamedSide::Src => src == *anchor,
                NamedSide::Dst => dst == *anchor,
            };
            if on_side && (viewer.is_none() || (before && !viewer_before)) {
                viewer = Some(*anchor);
                viewer_before = before;
            }
        }
        let Some(viewer) = viewer else {
            continue;
        };
        let cosine = engine
            .db
            .sql()
            .query_row(
                "SELECT embedding FROM relations_vec WHERE rowid=?1",
                [id],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .and_then(|blob| engine.db.blob_to_vec(&blob))
            .map(|v| f64::from(rank::cosine(query_vec, &v)))
            .unwrap_or(0.0);
        // From the other side only when the named person leads the sentence;
        // "who manages Mira" keeps the canonical "Noah manages Mira".
        let other_side =
            viewer_before && viewer == dst && (inverse_pred.is_some() || symmetric != 0);
        let (source, target, predicate) = if other_side {
            (
                target.clone(),
                source.clone(),
                inverse_pred.clone().unwrap_or(pred.clone()),
            )
        } else {
            (source.clone(), target.clone(), pred.clone())
        };
        let hit = FactHit {
            id,
            source,
            predicate,
            target,
            fact,
            valid_from: vf,
            valid_to: vt,
            ended: expired.is_some() || vt.is_some_and(|v| v <= cutoff),
            retracted: expired.is_some() && vt.is_none(),
            polarity,
            memory: false,
            current_values: None,
            replaced_by: ended_by.zip(by_fact),
            duration: None,
        };
        ranked.push((cosine, hit));
    }
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| b.1.id.cmp(&a.1.id)));
    Ok(ranked.into_iter().take(limit).map(|(_, hit)| hit).collect())
}

/// Resolve `e<id>`, an exact alias, or (as a last resort) the nearest entity
/// card. Shared with the graph tools so they resolve names like `entity` does.
pub(crate) fn resolve_entity(engine: &Engine, name: &str) -> Result<i64> {
    crate::engine::parse_id(name, 'e')
        .or_else(|| {
            let n = text::norm_name(name);
            engine
                .db
                .sql()
                .query_row(
                    "SELECT entity_id FROM entity_aliases WHERE alias_norm=?1 LIMIT 1",
                    [&n],
                    |r| r.get(0),
                )
                .ok()
        })
        .map(Ok)
        .unwrap_or_else(|| lookup_entity_fuzzy(engine, name))
}

pub fn entity(engine: &Engine, name: &str, limit: usize) -> Result<EntityOutput> {
    let limit = limit.min(200);
    let id = resolve_entity(engine, name)?;
    let (homonyms, homonym_total) = first_name_homonyms(engine, name, id)?;
    let ambiguous = homonym_total >= 2;
    let (candidates, candidates_more) = if ambiguous {
        let more = homonym_total.saturating_sub(homonyms.len());
        (homonyms, (more > 0).then_some(more))
    } else {
        (entity_candidates(engine, name, id)?, None)
    };
    let now = engine.clock().now;
    let mut card = card(engine, id)?;
    card.community = community_ref(engine, id)?;
    let all = facts_for_view(
        engine,
        &[id],
        true,
        now,
        (None, None),
        limit.min(200),
        None,
        None,
        Some(id),
    )?;
    let current: Vec<_> = all.iter().filter(|f| !f.ended).cloned().collect();
    let past: Vec<_> = all
        .into_iter()
        .filter(|f| !current.iter().any(|x| x.id == f.id))
        .collect();
    let mut st = engine.db.sql().prepare(
        "SELECT CASE WHEN a=?1 THEN b ELSE a END, score FROM similar WHERE a=?1 OR b=?1",
    )?;
    let similar = st
        .query_map([id], |r| {
            let other: i64 = r.get(0)?;
            let score: f32 = r.get(1)?;
            let name: String = engine.db.sql().query_row(
                "SELECT name FROM entities WHERE id=?1",
                [other],
                |x| x.get(0),
            )?;
            Ok((other, name, score))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut ts = engine.db.sql().prepare("SELECT id,kind,content,valid_from,valid_to,expired_at,superseded_by,tags,importance FROM memories WHERE id IN (SELECT memory_id FROM mentions WHERE entity_id=?1) AND NOT (expired_at IS NOT NULL AND superseded_by IS NULL) ORDER BY valid_from DESC LIMIT ?2")?;
    let timeline = ts
        .query_map(params![id, limit.max(1) as i64], |r| {
            let valid_to: Option<i64> = r.get(4)?;
            let expired: Option<i64> = r.get(5)?;
            Ok(MemoryHit {
                id: r.get(0)?,
                kind: r.get(1)?,
                content: r.get(2)?,
                valid_from: r.get(3)?,
                valid_to,
                ended: expired.is_some() || valid_to.is_some_and(|x| x <= now),
                superseded_by: r.get(6)?,
                tags: r
                    .get::<_, String>(7)?
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
                score: 0.0,
                reasons: vec![],
                duration: None,
                maybe_superseded_by: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let limit = limit.min(200);
    Ok(EntityOutput {
        card,
        current_facts: current,
        past_facts: past.into_iter().take(limit).collect(),
        similar,
        candidates,
        candidates_more,
        status: ambiguous.then(|| "ambiguous".to_string()),
        timeline,
    })
}

/// H2-13/V2/V3: every entity the bare `name` can mean: the people whose first
/// name it is, plus any entity named exactly that (a project or tool called
/// "Jasmine" is a homonym of the people, not invisible). The resolved card
/// `chosen` comes first, then the most-mentioned others, capped at
/// `HOMONYM_LIMIT`; the count is the full total so the caller can say
/// "and N more". Empty for a multi-word name, which names one entity.
fn first_name_homonyms(
    engine: &Engine,
    name: &str,
    chosen: i64,
) -> Result<(Vec<EntityCandidate>, usize)> {
    let n = text::norm_name(name);
    if n.is_empty() || n.contains(' ') {
        return Ok((Vec::new(), 0));
    }
    let persons = crate::resolve::PERSON_TYPES
        .iter()
        .map(|t| format!("'{t}'"))
        .collect::<Vec<_>>()
        .join(",");
    let filter = format!(
        "(norm=?1 OR substr(norm,1,length(?1)+1)=?1||' ')
         AND (norm=?1 OR type IN ({persons}))"
    );
    let total: i64 = engine.db.sql().query_row(
        &format!("SELECT COUNT(*) FROM entities WHERE {filter}"),
        [&n],
        |r| r.get(0),
    )?;
    let mut st = engine.db.sql().prepare(&format!(
        "SELECT id, name, type, description FROM entities
         WHERE {filter}
         ORDER BY (id=?2) DESC, mention_count DESC, id LIMIT {HOMONYM_LIMIT}"
    ))?;
    let rows = st.query_map(rusqlite::params![&n, chosen], |r| {
        Ok(EntityCandidate {
            id: r.get(0)?,
            name: r.get(1)?,
            kind: r.get(2)?,
            description: r.get(3)?,
            score: None,
        })
    })?;
    let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((rows, usize::try_from(total).unwrap_or(0)))
}

/// V3: a very common first name lists too many homonyms; the card shows 10 and
/// reports the rest as a count.
const HOMONYM_LIMIT: usize = 10;

/// H2-13: the other people a name can mean. A bare first name (`Maya`) or a
/// name prefix (`Maya L`) that is not the exact alias of one entity matches
/// every entity whose name starts with the same token; the best card still
/// wins, but the others must be visible so the caller does not answer about
/// the wrong person. Exact full-name lookups return nothing.
fn entity_candidates(engine: &Engine, name: &str, chosen: i64) -> Result<Vec<EntityCandidate>> {
    let n = text::norm_name(name);
    if n.is_empty() {
        return Ok(Vec::new());
    }
    let exact: i64 = engine.db.sql().query_row(
        "SELECT COUNT(DISTINCT entity_id) FROM entity_aliases WHERE alias_norm=?1",
        [&n],
        |r| r.get(0),
    )?;
    if exact == 1 {
        return Ok(Vec::new());
    }
    let mut st = engine.db.sql().prepare(
        "SELECT DISTINCT e.id, e.name, e.type, e.description
         FROM entity_aliases a JOIN entities e ON e.id=a.entity_id
         WHERE (a.alias_norm=?1 OR substr(a.alias_norm,1,length(?1)+1)=?1||' ')
           AND e.id<>?2
         ORDER BY e.mention_count DESC, e.id LIMIT 5",
    )?;
    let rows = st.query_map(params![n, chosen], |r| {
        Ok(EntityCandidate {
            id: r.get(0)?,
            name: r.get(1)?,
            kind: r.get(2)?,
            description: r.get(3)?,
            score: None,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Fallbacks after an exact alias miss: the unique-best alias prefix
/// ("UltraBERT" -> "UltraBERT-Embed-150M"), then the ER-07 near-name stage
/// ("Bergn" -> "Bergen": JW on the transliteration key, blockers applied,
/// unique winner required), then the nearest entity card at cosine >= 0.8;
/// otherwise an error that names the closest entities.
fn lookup_entity_fuzzy(engine: &Engine, name: &str) -> Result<i64> {
    let n = text::norm_name(name);
    if n.chars().count() >= 3 {
        let prefix: Option<i64> = engine
            .db
            .sql()
            .query_row(
                "SELECT a.entity_id FROM entity_aliases a JOIN entities e ON e.id = a.entity_id
                 WHERE a.alias_norm >= ?1 AND a.alias_norm < ?1 || char(1114111)
                 ORDER BY e.mention_count DESC, length(a.alias_norm) LIMIT 1",
                [&n],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = prefix {
            return Ok(id);
        }
    }
    // ER-07 Q-08: a typo'd query ("Bergn") must resolve the entity, not just
    // the memory. Blockers keep "v1"/"v2"-style and person-name pairs apart;
    // a near tie between two entities falls through to the vector stage.
    if n.chars().count() >= 4 {
        let key = text::translit_norm(&n);
        let mut st = engine.db.sql().prepare(
            "SELECT a.alias_norm, a.entity_id FROM entity_aliases a
             JOIN entities e ON e.id = a.entity_id
             ORDER BY e.mention_count DESC, a.alias_norm LIMIT 8000",
        )?;
        let matches: Vec<(f64, i64)> = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .filter_map(|row| row.ok())
            .filter_map(|(alias, id)| {
                let jw = text::jaro_winkler(&key, &text::translit_norm(&alias));
                if jw < f64::from(engine.cfg.name_merge) || text::name_blocker(&n, &alias).is_some()
                {
                    None
                } else {
                    Some((jw, id))
                }
            })
            .collect();
        let best = matches
            .iter()
            .map(|(j, _)| *j)
            .fold(f64::NEG_INFINITY, f64::max);
        let winners: std::collections::HashSet<i64> = matches
            .iter()
            .filter(|(j, _)| *j >= best - 0.005)
            .map(|(_, id)| *id)
            .collect();
        if winners.len() == 1 {
            if let Some(id) = winners.into_iter().next() {
                return Ok(id);
            }
        }
    }
    let vector = engine.emb.embed(&[name], Mode::Document)?.remove(0);
    let mut st = engine.db.sql().prepare(&format!(
        "SELECT v.rowid, 1 - v.distance, e.name FROM entities_vec v JOIN entities e ON e.id = v.rowid
         WHERE v.embedding MATCH {} AND k = 3 ORDER BY v.distance",
        engine.db.vec_sql("?1")
    ))?;
    let near: Vec<(i64, f64, String)> = st
        .query_map([engine.db.vec_blob(&vector)], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    match near.first() {
        Some((id, cos, _)) if *cos >= 0.8 => Ok(*id),
        _ => Err(crate::err!(
            "unknown entity '{name}'{}",
            if near.is_empty() {
                String::new()
            } else {
                format!(
                    "; closest: {}",
                    near.iter()
                        .map(|(id, _, n)| format!("{n} (e{id})"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )),
    }
}

fn overview_ppr(engine: &Engine, query: &str) -> Result<HashMap<i64, f64>> {
    let vector = engine.emb.embed(&[query], Mode::Query)?.remove(0);
    let snap = snapshot(engine, None, false)?;
    let mut seeds = Vec::new();
    let names = extract::ngrams(query, 4)
        .into_iter()
        .map(|x| x.0)
        .collect::<Vec<_>>();
    for id in alias_entities(&engine.db, &names)? {
        if let Some(&node) = snap.ent_node.get(&id) {
            seeds.push((node, 1.0));
        }
    }
    let mut st = engine.db.sql().prepare(&format!(
        "SELECT rowid,1-distance FROM entities_vec
         WHERE embedding MATCH {} AND k=10 ORDER BY distance",
        engine.db.vec_sql("?1")
    ))?;
    for row in st.query_map([engine.db.vec_blob(&vector)], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
    })? {
        let (id, score) = row?;
        if score >= f64::from(engine.cfg.entity_seed) {
            if let Some(&node) = snap.ent_node.get(&id) {
                seeds.push((node, score));
            }
        }
    }
    // Topical queries ("embedding performance") rarely name an entity: seed the
    // walk from the nearest memories too, with the same share recall uses.
    let ent_total: f64 = seeds.iter().map(|(_, w)| w).sum();
    let mut st = engine.db.sql().prepare(&format!(
        "SELECT rowid,1-distance FROM memories_vec
         WHERE embedding MATCH {} AND k=?2 AND live=1 ORDER BY distance",
        engine.db.vec_sql("?1")
    ))?;
    let mem: Vec<(i64, f64)> = st
        .query_map(
            params![engine.db.vec_blob(&vector), engine.cfg.mem_seed_top as i64],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
        .collect::<rusqlite::Result<_>>()?;
    let mem_total: f64 = mem.iter().map(|(_, w)| w.max(0.0)).sum();
    if mem_total > 0.0 {
        let share = if ent_total > 0.0 {
            engine.cfg.mem_seed_share
        } else {
            1.0
        };
        for (_, w) in seeds.iter_mut() {
            *w *= (1.0 - share) / ent_total;
        }
        for (id, w) in mem {
            if let Some(&node) = snap.mem_node.get(&id) {
                seeds.push((node, w.max(0.0) * share / mem_total));
            }
        }
    }
    Ok(
        graph::ppr_push(&snap.csr, &seeds, engine.cfg.alpha, engine.cfg.eps, 200_000)
            .into_iter()
            .filter_map(|(node, score)| match snap.node_kind.get(node as usize) {
                Some(('e', id)) => Some((*id, score)),
                _ => None,
            })
            .collect(),
    )
}

fn cached_communities(engine: &Engine) -> Result<HashMap<i64, i64>> {
    let gen = generation(&engine.db)?;
    let cached: Option<i64> = engine
        .db
        .sql()
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key='communities_gen'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if cached == Some(gen) {
        let mut st = engine
            .db
            .sql()
            .prepare("SELECT entity_id,community FROM communities")?;
        return Ok(st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?);
    }
    engine.db.write(kg::communities)
}

fn representative(engine: &Engine, ids: &[i64]) -> Result<Option<MemoryHit>> {
    if ids.is_empty() {
        return Ok(None);
    }
    let marks = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT m.id,m.kind,m.content,m.valid_from,m.valid_to,m.expired_at,m.superseded_by,m.tags
         FROM memories m JOIN mentions mn ON mn.memory_id=m.id
         WHERE mn.entity_id IN ({marks}) AND m.expired_at IS NULL
         GROUP BY m.id
         ORDER BY COUNT(DISTINCT mn.entity_id) DESC,m.importance DESC,m.valid_from DESC
         LIMIT 1"
    );
    let row = engine
        .db
        .sql()
        .query_row(&sql, params_from_iter(ids.iter()), |r| {
            let valid_to: Option<i64> = r.get(4)?;
            let expired: Option<i64> = r.get(5)?;
            Ok(MemoryHit {
                id: r.get(0)?,
                kind: r.get(1)?,
                content: r.get(2)?,
                valid_from: r.get(3)?,
                valid_to,
                ended: expired.is_some() || valid_to.is_some_and(|x| x <= engine.clock().now),
                superseded_by: r.get(6)?,
                tags: r
                    .get::<_, String>(7)?
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
                score: 0.0,
                reasons: Vec::new(),
                duration: None,
                maybe_superseded_by: None,
            })
        })
        .optional()?;
    Ok(row)
}

pub fn overview(engine: &Engine, query: Option<&str>, k: usize) -> Result<OverviewOutput> {
    let k = k.min(50);
    let comm = cached_communities(engine)?;
    let mut groups: HashMap<i64, Vec<i64>> = HashMap::new();
    for (e, c) in comm {
        groups.entry(c).or_default().push(e);
    }
    let ppr = query.map(|q| overview_ppr(engine, q)).transpose()?;
    let all_ids: Vec<i64> = groups.values().flatten().copied().collect();
    let mut info: HashMap<i64, (String, String, i64)> = HashMap::new();
    for chunk in all_ids.chunks(400) {
        if chunk.is_empty() {
            continue;
        }
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql =
            format!("SELECT id,type,description,mention_count FROM entities WHERE id IN ({marks})");
        let mut st = engine.db.sql().prepare(&sql)?;
        for row in st.query_map(params_from_iter(chunk.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })? {
            let (id, kind, description, mentions) = row?;
            info.insert(id, (kind, description, mentions));
        }
    }
    let mut out = Vec::new();
    for (cid, ids) in groups {
        let mut ranked = ids
            .iter()
            .map(|id| {
                let (kind, description, mentions) = info.get(id).cloned().unwrap_or_default();
                Ok((
                    *id,
                    (!kind.is_empty() || !description.is_empty()),
                    mentions,
                    ppr.as_ref().and_then(|x| x.get(id)).copied().unwrap_or(0.0),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        if ranked
            .iter()
            .all(|(_, typed, mentions, _)| !*typed && *mentions <= 1)
        {
            continue;
        }
        let label = label_of(&community_names(engine, cid)?, ids.len());
        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| b.3.partial_cmp(&a.3).unwrap())
        });
        let ordered_ids: Vec<i64> = ranked.iter().map(|x| x.0).collect();
        let entities = ordered_ids
            .iter()
            .take(3)
            .map(|id| card(engine, *id))
            .collect::<Result<Vec<_>>>()?;
        let facts = facts_for(
            engine,
            &ordered_ids,
            false,
            engine.clock().now,
            (None, None),
            3,
            ppr.as_ref(),
            None,
        )?;
        let score = query
            .map(|_| ranked.iter().map(|x| x.3).sum())
            .unwrap_or_else(|| ranked.iter().map(|x| x.2 as f64).sum());
        out.push(Community {
            id: cid,
            label,
            size: ordered_ids.len(),
            entities,
            predicates: Vec::new(),
            span: None,
            facts,
            representative: representative(engine, &ordered_ids)?,
            score,
        });
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap()
            .then_with(|| a.id.cmp(&b.id))
    });
    out.truncate(k.max(1));
    for c in &mut out {
        c.predicates = community_predicates(engine, c.id)?;
        c.span = community_span(engine, c.id)?;
    }
    Ok(OverviewOutput { communities: out })
}

#[cfg(test)]
mod hint_tests {
    use super::*;
    use crate::embed::HashEmbedder;
    use crate::engine::{EntityIn, RelationIn, RememberInput};

    fn entity(name: &str, kind: &str) -> EntityIn {
        EntityIn {
            name: name.into(),
            kind: Some(kind.into()),
            ..Default::default()
        }
    }

    fn rel(source: &str, predicate: &str, target: &str) -> RelationIn {
        RelationIn {
            source: source.into(),
            predicate: predicate.into(),
            target: target.into(),
            fact: Some(format!("{source} {predicate} {target}.")),
            ..Default::default()
        }
    }

    /// Priya manages Mira Shah; Mira lives in Oslo; a chain Priya -> Mira whose
    /// last hop is "manages" leaves the question "where ... live" unfinished.
    struct Fixture {
        engine: Engine,
        priya: i64,
        mira: i64,
        step_to_mira: paths::Step,
        step_to_anika: paths::Step,
        lives_mira: i64,
        lives_fact: String,
    }

    fn fixture() -> Fixture {
        let engine = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
        let out = engine
            .remember(RememberInput {
                content: "Priya manages Mira Shah and mentors Anika Singh.".into(),
                entities: vec![
                    entity("Priya", "person"),
                    entity("Mira Shah", "person"),
                    entity("Anika Singh", "person"),
                ],
                relations: vec![
                    rel("Priya", "manages", "Mira Shah"),
                    rel("Priya", "mentors", "Anika Singh"),
                ],
                ..Default::default()
            })
            .unwrap();
        let mut lives: Option<(i64, String)> = None;
        for (name, place) in [("Mira Shah", "Oslo"), ("Anika Singh", "Pune")] {
            let out = engine
                .remember(RememberInput {
                    content: format!("{name} lives in {place}."),
                    entities: vec![entity(name, "person"), entity(place, "place")],
                    relations: vec![rel(name, "lives in", place)],
                    ..Default::default()
                })
                .unwrap();
            if name == "Mira Shah" {
                lives = Some((out.relations[0].id, out.relations[0].fact.clone()));
            }
        }
        let (lives_mira, lives_fact) = lives.unwrap();
        let priya = resolve_entity(&engine, "Priya").unwrap();
        let mira = resolve_entity(&engine, "Mira Shah").unwrap();
        let anika = resolve_entity(&engine, "Anika Singh").unwrap();
        let step = |relation_id: i64, fact: &str, dst: i64| paths::Step {
            relation_id,
            src: priya,
            dst,
            predicate: "manages".into(),
            fact: fact.into(),
            reverse: false,
            matched: 1,
            memory: None,
        };
        let relations: Vec<(i64, String)> = out
            .relations
            .iter()
            .map(|r| (r.id, r.fact.clone()))
            .collect();
        let to_mira = relations
            .iter()
            .find(|(_, fact)| fact.contains("Mira"))
            .unwrap()
            .clone();
        let to_anika = relations
            .iter()
            .find(|(_, fact)| fact.contains("Anika"))
            .unwrap()
            .clone();
        Fixture {
            engine,
            priya,
            mira,
            step_to_mira: step(to_mira.0, &to_mira.1, mira),
            step_to_anika: step(to_anika.0, &to_anika.1, anika),
            lives_mira,
            lives_fact,
        }
    }

    const NOW: i64 = 1_800_000_000;

    fn hint(
        f: &Fixture,
        chains: &[&paths::PathHit],
        seeds: &[(i64, f64)],
        facts: &[FactHit],
        memories: &[MemoryHit],
        tau: f32,
    ) -> Vec<NextHop> {
        let anchors: HashSet<i64> = [f.priya].into();
        next_hops(
            &f.engine,
            "Where does Priya's manager live?",
            &anchors,
            &[f.priya],
            chains,
            seeds,
            facts,
            memories,
            &HashMap::new(),
            NOW,
            tau,
        )
        .unwrap()
    }

    fn chain(step: paths::Step) -> paths::PathHit {
        paths::PathHit {
            steps: vec![step],
            coverage: 1,
            score: 1.0,
        }
    }

    #[test]
    fn a_partial_chain_hints_the_bridge_entity() {
        let f = fixture();
        let chain = chain(f.step_to_mira.clone());
        let out = hint(&f, &[&chain], &[], &[], &[], 0.40);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].entity, format!("e{}", f.mira));
        assert_eq!(out[0].name, "Mira Shah");
        assert!(out[0].want.starts_with("where"), "{out:?}");
        assert!(out[0].want.contains("live"), "{out:?}");
        assert_eq!(out[0].why, "chain ends at a person");
    }

    #[test]
    fn answered_context_suppresses_the_hint() {
        let f = fixture();
        let chain = chain(f.step_to_mira.clone());
        let fact = FactHit {
            id: 0,
            source: "Mira Shah".into(),
            predicate: "lives in".into(),
            target: "Oslo".into(),
            fact: "Mira Shah lives in Oslo.".into(),
            valid_from: NOW,
            valid_to: None,
            ended: false,
            retracted: false,
            polarity: 1,
            memory: false,
            current_values: None,
            replaced_by: None,
            duration: None,
        };
        assert!(hint(&f, &[&chain], &[], &[fact], &[], 0.40).is_empty());
        let memory = MemoryHit {
            id: 1,
            kind: "fact".into(),
            content: "Mira Shah lives in Oslo.".into(),
            valid_from: NOW,
            valid_to: None,
            ended: false,
            superseded_by: None,
            tags: Vec::new(),
            score: 1.0,
            reasons: Vec::new(),
            duration: None,
            maybe_superseded_by: None,
        };
        assert!(hint(&f, &[&chain], &[], &[], &[memory], 0.40).is_empty());
    }

    #[test]
    fn a_last_hop_that_answers_the_frame_suppresses_the_hint() {
        let f = fixture();
        let step = paths::Step {
            relation_id: f.lives_mira,
            src: f.priya,
            dst: f.mira,
            predicate: "lives in".into(),
            fact: f.lives_fact.clone(),
            reverse: false,
            matched: 1,
            memory: None,
        };
        let chain = chain(step);
        assert!(hint(&f, &[&chain], &[], &[], &[], 0.01).is_empty());
    }

    #[test]
    fn a_seed_bridge_gets_a_hint_without_any_chain() {
        let f = fixture();
        let out = hint(&f, &[], &[(f.mira, 0.9)], &[], &[], 0.40);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].name, "Mira Shah");
        assert_eq!(out[0].why, "no chain covers the question");
    }

    #[test]
    fn at_most_two_hints_are_returned() {
        let f = fixture();
        let first = chain(f.step_to_mira.clone());
        let second = chain(f.step_to_anika.clone());
        let out = hint(&f, &[&first, &second], &[], &[], &[], 0.40);
        assert_eq!(out.len(), 2, "{out:?}");
        let names: HashSet<&str> = out.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(names, HashSet::from(["Mira Shah", "Anika Singh"]));
    }

    #[test]
    fn hints_are_off_when_tau_is_zero() {
        let f = fixture();
        let chain = chain(f.step_to_mira.clone());
        assert!(hint(&f, &[&chain], &[(f.mira, 0.9)], &[], &[], 0.0).is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query_plan(db: &Db, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Vec<String> {
        let mut st = db
            .sql()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let mut out = Vec::new();
        for row in st
            .query_map(params_from_iter(params.iter()), |r| r.get::<_, String>(3))
            .unwrap()
        {
            out.push(row.unwrap());
        }
        out
    }

    /// D2: the status lookup must run the FTS MATCH once, not once per stored
    /// memory. The old query joined `memories`, so its plan had two data
    /// sources (the plan SQLite picked on a vault with stats was `SCAN m` then
    /// a virtual-table lookup per row) and a status question runs up to
    /// STATUS_LOOKUPS of these lookups. One source is the FTS table alone.
    #[test]
    fn the_status_lookup_is_driven_by_the_fts_index_alone() {
        let db = Db::open_in_memory().unwrap();
        let plan = query_plan(&db, STATUS_FTS_SQL, &[&"\"export\"*", &10i64]);
        let sources: Vec<&String> = plan
            .iter()
            .filter(|step| step.contains("SCAN") || step.contains("SEARCH"))
            .collect();
        assert_eq!(
            sources.len(),
            1,
            "the lookup reads more than the FTS index: {plan:?}"
        );
        assert!(sources[0].contains("memories_fts"), "{plan:?}");
    }
}
