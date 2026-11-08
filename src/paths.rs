//! Query-directed relation-path retrieval and agent-facing graph tools.
//!
//! This is the LLM-free shape of Think-on-Graph / RoG path retrieval: start at
//! the entities the question names, expand only over relations whose predicate
//! or fact text matches question terms, and keep the paths that cover the most
//! distinct question terms. The last hop's evidence memory is the answer, even
//! when that memory never names the entity the question started from.
//!
//! Terms are matched on a small expansion set (`lives` / `live`, plurals,
//! possessives) instead of a porter-style stem, because the same word is often
//! a noun in one fact and a verb in another ("lives in" vs "a life in").

use crate::db::Db;
use crate::engine::{Engine, FactHit};
use crate::text;
use crate::Result;
use rusqlite::{params, params_from_iter, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Paths kept per expansion step.
const BEAM: usize = 24;
/// Longest chain considered (world three-hop queries need 3).
const MAX_DEPTH: usize = 3;
/// Relations taken from one entity, best term match first.
const EDGES_PER_ENTITY: usize = 12;
/// Ranked paths kept per query.
const PATH_HITS: usize = 12;
/// Reverse traversal is allowed but less likely than forward in stored facts.
const REVERSE_PENALTY: f64 = 0.75;
/// Words that name the answer rather than a relation: they must not steer paths.
const ANSWER_WORDS: &[&str] = &[
    "address", "city", "count", "country", "date", "email", "kind", "language", "list", "name",
    "names", "number", "phone", "place", "town", "type", "value",
];

const NEGATION_WORDS: &[&str] = &[
    "arent", "cannot", "cant", "didnt", "doesnt", "dont", "except", "hadnt", "hasnt", "havent",
    "isnt", "neither", "never", "no", "nobody", "none", "nor", "not", "nothing", "wasnt", "werent",
    "without", "wont",
];

/// Which relations a query may traverse: the same bi-temporal rules `recall`
/// applies to its graph channel.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Visibility {
    pub now: i64,
    pub as_of: Option<i64>,
    pub history: bool,
}

impl Visibility {
    pub(crate) fn shows(
        &self,
        valid_from: i64,
        valid_to: Option<i64>,
        expired: Option<i64>,
    ) -> bool {
        let t = self.as_of.unwrap_or(self.now);
        if valid_from > t {
            return false;
        }
        if self.as_of.is_some() {
            return valid_to.is_none_or(|v| t < v);
        }
        if self.history {
            return true;
        }
        valid_to.is_none_or(|v| self.now < v) && expired.is_none()
    }
}

/// True when the text states a negative or retracted fact.
pub fn has_negation_cue(value: &str) -> bool {
    let folded = text::fold(value);
    if folded.contains("no longer") {
        return true;
    }
    folded.split_whitespace().any(|word| {
        let bare: String = word.chars().filter(|c| *c != '\'' && *c != '’').collect();
        NEGATION_WORDS.contains(&bare.as_str())
    })
}

/// True when the question asks for what is not the case.
///
/// Negation that scopes the speaker's own memory verb ("I don't remember
/// what database Lumen uses") is a request, not a question about what is not
/// the case, so that cue is screened out first. An exclusion question
/// ("which service is not in Rust") keeps its cue but is not a negation
/// question either: the answer is the item that lacks the property.
pub fn negation_intent(query: &str) -> bool {
    let folded = text::fold(query);
    let words: Vec<&str> = folded.split_whitespace().collect();
    let kept: Vec<&str> = {
        let mut kept = Vec::with_capacity(words.len());
        let mut i = 0;
        while i < words.len() {
            if memory_verb_negation(&words, i) {
                i += 2;
                continue;
            }
            kept.push(words[i]);
            i += 1;
        }
        kept
    };
    let kept = kept.join(" ");
    if exclusion_intent(&kept) {
        return false;
    }
    has_negation_cue(&kept)
}

/// True when the question asks for the value it excludes or that has ended
/// (H2-03): the negation sits inside the question's verb phrase ("what
/// database does Lumen not use", "what does Ana no longer work on", "who is
/// not on the team anymore", "which tools did we stop using"). Discourse
/// negations ("not sure", "why not", "isn't it") and the speaker's own
/// memory verb are not this frame.
pub fn negated_value_frame(query: &str) -> bool {
    let folded = text::fold(query);
    let words = bare_words(&folded);
    if words.is_empty() {
        return false;
    }
    // Speaker-memory negation ("I don't remember ...") is not about the world.
    {
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        let mut i = 0;
        while i < refs.len() {
            if memory_verb_negation(&refs, i) {
                return false;
            }
            i += 1;
        }
    }
    if folded.contains("not sure") {
        return false;
    }
    if folded.contains("why not") {
        return false;
    }
    // "Why not ..." asks for a reason to use something, not for the excluded value.
    if words.first().map(String::as_str) == Some("why")
        && words.get(1).map(String::as_str) == Some("not")
    {
        return false;
    }
    // "Isn't it" / "aren't they": a tag question, not a negated predicate.
    // A contraction + pronoun instance alone never counts.
    let is_pronoun = |w: &str| {
        matches!(
            w,
            "it" | "they"
                | "them"
                | "he"
                | "she"
                | "we"
                | "you"
                | "i"
                | "this"
                | "that"
                | "these"
                | "those"
                | "there"
        )
    };
    // "No longer" + a verb within three tokens ("no longer work on").
    if folded.contains("no longer") {
        if let Some(at) = folded.find("no longer") {
            let tail = &folded[at + "no longer".len()..];
            let tail_words = bare_words(tail);
            if tail_words
                .iter()
                .take(3)
                .any(|w| !w.is_empty() && w != "just" && w != "only")
            {
                return true;
            }
        }
        // A bare "no longer" with no verb after it is not a frame on its own;
        // the generic checks below may still fire on another cue.
    }
    // "Anymore" ("who is not on the team anymore") only counts with a negation
    // elsewhere; the AUX+NOT check below covers the brief's example.
    // "Stop" + a gerund ("did we stop using") names a property the subject had.
    for (i, word) in words.iter().enumerate() {
        if matches!(
            word.as_str(),
            "stop" | "stopped" | "stopping" | "cease" | "ceased"
        ) && words.get(i + 1).is_some()
        {
            return true;
        }
    }
    // AUX + negation + content ("does Lumen not use", "is not on"). The content
    // word must read as a predicate (verb/phrase), not a state adjective: "which
    // tests are not flaky" asks about a property, not an excluded/ended value.
    for (i, word) in words.iter().enumerate() {
        if AUXILIARIES.contains(&word.as_str()) {
            let window_end = (i + 4).min(words.len());
            for j in i + 1..window_end {
                let neg = &words[j];
                if (NEGATION_WORDS.contains(&neg.as_str()) || CONTRACTIONS.contains(&neg.as_str()))
                    && j + 1 < words.len()
                {
                    if let Some(follow) = words.get(j + 1) {
                        if follow == "sure" || is_pronoun(follow) || state_property_word(follow) {
                            continue;
                        }
                        return true;
                    }
                }
            }
        }
    }
    // Contracted negation + verb ("services that don't use Redis").
    for (i, word) in words.iter().enumerate() {
        if CONTRACTIONS.contains(&word.as_str()) {
            if let Some(follow) = words.get(i + 1) {
                if follow != "sure" && !is_pronoun(follow) && !state_property_word(follow) {
                    return true;
                }
            }
        }
    }
    false
}

/// State/property adjectives: "not flaky", "not ready" describe a property, not
/// an excluded or ended value (H2-03). Kept narrow so verb predicates still fire.
fn state_property_word(word: &str) -> bool {
    matches!(
        word,
        "flaky"
            | "buggy"
            | "stale"
            | "ready"
            | "safe"
            | "unsafe"
            | "risky"
            | "slow"
            | "fast"
            | "stable"
            | "unstable"
            | "valid"
            | "invalid"
            | "empty"
            | "null"
            | "nil"
            | "true"
            | "false"
            | "local"
            | "remote"
            | "public"
            | "private"
            | "internal"
            | "external"
            | "active"
            | "inactive"
            | "online"
            | "offline"
            | "available"
            | "unavailable"
            | "possible"
            | "impossible"
            | "visible"
            | "hidden"
            | "enabled"
            | "disabled"
            | "optional"
            | "required"
            | "correct"
            | "incorrect"
            | "wrong"
            | "right"
            | "equal"
            | "same"
            | "different"
            | "new"
            | "old"
            | "good"
            | "bad"
            | "fine"
            | "okay"
            | "ok"
            | "healthy"
            | "unhealthy"
            | "clean"
            | "dirty"
            | "open"
            | "closed"
            | "free"
            | "busy"
    )
}

fn bare_words(value: &str) -> Vec<String> {
    value.split_whitespace().map(bare_word).collect()
}

const AUXILIARIES: &[&str] = &[
    "are", "can", "could", "did", "do", "does", "had", "has", "have", "is", "might", "must",
    "shall", "should", "was", "were", "will", "would",
];

/// Negated contractions ("don't", "doesn't", ...) already folded to bare words.
const CONTRACTIONS: &[&str] = &[
    "arent", "cannot", "cant", "didnt", "doesnt", "dont", "hadnt", "hasnt", "havent", "isnt",
    "wasnt", "werent", "wont",
];

/// True when the question asks which item lacks a property ("which service is
/// not in Rust", "which services don't use Redis", "everything except Rust").
///
/// The negation qualifies the item the question wants, so recall must not
/// restrict the answer to memories that carry a negation cue.
pub fn exclusion_intent(query: &str) -> bool {
    let folded = text::fold(query);
    let words = bare_words(&folded);
    if folded.contains("other than") || folded.contains("apart from") {
        return true;
    }
    if words.iter().any(|w| w == "except" || w == "excluding") {
        return true;
    }
    exclusion_noun(&words).is_some()
}

/// Position of the noun an exclusion question asks about: the noun after a
/// leading wh-word ("which **service** is not ...", "what **database** does
/// lumen not use"), or the phrase before a negated relative clause
/// ("**services** that don't use Redis"). None when the negation belongs to
/// the question's own verb ("what did we decide not to use").
fn exclusion_noun(words: &[String]) -> Option<usize> {
    match words.first().map(String::as_str) {
        Some("which" | "what") => {
            let noun = words.iter().enumerate().skip(1).find_map(|(i, word)| {
                (!text::is_stopword(word) && !NEGATION_WORDS.contains(&word.as_str())).then_some(i)
            })?;
            negated_predicate(&words[noun + 1..]).then_some(noun)
        }
        _ => {
            let relative = words.iter().position(|w| w == "that" || w == "which")?;
            let noun = words[..relative]
                .iter()
                .rposition(|w| !text::is_stopword(w))?;
            negated_predicate(&words[relative + 1..]).then_some(noun)
        }
    }
}

/// True when the words after the noun hold a negated predicate of that noun:
/// "is not in Rust", "don't use Redis", "does Lumen not use Redis".
fn negated_predicate(rest: &[String]) -> bool {
    let window = &rest[..rest.len().min(4)];
    for (i, word) in window.iter().enumerate() {
        if CONTRACTIONS.contains(&word.as_str()) {
            return true;
        }
        if AUXILIARIES.contains(&word.as_str()) {
            return window[i + 1..]
                .iter()
                .take(2)
                .any(|w| NEGATION_WORDS.contains(&w.as_str()));
        }
    }
    false
}

/// Words that build the verb phrase of a property question ("is **written in**
/// Rust") instead of naming the property itself.
const PROPERTY_SKIP: &[&str] = &[
    "anymore", "belong", "belongs", "depend", "depends", "each", "either", "every", "have",
    "having", "just", "like", "likes", "member", "need", "needs", "only", "part", "run", "running",
    "runs", "still", "support", "supports", "use", "used", "uses", "using", "want", "wants",
    "work", "worked", "working", "works", "write", "writes", "written", "wrote", "yet",
];

/// Cap on the type population an exclusion question resolves. Above it the
/// channel is skipped and the ordinary channels answer, so one question can
/// never scan a whole type (and its memories) on the low-resource target.
const EXCLUSION_MAX_ENTITIES: usize = 1000;

/// The typed entities an exclusion question asks about: the ones with no live
/// positive fact naming the property (`lacking`), and the ones that do have it
/// (the wrong answers for this question, `with_property`) together with the
/// relation ids that say so (`facts`).
pub struct Exclusion {
    pub lacking: Vec<i64>,
    pub with_property: Vec<i64>,
    pub facts: Vec<i64>,
}

