use crate::db::Db;
use crate::embed::Mode;
use crate::engine::Engine;
use crate::engine::{
    Community, CommunityRef, EntityCard, EntityOutput, FactHit, MemoryHit, NextHop, OverviewOutput,
    RecallInput, RecallOutput,
};
use crate::{extract, graph, kg, paths, rank, rerank, temporal, text, Result};
use rusqlite::{params, params_from_iter, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const MEMORY_COLUMNS: &str =
    "id,content,kind,tags,importance,scope,source,session,created_at,valid_from,valid_to,expired_at,superseded_by";

/// Weight of the query-directed path channel in the RRF fusion.
const PATH_WEIGHT: f64 = 2.5;
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

/// World-time end of a memory (valid_to reached) or system retirement.
fn is_ended(row: &MemoryRow, cutoff: i64) -> bool {
    row.10.is_some_and(|x| x <= cutoff) || row.11.is_some()
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
}

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
    {
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
    let causal_query = paths::causal_intent(&input.query);
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
    let mut knn: Vec<(i64, f32)> = st
        .query_map(
            params![
                engine.db.vec_blob(&query_vec),
                fetch_depth.max(paths::SEM_FACTS) as i64
            ],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f32>(1)?)),
        )?
        .collect::<rusqlite::Result<_>>()?;
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

    let channels = [
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
    ];
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
    let mut candidates = Vec::new();
    for f in fused {
        let Some(row) = rows.get(&f.id) else { continue };
        let mut score = f.score;
        if is_ended(row, cutoff) {
            if !history {
                score *= engine.cfg.expired_penalty;
            } else if as_of.is_none() {
                // "where did X live before": the past state is the answer.
                score *= engine.cfg.history_boost;
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
            window_empty: false,
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
    let mut facts = if negated {
        facts
            .into_iter()
            .filter(|fact| fact.polarity == 0 || paths::has_negation_cue(&fact.fact))
            .collect()
    } else if top_chains.is_empty() {
        facts
    } else {
        let prose = paths::prose_facts(&engine.db, &sem_hits, engine.cfg.prose_margin, cutoff)?
            .into_iter()
            .filter(|fact| {
                rows.get(&fact.id).is_some_and(|row| {
                    eligible(row, as_of, history, input, since, until, now, planned_ok)
                })
            });
        let mut merged: Vec<FactHit> = prose.collect();
        merged.extend(paths::facts(&engine.db, &top_chains, cutoff, 8)?);
        for fact in facts {
            if !merged.iter().any(|x| x.id == fact.id && !x.memory) {
                merged.push(fact);
            }
        }
        merged.truncate(8);
        merged
    };
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
    if ordered {
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
         WHERE r.src IN ({marks}) OR r.dst IN ({marks})"
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
                let n: i64 = engine.db.sql().query_row(
                    "SELECT count(*) FROM relations WHERE subj=?1 AND predicate_id IN
                         (SELECT id FROM predicates WHERE group_id=?2) AND polarity=1
                     AND expired_at IS NULL AND valid_from<=?3 AND (valid_to IS NULL OR valid_to>?3)",
                    params![subj, group, cutoff],
                    |r| r.get(0),
                )?;
                fact.current_values = (n >= 2).then_some(n as usize);
            }
            Ok(fact)
        })
        .collect()
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
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let limit = limit.min(200);
    Ok(EntityOutput {
        card,
        current_facts: current,
        past_facts: past.into_iter().take(limit).collect(),
        similar,
        timeline,
    })
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
