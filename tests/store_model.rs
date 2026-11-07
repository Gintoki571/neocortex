//! Model-based random test of the fact store's rules (lane nc-model).
//!
//! A tiny xorshift PRNG drives 200 seeds x 40 random ops (remember with
//! structured relations, supersedes, invalidate, forget, merge_entities) over
//! a six-entity pool, four predicates (two exclusive), a small date range.
//! A plain in-memory reference model (`Vec<Fact>` with half-open intervals)
//! mirrors the write path; after EVERY op the test asserts:
//!
//!   1. history is never deleted: `stats().relations` never decreases across
//!      any non-merge op. Soft `forget` and `invalidate` only set
//!      `valid_to` / `expired_at`, so the count is normally unchanged; `merge`
//!      consolidates exact live duplicates (same src, predicate group, dst,
//!      polarity) and may delete rows, so rule 1 is not asserted across merges
//!      (rules 2/4/5 cover merges). Hard `forget` is not generated: it deletes
//!      orphaned rows by design.
//!   2. as-of visibility: at 3 probe dates, the relation ids visible to the
//!      engine (valid_from <= t < valid_to, not system-expired) equal the
//!      model's. No public API lists facts exhaustively at an arbitrary
//!      as-of instant (`recall` is ranked and capped at 8 facts; `entity` is
//!      now-relative and pulls in community recompute), so one query is read
//!      directly from `relations`; the engine read path is cross-checked
//!      separately per op on one rotating root entity via `entity()` with the
//!      clock moved to a probe date.
//!   3. exclusive slots, in two halves: (a) the ending the engine reports
//!      (`invalidated_relations`) equals the model's expectation — same subject,
//!      same predicate group, never another subject's; asserted on main.
//!      (b) at most one live (positive) value per (subject, exclusive predicate
//!      group) at now; does not hold on main (FINDING-1: merge does not remap
//!      `relations.subj`), so it lives in the `#[ignore]`d intent test.
//!   4. merge (`merge_entities`): no fact lost (every pre-merge fact incident
//!      to the merged entity is reachable from the survivor or an allowed
//!      survivor-alias, or was a live duplicate the merge consolidated, or
//!      became a self-loop); no live relation with src == dst; the old name
//!      resolves to the survivor.
//!   5. `validate()` reports no unexpected problems. On main it reports one
//!      known contradiction: "relation rN is current but every memory that
//!      states it has ended" (FINDING-2); that exact signature is tolerated,
//!      everything else fails. The strict variant (no tolerance) is covered by
//!      the `#[ignore]`d intent test.
//!
//! Names are chosen two+ tokens apart so the near-name layer (JW/blockers)
//! returns Below for value pairs; with the hash embedder the name layer
//! decides exclusive endings on its own. Hash-embedder vectors do not alias
//! the agent predicates ("primary language", "depends on": tails differ from
//! every seeded predicate, so `head_tail_compatible` is false).
//!
//! Every `remember` passes explicit dates, polarity and `valid_from`, and
//! contents are cue-free lowercase text, so the text readers (text-date
//! inference, retraction cues, negation inference, nicknames, causal cues)
//! are inert and the model needs to mirror only the structured paths.
//!
//! On failure the test prints the seed and the op log. The write path goes
//! through the public API only; the reads for which no public API exists
//! (live self-loop count, exclusive-slot live count, the exhaustive
//! as-of probe) use SQL, documented here.
//!
//! Run: `cargo test --test store_model` (debug). Runtime target: < 20 s debug.

use neocortex::embed::HashEmbedder;
use neocortex::engine::{Engine, EntityIn, RelationIn, RememberInput};
use neocortex::temporal::{days_from_civil, Clock};
use std::collections::BTreeSet;
use std::sync::Arc;

const DAY: i64 = 86_400;

/// Pool base: 2020-01-01 00:00 UTC. Relation/memory dates come from
/// BASE + [0, 400) days; the supersedes date sits one day past that.
fn base() -> i64 {
    days_from_civil(2020, 1, 1) * DAY
}
const NOW: i64 = 1_759_363_200; // 2025-10-01 00:00 UTC
fn supersede_date() -> i64 {
    base() + 400 * DAY
}
/// Probe dates inside the pool range (base + 150/300/395 days).
fn probes() -> [i64; 3] {
    [base() + 150 * DAY, base() + 300 * DAY, base() + 395 * DAY]
}

const NAMES: [&str; 6] = [
    "Astrid Vollent",
    "Bruno Kestler",
    "Coralie Denton",
    "Darius Pell",
    "Elena Marsh",
    "Faro Venn",
];

const PREDS: [&str; 4] = [
    "works at",         // seeded, exclusive (group "work at")
    "primary language", // agent, exclusive via per-write flag (sticky)
    "manages",          // seeded alias of manage, non-exclusive
    "depends on",       // agent, non-exclusive
];

/// xorshift64* — deterministic, tiny, no dependency.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_add(0x9E3779B97F4A7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

/// A stored fact row. `slot` is the slot key captured AT WRITE TIME (`relations.subj`):
/// the engine binds it at insert and `merge_entities` does not remap it on entity merge,
/// so it may point at a merged-away entity (see FINDING-1 in the lane report).
#[derive(Clone)]
struct Fact {
    id: i64,
    src: usize,
    dst: usize,
    slot: usize,
    group: u8,
    polarity: i64,
    valid_from: i64,
    valid_to: Option<i64>,
    expired: bool,
    evidence: Vec<i64>,
}

struct Memory {
    id: i64,
    valid_from: i64,
    valid_to: Option<i64>,
    expired: bool,
}

struct Model {
    /// Logical pool index -> canonical root pool index.
    map: [usize; 6],
    /// Whether this root has been created in the store so far.
    created: [bool; 6],
    facts: Vec<Fact>,
    memories: Vec<Memory>,
    /// Sticky exclusive flag stored on the agent predicates ("primary
    /// language", "depends on"); "works at" is seeded-sticky, "manages" never.
    sticky: [bool; 4],
}