/// Resolve the type and the property of an exclusion question through the
/// stored graph: "which service is not in Rust" -> the `service` entities with
/// no live relation to the `Rust` entity. None when the question is not an
/// exclusion question or the type/property names nothing stored, so the
/// ordinary channels answer as before.
pub fn exclusion_entities(db: &Db, query: &str, now: i64) -> Result<Option<Exclusion>> {
    let words = bare_words(&text::fold(query));
    let Some(noun) = exclusion_noun(&words) else {
        return Ok(None);
    };
    let kind = text::singularize(&words[noun]);
    let mut st = db
        .sql()
        .prepare("SELECT id FROM entities WHERE type=?1 ORDER BY id")?;
    let typed: Vec<i64> = st
        .query_map([&kind], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    if typed.is_empty() || typed.len() > EXCLUSION_MAX_ENTITIES {
        return Ok(None);
    }
    let property = words[noun + 1..].iter().find_map(|word| {
        if word.chars().count() < 2
            || text::is_stopword(word)
            || NEGATION_WORDS.contains(&word.as_str())
            || AUXILIARIES.contains(&word.as_str())
            || CONTRACTIONS.contains(&word.as_str())
            || PROPERTY_SKIP.contains(&word.as_str())
        {
            return None;
        }
        db.sql()
            .query_row(
                "SELECT entity_id FROM entity_aliases WHERE alias_norm=?1 LIMIT 1",
                [word],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
    });
    let Some(property) = property else {
        return Ok(None);
    };
    let mut lacking = Vec::new();
    let mut with_property = Vec::new();
    let mut facts = Vec::new();
    let mut linked = db.sql().prepare_cached(
        "SELECT r.id FROM relations r
          WHERE r.polarity=1 AND r.expired_at IS NULL
            AND (r.valid_to IS NULL OR r.valid_to>?3)
            AND ((r.src=?1 AND r.dst=?2) OR (r.dst=?1 AND r.src=?2))",
    )?;
    for id in typed {
        if id == property {
            continue;
        }
        let ids: Vec<i64> = linked
            .query_map(params![id, property, now], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if ids.is_empty() {
            lacking.push(id);
        } else {
            with_property.push(id);
            facts.extend(ids);
        }
    }
    Ok(Some(Exclusion {
        lacking,
        with_property,
        facts,
    }))
}

fn bare_word(word: &str) -> String {
    word.chars().filter(|c| *c != '\'' && *c != '’').collect()
}

/// True when `words[i]` is a negation cue that negates a memory verb at
/// `words[i + 1]` ("don't remember", "cannot recall", "can't recollect").
fn memory_verb_negation(words: &[&str], i: usize) -> bool {
    if i + 1 >= words.len() || !NEGATION_WORDS.contains(&bare_word(words[i]).as_str()) {
        return false;
    }
    matches!(words[i + 1], "remember" | "recall" | "recollect")
}

/// True when the question asks for a cause ("why did X happen", "what caused
/// X", "reason for X"). E4b uses this to surface causal evidence first.
pub fn causal_intent(query: &str) -> bool {
    let q = text::fold(query);
    q.starts_with("why")
        || q.contains("what caused")
        || q.contains("what led to")
        || q.contains("reason for")
        || q.contains("how come")
}

/// H2-15: a question that asks which defects are *unresolved* ("still open",
/// "not fixed yet"). The question must name a defect of its own: "is the API
/// still open to the public" and "which PRs are outstanding" describe a
/// state, not a bug. "open question" is not a bug status.
pub fn status_intent(query: &str) -> bool {
    let q = text::fold(query);
    // An open question is a pending decision, not an open defect: the lexical
    // "open" must not turn it into a status word.
    if q.contains("open question") {
        return false;
    }
    const CUES: &[&str] = &[
        "still open",
        "still broken",
        "still failing",
        "still unresolved",
        "unresolved",
        "unfixed",
        "not fixed",
        "not yet fixed",
        "not been fixed",
        "open bug",
        "open bugs",
        "open issue",
        "open issues",
        "open defect",
        "open defects",
        "open ticket",
        "open tickets",
        "outstanding",
        "left to fix",
        "remaining bug",
        "remaining issue",
    ];
    if !CUES.iter().any(|cue| q.contains(cue)) {
        return false;
    }
    // A predicate that can only be about a defect asks for one on its own
    // ("what is still broken", "what is left to fix").
    const ASKS_DEFECT: &[&str] = &[
        "still broken",
        "still failing",
        "still crashing",
        "left to fix",
        "unfixed",
        "not fixed",
        "not yet fixed",
        "not been fixed",
    ];
    if ASKS_DEFECT.iter().any(|cue| q.contains(cue)) {
        return true;
    }
    // Otherwise the question names the defect it asks about.
    const DEFECT_NOUNS: &[&str] = &[
        "bug",
        "bugs",
        "issue",
        "issues",
        "defect",
        "defects",
        "crash",
        "crashes",
        "regression",
        "regressions",
        "error",
        "errors",
        "failure",
        "failures",
    ];
    let tokens = text::tokens(&q);
    if tokens
        .iter()
        .any(|token| DEFECT_NOUNS.contains(&token.as_str()))
    {
        return true;
    }
    // A ticket or an incident is a defect only when the question says so too
    // ("which incident tickets are still open for the 500 error").
    const CONTAINERS: &[&str] = &["ticket", "tickets", "incident", "incidents"];
    tokens
        .iter()
        .any(|token| CONTAINERS.contains(&token.as_str()))
        && has_cue(&q, BUG_CUES)
}

/// Tokens that report a defect ("Bug on ...", "Error on ...", "the sync loop
/// panicked", "the nightly export crashes"). Inflections matter: an agent
/// writes "crashes", "fails" or "can't" far more often than "crashed".
const BUG_CUES: &[&str] = &[
    "bug",
    "bugs",
    "broken",
    "regression",
    "regressions",
    "panic",
    "panicked",
    "crash",
    "crashes",
    "crashing",
    "crashed",
    "error",
    "errors",
    "fail",
    "fails",
    "failed",
    "failing",
    "failure",
    "failures",
    "defect",
    "defects",
    "hang",
    "hangs",
    "hanging",
    "cannot",
    "cant",
];

/// Tokens that say a defect was dealt with ("Fix for ...", "resolved",
/// "released", "merged"). "merge" is not one: `merge_note_versions` is a
/// function name.
const FIX_CUES: &[&str] = &[
    "fix", "fixed", "fixes", "fixing", "resolved", "resolve", "resolves", "closed", "merged",
    "patched", "hotfix", "released", "shipped", "bump", "bumped",
];

/// Words that say the work is still to be done ("needs a fix", "pending").
const FIX_PENDING: &[&str] = &[
    "await", "awaiting", "need", "needed", "needing", "needs", "pending", "require", "required",
    "requires",
];

/// Noun forms of a fix: only these also take the negation right after them
/// ("the fix is not deployed").
const FIX_NOUNS: &[&str] = &["fix", "fixes", "fixing", "hotfix", "patch"];

/// True when the fix cue at `words[i]` is negated or still pending, so the
/// sentence reports an *unfixed* defect ("not fixed yet", "no fix yet",
/// "needs a fix", "the fix is not deployed").
fn fix_cue_negated(words: &[String], i: usize) -> bool {
    let lo = i.saturating_sub(3);
    if words[lo..i]
        .iter()
        .any(|w| NEGATION_WORDS.contains(&w.as_str()) || FIX_PENDING.contains(&w.as_str()))
    {
        return true;
    }
    FIX_NOUNS.contains(&words[i].as_str())
        && i + 2 < words.len()
        && AUXILIARIES.contains(&words[i + 1].as_str())
        && matches!(
            words[i + 2].as_str(),
            "not" | "never" | "no" | "needed" | "pending" | "required" | "awaited"
        )
}

/// Generic words that say nothing about *which* bug a memory is about.
const STATUS_GENERIC: &[&str] = &[
    "bug", "bugs", "issue", "issues", "error", "errors", "defect", "defects", "fix", "fixed",
    "fixes", "fixing", "lumen", "project", "reported", "report", "release", "released", "shipped",
    "still", "open", "question", "closed", "merged", "resolved", "patched",
];

fn has_cue(text: &str, cues: &[&str]) -> bool {
    text::tokens(text).iter().any(|token| {
        // "can't" and "can’t" are the same report as "cant".
        cues.contains(&token.as_str()) || cues.contains(&bare_word(token).as_str())
    })
}

/// A memory that reports a defect, not one that fixes or releases past it.
pub(crate) fn is_bug_memory(text: &str) -> bool {
    has_cue(text, BUG_CUES)
}

pub(crate) fn has_fix_cue(text: &str) -> bool {
    // A sentence boundary ends a negation's scope: "Fixed the crash. No
    // regressions since." reports a fix.
    text.split(['.', ';', '!', '?', '\n']).any(|clause| {
        let words = bare_words(&text::fold(clause));
        words
            .iter()
            .enumerate()
            .any(|(i, w)| FIX_CUES.contains(&w.as_str()) && !fix_cue_negated(&words, i))
    })
}

/// The words two memories must share to be about the same bug.
pub(crate) fn status_terms(text: &str) -> HashSet<String> {
    text::tokens(text)
        .into_iter()
        .filter(|token| {
            token.chars().count() >= 3
                // A year says nothing about which defect a memory is about;
                // two bugs filed the same day share it and nothing else.
                && !token.chars().all(|c| c.is_ascii_digit())
                && !text::is_stopword(token)
                && !STATUS_GENERIC.contains(&token.as_str())
        })
        .collect()
}

/// The specific defect a status question names ("nightly", "export" in "is
/// the nightly export bug still open"), without the status scaffolding.
/// Empty when the question asks about defects in general ("what bug is still
/// open"), in which case every open report is an answer.
pub(crate) fn status_question_terms(query: &str) -> HashSet<String> {
    const SCAFFOLD: &[&str] = &[
        "any",
        "broken",
        "crash",
        "crashes",
        "crashing",
        "failing",
        "incident",
        "incidents",
        "known",
        "left",
        "list",
        "open",
        "outstanding",
        "remaining",
        "show",
        "status",
        "still",
        "ticket",
        "tickets",
        "unfixed",
        "unresolved",
        "yet",
    ];
    status_terms(query)
        .into_iter()
        .filter(|token| !SCAFFOLD.contains(&token.as_str()))
        .collect()
}

/// `YYYY-MM-DD` literals: the strongest "same bug" signal a report carries.
fn date_literals(text: &str) -> HashSet<String> {
    let bytes = text.as_bytes();
    let mut out = HashSet::new();
    let mut i = 0;
    while i + 10 <= bytes.len() {
        let window = &bytes[i..i + 10];
        let date = window[4] == b'-'
            && window[7] == b'-'
            && window
                .iter()
                .enumerate()
                .all(|(k, b)| k == 4 || k == 7 || b.is_ascii_digit());
        if date {
            out.insert(text[i..i + 10].to_string());
            i += 10;
        } else {
            i += 1;
        }
    }
    out
}

/// True when `fix` is about the same defect as `bug`: two content words, or a
/// shared date plus one. A date alone is a coincidence: a release note dated
/// the day a bug was filed says nothing about that bug.
pub(crate) fn same_bug(bug: &str, fix: &str) -> bool {
    let shared = status_terms(bug).intersection(&status_terms(fix)).count();
    if !date_literals(bug).is_disjoint(&date_literals(fix)) {
        return shared >= 1;
    }
    shared >= 2
}

/// Every surface form a word may take in a stored fact.
fn token_forms(token: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut push = |w: &str| {
        if w.chars().count() > 1 {
            out.insert(w.to_string());
        }
    };
    push(token);
    let base = token.strip_suffix("'s").unwrap_or(token);
    push(base);
    push(&text::singularize(base));
    let verb = text::norm_predicate(base);
    if !verb.contains(' ') {
        push(&verb);
    }
    out
}

fn text_forms(value: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for token in text::tokens(value) {
        out.extend(token_forms(&token));
    }
    out
}

/// Question words mapped onto the relation vocabulary. "home" never appears
/// in "Liam Vijappa lives in Denver", so a purely lexical walk would miss the
/// last hop of "the manager of X's sister's home city". Synonyms are added to
/// the surface forms rather than replacing them.
const TERM_SYNONYMS: &[(&str, &[&str])] = &[
    ("home", &["live"]),
    ("lives", &["home"]),
    ("live", &["home"]),
    ("job", &["work"]),
    ("employer", &["work"]),
    ("company", &["work"]),
    ("work", &["company"]),
    ("works", &["company"]),
    ("boss", &["manager"]),
    ("supervisor", &["manager"]),
    ("sister", &["sibling"]),
    ("brother", &["sibling"]),
    ("sibling", &["brother", "sister"]),
    ("language", &["use"]),
    ("stack", &["use"]),
    ("prefer", &["like"]),
    ("prefers", &["like"]),
    ("like", &["prefer"]),
    ("likes", &["prefer"]),
];

/// Question terms in the order they appear, minus question words, answer-type
/// words and the entity names the question anchors on.
fn query_terms(query: &str, anchor_names: &[String]) -> Vec<HashSet<String>> {
    let anchors: HashSet<String> = anchor_names
        .iter()
        .flat_map(|name| text::tokens(name))
        .flat_map(|token| token_forms(&token))
        .collect();
    let mut out: Vec<HashSet<String>> = Vec::new();
    for token in text::tokens(query) {
        let bare: String = token.chars().filter(|c| *c != '\'').collect();
        if token.chars().count() < 2 || text::is_stopword(&token) {
            continue;
        }
        let synonyms: HashSet<String> = TERM_SYNONYMS
            .iter()
            .filter(|(word, _)| *word == token)
            .flat_map(|(_, extra)| extra.iter().map(|word| (*word).to_string()))
            .collect();
        // An answer-type word ("language", "city") says what the answer is,
        // not which relation a hop states; it steers the path only through
        // its relation synonyms ("language" -> use).
        let forms = if ANSWER_WORDS.contains(&bare.as_str()) {
            if synonyms.is_empty() {
                continue;
            }
            synonyms
        } else {
            let mut forms = token_forms(&token);
            forms.extend(synonyms);
            forms
        };
        if !forms.is_disjoint(&anchors) || out.iter().any(|existing| !existing.is_disjoint(&forms))
        {
            continue;
        }
        out.push(forms);
        if out.len() >= 16 {
            break;
        }
    }
    out
}

fn edge_match(forms: &HashSet<String>, terms: &[HashSet<String>]) -> (u64, usize) {
    let mut mask = 0u64;
    let mut matched = 0usize;
    for (index, term) in terms.iter().enumerate() {
        if !forms.is_disjoint(term) {
            mask |= 1 << index;
            matched += 1;
        }
    }
    (mask, matched)
}

#[derive(Clone, Debug)]
struct Edge {
    relation_id: i64,
    src: i64,
    dst: i64,
    other: i64,
    predicate: String,
    fact: String,
    reverse: bool,
    mask: u64,
    quality: f64,
}

/// One traversed relation. `reverse` means the walk went dst -> src, which is
/// how "Mira Shah is Noah's manager" is followed from Noah.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Step {
    pub relation_id: i64,
    pub src: i64,
    pub dst: i64,
    pub predicate: String,
    pub fact: String,
    pub reverse: bool,
    pub matched: usize,
    /// A text-only memory used as the hop (no relation behind it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PathHit {
    pub steps: Vec<Step>,
    /// Distinct question terms this chain covers.
    pub coverage: usize,
    pub score: f64,
}

#[derive(Clone, Debug)]
struct State {
    entity: i64,
    covered: u64,
    score: f64,
    steps: Vec<Step>,
}

fn incident(
    db: &Db,
    entity: i64,
    vis: Visibility,
    terms: &[HashSet<String>],
    cache: &mut HashMap<i64, Vec<Edge>>,
) -> Result<Vec<Edge>> {
    if let Some(edges) = cache.get(&entity) {
        return Ok(edges.clone());
    }
    let mut st = db.sql().prepare_cached(
        "SELECT r.id,r.src,r.dst,p.canonical,r.fact,r.valid_from,r.valid_to,r.expired_at
         FROM relations r JOIN predicates p ON p.id=r.predicate_id
         WHERE r.src=?1 OR r.dst=?2",
    )?;
    let rows = st.query_map(params![entity, entity], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<i64>>(7)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (relation_id, src, dst, predicate, fact, valid_from, valid_to, expired) = row?;
        if src == dst || !vis.shows(valid_from, valid_to, expired) {
            continue;
        }
        let reverse = dst == entity;
        let forms = text_forms(&format!("{predicate} {fact}"));
        let (mask, matched) = edge_match(&forms, terms);
        if mask == 0 {
            continue;
        }
        let quality = (0.4 + 0.6 * matched as f64 / terms.len() as f64)
            * if reverse { REVERSE_PENALTY } else { 1.0 };
        out.push(Edge {
            relation_id,
            src,
            dst,
            other: if reverse { src } else { dst },
            predicate,
            fact,
            reverse,
            mask,
            quality,
        });
    }
    out.sort_by(|a, b| {
        b.mask
            .count_ones()
            .cmp(&a.mask.count_ones())
            .then(b.quality.total_cmp(&a.quality))
            .then(a.relation_id.cmp(&b.relation_id))
    });
    out.truncate(EDGES_PER_ENTITY);
    cache.insert(entity, out.clone());
    Ok(out)
}

/// "<word>'s <noun>" pairs of a question. `text::fold` drops the apostrophe
/// inside a word, so the possessive is read from the raw query, not `tokens`.
fn possessive_pairs(query: &str) -> Vec<(String, String)> {
    let raw: Vec<&str> = query.split_whitespace().collect();
    let mut out = Vec::new();
    for (i, word) in raw.iter().enumerate() {
        let lower = word.to_lowercase();
        let Some(stem) = lower
            .strip_suffix("'s")
            .or_else(|| lower.strip_suffix("\u{2019}s"))
        else {
            continue;
        };
        let Some(noun) = raw.get(i + 1) else {
            continue;
        };
        let (stem, noun) = (text::fold(stem), text::fold(noun));
        if stem.is_empty() || noun.is_empty() {
            continue;
        }
        out.push((stem, noun));
    }
    out
}

/// The term a possessive head noun spends, when the question names one.
fn possessive_noun_bit(terms: &[HashSet<String>], noun: &str) -> Option<usize> {
    let forms = token_forms(noun);
    terms.iter().position(|term| !term.is_disjoint(&forms))
}

/// "<Person>'s project" names the entity of that type the person holds: the
/// possessive is itself the hop, so path search starts at that entity with the
/// noun's term already covered. One clear entity only, like an anchor's role
/// noun; an untyped or ambiguous holder leaves the beam as it was.
fn possessive_starts(
    db: &Db,
    query: &str,
    terms: &[HashSet<String>],
    seeds: &[i64],
    vis: Visibility,
) -> Result<Vec<(i64, u64)>> {
    let mut out: Vec<(i64, u64)> = Vec::new();
    for (stem, noun) in possessive_pairs(query) {
        let Some(index) = possessive_noun_bit(terms, &noun) else {
            continue;
        };
        let want = text::singularize(&noun);
        for id in seeds {
            if out.iter().any(|(entity, _)| entity == id) {
                continue;
            }
            let name: String = db
                .sql()
                .query_row("SELECT name FROM entities WHERE id=?1", [id], |r| r.get(0))
                .unwrap_or_default();
            if !text::tokens(&name).contains(&stem) {
                continue;
            }
            let mut st = db.sql().prepare_cached(
                "SELECT r.dst,e.type,r.valid_from,r.valid_to,r.expired_at
                   FROM relations r JOIN entities e ON e.id=r.dst
                  WHERE r.src=?1 AND r.polarity=1",
            )?;
            let rows = st.query_map([id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                ))
            })?;
            let mut found: Vec<i64> = Vec::new();
            for row in rows {
                let (dst, kind, from, to, expired) = row?;
                if !vis.shows(from, to, expired) || text::norm_name(&kind) != want {
                    continue;
                }
                if !found.contains(&dst) {
                    found.push(dst);
                }
            }
            if found.len() == 1 && !out.iter().any(|(entity, _)| *entity == found[0]) {
                out.push((found[0], 1u64 << index));
            }
        }
    }
    Ok(out)
}

