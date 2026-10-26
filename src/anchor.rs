//! Query anchors: which stored entities a question names.
//!
//! Tier 0 is today's exact alias match. `anchor_tiers` (Config) adds:
//! - `w`: generic-alias demotion. An alias hit is *weak* when the alias names
//!   several entities ("Simão" for two people) or is one word that is not a
//!   word of the entity's own name ("the company", "the CEO", "bakery").
//! - `1`: a capitalised query word (not the first word) that is the first or
//!   last word of exactly one multi-word entity name ("Noor's" -> Noor
//!   Haddad) is a weak anchor; lower-case words never are ("student" must
//!   not anchor "PhD student").
//! - `2`: a role noun next to an anchor ("the CFO of Mirante Bio", "Noah's
//!   boss") resolves through the one live relation that carries the role.
//! - `b`: weak anchors also block pass-through in the semantic beam.
//! - `x`: weak anchors alone also enable prose hops in the semantic beam.
//! - `p`: both `b` and `x` (weak anchors act as strong ones).
//!
//! Strong anchors keep every existing role. Weak anchors seed PPR and start
//! both beams, but do not make the semantic beam refuse to walk through them
//! nor switch on prose hops: those side effects cost holdout when every
//! token anchor was made exact (research/failure-analysis.md, E1).
use crate::db::Db;
use crate::text;
use rusqlite::params_from_iter;
use std::collections::{HashMap, HashSet};