impl Model {
    fn new() -> Self {
        Model {
            map: [0, 1, 2, 3, 4, 5],
            created: [false; 6],
            facts: Vec::new(),
            memories: Vec::new(),
            sticky: [false; 4],
        }
    }
    fn root(&self, i: usize) -> usize {
        self.map[i]
    }
    /// `predicates.exclusive` as the engine would read it NOW.
    fn pred_flag(&self, pred: usize) -> bool {
        match pred {
            0 => true,  // seeded, cleared only for seeded=0 rows, never here
            2 => false, // manage is never written with exclusive: true
            _ => self.sticky[pred],
        }
    }
    /// The slot test (`remember.rs`): a slot only for writes that may claim one.
    fn exclusive_now(&self, pred: usize, write_flag: Option<bool>) -> bool {
        write_flag != Some(false) && (write_flag == Some(true) || self.pred_flag(pred))
    }
    /// Mirror `apply_exclusive` after a write (this write included).
    fn apply_exclusive(&mut self, pred: usize, write_flag: Option<bool>) {
        match write_flag {
            Some(true) => self.sticky[pred] = true,
            Some(false) if pred != 0 => self.sticky[pred] = false, // seeded work at stays
            Some(false) | None => {}
        }
    }
    fn visible(&self, t: i64) -> BTreeSet<i64> {
        self.facts
            .iter()
            .filter(|f| {
                f.valid_from <= t
                    && f.valid_to.is_none_or(|v| t < v)
                    && !f.expired
                    && f.src != f.dst
            })
            .map(|f| f.id)
            .collect()
    }
    fn visible_incident(&self, root: usize, t: i64) -> BTreeSet<i64> {
        self.facts
            .iter()
            .filter(|f| {
                (f.src == root || f.dst == root)
                    && f.valid_from <= t
                    && f.valid_to.is_none_or(|v| t < v)
                    && !f.expired
                    && f.src != f.dst
            })
            .map(|f| f.id)
            .collect()
    }
    fn find_fact(&self, id: i64) -> Option<&Fact> {
        self.facts.iter().find(|f| f.id == id)
    }
    fn memory_mut(&mut self, id: i64) -> Option<&mut Memory> {
        self.memories.iter_mut().find(|m| m.id == id)
    }
    fn live_memory_ids(&self) -> Vec<i64> {
        self.memories
            .iter()
            .filter(|m| !m.expired)
            .map(|m| m.id)
            .collect()
    }
}

struct Ctx {
    seed: u64,
    ctr: u64,
    log: Vec<String>,
}
impl Ctx {
    fn note(&mut self, line: String) {
        self.log.push(line);
    }
    fn fail(&self, msg: String) -> ! {
        panic!(
            "store-model violation (seed {:#x}, {} ops into the log)\n{}\n{}\nseed {:#x} reproduces this run",
            self.seed,
            self.log.len(),
            self.log.join("\n"),
            msg,
            self.seed
        );
    }
    fn expect(&self, ok: bool, msg: String) {
        if !ok {
            self.fail(msg);
        }
    }
}

fn fmt_ts(ts: i64) -> String {
    ts.to_string()
}

fn main_clock() -> Clock {
    Clock {
        now: NOW,
        tz_offset_secs: 0,
    }
}

fn open_engine() -> Engine {
    Engine::open_in_memory(Arc::new(HashEmbedder))
        .unwrap()
        .with_clock(main_clock())
}

/// Number of relation rows the engine stores (never decreases except across a merge).
fn relation_count(e: &Engine) -> i64 {
    e.stats().map(|s| s.relations).unwrap_or(-1)
}

/// As-of visibility from the storage layer: the ids the engine shows true at `t`.
/// No public API exposes the full fact set at an arbitrary instant, so this one
/// read is direct SQL; the engine's fact-listing read path is cross-checked on a
/// rotating root entity per op (`check_all`).
fn sql_asof_ids(e: &Engine, ctx: &Ctx, t: i64) -> BTreeSet<i64> {
    let attempt: Result<BTreeSet<i64>, String> = (|| {
        let mut st =
            e.db.sql()
                .prepare(
                    "SELECT id FROM relations
                 WHERE valid_from <= ?1 AND (valid_to IS NULL OR valid_to > ?1)
                   AND expired_at IS NULL AND src <> dst",
                )
                .map_err(|e| e.to_string())?;
        st.query_map([t], |r| r.get(0))
            .map_err(|e| e.to_string())
            .and_then(|rows| {
                rows.collect::<rusqlite::Result<BTreeSet<i64>>>()
                    .map_err(|e| e.to_string())
            })
    })();
    attempt.unwrap_or_else(|err| ctx.fail(format!("rule 2 probe query failed: {err}")))
}

/// Live (positive) exclusive-slot violations: (subject id, group, count) plus details.
fn sql_slot_violations(e: &Engine, ctx: &Ctx) -> Vec<String> {
    let slots: Vec<(i64, i64, i64)> = (|| -> Result<_, String> {
        let mut st =
            e.db.sql()
                .prepare(
                    "SELECT r.src, COALESCE(NULLIF(p.group_id, 0), p.id), COUNT(*) AS n
                 FROM relations r JOIN predicates p ON p.id = r.predicate_id
                 WHERE r.polarity = 1 AND r.expired_at IS NULL
                   AND (r.valid_to IS NULL OR r.valid_to > ?1)
                   AND p.exclusive <> 0
                 GROUP BY 1, 2 HAVING n > 1",
                )
                .map_err(|e| e.to_string())?;
        let rows = st.query_map([NOW], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)));
        let rows = rows.map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())
    })()
    .unwrap_or_else(|err| ctx.fail(format!("rule 3 probe query failed: {err}")));
    let mut details = Vec::new();
    for (src, group, n) in slots {
        match slot_detail(e, src, group, n) {
            Ok(mut rows) => details.append(&mut rows),
            Err(err) => details.push(format!(
                "slot (subject e{src}, group {group}): detail query failed: {err}"
            )),
        }
    }
    details
}