/// Ranked paths that start at one of `seeds` and add at least one new question
/// term per hop.
pub(crate) fn search(db: &Db, query: &str, seeds: &[i64], vis: Visibility) -> Result<Vec<PathHit>> {
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for id in seeds {
        if let Ok(name) = db
            .sql()
            .query_row("SELECT name FROM entities WHERE id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
        {
            names.push(name);
        }
    }
    let terms = query_terms(query, &names);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let min_coverage = if terms.len() >= 2 { 2 } else { 1 };
    let mut frontier: Vec<State> = seeds
        .iter()
        .map(|id| State {
            entity: *id,
            covered: 0,
            score: 1.0,
            steps: Vec::new(),
        })
        .collect();
    // "<Person>'s project" already names the entity of that type the person
    // holds: start there with the noun's own term spent, so the next hop is
    // the one the question actually asks.
    for (entity, covered) in possessive_starts(db, query, &terms, seeds, vis)? {
        match frontier.iter_mut().find(|state| state.entity == entity) {
            Some(state) => state.covered |= covered,
            None => frontier.push(State {
                entity,
                covered,
                score: 1.0,
                steps: Vec::new(),
            }),
        }
    }
    frontier.sort_by_key(|state| state.entity);
    frontier.dedup_by_key(|state| state.entity);

    let mut cache: HashMap<i64, Vec<Edge>> = HashMap::new();
    let mut hits: Vec<PathHit> = Vec::new();
    for _ in 0..MAX_DEPTH {
        let mut next: Vec<State> = Vec::new();
        for state in &frontier {
            for edge in incident(db, state.entity, vis, &terms, &mut cache)? {
                let covered = state.covered | edge.mask;
                if covered == state.covered {
                    // Every hop must earn its place with a new question term.
                    continue;
                }
                let mut steps = state.steps.clone();
                steps.push(Step {
                    relation_id: edge.relation_id,
                    src: edge.src,
                    dst: edge.dst,
                    predicate: edge.predicate.clone(),
                    fact: edge.fact.clone(),
                    reverse: edge.reverse,
                    matched: edge.mask.count_ones() as usize,
                    memory: None,
                });
                next.push(State {
                    entity: edge.other,
                    covered,
                    score: state.score * edge.quality,
                    steps,
                });
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by(|a, b| {
            a.entity
                .cmp(&b.entity)
                .then(a.covered.cmp(&b.covered))
                .then(b.score.total_cmp(&a.score))
        });
        next.dedup_by(|a, b| a.entity == b.entity && a.covered == b.covered);
        for state in &next {
            let coverage = state.covered.count_ones() as usize;
            if coverage >= min_coverage {
                hits.push(PathHit {
                    steps: state.steps.clone(),
                    coverage,
                    score: state.score,
                });
            }
        }
        next.sort_by(|a, b| {
            b.covered
                .count_ones()
                .cmp(&a.covered.count_ones())
                .then(b.score.total_cmp(&a.score))
                .then(a.steps.len().cmp(&b.steps.len()))
        });
        next.truncate(BEAM);
        frontier = next;
    }

    hits.sort_by(|a, b| {
        b.coverage
            .cmp(&a.coverage)
            .then(b.score.total_cmp(&a.score))
            .then(a.steps.len().cmp(&b.steps.len()))
    });
    let mut seen: HashSet<(i64, usize)> = HashSet::new();
    hits.retain(|hit| {
        let end = hit
            .steps
            .last()
            .map(|step| if step.reverse { step.src } else { step.dst })
            .unwrap_or(0);
        seen.insert((end, hit.coverage))
    });
    hits.truncate(PATH_HITS);
    Ok(hits)
}

/// Facts recognised as relevant to the question: one KNN over relation fact
/// vectors bounds the semantic beam below.
pub(crate) const SEM_FACTS: usize = 100;

/// The `SEM_FACTS` relations nearest the question, best first, with cosine.
pub(crate) fn relation_knn(db: &Db, query_vec: &[f32]) -> Result<Vec<(i64, f32)>> {
    Ok(db
        .sql()
        .prepare_cached(&format!(
            "SELECT rowid,1-distance FROM relations_vec
             WHERE embedding MATCH {} AND k=?2 ORDER BY distance",
            db.vec_sql("?1")
        ))?
        .query_map(params![db.vec_blob(query_vec), SEM_FACTS as i64], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?)
}

/// Edges taken from one entity in the semantic beam.
const SEM_EDGES: usize = 6;
/// Most relations an entity may carry before the semantic beam stops
/// admitting its facts wholesale (outside the relevance KNN): a hop's
/// relation can rank below the top-100 KNN at 108k distractors, and a
/// small-degree bridge's own facts must stay reachable.
const SEM_WIDEN_CAP: usize = 32;
/// The widening (and the extra beam depth) engage only once the store is
/// past this size: below it, KNN-100 spans essentially the whole relation
/// set, so the stranded-hop mechanism cannot occur anyway.
const SEM_WIDEN_MIN_STORE: i64 = 50_000;
/// Deepest semantic walk (edges): the chain form the seeded benchmarks ask
/// for (depth 4 walked to the world hubs and buried the answers, so the
/// wider walk stays off).
const SEM_MAX_DEPTH: usize = 3;
/// Score factor for a hop through a text-only memory.
const PROSE_HOP: f64 = 0.8;
/// Score factor for a hop through a name-containment link (B5): the write
/// path stores the same weight, and it stays below a stated relation and
/// below prose. Such a hop carries no fact, only the next entity.
pub(crate) const NAME_PART_HOP: f64 = 0.6;
/// Target of a hop through a memory that names no other entity.
const NO_ENTITY: i64 = -1;

/// Learned hop scorer weights (L2 logistic regression, raw feature scale) over
/// [cos(question,fact), cos(residual,fact), hop_index, prose, pred_overlap,
///  log(1+degree), far_mentioned]. Trained by `scripts/train_hop_scorer.py` on
/// `NEOCORTEX_DUMP_HOPS` dumps of eval/fixtures.json and gen_world seeds 42 and
/// 11 (100 people each), 2026-10-26. Used only when `hop_scorer=on`.
const HOP_WEIGHTS: [f64; 7] = [
    19.9840337,
    -2.7410487,
    -0.41490904,
    -0.151894685,
    10.0273381,
    -0.0860151107,
    -0.672498487,
];
const HOP_BIAS: f64 = -10.0124513;

fn learned_hop_score(features: &[f64]) -> f64 {
    let mut z = HOP_BIAS;
    for (w, f) in HOP_WEIGHTS.iter().zip(features) {
        z += w * f;
    }
    1.0 / (1.0 + (-z).exp())
}

/// Hop score in the semantic beam: the learned scorer when on, else the cosine.
fn hop_value(use_scorer: bool, cosine: f64, features: &[f64]) -> f64 {
    if use_scorer {
        learned_hop_score(features)
    } else {
        cosine
    }
}

fn entity_degree(db: &Db, id: i64, cache: &mut HashMap<i64, f64>) -> f64 {
    if id == NO_ENTITY {
        return 0.0;
    }
    if let Some(v) = cache.get(&id) {
        return *v;
    }
    let count: i64 = db
        .sql()
        .query_row(
            "SELECT count(*) FROM mentions WHERE entity_id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let v = (count as f64 + 1.0).ln();
    cache.insert(id, v);
    v
}

/// `Config.name_part_hop` is on: finite and positive.
pub(crate) fn name_hop_on(name_hop: f64) -> bool {
    name_hop.is_finite() && name_hop > 0.0
}

/// Semantic twin of [`search`] for questions that paraphrase the stored
/// predicates ("who guides Noor's career" vs `mentors`): hops may only use
/// relations among the question's nearest facts. Each hop is scored against
/// the residual question (the question vector minus what earlier hops already
/// explained) and a path scores the sum, so a chain beats a single fact that
/// matches only part of the question. Only paths of >= 2 hops are emitted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn search_semantic(
    db: &Db,
    query: &str,
    query_vec: &[f32],
    seeds: &[i64],
    anchors: &[i64],
    prose_hops: bool,
    nearest_memories: &[i64],
    relation_knn: &[(i64, f32)],
    vis: Visibility,
    use_scorer: bool,
    name_hop: f64,
) -> Result<Vec<PathHit>> {
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let mut query_forms: HashSet<String> = HashSet::new();
    for t in text::tokens(query) {
        query_forms.extend(token_forms(&t));
    }
    // Facts the question recognises (`relation_knn`); their vectors are read
    // lazily, only for the edges the beam reaches (a vec0 vector read costs
    // ~0.3 ms).
    let relevant: HashSet<i64> = relation_knn.iter().map(|(id, _)| *id).collect();
    let mut relation_vector_st = db
        .sql()
        .prepare_cached("SELECT embedding FROM relations_vec WHERE rowid=?1")?;
    // Text-only memories are hops too (passage nodes): a memory that states
    // "Atlântico Ventures was founded in Oeiras" in prose links the entities
    // it mentions, scored by its own vector. Only the question's nearest
    // memories qualify, indexed once by the entities they mention.
    type Prose = (i64, String, Vec<i64>);
    let mut prose: HashMap<i64, Vec<Prose>> = HashMap::new();
    // Prose hops need an exact anchor to stand on (`prose_hops`): from fuzzy
    // seeds alone they chain unrelated memories ("the boat she takes out on
    // the river").
    if prose_hops {
        // The recall vector channel's neighbours; vectors are read lazily
        // (a vec0 vector read costs ~0.3 ms) for the memories a hop reaches.
        let mut memory_st = db.sql().prepare_cached(
            "SELECT content,valid_from,valid_to,expired_at FROM memories m WHERE id=?1
               AND NOT EXISTS (SELECT 1 FROM relation_evidence e WHERE e.memory_id=m.id)",
        )?;
        let mut mentions_st = db
            .sql()
            .prepare_cached("SELECT DISTINCT entity_id FROM mentions WHERE memory_id=?1")?;
        for &id in nearest_memories {
            let Some((content, from, to, expired)) = memory_st
                .query_row([id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, Option<i64>>(3)?,
                    ))
                })
                .optional()?
            else {
                continue;
            };
            if !vis.shows(from, to, expired) {
                continue;
            }
            let mentioned: Vec<i64> = mentions_st
                .query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            for &entity in &mentioned {
                let others = mentioned.iter().copied().filter(|x| *x != entity).collect();
                prose
                    .entry(entity)
                    .or_default()
                    .push((id, content.clone(), others));
            }
        }
    }
    let mut edges_st = db.sql().prepare_cached(
        "SELECT r.id,r.src,r.dst,p.canonical,r.fact,r.valid_from,r.valid_to,r.expired_at
         FROM relations r JOIN predicates p ON p.id=r.predicate_id
         WHERE (r.src=?1 OR r.dst=?1) AND r.polarity=1",
    )?;
    // B5: containment links are weak hops ("Aveiro" -> "Hackerspace Aveiro"),
    // scored by the target entity's own card vector. Loaded once: the beam
    // visits hundreds of entities and a per-entity query costs ~14% of p50.
    let mut name_parts: HashMap<i64, Vec<i64>> = HashMap::new();
    if name_hop_on(name_hop) {
        let mut st = db.sql().prepare("SELECT a,b FROM name_part")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (a, b) = row?;
            name_parts.entry(a).or_default().push(b);
            name_parts.entry(b).or_default().push(a);
        }
    }
    let mut entity_vector_st = db
        .sql()
        .prepare_cached("SELECT embedding FROM entities_vec WHERE rowid=?1")?;
    // The widened beam depth + bridge admission: only at scale (see
    // SEM_WIDEN_MIN_STORE). Small stores get the old beam byte for byte.
    let at_scale = db
        .sql()
        .query_row("SELECT count(*) FROM memories", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0)
        >= SEM_WIDEN_MIN_STORE;
    let depth = if at_scale { SEM_MAX_DEPTH } else { MAX_DEPTH };
    let widen_enabled = at_scale;
    let mut vector_st = db
        .sql()
        .prepare_cached("SELECT embedding FROM memories_vec WHERE rowid=?1")?;
    let mut memory_vectors: HashMap<i64, Vec<f32>> = HashMap::new();
    let mut cache: HashMap<i64, Vec<(Step, i64, Vec<f32>)>> = HashMap::new();
    let mut degree_cache: HashMap<i64, f64> = HashMap::new();
    // Training dump (`NEOCORTEX_DUMP_HOPS=<file>`): features of every candidate.
    let dump = std::env::var("NEOCORTEX_DUMP_HOPS")
        .ok()
        .filter(|p| !p.is_empty());
    let want_features = use_scorer || dump.is_some();
    let mut edges = |entity: i64| -> Result<Vec<(Step, i64, Vec<f32>)>> {
        if let Some(hit) = cache.get(&entity) {
            return Ok(hit.clone());
        }
        let mut out = Vec::new();
        // A chain can always continue through a small-degree bridge's own
        // facts, whatever their cosine to the question (nc-hay1): at 108k
        // distractors a hop's relation can fall outside the top-100 KNN that
        // bounds `relevant` (cos 0.10-0.16 loses to the world's generic
        // facts), yet the residual walk must still reach it via the bridge
        // that hop 1 just landed on. Hub entities stay KNN-bounded so the
        // fan-out (and the vector reads) stay capped.
        let widen = widen_enabled
            && db
                .sql()
                .query_row(
                    "SELECT count(*) FROM relations WHERE (src=?1 OR dst=?1) AND polarity=1",
                    [entity],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(i64::MAX)
                <= SEM_WIDEN_CAP as i64;
        for row in edges_st.query_map([entity], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
            ))
        })? {
            let (relation_id, src, dst, predicate, fact, from, to, expired) = row?;
            if (!relevant.contains(&relation_id) && !widen)
                || src == dst
                || !vis.shows(from, to, expired)
            {
                continue;
            }
            let Some(vector) = relation_vector_st
                .query_row([relation_id], |r| r.get::<_, Vec<u8>>(0))
                .optional()?
                .and_then(|blob| db.blob_to_vec(&blob))
            else {
                continue;
            };
            let reverse = dst == entity;
            let other = if reverse { src } else { dst };
            out.push((
                Step {
                    relation_id,
                    src,
                    dst,
                    predicate,
                    fact,
                    reverse,
                    matched: 0,
                    memory: None,
                },
                other,
                vector,
            ));
        }
        for (memory_id, content, others) in prose.get(&entity).into_iter().flatten() {
            let vector = match memory_vectors.get(memory_id) {
                Some(v) => v.clone(),
                None => {
                    let Some(v) = vector_st
                        .query_row([memory_id], |r| r.get::<_, Vec<u8>>(0))
                        .optional()?
                        .and_then(|blob| db.blob_to_vec(&blob))
                    else {
                        continue;
                    };
                    memory_vectors.insert(*memory_id, v.clone());
                    v
                }
            };
            // A memory naming nobody else ends the chain (NO_ENTITY).
            let targets = if others.is_empty() {
                vec![NO_ENTITY]
            } else {
                others.clone()
            };
            for other in targets {
                out.push((
                    Step {
                        relation_id: 0,
                        src: entity,
                        dst: other,
                        predicate: String::new(),
                        fact: content.clone(),
                        reverse: false,
                        matched: 0,
                        memory: Some(*memory_id),
                    },
                    other,
                    vector.clone(),
                ));
            }
        }
        for &other in name_parts.get(&entity).into_iter().flatten() {
            if other == entity {
                continue;
            }
            let Some(vector) = entity_vector_st
                .query_row([other], |r| r.get::<_, Vec<u8>>(0))
                .optional()?
                .and_then(|blob| db.blob_to_vec(&blob))
            else {
                continue;
            };
            out.push((
                Step {
                    relation_id: 0,
                    src: entity,
                    dst: other,
                    predicate: String::new(),
                    fact: String::new(),
                    reverse: false,
                    matched: 0,
                    memory: None,
                },
                other,
                vector,
            ));
        }
        out.truncate(64);
        cache.insert(entity, out.clone());
        Ok(out)
    };
    // (entity, visited entities, cosine sum, steps, residual question vector)
    type SemState = (i64, Vec<i64>, f64, Vec<Step>, Vec<f32>);
    // (step, far entity, far vector, hop score, scorer features)
    type ScoredHop = (Step, i64, Vec<f32>, f64, Vec<f64>);
    let mut frontier: Vec<SemState> = seeds
        .iter()
        .map(|id| (*id, vec![*id], 0.0, Vec::new(), query_vec.to_vec()))
        .collect();
    let mut hits: Vec<PathHit> = Vec::new();
    for _ in 0..depth {
        let mut next = Vec::new();
        for (entity, visited, sum, steps, residual) in &frontier {
            // First compute features + base cosine for every candidate.
            let mut scored: Vec<ScoredHop> = edges(*entity)?
                .into_iter()
                // A chain may start at a question anchor but never pass
                // through one: that walk lists sibling facts about the anchor.
                .filter(|(_, other, _)| !visited.contains(other) && !anchors.contains(other))
                .map(|(step, other, vector)| {
                    // Prose is weaker evidence of a link than a stated
                    // relation; a containment hop carries no fact at all.
                    let prose = step.memory.is_some();
                    let containment = step.relation_id == 0 && !prose;
                    let weight = if containment {
                        name_hop
                    } else if prose {
                        PROSE_HOP
                    } else {
                        1.0
                    };
                    let cos_residual = f64::from(crate::rank::cosine(residual, &vector));
                    let cosine = weight * cos_residual;
                    // Features cost a degree lookup per entity: built only when used.
                    let features = if want_features {
                        let cos_question = f64::from(crate::rank::cosine(query_vec, &vector));
                        // Predicate-to-question token overlap (stemmed forms).
                        let pred_overlap = if step.predicate.is_empty() {
                            0.0
                        } else {
                            let pf = text_forms(&step.predicate);
                            let inter = pf.intersection(&query_forms).count() as f64;
                            (inter / (query_forms.len().max(1) as f64)).min(1.0)
                        };
                        let hop_index = (steps.len() + 1) as f64;
                        let degree = if other == NO_ENTITY {
                            0.0
                        } else {
                            entity_degree(db, other, &mut degree_cache)
                        };
                        let far_mentioned = if anchors.contains(&other) || seeds.contains(&other) {
                            1.0
                        } else {
                            0.0
                        };
                        vec![
                            cos_question,
                            cos_residual,
                            hop_index,
                            if prose { 1.0 } else { 0.0 },
                            pred_overlap,
                            degree,
                            far_mentioned,
                        ]
                    } else {
                        Vec::new()
                    };
                    let hop_score = hop_value(use_scorer, cosine, &features);
                    (step, other, vector, hop_score, features)
                })
                .collect();
            scored.sort_by(|a, b| {
                b.3.total_cmp(&a.3)
                    .then(a.0.relation_id.cmp(&b.0.relation_id))
            });
            // Optional dump of every candidate considered (training data).
            if let Some(path) = &dump {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    use std::io::Write;
                    for (step, _other, _vector, _score, features) in &scored {
                        let memory_ids: Vec<i64> = if let Some(mid) = step.memory {
                            vec![mid]
                        } else {
                            db.sql()
                                .prepare_cached(
                                    "SELECT memory_id FROM relation_evidence WHERE relation_id=?1",
                                )
                                .ok()
                                .and_then(|mut st| {
                                    st.query_map([step.relation_id], |r| r.get::<_, i64>(0))
                                        .ok()
                                        .map(|rows| rows.filter_map(|x| x.ok()).collect())
                                })
                                .unwrap_or_default()
                        };
                        let rec = serde_json::json!({
                            "query": query,
                            "depth": steps.len() + 1,
                            "features": features,
                            "memory_ids": memory_ids,
                            "relation_id": step.relation_id,
                            "prose": step.memory.is_some(),
                        });
                        let _ = writeln!(f, "{}", rec);
                    }
                }
            }
            scored.truncate(SEM_EDGES);
            for (step, other, vector, hop_score, _features) in scored {
                let mut steps = steps.clone();
                steps.push(step);
                let mut visited = visited.clone();
                visited.push(other);
                let sum = sum + hop_score;
                // The next hop must explain what this fact did not.
                let residual = residual_after(residual, &vector);
                // One-hop facts are already the vector channel's job.
                if steps.len() >= 2 {
                    hits.push(PathHit {
                        score: sum,
                        steps: steps.clone(),
                        coverage: 1,
                    });
                }
                if other != NO_ENTITY {
                    next.push((other, visited, sum, steps, residual));
                }
            }
        }
        next.sort_by(|a, b| b.2.total_cmp(&a.2));
        next.truncate(BEAM);
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.steps.len().cmp(&b.steps.len()))
    });
    hits.truncate(PATH_HITS);
    Ok(hits)
}