/// Role words that name the manager side of the seeded `manage` group.
const BOSS_WORDS: &[&str] = &["boss", "manager", "supervisor"];
/// Role words matched as a word of a stored predicate ("is the CFO of").
const ROLE_WORDS: &[&str] = &[
    "ceo",
    "cto",
    "cfo",
    "coo",
    "cio",
    "founder",
    "owner",
    "director",
    "head",
    "lead",
    "chair",
    "president",
    "principal",
    "mentor",
    "coach",
    "doctor",
    "gp",
    "dentist",
    "lawyer",
    "landlord",
    "tutor",
    "supplier",
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anchors {
    /// Exact anchors: start beams, block pass-through, enable prose hops.
    pub strong: Vec<i64>,
    /// Weak anchors: start beams and seed PPR only. Disjoint from `strong`.
    pub weak: Vec<i64>,
    /// Anchors the semantic beam never walks through (strong, plus weak
    /// under `b`/`p`).
    pub block: Vec<i64>,
    /// Prose hops are allowed: a strong anchor exists, or a weak one under
    /// `x`/`p`.
    pub prose: bool,
}

impl Anchors {
    /// Strong then weak, the order path starts are taken in.
    pub fn all(&self) -> Vec<i64> {
        self.strong.iter().chain(&self.weak).copied().collect()
    }
}

/// Anchors for `query` and its normalised n-grams `names` (`extract::ngrams`
/// + `text::norm_name`, plus agent-supplied entities), under `tiers`.
pub fn query_anchors(
    db: &Db,
    query: &str,
    names: &[String],
    tiers: &str,
    now: i64,
) -> crate::Result<Anchors> {
    let demote = tiers.contains('w');
    // alias -> (entity, entity norm)
    let mut hits: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    for chunk in names.chunks(400) {
        let marks = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT DISTINCT a.alias_norm, a.entity_id, e.norm FROM entity_aliases a
             JOIN entities e ON e.id = a.entity_id WHERE a.alias_norm IN ({marks})"
        );
        let mut st = db.sql().prepare(&sql)?;
        let rows = st.query_map(params_from_iter(chunk.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (alias, id, norm) = row?;
            hits.entry(alias).or_default().push((id, norm));
        }
    }
    let mut strong: HashSet<i64> = HashSet::new();
    let mut weak: HashSet<i64> = HashSet::new();
    for (alias, entities) in &hits {
        for (id, norm) in entities {
            if demote && weak_alias(alias, norm, entities.len()) {
                weak.insert(*id);
            } else {
                strong.insert(*id);
            }
        }
    }
    if tiers.contains('1') {
        let capitalised: HashSet<String> = query
            .split_whitespace()
            .skip(1)
            .filter(|w| w.chars().next().is_some_and(char::is_uppercase))
            .map(text::norm_name)
            .collect();
        for token in names.iter().filter(|n| capitalised.contains(*n)) {
            if hits.contains_key(token) || token.chars().count() < 3 || text::is_stopword(token) {
                continue;
            }
            if let Some(id) = unique_name_token(db, token)? {
                weak.insert(id);
            }
        }
    }
    if tiers.contains('2') {
        let around: HashSet<i64> = strong.union(&weak).copied().collect();
        for token in names.iter().filter(|n| !n.contains(' ')) {
            if let Some(id) = role_anchor(db, token, &around, now)? {
                weak.insert(id);
            }
        }
    }
    weak.retain(|id| !strong.contains(id));
    let mut strong: Vec<i64> = strong.into_iter().collect();
    let mut weak: Vec<i64> = weak.into_iter().collect();
    strong.sort_unstable();
    weak.sort_unstable();
    let both = tiers.contains('p');
    let mut block = strong.clone();
    if both || tiers.contains('b') {
        block.extend(&weak);
        block.sort_unstable();
    }
    let prose = !strong.is_empty() || ((both || tiers.contains('x')) && !weak.is_empty());
    Ok(Anchors {
        strong,
        weak,
        block,
        prose,
    })
}

/// A generic descriptor or a shared short name, not an identifying alias.
fn weak_alias(alias: &str, entity_norm: &str, entities: usize) -> bool {
    entities > 1 || (!alias.contains(' ') && !entity_norm.split_whitespace().any(|w| w == alias))
}

/// The single multi-word entity whose name starts or ends with `token`.
fn unique_name_token(db: &Db, token: &str) -> crate::Result<Option<i64>> {
    let clean: String = token.chars().filter(|c| c.is_alphanumeric()).collect();
    if clean.is_empty() {
        return Ok(None);
    }
    // FTS (porter) narrows the candidates; the word test below is exact.
    let mut st = db.sql().prepare_cached(
        "SELECT e.id, e.norm FROM entities_fts f JOIN entities e ON e.id = f.rowid
         WHERE entities_fts MATCH ?1 LIMIT 64",
    )?;
    let rows = st.query_map([format!("name : \"{clean}\"*")], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut found: Option<i64> = None;
    for row in rows {
        let (id, norm) = row?;
        let words: Vec<&str> = norm.split_whitespace().collect();
        if words.len() < 2 {
            continue;
        }
        let same = |w: &str| w == token || text::singularize(w) == token;
        if same(words[0]) || same(words[words.len() - 1]) {
            if found.is_some_and(|f| f != id) {
                return Ok(None);
            }
            found = Some(id);
        }
    }
    Ok(found)
}

/// The one entity a role noun names through a live relation to an anchor.
fn role_anchor(
    db: &Db,
    token: &str,
    around: &HashSet<i64>,
    now: i64,
) -> crate::Result<Option<i64>> {
    let boss = BOSS_WORDS.contains(&token);
    if around.is_empty() || !(boss || ROLE_WORDS.contains(&token)) {
        return Ok(None);
    }
    let mut st = db.sql().prepare_cached(
        "SELECT r.src, r.dst, p.canonical, COALESCE(g.canonical, p.canonical)
         FROM relations r JOIN predicates p ON p.id = r.predicate_id
         LEFT JOIN predicates g ON g.id = p.group_id
         WHERE (r.src = ?1 OR r.dst = ?1) AND r.polarity = 1 AND r.expired_at IS NULL
           AND r.valid_from <= ?2 AND (r.valid_to IS NULL OR r.valid_to > ?2)",
    )?;
    let mut found: HashSet<i64> = HashSet::new();
    for &anchor in around {
        let rows = st.query_map(rusqlite::params![anchor, now], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (src, dst, canonical, group) = row?;
            let other = if boss {
                // Stored canonically as "manager manages subordinate".
                (group == "manage" && canonical == "manage" && dst == anchor).then_some(src)
            } else if canonical.split_whitespace().any(|w| w == token) {
                Some(if src == anchor { dst } else { src })
            } else {
                None
            };
            if let Some(other) = other.filter(|o| !around.contains(o)) {
                found.insert(other);
            }
        }
    }
    Ok((found.len() == 1).then(|| found.into_iter().next().unwrap()))
}