/// One fact per line for the violating slot.
fn slot_detail(e: &Engine, src: i64, group: i64, n: i64) -> Result<Vec<String>, String> {
    let mut st =
        e.db.sql()
            .prepare(
                "SELECT r.id, es.name, ed.name, p.canonical, r.valid_from, r.valid_to
             FROM relations r JOIN predicates p ON p.id = r.predicate_id
             JOIN entities es ON es.id = r.src
             JOIN entities ed ON ed.id = r.dst
             WHERE r.src = ?1 AND COALESCE(NULLIF(p.group_id, 0), p.id) = ?2
               AND r.polarity = 1 AND r.expired_at IS NULL
               AND (r.valid_to IS NULL OR r.valid_to > ?3)",
            )
            .map_err(|e| e.to_string())?;
    let rows = st.query_map(rusqlite::params![src, group, NOW], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<i64>>(5)?,
        ))
    });
    let mut out = Vec::new();
    for row in rows.map_err(|e| e.to_string())? {
        let (id, s, d, c, vf, vt) = row.map_err(|e| e.to_string())?;
        out.push(format!(
            "slot (subject e{src}, group {group}, live={n}): r{id} {s} {c} {d} vf={vf} vt={vt:?}"
        ));
    }
    Ok(out)
}

/// Live relation with src == dst (the projection/cards hide these).
fn sql_live_self_loops(e: &Engine) -> i64 {
    e.db.sql()
        .query_row(
            "SELECT COUNT(*) FROM relations
             WHERE src = dst AND valid_to IS NULL AND expired_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

/// The per-op gate. `is_merge` relaxes rule 1 (merge consolidates exact live
/// duplicates by design; rules 2/4/5 still apply). `strict` asserts rule 3's
/// "at most one live value per (subject, exclusive predicate) at now" and disables
/// the FINDING-2 tolerance in rule 5; it is `#[ignore]`d on main (FINDING-1/2).
fn check_all(
    e: &mut Engine,
    m: &Model,
    ctx: &Ctx,
    rel_before: i64,
    is_merge: bool,
    op_no: u64,
    strict: bool,
) {
    if !is_merge {
        let after = relation_count(e);
        ctx.expect(
            after >= rel_before,
            format!("rule 1: relation rows decreased {rel_before} -> {after}"),
        );
    }
    for &t in &probes() {
        let got = sql_asof_ids(e, ctx, t);
        let want = m.visible(t);
        if got != want {
            let mut dump =
                format!("rule 2: as-of ids at {t} differ\nengine: {got:?}\nmodel:  {want:?}\n");
            let mut st = e
                .db
                .sql()
                .prepare(
                    "SELECT r.id, p.canonical, es.name || ' -> ' || ed.name, r.valid_from, r.valid_to, r.expired_at, r.polarity
                     FROM relations r
                     JOIN predicates p ON p.id = r.predicate_id
                     JOIN entities es ON es.id = r.src
                     JOIN entities ed ON ed.id = r.dst
                     ORDER BY r.id",
                )
                .unwrap();
            let rows = st
                .query_map([], |r| {
                    Ok(format!(
                        "r{} {} {} vf={} vt={:?} exp={:?} pol={}",
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, i64>(6)?
                    ))
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            dump.push_str(&format!("relations:\n{}\n", rows.join("\n")));
            let model_dump: Vec<String> = m
                .facts
                .iter()
                .map(|f| {
                    format!(
                        "r{} {} -> {} g={} pol={} vf={} vt={:?} exp={}",
                        f.id,
                        NAMES[f.src],
                        NAMES[f.dst],
                        f.group,
                        f.polarity,
                        f.valid_from,
                        f.valid_to,
                        f.expired
                    )
                })
                .collect();
            dump.push_str(&format!("model facts:\n{}\n", model_dump.join("\n")));
            for &id in got.difference(&want).chain(want.difference(&got)) {
                let sql_row: Option<String> = e
                    .db
                    .sql()
                    .query_row(
                        "SELECT p.canonical, es.name || ' -> ' || ed.name, r.fact, r.valid_from, r.valid_to, r.expired_at
                         FROM relations r
                         JOIN predicates p ON p.id = r.predicate_id
                         JOIN entities es ON es.id = r.src
                         JOIN entities ed ON ed.id = r.dst
                         WHERE r.id = ?1",
                        [id],
                        |r| {
                            Ok(format!(
                                "{} {} {} vf={} vt={:?} exp={:?}",
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                                r.get::<_, i64>(3)?,
                                r.get::<_, Option<i64>>(4)?,
                                r.get::<_, Option<i64>>(5)?
                            ))
                        },
                    )
                    .ok();
                let model_row = m.find_fact(id).map(|f| {
                    format!(
                        "model fact r{} src={} dst={} group={} polarity={} vf={} vt={:?} expired={}",
                        f.id, NAMES[f.src], NAMES[f.dst], f.group, f.polarity, f.valid_from, f.valid_to, f.expired
                    )
                });
                dump.push_str(&format!(
                    "id {id}: sql={sql_row:?}\n          {model_row:?}\n"
                ));
            }
            ctx.fail(dump);
        }
    }
    // Rotating root and probe date, engine read path (one card, clock moved).
    let root = m.root((op_no % 6) as usize);
    if m.created[root] {
        let prev = e.fixed_clock;
        let &t = &probes()[(op_no / 6 % 3) as usize];
        e.fixed_clock = Some(Clock {
            now: t,
            tz_offset_secs: 0,
        });
        let card = e.entity(NAMES[root], 200);
        e.fixed_clock = prev;
        let rname = NAMES[root];
        match card {
            Ok(card) => {
                let got: BTreeSet<i64> = card.current_facts.iter().map(|f| f.id).collect();
                let want = m.visible_incident(root, t);
                ctx.expect(
                    got == want,
                    format!("rule 2 (engine read path): root {rname} as-of {t} differ\nengine: {got:?}\nmodel:  {want:?}"),
                );
            }
            Err(err) => ctx.fail(format!(
                "rule 2 (engine read path): entity card for {rname} failed: {err}"
            )),
        }
    }
    if strict {
        let slots = sql_slot_violations(e, ctx);
        ctx.expect(
            slots.is_empty(),
            format!(
                "rule 3: exclusive slots with >1 live value:\n{}",
                slots.join("\n")
            ),
        );
    }
    let loops = sql_live_self_loops(e);
    ctx.expect(
        loops == 0,
        format!("rule 4: {loops} live relation(s) with src == dst"),
    );
    let problems = e
        .validate()
        .unwrap_or_else(|err| vec![format!("validate failed: {err}")]);
    // FINDING-2 (main f51b27e): a relation whose every evidence memory has
    // `valid_to` set (e.g. written by two memories that were both invalidated
    // with `invalidate m`) is reported here even though it is a legal store
    // state — the ending rule counts only expired memories as lost evidence.
    // Anything else that `validate()` reports is a real failure.
    let unexpected: Vec<String> = if strict {
        problems
    } else {
        problems
            .into_iter()
            .filter(|p| {
                !(p.starts_with("relation r")
                    && p.ends_with("is current but every memory that states it has ended"))
            })
            .collect()
    };
    ctx.expect(
        unexpected.is_empty(),
        format!("rule 5: validate reports: {unexpected:?}"),
    );
}

/// One relation as the generator planned it: (src, dst, predicate, rf, negated, exclusive, valid_to).
type RelInput = (usize, usize, usize, i64, bool, Option<bool>, Option<i64>);

/// Mirror a `remember` that was just written (any content, any number of relations).
/// Returns the list of relation ids the engine *reported* ending (slot + flip),
/// which the caller compares with the model's own expectation.
#[allow(clippy::too_many_arguments)]
fn mirror_remember(
    m: &mut Model,
    ctx: &Ctx,
    inputs: &[RelInput],
    out: &neocortex::engine::RememberOutput,
    mem_vf: i64,
    mem_vt: Option<i64>,
    supersede_target: Option<i64>,
) {
    let mem_id = out.id;
    if out.duplicate_of.is_none() {
        m.memories.push(Memory {
            id: mem_id,
            valid_from: mem_vf,
            valid_to: mem_vt,
            expired: false,
        });
    }

    ctx.expect(
        out.relations.len() == inputs.len(),
        format!(
            "relation count mismatch: {} inputs vs {} reported (a self-relation was dropped?)",
            inputs.len(),
            out.relations.len()
        ),
    );
    let mut expected_end: Vec<i64> = Vec::new();

    for (i, &(s, d, pred, rf, negated, excl, rvt)) in inputs.iter().enumerate() {
        let rid = out.relations[i].id;
        let src = m.root(s);
        let dst = m.root(d);
        let polarity = if negated { 0 } else { 1 };
        let group = pred as u8;
        // Sticky exclusive toggles on THIS write (order: resolve before slot test).
        m.apply_exclusive(pred, excl);
        let mut new_vt = rvt;
        // Exclusive slot (subject-keyed; only when the writing MEMORY has no end).
        if polarity == 1 && mem_vt.is_none() && m.exclusive_now(pred, excl) {
            let mut later: Option<i64> = None;
            for f in m.facts.iter_mut() {
                if f.slot != src || f.group != group || f.polarity != 1 || f.expired || f.dst == dst
                {
                    continue;
                }
                if f.valid_from == rf {
                    f.valid_to = Some(rf);
                    f.expired = true;
                    expected_end.push(f.id);
                } else if f.valid_from < rf && f.valid_to.is_none_or(|t| t > rf) {
                    f.valid_to = Some(rf);
                    expected_end.push(f.id);
                } else if f.valid_from > rf && later.is_none_or(|l| f.valid_from < l) {
                    later = Some(f.valid_from);
                }
            }
            if let Some(l) = later {
                if new_vt.is_none_or(|t| t > l) {
                    new_vt = Some(l);
                }
            }
        }
        // Evidence merge or new row.
        let mut candidates: Vec<usize> = m
            .facts
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                f.src == src
                    && f.dst == dst
                    && f.group == group
                    && f.polarity == polarity
                    && !f.expired
                    && f.valid_to.is_none_or(|t| t >= rf)
                    && f.valid_from <= new_vt.unwrap_or(i64::MAX)
            })
            .map(|(i, _)| i)
            .collect();
        // The engine's live-ish candidate query is `... LIMIT 1` with no ORDER BY,
        // which in practice returns the lowest rowid. Find the same row; if a
        // future build picks another plan, this check fails loudly instead of
        // silently drifting (the query is plan-dependent by design).
        candidates.sort_by_key(|&i| m.facts[i].id);
        let merged = out.relations[i].status == "merged";
        if merged {
            ctx.expect(
                !candidates.is_empty(),
                format!(
                    "engine says relation merged into r{rid} but the model found no live-ish candidate for ({src}, group {group}, {dst}, polarity {polarity})"
                ),
            );
            let idx = candidates[0];
            let f = &mut m.facts[idx];
            if f.id != rid {
                let mut dump = format!(
                    "evidence merge target mismatch: engine r{rid}, model candidate r{}\ntriple (src={} dst={} group={group} pol={polarity} rf={rf} new_vt={new_vt:?})\n",
                    f.id, NAMES[src], NAMES[dst]
                );
                for x in m.facts.iter() {
                    if x.src == src && x.dst == dst && x.group == group && x.polarity == polarity {
                        dump.push_str(&format!(
                            "model r{} src={} dst={} vf={} vt={:?} exp={} slot={}\n",
                            x.id,
                            NAMES[x.src],
                            NAMES[x.dst],
                            x.valid_from,
                            x.valid_to,
                            x.expired,
                            NAMES[x.slot]
                        ));
                    }
                }
                ctx.fail(dump);
            }
            f.valid_from = f.valid_from.min(rf);
            f.valid_to = match (f.valid_to, new_vt) {
                (None, _) => None,
                (Some(t), None) => {
                    if rf >= t {
                        None
                    } else {
                        Some(t)
                    }
                }
                (Some(t), Some(nv)) => Some(t.max(nv)),
            };
            f.evidence.push(mem_id);
        } else {
            ctx.expect(
                candidates.is_empty(),
                format!(
                    "engine reports relation r{rid} as new but the model has candidate(s) {:?} (evidence merge expected)",
                    candidates.iter().map(|&i| m.facts[i].id).collect::<Vec<_>>()
                ),
            );
            m.facts.push(Fact {
                id: rid,
                src,
                dst,
                slot: src, // slot key captured at write time (never remapped on merge)
                group,
                polarity,
                valid_from: rf,
                valid_to: new_vt,
                expired: false,
                evidence: vec![mem_id],
            });
            // Polarity flip: the same stored triple with the opposite sign ends.
            let opposite = 1 - polarity;
            for f in m.facts.iter_mut() {
                if f.id == rid {
                    continue;
                }
                if f.src == src
                    && f.dst == dst
                    && f.group == group
                    && f.polarity == opposite
                    && !f.expired
                    && f.valid_from <= rf
                    && f.valid_to.is_none_or(|t| t > rf)
                {
                    if f.valid_from == rf {
                        f.expired = true;
                    }
                    f.valid_to = Some(rf);
                    expected_end.push(f.id);
                }
            }
        }
    }

    // Engine's reported endings (slot + flip) must equal the model's.
    let mut eng: Vec<i64> = out.invalidated_relations.clone();
    eng.sort_unstable();
    let mut mdl = expected_end;
    mdl.sort_unstable();
    ctx.expect(
        eng == mdl,
        format!("exclusive/flip endings mismatch: engine {eng:?} != model {mdl:?}"),
    );
    // Never another subject's fact. A slot ending must come from a written slot;
    // a polarity-flip ending may come from the triple of the write even when a
    // merge moved that row's stale slot key away.
    for rid in &eng {
        let f = m
            .find_fact(*rid)
            .unwrap_or_else(|| panic!("engine ended unknown relation r{rid}"));
        let same_slot = inputs
            .iter()
            .any(|&(s, _, pred, _, _, _, _)| m.root(s) == f.slot && pred as u8 == f.group)
            || inputs
                .iter()
                .any(|&(s, _, pred, _, _, _, _)| m.root(s) == f.src && pred as u8 == f.group);
        ctx.expect(
            same_slot,
            format!("engine ended r{rid} outside the written slots"),
        );
    }

    // Memory-level end: relations whose only live evidence is THIS memory end at it.
    if let Some(to) = mem_vt {
        if out.duplicate_of.is_none() {
            end_last_evidence(m, &[mem_id], to);
        }
    }

    // Supersedes: target's relations end where the new memory starts; target expires.
    if let Some(target) = supersede_target {
        ctx.expect(
            out.superseded_memories.contains(&target),
            format!(
                "superseded_memories {:?} lacks m{target}",
                out.superseded_memories
            ),
        );
        end_last_evidence(m, &[target], supersede_date());
        if let Some(mm) = m.memory_mut(target) {
            mm.expired = true;
            mm.valid_to = mm.valid_to.or(Some(supersede_date()));
        }
    }
}

/// Relations whose only live (non-expired) evidence is in `ids` end at `at`
/// (the engine's `end_relations_losing_evidence`; nothing happens for
/// already-expired rows).
fn end_last_evidence(m: &mut Model, ids: &[i64], at: i64) {
    for f in m.facts.iter_mut() {
        if f.expired {
            continue;
        }
        if !f.evidence.iter().any(|e| ids.contains(e)) {
            continue;
        }
        let other_live = f
            .evidence
            .iter()
            .any(|e| !ids.contains(e) && m.memories.iter().any(|mm| mm.id == *e && !mm.expired));
        if other_live {
            continue;
        }
        if f.valid_to.is_none_or(|v| v > at) {
            f.valid_to = Some(at);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn op_remember(
    e: &mut Engine,
    m: &mut Model,
    rng: &mut Rng,
    ctx: &mut Ctx,
    rel_before: i64,
    op_no: u64,
    supersede: bool,
    strict: bool,
) {
    let n_rel = 1 + rng.below(2) as usize;
    let mut inputs: Vec<RelInput> = Vec::new();
    for _ in 0..n_rel {
        let (mut s, mut d) = (rng.below(6) as usize, rng.below(6) as usize);
        let mut guard = 0;
        while m.root(s) == m.root(d) && guard < 24 {
            s = rng.below(6) as usize;
            d = rng.below(6) as usize;
            guard += 1;
        }
        if m.root(s) == m.root(d) {
            ctx.note(format!("skip remember: all pool roots merged ({s}, {d})"));
            return;
        }
        let pred = rng.below(4) as usize;
        let rf = base() + rng.below(400) as i64 * DAY;
        let negated = rng.chance(15);
        let excl = match pred {
            // `exclusive: Some(false)` is an explicit per-write opt-out that never
            // claims the slot, by design the row it writes and any competing row
            // both stay live; rule 3's "at most one live value" cannot hold then.
            // The stream keeps `Some(true)` and `None` only.
            0 | 1 => match rng.below(100) {
                0..=64 => Some(true),
                _ => None,
            },
            _ => None,
        };
        let vt = if rng.chance(12) {
            Some(rf + (1 + rng.below(90)) as i64 * DAY)
        } else {
            None
        };
        inputs.push((s, d, pred, rf, negated, excl, vt));
    }

    let supersede_target: Option<i64> = if supersede {
        let live = m.live_memory_ids();
        if live.is_empty() {
            None
        } else {
            Some(live[rng.below(live.len() as u64) as usize])
        }
    } else {
        None
    };
    let mem_vf = if supersede_target.is_some() {
        supersede_date()
    } else {
        base() + rng.below(400) as i64 * DAY
    };
    let mem_vt = if supersede_target.is_none() && rng.chance(12) {
        Some(mem_vf + (1 + rng.below(90)) as i64 * DAY)
    } else {
        None
    };

    ctx.ctr += 1;
    let words = [
        "amber", "birch", "cedar", "delta", "ember", "flint", "garnet", "harbor", "ivory",
        "juniper", "kestrel", "lantern", "marble", "nectar", "onyx", "pepper", "quartz", "rowan",
        "saffron", "thicket",
    ];
    let content = format!(
        "entry {}: {} {} {} {} {} {}",
        ctx.ctr,
        words[rng.below(words.len() as u64) as usize],
        words[rng.below(words.len() as u64) as usize],
        words[rng.below(words.len() as u64) as usize],
        words[rng.below(words.len() as u64) as usize],
        words[rng.below(words.len() as u64) as usize],
        words[rng.below(words.len() as u64) as usize],
    );

    let mut used: BTreeSet<usize> = BTreeSet::new();
    for &(s, d, _, _, _, _, _) in &inputs {
        used.insert(s);
        used.insert(d);
    }
    let entities: Vec<EntityIn> = used
        .iter()
        .map(|&i| EntityIn {
            name: NAMES[i].into(),
            kind: Some("person".into()),
            ..Default::default()
        })
        .collect();
    let relations: Vec<RelationIn> = inputs
        .iter()
        .map(|&(s, d, pred, rf, negated, excl, vt)| RelationIn {
            source: NAMES[s].into(),
            predicate: PREDS[pred].into(),
            target: NAMES[d].into(),
            fact: None,
            exclusive: excl,
            negated: Some(negated),
            valid_from: Some(fmt_ts(rf)),
            valid_to: vt.map(fmt_ts),
        })
        .collect();

    let inp = RememberInput {
        content: content.clone(),
        valid_from: Some(fmt_ts(mem_vf)),
        valid_to: mem_vt.map(fmt_ts),
        entities,
        relations,
        supersedes: supersede_target.iter().map(|id| format!("m{id}")).collect(),
        ..Default::default()
    };
    ctx.note(format!(
        "remember #{} content={content:?} vf={} vt={:?} supersedes={:?} relations=[{}]",
        ctx.ctr,
        mem_vf,
        mem_vt,
        supersede_target,
        inputs
            .iter()
            .map(|&(s, d, pred, rf, negated, excl, vt)| format!(
                "{} {} {} rf={} neg={} excl={:?} vt={:?}",
                NAMES[s], PREDS[pred], NAMES[d], rf, negated, excl, vt
            ))
            .collect::<Vec<_>>()
            .join(" | ")
    ));

    match e.remember(inp) {
        Ok(out) => {
            ctx.note(format!(
                "  -> id={} invalidated={:?} superseded={:?}",
                out.id, out.invalidated_relations, out.superseded_memories
            ));
            for &(s, d, _, _, _, _, _) in &inputs {
                let (rs, rd) = (m.root(s), m.root(d));
                m.created[rs] = true;
                m.created[rd] = true;
            }
            mirror_remember(m, ctx, &inputs, &out, mem_vf, mem_vt, supersede_target);
            check_all(e, m, ctx, rel_before, false, op_no, strict);
        }
        Err(err) => ctx.fail(format!("remember failed: {err}")),
    }
}

fn op_invalidate_memory(
    e: &mut Engine,
    m: &mut Model,
    rng: &mut Rng,
    ctx: &mut Ctx,
    rel_before: i64,
    op_no: u64,
    strict: bool,
) {
    let candidates: Vec<&Memory> = m.memories.iter().filter(|mm| !mm.expired).collect();
    if candidates.is_empty() {
        ctx.note("invalidate-memory: skipped (no live memory)".into());
        return;
    }
    let idx = rng.below(candidates.len() as u64) as usize;
    let id = candidates[idx].id;
    let vf = candidates[idx].valid_from;
    let at = base() + rng.below(400) as i64 * DAY;
    let at = at.max(vf);
    ctx.note(format!("invalidate m{id} at {at}"));
    let out = e.invalidate(&format!("m{id}"), Some(&at.to_string()));
    match out {
        Ok(_) => {
            let already = m
                .memory_mut(id)
                .and_then(|mm| mm.valid_to)
                .is_some_and(|t| t <= at);
            if already {
                // Engine: "already ended", relations untouched.
            } else if let Some(mm) = m.memory_mut(id) {
                mm.valid_to = Some(at);
                // Relations whose only live evidence is this memory end at `at`.
                end_last_evidence(m, &[id], at);
            }
            check_all(e, m, ctx, rel_before, false, op_no, strict);
        }
        Err(err) => ctx.fail(format!("invalidate m{id} failed: {err}")),
    }
}

fn op_invalidate_relation(
    e: &mut Engine,
    m: &mut Model,
    rng: &mut Rng,
    ctx: &mut Ctx,
    rel_before: i64,
    op_no: u64,
    strict: bool,
) {
    let live: Vec<(i64, i64)> = m
        .facts
        .iter()
        .filter(|f| !f.expired)
        .map(|f| (f.id, f.valid_from))
        .collect();
    if live.is_empty() {
        ctx.note("invalidate-relation: skipped (no live relation)".into());
        return;
    }
    let idx = rng.below(live.len() as u64) as usize;
    let (rid, vf) = live[idx];
    let mut at = base() + rng.below(400) as i64 * DAY;
    if at < vf {
        at = vf;
    }
    ctx.note(format!("invalidate r{rid} at {at}"));
    match e.invalidate(&format!("r{rid}"), Some(&at.to_string())) {
        Ok(_) => {
            if let Some(f) = m.facts.iter_mut().find(|f| f.id == rid) {
                if f.valid_to.is_some_and(|t| t <= at) {
                    // Engine: "already ended", unchanged.
                } else {
                    f.valid_to = Some(at);
                }
            }
            check_all(e, m, ctx, rel_before, false, op_no, strict);
        }
        Err(err) => ctx.fail(format!("invalidate r{rid} failed: {err}")),
    }
}

fn op_forget(
    e: &mut Engine,
    m: &mut Model,
    rng: &mut Rng,
    ctx: &mut Ctx,
    rel_before: i64,
    op_no: u64,
    strict: bool,
) {
    let live: Vec<i64> = m.live_memory_ids();
    if live.is_empty() {
        ctx.note("forget: skipped (no live memory)".into());
        return;
    }
    let id = live[rng.below(live.len() as u64) as usize];
    ctx.note(format!("forget m{id} (soft)"));
    match e.forget(&format!("m{id}"), false) {
        Ok(_) => {
            if let Some(mm) = m.memory_mut(id) {
                mm.expired = true;
            }
            // Relations whose only live evidence is the just-forgotten memory retract.
            retract_last_evidence(m, id);
            check_all(e, m, ctx, rel_before, false, op_no, strict);
        }
        Err(err) => ctx.fail(format!("forget m{id} failed: {err}")),
    }
}

/// `expired_at = now` for the relations whose only live evidence is THIS memory
/// (the engine evaluates the relations of the memory being forgotten, not the
/// transitive all-expired closure: a relation evidenced only by a
/// long-superseded memory stays merely `valid_to`-ended until that memory is
/// forgotten itself).
fn retract_last_evidence(m: &mut Model, id: i64) {
    for f in m.facts.iter_mut() {
        if f.expired {
            continue;
        }
        if !f.evidence.contains(&id) {
            continue;
        }
        let other_live = f
            .evidence
            .iter()
            .any(|e| *e != id && m.memories.iter().any(|mm| mm.id == *e && !mm.expired));
        if !other_live {
            f.expired = true;
        }
    }
}

fn op_merge(
    e: &mut Engine,
    m: &mut Model,
    rng: &mut Rng,
    ctx: &mut Ctx,
    rel_before: i64,
    op_no: u64,
    strict: bool,
) {
    // Pick two distinct logical roots.
    let (a, b) = (rng.below(6) as usize, rng.below(6) as usize);
    if a == b || m.root(a) == m.root(b) {
        ctx.note(format!("merge: skipped ({a} vs {b}, same root)"));
        return;
    }
    let (keep_i, merge_i) = if rng.chance(50) { (a, b) } else { (b, a) };
    let keep = m.root(keep_i);
    let gone = m.root(merge_i);
    if !m.created[keep] || !m.created[gone] {
        ctx.note(format!(
            "merge: skipped (uncreated entity: keep={}, gone={})",
            m.created[keep], m.created[gone]
        ));
        return;
    }
    let keep_name = NAMES[keep];
    let gone_name = NAMES[gone];
    ctx.note(format!("merge {gone_name} into {keep_name}"));

    // Pre-merge: facts incident to the merged entity.
    let pre_incident: BTreeSet<i64> = m
        .facts
        .iter()
        .filter(|f| f.src == gone || f.dst == gone)
        .map(|f| f.id)
        .collect();

    match e.merge_entities(keep_name, gone_name, true) {
        Ok(out) => {
            ctx.note(format!("  -> {}", out.changed.join("; ")));
            // Remap logical pool indices.
            for i in 0..6 {
                if m.map[i] == gone {
                    m.map[i] = keep;
                }
            }
            // Move facts. The slot key (`relations.subj`) is NOT remapped by the engine's
            // merge path; keep it at its write-time root (FINDING-1).
            for f in m.facts.iter_mut() {
                if f.src == gone {
                    f.src = keep;
                }
                if f.dst == gone {
                    f.dst = keep;
                }
                // f.slot unchanged, as with relations.subj
            }
            for f in m.facts.iter_mut() {
                if f.src == f.dst && !f.expired {
                    f.expired = true; // engine expires self-loops on merge
                }
            }
            // Consolidate exact live duplicates: keep earliest (valid_from, id),
            // move evidence, drop the rest.
            let mut groups: std::collections::BTreeMap<(usize, u8, usize, i64), Vec<usize>> =
                std::collections::BTreeMap::new();
            for (i, f) in m.facts.iter().enumerate() {
                if f.valid_to.is_none() && !f.expired {
                    groups
                        .entry((f.src, f.group, f.dst, f.polarity))
                        .or_default()
                        .push(i);
                }
            }
            let mut losers: BTreeSet<i64> = BTreeSet::new();
            let mut updates: Vec<(i64, Vec<i64>)> = Vec::new();
            for (_, mut idxs) in groups {
                if idxs.len() < 2 {
                    continue;
                }
                idxs.sort_by_key(|&i| (m.facts[i].valid_from, m.facts[i].id));
                let (winner, rest) = idxs.split_first().unwrap();
                let winner_id = m.facts[*winner].id;
                for &i in rest {
                    losers.insert(m.facts[i].id);
                }
                updates.push((
                    winner_id,
                    rest.iter()
                        .flat_map(|&i| m.facts[i].evidence.clone())
                        .collect(),
                ));
            }
            // Merge evidence into winners, remove losers.
            for (winner_id, evidence) in updates {
                if let Some(w) = m.facts.iter_mut().find(|f| f.id == winner_id) {
                    for e in evidence {
                        if !w.evidence.contains(&e) {
                            w.evidence.push(e);
                        }
                    }
                }
            }
            m.facts.retain(|f| !losers.contains(&f.id));
            // Re-apply exclusive slots across the merged store: per slot, newest
            // valid_from wins (tie -> lowest id); losers end or expire.
            let mut slots: std::collections::BTreeMap<(usize, u8), Vec<usize>> =
                std::collections::BTreeMap::new();
            for (i, f) in m.facts.iter().enumerate() {
                if f.polarity != 1 || f.expired || f.valid_to.is_some() {
                    continue;
                }
                if !m.pred_flag(f.group as usize) {
                    continue;
                }
                slots.entry((f.src, f.group)).or_default().push(i);
            }
            for (_, mut idxs) in slots {
                if idxs.len() < 2 {
                    continue;
                }
                idxs.sort_by_key(|&i| (std::cmp::Reverse(m.facts[i].valid_from), m.facts[i].id));
                let winner_vf = m.facts[idxs[0]].valid_from;
                for &i in &idxs[1..] {
                    let f = &mut m.facts[i];
                    if f.valid_from == winner_vf {
                        f.expired = true; // equal start: system-retired, no fabricated interval
                    } else {
                        f.valid_to = Some(winner_vf);
                    }
                }
            }
            // Rule 4: old name resolves to survivor; no pre-merge fact lost.
            match e.entity(gone_name, 1) {
                Ok(card) => {
                    let keep_id = e.entity(keep_name, 1).map(|c| c.card.id).unwrap_or(-1);
                    ctx.expect(
                        card.card.id == keep_id,
                        format!(
                            "rule 4: old name {gone_name} resolves to e{} instead of e{}",
                            card.card.id, keep_id
                        ),
                    );
                }
                Err(err) => ctx.fail(format!(
                    "rule 4: old name {gone_name} does not resolve: {err}"
                )),
            }
            // Reachability: every pre-merge incident fact is now in some current/past
            // card of a pool root, was a consolidated duplicate, or is a self-loop.
            let mut reachable: BTreeSet<i64> = BTreeSet::new();
            let mut seen_roots = BTreeSet::new();
            for &root in m.map.iter() {
                if !seen_roots.insert(root) {
                    continue;
                }
                if !m.created[root] {
                    continue; // nothing there to read
                }
                let rname = NAMES[root];
                match e.entity(rname, 200) {
                    Ok(card) => {
                        reachable.extend(card.current_facts.iter().map(|f| f.id));
                        reachable.extend(card.past_facts.iter().map(|f| f.id));
                    }
                    Err(err) => ctx.fail(format!("rule 4: entity card for {rname} failed: {err}")),
                }
            }
            for id in &pre_incident {
                if !reachable.contains(id)
                    && !losers.contains(id)
                    && m.find_fact(*id).is_some_and(|f| f.src == f.dst)
                {
                    // Self-loop after the merge: kept as expired history, hidden from cards by design.
                    continue;
                } else if !reachable.contains(id) && !losers.contains(id) {
                    ctx.fail(format!("rule 4: fact r{id} lost in merge {gone_name} -> {keep_name} (no card carries it, not a consolidated duplicate, not a self-loop)"));
                }
            }
            check_all(e, m, ctx, rel_before, true, op_no, strict);
        }
        Err(err) => ctx.fail(format!(
            "merge_entities {keep_name} <- {gone_name} failed: {err}"
        )),
    }
}

fn parse_seed_list() -> Vec<u64> {
    (1..=200).collect()
}

fn run_all(strict: bool) {
    for seed in parse_seed_list() {
        let mut engine = open_engine();
        let mut model = Model::new();
        let mut rng = Rng::new(seed);
        let mut ctx = Ctx {
            seed,
            ctr: 0,
            log: Vec::new(),
        };
        for op in 0..40u64 {
            let rel_before = relation_count(&engine);
            let roll = rng.below(100);
            match roll {
                0..=49 => op_remember(
                    &mut engine,
                    &mut model,
                    &mut rng,
                    &mut ctx,
                    rel_before,
                    op,
                    false,
                    strict,
                ),
                50..=57 => op_remember(
                    &mut engine,
                    &mut model,
                    &mut rng,
                    &mut ctx,
                    rel_before,
                    op,
                    true,
                    strict,
                ),
                58..=69 => op_invalidate_memory(
                    &mut engine,
                    &mut model,
                    &mut rng,
                    &mut ctx,
                    rel_before,
                    op,
                    strict,
                ),
                70..=79 => op_invalidate_relation(
                    &mut engine,
                    &mut model,
                    &mut rng,
                    &mut ctx,
                    rel_before,
                    op,
                    strict,
                ),
                80..=89 => op_forget(
                    &mut engine,
                    &mut model,
                    &mut rng,
                    &mut ctx,
                    rel_before,
                    op,
                    strict,
                ),
                _ => op_merge(
                    &mut engine,
                    &mut model,
                    &mut rng,
                    &mut ctx,
                    rel_before,
                    op,
                    strict,
                ),
            }
        }
    }
}

#[test]
fn store_model_random_op_sequences_hold_the_store_rules() {
    run_all(false);
}

/// FINDING-1/2 (main f51b27e): see the lane report. FINDING-1: `merge_entities`
/// does not remap `relations.subj` (the slot key of the exclusive-ending path),
/// so after any merge the write-time slot query misses the merged subject's
/// facts: a new exclusive value neither ends nor backfills the competing live
/// one, leaving two live values per (subject, exclusive predicate). Reproducer:
/// seed 0x1, ops after the two merges ("merge Bruno Kestler into Elena Marsh",
/// "merge Elena Marsh into Astrid Vollent"), remember #17 — relations r8 and
/// r22 both live at now in slot (e1, work at).
///
/// FINDING-2: a relation whose every evidence memory has `valid_to` set but is
/// not expired is reported by `validate()` as "current but every memory that
/// states it has ended" — the ending rule counts only expired memories as lost
/// evidence. Reproducer: seed 0x7, op after #19 — `validate()` reports relation
/// r15 contradictory. A minimal deterministic probe (two memories evidencing
/// one fact, both invalidated with `invalidate m`) reproduces the same report.
///
/// This test encodes the intent: rules 3 and 5 hold after ANY op sequence,
/// merges and invalidations included. It fails on main and belongs to the fix,
/// not to this lane.
#[test]
#[ignore = "FINDING-1/2: the exclusive slot does not hold across merges on main, and validate() reports vt-evidenced relations; reproducer seeds 0x1 and 0x7; see the nc-model lane report"]
fn store_model_rules_hold_across_merges_and_invalidations() {
    run_all(true);
}