/// `query` with the component along unit vector `fact` removed, renormalised:
/// what a later hop still has to explain.
fn residual_after(query: &[f32], fact: &[f32]) -> Vec<f32> {
    let dot: f32 = query.iter().zip(fact).map(|(a, b)| a * b).sum();
    let mut r: Vec<f32> = query.iter().zip(fact).map(|(a, b)| a - dot * b).collect();
    let norm = r.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
    r.iter_mut().for_each(|x| *x /= norm);
    r
}

/// E4b: evidence memories of causal facts, ranked against the question. The
/// causal group is small, so a scan of its live relations is cheaper than a
/// second FTS/vector pass and keeps every why-question on the same path.
pub(crate) fn causal_memories(
    db: &Db,
    query_vec: &[f32],
    query: &str,
    vis: Visibility,
    limit: usize,
) -> Result<Vec<i64>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let group: Option<i64> = db
        .sql()
        .query_row(
            "SELECT COALESCE(NULLIF(group_id,0),id) FROM predicates WHERE canonical='cause'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(group) = group else {
        return Ok(Vec::new());
    };
    let terms: HashSet<String> = text::tokens(query)
        .into_iter()
        .filter(|word| word.chars().count() > 1 && !text::is_stopword(word))
        .map(|word| text::norm_predicate(&word))
        .collect();
    let mut st = db.sql().prepare(
        "SELECT r.id,r.fact,r.polarity,r.valid_from,r.valid_to,r.expired_at,
                (SELECT embedding FROM relations_vec v WHERE v.rowid=r.id)
         FROM relations r JOIN predicates p ON p.id=r.predicate_id
         WHERE COALESCE(NULLIF(p.group_id,0),p.id)=?1 AND r.polarity=1",
    )?;
    let mut ranked: Vec<(i64, f64)> = Vec::new();
    for row in st.query_map([group], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<Vec<u8>>>(6)?,
        ))
    })? {
        let (id, fact, _polarity, from, to, expired, blob) = row?;
        if !vis.shows(from, to, expired) {
            continue;
        }
        let cosine = blob
            .and_then(|b| db.blob_to_vec(&b))
            .map(|v| f64::from(crate::rank::cosine(query_vec, &v)))
            .unwrap_or(0.0);
        let fact_terms: HashSet<String> = text::tokens(&fact)
            .into_iter()
            .filter(|word| word.chars().count() > 1 && !text::is_stopword(word))
            .map(|word| text::norm_predicate(&word))
            .collect();
        let overlap = if terms.is_empty() {
            0.0
        } else {
            terms.intersection(&fact_terms).count() as f64 / terms.len() as f64
        };
        ranked.push((id, 0.75 * cosine.max(0.0) + 0.25 * overlap));
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut out = Vec::new();
    let mut st = db
        .sql()
        .prepare_cached("SELECT memory_id FROM relation_evidence WHERE relation_id=?1")?;
    for (relation, _) in ranked.into_iter().take(limit) {
        for memory in st
            .query_map([relation], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
        {
            if !out.contains(&memory) {
                out.push(memory);
            }
        }
    }
    Ok(out)
}

/// An entity is a hub when many memories mention it *and* it is a large share
/// of the database: a team, a product, the company. A cause that shares only
/// a hub with the answer is about something else.
const HUB_MENTIONS: i64 = 8;
const HUB_SHARE: i64 = 8;

/// The entities of the seeds that are specific to them. Everything a broad
/// entity touches is connected to everything else through it, so a shared hub
/// is no evidence of a shared cause.
fn specific_entities(db: &Db, ids: &HashSet<i64>) -> Result<HashSet<i64>> {
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    let total: i64 = db
        .sql()
        .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?;
    let marks = vec!["?"; ids.len()].join(",");
    let mut st = db.sql().prepare(&format!(
        "SELECT entity_id,COUNT(DISTINCT memory_id) FROM mentions
         WHERE entity_id IN ({marks}) GROUP BY entity_id"
    ))?;
    let mut out = HashSet::new();
    for row in st.query_map(params_from_iter(ids.iter()), |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
    })? {
        let (id, mentions) = row?;
        if mentions < HUB_MENTIONS || mentions * HUB_SHARE <= total {
            out.insert(id);
        }
    }
    Ok(out)
}

/// Memories that share a specific entity with a seed, the seeds themselves
/// excluded: the only ones whose cause can be about the same thing as the
/// answer.
fn related_memories(db: &Db, seeds: &[i64], specific: &HashSet<i64>) -> Result<HashSet<i64>> {
    if specific.is_empty() {
        return Ok(HashSet::new());
    }
    let marks = |n: usize| vec!["?"; n].join(",");
    let mut st = db.sql().prepare(&format!(
        "SELECT DISTINCT a.memory_id FROM mentions a
         JOIN mentions b ON b.entity_id=a.entity_id
         WHERE b.memory_id IN ({}) AND a.memory_id NOT IN ({}) AND a.entity_id IN ({})",
        marks(seeds.len()),
        marks(seeds.len()),
        marks(specific.len())
    ))?;
    let mut out = HashSet::new();
    let p = seeds.iter().chain(seeds).chain(specific);
    for id in st.query_map(params_from_iter(p), |r| r.get::<_, i64>(0))? {
        out.insert(id?);
    }
    Ok(out)
}

/// The live cause relations that can explain the answer: those evidenced by
/// one of `candidates`, plus those that end at a question anchor. The endpoint
/// lists are literal placeholders so the plan is driven by `idx_relev_memory`
/// and `idx_rel_src`/`idx_rel_dst` instead of a scan of every stored relation.
fn cause_relation_query(group: i64, candidates: &[i64], anchors: &[i64]) -> (String, Vec<i64>) {
    let marks = |n: usize| vec!["?"; n].join(",");
    let mut branches = Vec::new();
    let mut params = vec![group];
    if !candidates.is_empty() {
        branches.push(format!(
            "SELECT relation_id FROM relation_evidence WHERE memory_id IN ({})",
            marks(candidates.len())
        ));
        params.extend(candidates);
    }
    if !anchors.is_empty() {
        branches.push(format!(
            "SELECT id FROM relations WHERE src IN ({0}) OR dst IN ({0})",
            marks(anchors.len())
        ));
        params.extend(anchors);
        params.extend(anchors);
    }
    let sql = format!(
        "SELECT r.id,r.valid_from,r.valid_to,r.expired_at,e.memory_id,r.src,r.dst
         FROM relations r
         JOIN predicates p ON p.id=r.predicate_id
         JOIN relation_evidence e ON e.relation_id=r.id
         WHERE COALESCE(NULLIF(p.group_id,0),p.id)=? AND r.polarity=1
           AND r.id IN ({})
         ORDER BY r.id,e.memory_id",
        branches.join(" UNION ")
    );
    (sql, params)
}

/// H2-16: the cause of what the first pass answered. A why-question is about
/// the cause of the memory that answered it, and that memory rarely repeats
/// the cause relation, so follow the stored causal facts one step out from the
/// seeds: an evidence memory of a live `cause` relation is the cause when the
/// relation ends at a question anchor, or when that memory mentions an entity
/// the answer is specific about. A cause that shares only a hub entity (a
/// team) is about something else.
/// Returns the cause memories and the relations to render, in id order.
pub(crate) fn cause_follow(
    db: &Db,
    seeds: &[i64],
    anchors: &[i64],
    vis: Visibility,
    limit: usize,
) -> Result<(Vec<i64>, Vec<i64>)> {
    if seeds.is_empty() || limit == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let group: Option<i64> = db
        .sql()
        .query_row(
            "SELECT COALESCE(NULLIF(group_id,0),id) FROM predicates WHERE canonical='cause'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(group) = group else {
        return Ok((Vec::new(), Vec::new()));
    };
    let marks = vec!["?"; seeds.len()].join(",");
    let mut seed_entities: HashSet<i64> = HashSet::new();
    let mut st = db.sql().prepare(&format!(
        "SELECT entity_id FROM mentions WHERE memory_id IN ({marks})"
    ))?;
    for id in st.query_map(params_from_iter(seeds.iter()), |r| r.get::<_, i64>(0))? {
        seed_entities.insert(id?);
    }
    if seed_entities.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let specific = specific_entities(db, &seed_entities)?;
    // Only a cause evidenced by a memory that shares a specific entity with a
    // seed, or a relation that ends at a question anchor, can explain the
    // answer. Seeding the query with those two sets keeps its cost with the
    // seeds instead of every relation ever stored.
    let mut candidates: Vec<i64> = related_memories(db, seeds, &specific)?
        .into_iter()
        .collect();
    candidates.sort_unstable();
    if candidates.is_empty() && anchors.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let (sql, params) = cause_relation_query(group, &candidates, anchors);
    let mut st = db.sql().prepare(&sql)?;
    let mut pairs: Vec<(i64, i64)> = Vec::new();
    let mut anchored: HashSet<i64> = HashSet::new();
    for row in st.query_map(params_from_iter(&params), |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
        ))
    })? {
        let (relation, from, to, expired, memory, src, dst) = row?;
        if vis.shows(from, to, expired) {
            pairs.push((relation, memory));
            if anchors.contains(&src) || anchors.contains(&dst) {
                anchored.insert(relation);
            }
        }
    }
    if pairs.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // The seeds stay in: a cause relation the answer itself evidences must
    // still render, it just does not need a channel entry.
    let mut out_memories = Vec::new();
    let mut out_relations = Vec::new();
    for (relation, memory) in pairs {
        if !(anchored.contains(&relation) || candidates.binary_search(&memory).is_ok()) {
            continue;
        }
        // The relation is rendered even when its evidence memory was already
        // the answer; only the channel skips the seeds themselves.
        if !out_relations.contains(&relation) {
            out_relations.push(relation);
        }
        if !seeds.contains(&memory) && !out_memories.contains(&memory) {
            out_memories.push(memory);
        }
        if out_memories.len() >= limit {
            break;
        }
    }
    Ok((out_memories, out_relations))
}

/// Evidence memories for ranked paths: the destination first, then the links.
/// Deeper coverage wins: a three-hop chain that explains the whole question
/// outranks a two-hop fragment with a slightly better edge score.
pub(crate) fn evidence(db: &Db, hits: &[PathHit]) -> Result<Vec<i64>> {
    let mut scores: HashMap<i64, f64> = HashMap::new();
    let mut st = db
        .sql()
        .prepare_cached("SELECT memory_id FROM relation_evidence WHERE relation_id=?1")?;
    for hit in hits {
        let last = hit.steps.len().saturating_sub(1);
        let base = hit.score * (1.0 + hit.coverage as f64).powi(2);
        for (position, step) in hit.steps.iter().enumerate() {
            let weight = base * if position == last { 3.0 } else { 1.0 };
            let ids: Vec<i64> = match step.memory {
                Some(memory_id) => vec![memory_id],
                // A containment hop is a link between names, not evidence.
                None if step.relation_id == 0 => continue,
                None => st
                    .query_map([step.relation_id], |r| r.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<_>>()?,
            };
            for memory_id in ids {
                scores
                    .entry(memory_id)
                    .and_modify(|current| *current = current.max(weight))
                    .or_insert(weight);
            }
        }
    }
    let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(ranked.into_iter().map(|(id, _)| id).collect())
}

/// The relations along the best paths, destination hop first, ready to render
/// next to the retrieved memories.
pub(crate) fn facts(db: &Db, hits: &[PathHit], cutoff: i64, limit: usize) -> Result<Vec<FactHit>> {
    let mut ids = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    for hit in hits {
        for step in hit.steps.iter().rev() {
            // Prose and containment hops carry no relation to render.
            if step.relation_id == 0 {
                continue;
            }
            if seen.insert(step.relation_id) {
                ids.push(step.relation_id);
            }
        }
    }
    facts_by_ids(db, &ids, cutoff, limit)
}

/// The facts of specific relations, in the order given: what a path ends on,
/// or what a why-question's cause follow found.
pub(crate) fn facts_by_ids(
    db: &Db,
    ids: &[i64],
    cutoff: i64,
    limit: usize,
) -> Result<Vec<FactHit>> {
    let mut st = db.sql().prepare_cached(
        "SELECT COALESCE(NULLIF(r.predicate_text,''),NULLIF(p.display,''),p.canonical),
                r.fact,r.valid_from,r.valid_to,r.expired_at,s.name,d.name,r.polarity
         FROM relations r
         JOIN predicates p ON p.id=r.predicate_id
         JOIN entities s ON s.id=r.src
         JOIN entities d ON d.id=r.dst
         WHERE r.id=?1",
    )?;
    let mut out = Vec::new();
    for relation_id in ids {
        let Ok(mut rows) = st.query(params![relation_id]) else {
            continue;
        };
        let Some(row) = rows.next()? else { continue };
        let (predicate, fact, valid_from, valid_to, expired, source, target, polarity) = (
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, i64>(7)?,
        );
        if valid_from > cutoff {
            continue;
        }
        out.push(FactHit {
            id: *relation_id,
            source,
            predicate,
            target,
            fact,
            valid_from,
            valid_to,
            ended: expired.is_some() || valid_to.is_some_and(|v| v <= cutoff),
            retracted: expired.is_some() && valid_to.is_none(),
            polarity,
            memory: false,
            current_values: None,
            replaced_by: None,
            duration: None,
        });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// The chains whose relations are rendered: the best of each channel first
/// (lexical and semantic scores are not comparable), then runners-up, three in
/// all (the old lexical pair plus the best semantic chain).
pub(crate) fn top_chains(lexical: &[PathHit], semantic: &[PathHit]) -> Vec<PathHit> {
    let mut out = Vec::new();
    for i in 0..2 {
        out.extend(lexical.get(i).cloned());
        out.extend(semantic.get(i).cloned());
    }
    out.truncate(3);
    out
}

/// What a chain ends on: a prose memory or a relation.
fn terminal(hit: &PathHit) -> Option<(bool, i64)> {
    hit.steps
        .last()
        .map(|s| (s.memory.is_some(), s.memory.unwrap_or(s.relation_id)))
}

/// Prose steps rendered per query at most (B4, best chain only).
const PROSE_FACTS: usize = 2;

/// Semantic chains whose prose steps `prose_wide` renders.
const PROSE_WIDE_CHAINS: usize = 3;
/// Prose lines `prose_wide` renders per query. They take their own slots, so
/// relation and entity facts are never truncated away for them.
const PROSE_WIDE_FACTS: usize = 3;

/// One prose step as a fact line (`memory` = true): a relation-less hop is
/// never rendered by `facts`, so this is the only way it reaches the context.
fn prose_fact(db: &Db, step: &Step, cutoff: i64) -> Result<Option<FactHit>> {
    let name = |entity: i64| -> Result<String> {
        Ok(db
            .sql()
            .query_row("SELECT name FROM entities WHERE id=?1", [entity], |r| {
                r.get(0)
            })
            .optional()?
            .unwrap_or_default())
    };
    let id = step.memory.unwrap_or_default();
    let Some((valid_from, valid_to, expired)) = db
        .sql()
        .query_row(
            "SELECT valid_from,valid_to,expired_at FROM memories WHERE id=?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()?
        .filter(|(from, _, _)| *from <= cutoff)
    else {
        return Ok(None);
    };
    Ok(Some(FactHit {
        id,
        source: name(step.src)?,
        predicate: String::new(),
        target: name(step.dst)?,
        fact: step.fact.chars().take(200).collect(),
        valid_from,
        valid_to,
        ended: expired.is_some() || valid_to.is_some_and(|v| v <= cutoff),
        retracted: expired.is_some() && valid_to.is_none(),
        polarity: 1,
        memory: true,
        current_values: None,
        replaced_by: None,
        duration: None,
    }))
}

/// B4: when the best chain (`hits` sorted best first) uses prose memories and
/// beats the best chain with another terminal by `margin`, those memories carry
/// the answer (a relation-less hop is never rendered by `facts`); render them
/// as fact lines (`memory` = true), destination first.
pub(crate) fn prose_facts(
    db: &Db,
    hits: &[PathHit],
    margin: f64,
    cutoff: i64,
) -> Result<Vec<FactHit>> {
    let Some(best) = hits.first().filter(|_| margin.is_finite()) else {
        return Ok(Vec::new());
    };
    let rival = hits.iter().find(|h| terminal(h) != terminal(best));
    if rival.is_some_and(|r| best.score - r.score < margin) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for step in best.steps.iter().rev().filter(|s| s.memory.is_some()) {
        let Some(fact) = prose_fact(db, step, cutoff)? else {
            continue;
        };
        out.push(fact);
        if out.len() >= PROSE_FACTS {
            break;
        }
    }
    Ok(out)
}

/// `prose_wide` (knob, default off): the prose steps of the top three semantic
/// chains, at any hop position, render as fact lines (`memory` = true). A prose
/// hop is skipped when its memory is already one of `rendered` (it is in the
/// context already), and at most `PROSE_WIDE_FACTS` lines are returned,
/// destination hop first.
pub(crate) fn prose_wide_facts(
    db: &Db,
    hits: &[PathHit],
    rendered: &HashSet<i64>,
    cutoff: i64,
) -> Result<Vec<FactHit>> {
    let mut out = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    for hit in hits.iter().take(PROSE_WIDE_CHAINS) {
        for step in hit.steps.iter().rev() {
            let Some(id) = step.memory else { continue };
            if rendered.contains(&id) || !seen.insert(id) {
                continue;
            }
            if let Some(fact) = prose_fact(db, step, cutoff)? {
                out.push(fact);
            }
            if out.len() >= PROSE_WIDE_FACTS {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------- tools

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FollowInput {
    /// Entity to start from (name, alias or e<id>).
    pub from: String,
    /// Stop when this entity is reached; reports `reached`.
    #[serde(default)]
    pub to: Option<String>,
    /// Only follow relations whose predicate or fact matches these words.
    #[serde(default)]
    pub predicate: Option<String>,
    /// Hops to walk (1..=3, default 2).
    #[serde(default)]
    pub depth: Option<usize>,
    /// Max facts to return (default 20).
    #[serde(default)]
    pub k: Option<usize>,
    #[serde(default)]
    pub as_of: Option<String>,
    #[serde(default)]
    pub include_history: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct FollowFact {
    pub relation: String,
    pub source: String,
    pub predicate: String,
    pub target: String,
    pub fact: String,
    pub valid_from: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<i64>,
    pub ended: bool,
    /// 1 positive, 0 negative (rendered `—NOT→` in text).
    pub polarity: i64,
    /// "out" when the relation points away from the reached entity.
    pub direction: String,
    /// Hops from `from` (1 = direct).
    pub hops: usize,
    pub memories: Vec<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FollowOutput {
    pub from: String,
    pub to: Option<String>,
    /// The target was reached within `depth` hops.
    pub reached: bool,
    pub facts: Vec<FollowFact>,
    /// Facts reachable within depth before `k` was applied.
    pub total: usize,
    pub truncated: bool,
}

fn visibility(engine: &Engine, as_of: Option<&str>, history: bool) -> Result<Visibility> {
    let clock = engine.clock();
    let as_of = as_of
        .map(|value| {
            crate::temporal::parse_instant(value, &clock)
                .ok_or_else(|| crate::err!("invalid time '{value}'"))
        })
        .transpose()?;
    Ok(Visibility {
        now: clock.now,
        as_of,
        history: history || as_of.is_some(),
    })
}

/// Walk live relations out of one entity, hop by hop. This is the tool an agent
/// uses to finish a chain itself when `recall` returns the neighbourhood but
/// not the answer.
pub fn follow(engine: &Engine, follow: &FollowInput) -> Result<FollowOutput> {
    let db = &engine.db;
    let depth = follow.depth.unwrap_or(2).clamp(1, 3);
    let k = follow.k.unwrap_or(20).clamp(1, 100);
    let vis = visibility(engine, follow.as_of.as_deref(), follow.include_history)?;
    let start = crate::recall::resolve_entity(engine, &follow.from)?;
    let target = follow
        .to
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .map(|name| crate::recall::resolve_entity(engine, name))
        .transpose()?;
    let want: Vec<HashSet<String>> = follow
        .predicate
        .as_deref()
        .map(|value| query_terms(value, &[]))
        .unwrap_or_default();
    let start_name = pick_name(db, start)?;
    let mut frontier: Vec<i64> = vec![start];
    let mut seen_entities: HashSet<i64> = frontier.iter().copied().collect();
    let mut seen_relations: HashSet<i64> = HashSet::new();
    let mut out: Vec<FollowFact> = Vec::new();
    let mut reached = Some(start) == target;
    for hop in 1..=depth {
        let mut next = Vec::new();
        for entity in &frontier {
            let mut st = db.sql().prepare_cached(
                "SELECT r.id,r.src,r.dst,
                        COALESCE(NULLIF(r.predicate_text,''),NULLIF(p.display,''),p.canonical),
                        r.fact,r.valid_from,r.valid_to,r.expired_at,s.name,d.name,r.polarity
                 FROM relations r
                 JOIN predicates p ON p.id=r.predicate_id
                 JOIN entities s ON s.id=r.src
                 JOIN entities d ON d.id=r.dst
                 WHERE (r.src=?1 OR r.dst=?2) AND r.src != r.dst",
            )?;
            let rows = st.query_map(params![entity, entity], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, i64>(10)?,
                ))
            })?;
            for row in rows {
                let (
                    id,
                    src,
                    dst,
                    predicate,
                    fact,
                    valid_from,
                    valid_to,
                    expired,
                    sname,
                    dname,
                    polarity,
                ) = row?;
                if !vis.shows(valid_from, valid_to, expired) || !seen_relations.insert(id) {
                    continue;
                }
                if !want.is_empty() {
                    let forms = text_forms(&format!("{predicate} {fact}"));
                    if !want.iter().any(|term| !term.is_disjoint(&forms)) {
                        continue;
                    }
                }
                let (other, direction) = if *entity == src {
                    (dst, "out")
                } else {
                    (src, "in")
                };
                let memories: Vec<i64> = db
                    .sql()
                    .prepare_cached("SELECT memory_id FROM relation_evidence WHERE relation_id=?1")?
                    .query_map([id], |r| r.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                out.push(FollowFact {
                    relation: format!("r{id}"),
                    source: sname,
                    predicate,
                    target: dname,
                    fact,
                    valid_from,
                    valid_to,
                    ended: expired.is_some() || valid_to.is_some_and(|v| v <= vis.now),
                    polarity,
                    direction: direction.to_string(),
                    hops: hop,
                    memories,
                });
                if target == Some(other) {
                    reached = true;
                }
                if seen_entities.insert(other) {
                    next.push(other);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() || (target.is_some() && reached) {
            break;
        }
    }
    let total = out.len();
    // Forward edges first at the same hop: a hub entity reached by one hop
    // ("Yash lives in Nairobi") has hundreds of reverse edges ("X lives in
    // Nairobi") that would otherwise fill `k` before the next forward hop.
    out.sort_by(|a, b| {
        a.hops
            .cmp(&b.hops)
            .then((a.direction == "in").cmp(&(b.direction == "in")))
            .then(a.ended.cmp(&b.ended))
    });
    out.truncate(k);
    Ok(FollowOutput {
        from: start_name,
        to: follow.to.clone(),
        reached,
        facts: out,
        total,
        truncated: total > k,
    })
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AggregateInput {
    /// Relation the question is about ("likes", "pets", "works at").
    #[serde(default)]
    pub predicate: Option<String>,
    /// Count targets of this entity.
    #[serde(default)]
    pub source: Option<String>,
    /// Count sources that reach this entity.
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub include_history: bool,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AggregateItem {
    pub name: String,
    pub id: i64,
    pub facts: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AggregateOutput {
    pub predicate: Option<String>,
    pub source: Option<String>,
    pub target: Option<String>,
    /// Distinct entities, not facts.
    pub count: usize,
    pub items: Vec<AggregateItem>,
    /// Facts that explicitly state the opposite (text-level cue).
    pub negative: Vec<AggregateItem>,
    pub memories: Vec<i64>,
    pub truncated: bool,
}

fn pick_name(db: &Db, id: i64) -> Result<String> {
    Ok(db
        .sql()
        .query_row("SELECT name FROM entities WHERE id=?1", [id], |r| r.get(0))?)
}

/// Count or list the targets (or sources) of live relations, deduplicated by
/// entity. This answers "how many", "which ones" and "list" questions that a
/// ranked retrieval cannot answer by ranking alone.
pub fn aggregate(engine: &Engine, input: &AggregateInput) -> Result<AggregateOutput> {
    let db = &engine.db;
    let vis = visibility(engine, None, input.include_history)?;
    let source = input
        .source
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| crate::recall::resolve_entity(engine, value))
        .transpose()?;
    let target = input
        .target
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| crate::recall::resolve_entity(engine, value))
        .transpose()?;
    let terms = input
        .predicate
        .as_deref()
        .map(|value| query_terms(value, &[]))
        .unwrap_or_default();
    if source.is_none() && target.is_none() && terms.is_empty() {
        return Err(crate::err!(
            "aggregate needs a predicate, a source or a target"
        ));
    }
    let limit = input.limit.unwrap_or(50).clamp(1, 200);
    let mut st = db.sql().prepare_cached(
        "SELECT r.id,r.src,r.dst,p.canonical,r.fact,r.valid_from,r.valid_to,r.expired_at,
                s.name,d.name,r.polarity
         FROM relations r
         JOIN predicates p ON p.id=r.predicate_id
         JOIN entities s ON s.id=r.src
         JOIN entities d ON d.id=r.dst
         WHERE (?1 IS NULL OR r.src=?1) AND (?2 IS NULL OR r.dst=?2)
           AND r.src != r.dst
         ORDER BY r.id",
    )?;
    let rows = st.query_map(params![source, target], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, String>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, i64>(10)?,
        ))
    })?;
    let mut items: HashMap<i64, AggregateItem> = HashMap::new();
    let mut negative: HashMap<i64, AggregateItem> = HashMap::new();
    let mut memories: Vec<i64> = Vec::new();
    for row in rows {
        let (id, src, dst, predicate, fact, valid_from, valid_to, expired, sname, dname, polarity) =
            row?;
        if !vis.shows(valid_from, valid_to, expired) {
            continue;
        }
        if !terms.is_empty() {
            let forms = text_forms(&format!("{predicate} {fact}"));
            if !terms.iter().all(|term| !term.is_disjoint(&forms)) {
                continue;
            }
        }
        // Count the end the question is not anchored on.
        let (other, other_name) = if source.is_some() {
            (dst, dname)
        } else {
            (src, sname)
        };
        let bucket = if polarity == 0 || has_negation_cue(&fact) {
            &mut negative
        } else {
            &mut items
        };
        bucket
            .entry(other)
            .or_insert_with(|| AggregateItem {
                name: other_name,
                id: other,
                facts: Vec::new(),
            })
            .facts
            .push(fact);
        if memories.len() < 20 {
            let evidence: Vec<i64> = db
                .sql()
                .prepare_cached("SELECT memory_id FROM relation_evidence WHERE relation_id=?1")?
                .query_map([id], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<_>>()?;
            memories.extend(evidence);
        }
    }
    memories.sort_unstable();
    memories.dedup();
    let total = items.len();
    let mut items: Vec<AggregateItem> = items.into_values().collect();
    let mut negative: Vec<AggregateItem> = negative.into_values().collect();
    for list in [&mut items, &mut negative] {
        list.sort_by(|a, b| a.name.cmp(&b.name));
    }
    items.truncate(limit);
    negative.truncate(limit);
    Ok(AggregateOutput {
        predicate: input.predicate.clone(),
        source: input
            .source
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        target: input
            .target
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        count: if source.is_some() && target.is_some() {
            usize::from(!items.is_empty())
        } else {
            total
        },
        items,
        negative,
        memories,
        truncated: total > limit,
    })
}

#[cfg(test)]
mod relation_knn_tests {
    use crate::db::Db;
    use rusqlite::params;

    /// recall's relation-fact seeds are the first 10 hits of the shared
    /// SEM_FACTS scan; that equals a k=10 query only while vec0's top-k is
    /// prefix-stable, exact distance ties included.
    #[test]
    fn the_first_ten_shared_hits_are_a_k10_query() {
        let db = Db::open_in_memory().unwrap();
        for id in 1..=300i64 {
            let mut v = vec![0.0f32; 768];
            v[0] = 1.0;
            v[1 + (id % 7) as usize] = (id % 5) as f32 * 0.1;
            db.sql()
                .execute(
                    "INSERT INTO relations_vec(rowid,embedding) VALUES (?1,?2)",
                    params![id, db.vec_blob(&v)],
                )
                .unwrap();
        }
        let mut query = vec![0.0f32; 768];
        query[0] = 1.0;
        query[2] = 0.3;
        let shared = super::relation_knn(&db, &query).unwrap();
        let k10: Vec<(i64, f32)> = db
            .sql()
            .prepare(&format!(
                "SELECT rowid,1-distance FROM relations_vec
                 WHERE embedding MATCH {} AND k=10 ORDER BY distance",
                db.vec_sql("?1")
            ))
            .unwrap()
            .query_map([db.vec_blob(&query)], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(shared.len(), super::SEM_FACTS);
        assert_eq!(shared[..10], k10[..]);
    }
}

#[cfg(test)]
mod hop_scorer_tests {
    use super::{hop_value, HOP_BIAS, HOP_WEIGHTS};

    #[test]
    fn scorer_on_applies_the_weights_and_off_keeps_the_cosine() {
        let f = [0.9, 0.4, 1.0, 0.0, 0.2, 1.5, 0.0];
        assert_eq!(hop_value(false, 0.37, &f), 0.37);
        let z = HOP_BIAS + HOP_WEIGHTS.iter().zip(&f).map(|(w, x)| w * x).sum::<f64>();
        let want = 1.0 / (1.0 + (-z).exp());
        assert!((hop_value(true, 0.37, &f) - want).abs() < 1e-12);
    }
}

#[cfg(test)]
mod residual_tests {
    use super::residual_after;

    #[test]
    fn residual_is_orthogonal_to_the_explained_fact_and_unit_length() {
        let r = residual_after(&[0.6, 0.8, 0.0], &[1.0, 0.0, 0.0]);
        assert!(r[0].abs() < 1e-6, "{r:?}");
        assert!((r[1] - 1.0).abs() < 1e-6, "{r:?}");
    }
}

#[cfg(test)]
mod fact_render_tests {
    use super::{facts, PathHit, Step};
    use crate::db::Db;
    use rusqlite::params;

    #[test]
    fn path_facts_use_the_same_predicate_spelling_as_the_db_channel() {
        let db = Db::open_in_memory().unwrap();
        db.sql()
            .execute(
                "INSERT INTO entities(name,norm,type,homonym,created_at,updated_at)
                 VALUES ('platform team','platform team','org',0,0,0),
                        ('Jenkins','jenkins','tool',0,0,0)",
                [],
            )
            .unwrap();
        let src = 1;
        let dst = 2;
        db.sql()
            .execute(
                "INSERT INTO predicates(canonical,display,exclusive,group_id,symmetric,seeded)
                 VALUES ('use','uses',0,0,0,0)",
                [],
            )
            .unwrap();
        let pid = db.sql().last_insert_rowid();
        db.sql()
            .execute("UPDATE predicates SET group_id=?1 WHERE id=?1", [pid])
            .unwrap();
        db.sql()
            .execute(
                "INSERT INTO relations(src,dst,predicate_id,predicate_text,subj,fact,valid_from,created_at)
                 VALUES (?1,?2,?3,'uses',?1,'platform team uses Jenkins',0,0)",
                params![src, dst, pid],
            )
            .unwrap();
        let hit = PathHit {
            steps: vec![Step {
                relation_id: 1,
                src,
                dst,
                predicate: "use".into(),
                fact: "platform team uses Jenkins".into(),
                reverse: false,
                matched: 1,
                memory: None,
            }],
            coverage: 1,
            score: 1.0,
        };
        let out = facts(&db, &[hit], 10, 8).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].predicate, "uses");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(score: f64, tag: i64) -> PathHit {
        PathHit {
            steps: vec![Step {
                relation_id: tag,
                src: 0,
                dst: 0,
                predicate: String::new(),
                fact: String::new(),
                reverse: false,
                matched: 0,
                memory: None,
            }],
            coverage: 1,
            score,
        }
    }

    #[test]
    fn defect_cues_cover_report_inflections() {
        for report in [
            "The nightly export crashes with a null pointer in the CSV writer.",
            "The export cannot finish when the CSV writer sees a null.",
            "The export can't finish when the CSV writer sees a null.",
            "The nightly export fails on the last row.",
            "The sync loop hangs when two devices write at once.",
            "The sync loop is broken on Windows.",
        ] {
            assert!(is_bug_memory(report), "{report:?}");
        }
        // A fix note is a bug memory with a fix cue, never an open report.
        assert!(has_fix_cue("Fixed the export crash in the CSV writer."));
        assert!(!is_bug_memory("Lumen is written in Rust."));
    }

    #[test]
    fn a_negated_or_pending_fix_is_not_a_fix() {
        for report in [
            "Bug 2026-10-27: the nightly export crashes; it is not fixed yet.",
            "The nightly export crashes with a null pointer; no fix yet.",
            "The sync loop still hangs when two devices write at once and needs a fix.",
            "The nightly export crash is known; the fix is not deployed.",
            "The login bug is unresolved: it was never fixed.",
            "The export crash has not been fixed.",
        ] {
            assert!(!has_fix_cue(report), "{report:?}");
            assert!(is_bug_memory(report), "{report:?}");
        }
        for note in [
            "Fixed the export crash in the CSV writer.",
            "Fix for the 2026-10-25 login bug: the cookie is signed now.",
            "The login bug was resolved and released in v0.3.2.",
            "The sync loop panic is closed; no more DatabaseBusy errors.",
        ] {
            assert!(has_fix_cue(note), "{note:?}");
        }
    }

    #[test]
    fn lexical_hits_do_not_crowd_out_the_best_semantic_chain() {
        let lexical = [chain(9.0, 1), chain(8.0, 2), chain(7.0, 3)];
        let semantic = [chain(0.6, 10), chain(0.5, 11)];
        let tags: Vec<i64> = top_chains(&lexical, &semantic)
            .iter()
            .map(|h| h.steps[0].relation_id)
            .collect();
        assert_eq!(tags, vec![1, 10, 2]);
        // One channel alone still yields its two best chains.
        let only: Vec<i64> = top_chains(&lexical, &[])
            .iter()
            .map(|h| h.steps[0].relation_id)
            .collect();
        assert_eq!(only, vec![1, 2]);
    }

    /// D3: the cause follow must not scan every stored relation. The query is
    /// seeded with the answer's related memories and the question's anchors,
    /// so its cost follows the seeds, not the vault.
    #[test]
    fn the_cause_follow_does_not_scan_every_relation() {
        let db = Db::open_in_memory().unwrap();
        let (sql, params) = cause_relation_query(1, &[2, 3], &[4, 5]);
        let mut st = db
            .sql()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let plan: Vec<String> = st
            .query_map(params_from_iter(&params), |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            !plan.iter().any(|step| step.contains("SCAN r")),
            "every relation is scanned: {plan:?}"
        );
        assert!(
            plan.iter().any(|step| step.contains("idx_relev_memory")),
            "the seed index is not used: {plan:?}"
        );
    }
}

/// The exclusion channel resolves a whole type before ranking, so its
/// population is capped: past the cap the ordinary channels answer, which is
/// exactly the pre-H2-02 behaviour.
#[cfg(test)]
mod exclusion_tests {
    use super::{exclusion_entities, EXCLUSION_MAX_ENTITIES};
    use crate::db::Db;
    use rusqlite::params;

    fn typed(db: &Db, name: &str, kind: &str) -> i64 {
        db.sql()
            .execute(
                "INSERT INTO entities(name,norm,type,homonym,created_at,updated_at)
                 VALUES (?1,?1,?2,0,0,0)",
                params![name, kind],
            )
            .unwrap();
        db.sql().last_insert_rowid()
    }

    #[test]
    fn a_type_past_the_cap_falls_back_to_the_ordinary_channels() {
        let db = Db::open_in_memory().unwrap();
        let rust = typed(&db, "rust", "language");
        db.sql()
            .execute(
                "INSERT INTO entity_aliases(alias_norm,entity_id,origin) VALUES ('rust',?1,'name')",
                params![rust],
            )
            .unwrap();
        for i in 0..EXCLUSION_MAX_ENTITIES {
            typed(&db, &format!("widget {i}"), "widget");
        }
        let query = "which widget is not in rust";
        let at_cap = exclusion_entities(&db, query, 0).unwrap();
        assert_eq!(
            at_cap.map(|ex| ex.lacking.len()),
            Some(EXCLUSION_MAX_ENTITIES)
        );
        typed(&db, &format!("widget {EXCLUSION_MAX_ENTITIES}"), "widget");
        assert!(exclusion_entities(&db, query, 0).unwrap().is_none());
    }
}

/// B5: the semantic beam walks a containment link as a weak hop, so a
/// question seeded on a place reaches the memory that names the compound
/// ("Aveiro" -> "Hackerspace Aveiro" -> the memory).
#[cfg(test)]
mod name_part_tests {
    use super::{evidence, search_semantic, Visibility, NAME_PART_HOP};
    use crate::db::Db;
    use rusqlite::params;

    fn vector(axis: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; 768];
        v[axis] = 1.0;
        v
    }

    fn entity(db: &Db, name: &str, kind: &str, axis: usize) -> i64 {
        db.sql()
            .execute(
                "INSERT INTO entities(name,norm,type,homonym,created_at,updated_at)
                 VALUES (?1,?1,?2,0,0,0)",
                params![name, kind],
            )
            .unwrap();
        let id = db.sql().last_insert_rowid();
        db.sql()
            .execute(
                "INSERT INTO entities_vec(rowid,embedding) VALUES (?1,?2)",
                params![id, db.vec_blob(&vector(axis))],
            )
            .unwrap();
        id
    }

    fn memory(db: &Db, content: &str, axis: usize) -> i64 {
        db.sql()
            .execute(
                "INSERT INTO memories(content,kind,tags,importance,created_at,valid_from,content_hash)
                 VALUES (?1,'fact','',0.5,0,0,?1)",
                params![content],
            )
            .unwrap();
        let id = db.sql().last_insert_rowid();
        db.sql()
            .execute(
                "INSERT INTO memories_vec(rowid,embedding,live) VALUES (?1,?2,1)",
                params![id, db.vec_blob(&vector(axis))],
            )
            .unwrap();
        id
    }

    fn mention(db: &Db, memory: i64, entity: i64) {
        db.sql()
            .execute(
                "INSERT INTO mentions(memory_id,entity_id) VALUES (?1,?2)",
                params![memory, entity],
            )
            .unwrap();
    }

    /// Seed `Aveiro`, one prose memory naming `Hackerspace Aveiro`, and the
    /// containment row the write path would have written.
    fn fixture() -> (Db, i64, i64) {
        let db = Db::open_in_memory().unwrap();
        let aveiro = entity(&db, "aveiro", "place", 0);
        let space = entity(&db, "hackerspace aveiro", "org", 0);
        let m = memory(&db, "Hackerspace Aveiro meets on Fridays.", 0);
        mention(&db, m, space);
        (db, aveiro, space)
    }

    fn hits(db: &Db, seed: i64) -> Vec<super::PathHit> {
        let query = vector(0);
        let vis = Visibility {
            now: 0,
            as_of: None,
            history: false,
        };
        search_semantic(
            db,
            "who meets in Aveiro",
            &query,
            &[seed],
            &[seed],
            true,
            &[1],
            &super::relation_knn(db, &query).unwrap(),
            vis,
            false,
            NAME_PART_HOP,
        )
        .unwrap()
    }

    #[test]
    fn a_containment_link_carries_the_beam_to_the_compound_memory() {
        let (db, aveiro, space) = fixture();
        db.sql()
            .execute(
                "INSERT INTO name_part(a,b,weight) VALUES (?1,?2,0.6)",
                params![aveiro.min(space), aveiro.max(space)],
            )
            .unwrap();
        let hits = hits(&db, aveiro);
        let chain = hits
            .iter()
            .find(|h| h.steps.len() == 2)
            .unwrap_or_else(|| panic!("no two-hop chain: {hits:?}"));
        assert_eq!(chain.steps[0].dst, space, "first hop is the containment");
        assert_eq!(chain.steps[1].memory, Some(1));
        assert_eq!(evidence(&db, &hits).unwrap(), vec![1]);
    }

    #[test]
    fn without_the_link_the_beam_cannot_reach_the_compound() {
        let (db, aveiro, _space) = fixture();
        let hits = hits(&db, aveiro);
        assert!(evidence(&db, &hits).unwrap().is_empty(), "{hits:?}");
    }

    #[test]
    fn a_containment_hop_scores_below_a_relation_hop() {
        let (db, aveiro, space) = fixture();
        db.sql()
            .execute(
                "INSERT INTO name_part(a,b,weight) VALUES (?1,?2,0.6)",
                params![aveiro.min(space), aveiro.max(space)],
            )
            .unwrap();
        let hits = hits(&db, aveiro);
        let chain = hits.iter().find(|h| h.steps.len() == 2).unwrap();
        // The card and the question share one axis, so the containment hop
        // scores its full weight and leaves no residual for the prose hop.
        assert!(
            (chain.score - NAME_PART_HOP).abs() < 1e-9,
            "score {} should be the containment weight",
            chain.score
        );
        const { assert!(NAME_PART_HOP < super::PROSE_HOP) };
        const { assert!(super::PROSE_HOP < 1.0) };
    }
}

/// nc-hay1: at 108k distractors a hop's relation can fall outside the top-100
/// KNN that bounds the semantic beam's `relevant` set (its cosine to the
/// question collapses behind the world's generic facts), so a two-hop answer
/// needs the beam to keep walking through a small-degree bridge's own facts.
/// The hop that KNN alone would miss is the answer's last hop, incident to
/// the bridge hop 1 landed on.
#[cfg(test)]
mod widen_tests {
    use super::{search_semantic, Visibility};
    use crate::db::Db;
    use rusqlite::params;

    fn vector(axis: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; 768];
        v[axis] = 1.0;
        v
    }

    fn entity(db: &Db, name: &str, axis: usize) -> i64 {
        db.sql()
            .execute(
                "INSERT INTO entities(name,norm,type,homonym,created_at,updated_at)
                 VALUES (?1,?1,'person',0,0,0)",
                params![name],
            )
            .unwrap();
        let id = db.sql().last_insert_rowid();
        db.sql()
            .execute(
                "INSERT INTO entities_vec(rowid,embedding) VALUES (?1,?2)",
                params![id, db.vec_blob(&vector(axis))],
            )
            .unwrap();
        id
    }

    fn relation(db: &Db, src: i64, dst: i64, pid: i64, axis: usize, scale: f32) -> i64 {
        db.sql()
            .execute(
                "INSERT INTO relations(src,dst,predicate_id,predicate_text,subj,fact,valid_from,created_at)
                 VALUES (?1,?2,?3,'likes',?1,'fact text',0,0)",
                params![src, dst, pid],
            )
            .unwrap();
        let id = db.sql().last_insert_rowid();
        let mut v = vec![0.0f32; 768];
        v[axis] = scale;
        db.sql()
            .execute(
                "INSERT INTO relations_vec(rowid,embedding) VALUES (?1,?2)",
                params![id, db.vec_blob(&v)],
            )
            .unwrap();
        id
    }

    /// Chain: Noor(at ?2 axis) --hop1(axis 0)-- Delta --hop2(axis 7, weak)--> Aisha.
    /// 110 same-axis distractor relations sit above hop2's cosine, so it is
    /// outside the KNN-100 unless the widening admits it via the bridge.
    fn fixture(db: &Db, pid: i64) -> (i64, i64, i64) {
        // The widening engages only at scale; build a 50k-memory store so the
        // crowded dispatch the seam protects against is exercised.
        let sql = db.sql();
        sql.execute_batch("BEGIN").unwrap();
        let mut st = sql
            .prepare("INSERT INTO memories(content,kind,tags,importance,created_at,valid_from,content_hash) VALUES (?1,'note','',0.5,0,0,?1)")
            .unwrap();
        for n in 0..50_000 {
            st.execute(params![format!("fill {n}")]).unwrap();
        }
        sql.execute_batch("COMMIT").unwrap();
        drop(st);
        let noor = entity(db, "Noor", 0);
        let delta = entity(db, "Delta", 0);
        let aisha = entity(db, "Aisha", 0);
        // hop 1: matches the question fully
        relation(db, noor, delta, pid, 0, 1.0);
        // hop 2: weak cosine (axis 7), beyond the distractor crowd
        relation(db, delta, aisha, pid, 7, 0.3);
        // 110 distractor facts, incident to unrelated entities (not on the
        // chain), fully on axis 0: they exhaust the KNN-100 above hop2's
        // cosine. Delta's own incident count stays small.
        let d0 = entity(db, "distractor zero", 0);
        let d1 = entity(db, "distractor one", 0);
        for _ in 0..110 {
            relation(db, d0, d1, pid, 0, 1.0);
        }
        (noor, delta, aisha)
    }

    #[test]
    fn a_small_bridge_keeps_its_whole_frontier_reachable() {
        let db = Db::open_in_memory().unwrap();
        db.sql()
            .execute(
                "INSERT INTO predicates(canonical,display,exclusive,group_id,symmetric,seeded)
                 VALUES ('likes','likes',0,0,0,0)",
                [],
            )
            .unwrap();
        let pid = db.sql().last_insert_rowid();
        let (noor, delta, aisha) = fixture(&db, pid);
        let query = vector(0);
        let vis = Visibility {
            now: 0,
            as_of: None,
            history: false,
        };
        let hits = search_semantic(
            &db,
            "Who started the firm",
            &query,
            &[noor],
            &[noor],
            false,
            &[],
            &super::relation_knn(&db, &query).unwrap(),
            vis,
            false,
            super::NAME_PART_HOP,
        )
        .unwrap();
        let chain = hits
            .iter()
            .find(|h| h.steps.len() == 2)
            .unwrap_or_else(|| panic!("no two-hop chain: {hits:?}"));
        assert_eq!(chain.steps[0].dst, delta, "hop one lands on the bridge");
        assert_eq!(chain.steps[1].dst, aisha, "hop two reaches the answer");
    }
}

/// nc-prose2: `prose_wide` renders the prose steps of the top semantic chains
/// as their own fact lines - any hop position, a memory already rendered is
/// skipped, and the cap bounds the added context.
#[cfg(test)]
mod prose_wide_tests {
    use super::{prose_wide_facts, PathHit, Step, PROSE_WIDE_FACTS};
    use crate::db::Db;
    use crate::engine::FactHit;
    use rusqlite::params;
    use std::collections::HashSet;

    fn entity(db: &Db, name: &str) -> i64 {
        db.sql()
            .execute(
                "INSERT INTO entities(name,norm,type,homonym,created_at,updated_at)
                 VALUES (?1,?1,'',0,0,0)",
                params![name],
            )
            .unwrap();
        db.sql().last_insert_rowid()
    }

    fn memory(db: &Db, content: &str) -> i64 {
        db.sql()
            .execute(
                "INSERT INTO memories(content,kind,tags,importance,created_at,valid_from,content_hash)
                 VALUES (?1,'fact','',0.5,0,0,?1)",
                params![content],
            )
            .unwrap();
        db.sql().last_insert_rowid()
    }

    fn step(src: i64, dst: i64, memory: Option<i64>, fact: &str) -> Step {
        Step {
            relation_id: if memory.is_some() { 0 } else { src * 100 + dst },
            src,
            dst,
            predicate: String::new(),
            fact: fact.into(),
            reverse: false,
            matched: 0,
            memory,
        }
    }

    fn chain(steps: Vec<Step>) -> PathHit {
        PathHit {
            steps,
            coverage: 1,
            score: 1.0,
        }
    }

    fn ids(facts: &[FactHit]) -> Vec<i64> {
        facts.iter().map(|f| f.id).collect()
    }

    /// A prose hop between two relation hops is not the chain's terminal, so
    /// `facts` never renders it; `prose_wide` does.
    #[test]
    fn a_middle_prose_hop_is_rendered() {
        let db = Db::open_in_memory().unwrap();
        let (a, b, c) = (entity(&db, "a"), entity(&db, "b"), entity(&db, "c"));
        let bridge = memory(&db, "b trained at c");
        let hit = chain(vec![
            step(a, b, None, "a knows b"),
            step(b, c, Some(bridge), "b trained at c"),
            step(c, a, None, "c hosts a"),
        ]);
        let facts = prose_wide_facts(&db, &[hit], &HashSet::new(), 100).unwrap();
        assert_eq!(ids(&facts), vec![bridge]);
        assert!(facts[0].memory);
        assert_eq!(facts[0].source, "b");
        assert_eq!(facts[0].target, "c");
    }

    /// A prose memory the caller already renders as a memory needs no line.
    #[test]
    fn a_memory_already_rendered_is_skipped() {
        let db = Db::open_in_memory().unwrap();
        let (a, b) = (entity(&db, "a"), entity(&db, "b"));
        let seen = memory(&db, "a met b");
        let fresh = memory(&db, "b trained at a");
        let hit = chain(vec![
            step(a, b, Some(seen), "a met b"),
            step(b, a, Some(fresh), "b trained at a"),
        ]);
        let rendered: HashSet<i64> = [seen].into_iter().collect();
        let facts = prose_wide_facts(&db, &[hit], &rendered, 100).unwrap();
        assert_eq!(ids(&facts), vec![fresh]);
    }

    /// At most `PROSE_WIDE_FACTS` lines, destination hop first.
    #[test]
    fn the_line_cap_bounds_the_added_context() {
        let db = Db::open_in_memory().unwrap();
        let nodes: Vec<i64> = (0..5).map(|i| entity(&db, &format!("n{i}"))).collect();
        let memories: Vec<i64> = (0..4).map(|i| memory(&db, &format!("m{i}"))).collect();
        let steps: Vec<Step> = (0..4)
            .map(|i| step(nodes[i], nodes[i + 1], Some(memories[i]), "prose"))
            .collect();
        let facts = prose_wide_facts(&db, &[chain(steps)], &HashSet::new(), 100).unwrap();
        assert_eq!(facts.len(), PROSE_WIDE_FACTS);
        assert_eq!(ids(&facts), vec![memories[3], memories[2], memories[1]]);
    }

    /// Only the top three semantic chains contribute prose steps.
    #[test]
    fn only_the_top_three_chains_contribute() {
        let db = Db::open_in_memory().unwrap();
        let (a, b) = (entity(&db, "a"), entity(&db, "b"));
        let memories: Vec<i64> = (0..5).map(|i| memory(&db, &format!("m{i}"))).collect();
        let chains: Vec<PathHit> = memories
            .iter()
            .map(|m| chain(vec![step(a, b, Some(*m), "prose")]))
            .collect();
        let facts = prose_wide_facts(&db, &chains, &HashSet::new(), 100).unwrap();
        assert_eq!(ids(&facts), vec![memories[0], memories[1], memories[2]]);
    }

    /// The knob's default is off, and the eval/config surface names it.
    #[test]
    fn the_knob_defaults_off() {
        let cfg = crate::engine::Config::default();
        assert!(!cfg.prose_wide);
        let mut cfg = cfg;
        crate::eval::set_config(&mut cfg, "prose_wide", "on").unwrap();
        assert!(cfg.prose_wide);
        assert!(crate::eval::set_config(&mut cfg, "prose_wide", "maybe").is_err());
    }
}

/// nc-passive: "which language is used by <person>'s project" and "the main
/// language of the project led by <person>" ask the same two hops as the
/// active shape. The passive verb must read as its active predicate ("used" /
/// "led" -> use / lead), an answer-type noun must still steer through its
/// relation synonym ("language" -> use), and "<person>'s project" must start
/// the walk at the project the person leads. Before that the beams left the
/// person and rendered hop 1 only (world-300: five two-hop misses).
#[cfg(test)]
mod passive_query_tests {
    use super::{evidence, search, Visibility};
    use crate::embed::HashEmbedder;
    use crate::engine::{Engine, EntityIn, RelationIn, RememberInput};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn remember(
        engine: &Engine,
        content: &str,
        entities: &[(&str, &str)],
        (source, predicate, target, fact): (&str, &str, &str, &str),
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
                relations: vec![RelationIn {
                    source: source.into(),
                    predicate: predicate.into(),
                    target: target.into(),
                    fact: Some(fact.into()),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap()
            .id
    }

    /// Two people, two projects, two languages.
    fn fixture() -> (Engine, i64, i64) {
        let engine = Engine::open_in_memory(Arc::new(HashEmbedder)).unwrap();
        remember(
            &engine,
            "Cedric Narmani leads the Vesper Monitor project.",
            &[("Cedric Narmani", "person"), ("Vesper Monitor", "project")],
            (
                "Cedric Narmani",
                "leads",
                "Vesper Monitor",
                "Cedric Narmani leads Vesper Monitor",
            ),
        );
        let ruby = remember(
            &engine,
            "Vesper Monitor is written in Ruby.",
            &[("Vesper Monitor", "project"), ("Ruby", "tool")],
            ("Vesper Monitor", "uses", "Ruby", "Vesper Monitor uses Ruby"),
        );
        remember(
            &engine,
            "Ruby Chanyya leads the Borealis Inbox project.",
            &[("Ruby Chanyya", "person"), ("Borealis Inbox", "project")],
            (
                "Ruby Chanyya",
                "leads",
                "Borealis Inbox",
                "Ruby Chanyya leads Borealis Inbox",
            ),
        );
        let scala = remember(
            &engine,
            "Borealis Inbox is written in Scala.",
            &[("Borealis Inbox", "project"), ("Scala", "tool")],
            (
                "Borealis Inbox",
                "uses",
                "Scala",
                "Borealis Inbox uses Scala",
            ),
        );
        (engine, ruby, scala)
    }

    fn today() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    fn reached(engine: &Engine, query: &str, person: &str, memory: i64) -> bool {
        let seed: i64 = engine
            .db
            .sql()
            .query_row("SELECT id FROM entities WHERE name=?1", [person], |r| {
                r.get(0)
            })
            .unwrap();
        let vis = Visibility {
            now: today(),
            as_of: None,
            history: false,
        };
        let hits = search(&engine.db, query, &[seed], vis).unwrap();
        evidence(&engine.db, &hits).unwrap().contains(&memory)
    }

    #[test]
    fn a_possessive_passive_question_walks_to_the_language() {
        let (engine, ruby, _) = fixture();
        assert!(
            reached(
                &engine,
                "which language is used by Cedric Narmani's project",
                "Cedric Narmani",
                ruby
            ),
            "hop 2 (project -> language) was not walked"
        );
    }

    #[test]
    fn a_passive_relative_clause_walks_to_the_language() {
        let (engine, _, scala) = fixture();
        assert!(
            reached(
                &engine,
                "what is the main language of the project led by Ruby Chanyya",
                "Ruby Chanyya",
                scala
            ),
            "hop 2 (project -> language) was not walked"
        );
    }

    #[test]
    fn the_active_shape_still_walks_both_hops() {
        let (engine, ruby, _) = fixture();
        assert!(reached(
            &engine,
            "what language does the project Cedric Narmani leads use",
            "Cedric Narmani",
            ruby
        ));
    }
}
