use crate::{
    db::Db,
    embed::{Embedder, Mode},
    engine::{
        Config, EntityIn, EntityResolution, RelationIn, RelationResult, RememberInput,
        RememberOutput,
    },
    error::Result,
    extract, kg, resolve,
    temporal::{self, Clock},
    text,
};
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};

/// Words allowed between a negation cue and the predicate verb. Anything else
/// ("Bob" in "Alice, not Bob, lives in Denver") makes the cue belong to another
/// phrase, so the assertion stays positive.
const NEGATION_FILLERS: &[&str] = &[
    "does", "do", "did", "is", "was", "were", "has", "have", "had", "really", "even", "ever",
    "actually", "longer", "more", "anymore", "be", "been", "being", "will", "would", "could",
    "should", "can", "may", "might",
];

/// Multi-word cues (checked before single words).
const NEGATION_CUES: &[&[&str]] = &[
    &["no", "longer"],
    &["no", "more"],
    &["does", "not"],
    &["did", "not"],
    &["do", "not"],
    &["is", "not"],
    &["was", "not"],
    &["were", "not"],
    &["has", "not"],
    &["have", "not"],
    &["had", "not"],
    &["will", "not"],
    &["would", "not"],
    &["could", "not"],
    &["should", "not"],
    &["can", "not"],
    &["ya", "no"],
];

const NEGATION_WORDS: &[&str] = &[
    "not", "never", "stopped", "ceased", "doesn't", "didn't", "don't", "isn't", "wasn't",
    "weren't", "won't", "nor", "cannot", "can't", "quit", "left", "divorced", "sold",
];

/// Retraction phrasings, mapped to the predicate group they retract. An empty
/// group is generic ("is no longer at Acme") and applies to any predicate.
const RETRACTIONS: &[(&[&str], &str)] = &[
    (&["resigned", "from"], "work at"),
    (&["quit"], "work at"),
    (&["left", "the", "job", "at"], "work at"),
    (&["left", "his", "job", "at"], "work at"),
    (&["left", "her", "job", "at"], "work at"),
    (&["left", "their", "job", "at"], "work at"),
    (&["left", "his", "position", "at"], "work at"),
    (&["left", "her", "position", "at"], "work at"),
    (&["left", "their", "position", "at"], "work at"),
    (&["left"], "work at"),
    (&["moved", "out", "of"], "live in"),
    (&["broke", "up", "with"], "married to"),
    (&["split", "up", "with"], "married to"),
    (&["separated", "from"], "married to"),
    (&["divorced"], "married to"),
    (&["sold"], "own"),
    (&["stopped", "using"], "use"),
    (&["gave", "up"], "use"),
    (&["is", "no", "longer"], ""),
    (&["was", "no", "longer"], ""),
    (&["no", "longer"], ""),
    (&["ya", "no"], ""),
];

/// A clause boundary or a conjunction means the phrase and the target are not
/// in the same retraction.
const RETRACTION_BREAKS: &[&str] = &[
    "and", "but", "because", "while", "although", "though", "who", "which", "that",
];

/// A determiner between the cue and a name makes the name part of a longer
/// noun phrase ("the Acme office"), so it is not the object of the cue.
const OBJECT_DETERMINERS: &[&str] = &[
    "the", "a", "an", "some", "any", "his", "her", "their", "its", "my", "our", "your", "this",
    "that", "these", "those",
];

/// Followers that end the object with a clause boundary.
const CLAUSE_FOLLOWERS: &[&str] = &["and", "but", "then"];

/// Calendar and clock words a time expression may start with or contain.
const TIME_WORDS: &[&str] = &[
    "yesterday",
    "today",
    "tomorrow",
    "tonight",
    "ago",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "mar",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "sept",
    "oct",
    "nov",
    "dec",
    "day",
    "days",
    "week",
    "weeks",
    "month",
    "months",
    "year",
    "years",
    "hour",
    "hours",
    "minute",
    "minutes",
    "spring",
    "summer",
    "autumn",
    "fall",
    "winter",
    "noon",
    "midnight",
    "night",
    "morning",
    "evening",
    "afternoon",
];

/// Hedges and modality in the clause before the cue: the sentence reports a
/// belief, a plan or a near miss, so the fact is not ended. The relation is
/// offered as a conflict candidate instead.
const HEDGES: &[&str] = &[
    "almost",
    "nearly",
    "might",
    "may",
    "could",
    "would",
    "should",
    "plans",
    "plan",
    "planned",
    "wants",
    "want",
    "considering",
    "thinking",
    "thought",
    "if",
    "unless",
    "rumoured",
    "rumored",
    "reportedly",
    "thinks",
    "think",
    "believes",
    "believe",
    "said",
    "says",
    "say",
    "hopes",
    "hope",
];

/// How the object of a cue ends. Only the first three make the name the whole
/// object; `Noun` means the name continues into a noun phrase or is a
/// ditransitive object ("the Acme office", "Acme's parking lot", "left Acme a
/// voicemail"), and the sentence retracts nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ObjectTail {
    /// End of sentence, punctuation, or a clause boundary ("and", "but").
    Boundary,
    /// A time expression ("last month", "in June", "yesterday", "at 5pm").
    Time,
    /// An employment move ("to join Globex", "for Globex", "for good").
    Employment,
    /// The name continues into a noun phrase.
    Noun,
}

/// Classify what follows the object. Rule 1 of the retraction contract: the
/// object span must be the whole object of the cue.
fn object_tail(
    lower: &str,
    object_end: usize,
    words: &[(usize, usize, &str)],
    next: usize,
    group: &str,
    spans: &[(usize, usize, String)],
) -> ObjectTail {
    let rest = &lower[object_end..];
    // A possessive continues the noun phrase ("Acme's parking lot").
    if rest.starts_with("'s") || rest.starts_with('\u{2019}') {
        return ObjectTail::Noun;
    }
    let Some(&(start, _, word)) = words.get(next) else {
        return ObjectTail::Boundary;
    };
    if start < object_end
        || rest[..start - object_end]
            .chars()
            .any(|c| !c.is_whitespace())
    {
        // Punctuation right after the name closes the object.
        return ObjectTail::Boundary;
    }
    if CLAUSE_FOLLOWERS.contains(&word) {
        return ObjectTail::Boundary;
    }
    if time_tail(words, next) {
        return ObjectTail::Time;
    }
    match (word, words.get(next + 1).map(|w| w.2)) {
        ("for", Some("good")) | ("to", Some("join")) => ObjectTail::Employment,
        ("for", Some(_))
            if group == "work at" && spans.iter().any(|(s, _, _)| *s == words[next + 1].0) =>
        {
            ObjectTail::Employment
        }
        _ => ObjectTail::Noun,
    }
}

/// True when the word at `i` starts a time expression. `at` needs a clock
/// time, so "at the station" stays a noun phrase.
fn time_tail(words: &[(usize, usize, &str)], i: usize) -> bool {
    let word = words[i].2;
    let next = words.get(i + 1).map(|w| w.2);
    match word {
        "yesterday" | "today" | "tomorrow" | "tonight" | "ago" => true,
        "last" | "this" | "next" => next.is_some_and(|n| TIME_WORDS.contains(&n)),
        "in" | "on" => {
            next.is_some_and(|n| TIME_WORDS.contains(&n) || n.chars().all(|c| c.is_ascii_digit()))
        }
        "as" => next == Some("of"),
        "at" => next.is_some_and(|n| {
            n.chars().any(|c| c.is_ascii_digit())
                || matches!(
                    n,
                    "noon" | "midnight" | "night" | "morning" | "evening" | "afternoon"
                )
        }),
        _ => false,
    }
}

/// True when the clause from word `from` up to the cue hedges, plans or
/// reports the event.
fn hedged(words: &[(usize, usize, &str)], from: usize, cue: usize) -> bool {
    words[from..cue].iter().any(|w| HEDGES.contains(&w.2))
}

fn words_of(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Conservative negation cue for one relation: the clause must name the
/// relation's own source, `text` must name the target within 3 tokens after
/// the predicate verb, and a cue may only be separated from that verb by an
/// auxiliary/adverb. Retraction phrasings are matched separately and only for
/// their predicate group.
fn infer_negated(text: &str, predicate: &str, group: &str, target: &str, source: &str) -> bool {
    let target_words = words_of(target);
    if target_words.is_empty() {
        return false;
    }
    // Split on sentence/clause punctuation; commas stay because "Alice, not
    // Bob, lives" must see the cue and the intervening name.
    let clause = text
        .split(['.', ';', '!', '?', '\n'])
        .find(|c| {
            let w = words_of(c);
            w.windows(target_words.len())
                .any(|x| x == target_words.as_slice())
        })
        .unwrap_or(text);
    let words = words_of(clause);
    // A cue about another subject must not negate this relation ("Alice no
    // longer leads the core team" says nothing about Bob's relation).
    if !names_source(&words, source) {
        return false;
    }
    let Some(target_pos) = (0..words.len()).find(|&i| words[i..].starts_with(&target_words)) else {
        return false;
    };

    // (a)+(b): target within 3 tokens after the predicate verb, cue separated
    // from the verb only by allowlisted words.
    let verb = predicate.split_whitespace().next().unwrap_or("");
    let stem = verb.trim_end_matches('e');
    if !stem.is_empty() {
        if let Some(verb_pos) = (0..target_pos).rev().find(|&i| words[i].starts_with(stem)) {
            if target_pos.saturating_sub(verb_pos + 1) <= 3 {
                if let Some(cue_end) = cue_before(&words, verb_pos) {
                    if words[cue_end..verb_pos]
                        .iter()
                        .all(|w| NEGATION_FILLERS.contains(&w.as_str()))
                    {
                        return true;
                    }
                }
            }
        }
    }

    // (c): a retraction phrase immediately before the target, for its group.
    let mut best: Option<(usize, &str)> = None;
    for (phrase, retract_group) in RETRACTIONS {
        for start in (0..target_pos).rev() {
            if start + phrase.len() > target_pos {
                continue;
            }
            if words[start..start + phrase.len()]
                .iter()
                .map(String::as_str)
                .eq(phrase.iter().copied())
            {
                let gap = &words[start + phrase.len()..target_pos];
                if gap.len() <= 3 && !gap.iter().any(|w| RETRACTION_BREAKS.contains(&w.as_str())) {
                    let end = start + phrase.len();
                    if best.is_none_or(|(seen, _)| end > seen) {
                        best = Some((end, retract_group));
                    }
                }
                break;
            }
        }
    }
    if let Some((_, retract_group)) = best {
        if retract_group.is_empty() || retract_group == group {
            return true;
        }
    }
    false
}

/// True when the clause names the relation's own source. A multi-word source
/// also matches its head word, because running text shortens names ("Bob" for
/// "Bob Okafor").
fn names_source(words: &[String], source: &str) -> bool {
    let src = words_of(source);
    let Some(head) = src.first() else {
        return false;
    };
    if src.len() == 1 {
        return words.iter().any(|w| w == head);
    }
    words.windows(src.len()).any(|x| x == src.as_slice()) || words.iter().any(|w| w == head)
}

/// End index (exclusive) of the nearest cue that ends at or before `verb_pos`.
fn cue_before(words: &[String], verb_pos: usize) -> Option<usize> {
    for start in (0..verb_pos).rev() {
        for cue in NEGATION_CUES {
            if start + cue.len() <= verb_pos
                && words[start..start + cue.len()]
                    .iter()
                    .map(String::as_str)
                    .eq(cue.iter().copied())
            {
                return Some(start + cue.len());
            }
        }
        if NEGATION_WORDS.contains(&words[start].as_str()) {
            // "never stopped liking" is a double negative: still positive.
            if matches!(words[start].as_str(), "stopped" | "ceased")
                && start > 0
                && matches!(words[start - 1].as_str(), "never" | "not")
            {
                continue;
            }
            return Some(start + 1);
        }
    }
    None
}

/// A retraction the text states without agent-supplied relations: the
/// sentence names a subject and an object, and the phrase maps to a group.
struct TextRetraction {
    subject: String,
    object: String,
    group: &'static str,
    /// The relation noun the sentence names ("no longer partners"); when set,
    /// only relations whose fact uses that noun match.
    noun: Option<&'static str>,
    /// The sentence hedges or reports the event, or a name it uses is
    /// ambiguous: offer the relation as a conflict candidate, never end it.
    candidate: bool,
}

/// Spans where a name occurs as whole words in the content, as
/// (start, end, normalized name). The name is lower-cased first, so offsets
/// index the lower-cased copy used by the caller.
fn name_spans(
    content: &str,
    names: impl IntoIterator<Item = String>,
) -> Vec<(usize, usize, String)> {
    let lower = content.to_lowercase();
    let mut spans = Vec::new();
    for name in names {
        let needle = name.to_lowercase();
        let norm = text::norm_name(&name);
        if needle.is_empty() || norm.is_empty() {
            continue;
        }
        let mut from = 0;
        while let Some(rel) = lower[from..].find(&needle) {
            let start = from + rel;
            let end = start + needle.len();
            let before = lower[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric());
            let after = lower[end..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric());
            if before && after {
                spans.push((start, end, norm.clone()));
            }
            from = end;
        }
    }
    spans
}

/// Name spans found in the text plus the first-name mentions that are too
/// ambiguous to act on.
struct RetractionSpans {
    spans: Vec<(usize, usize, String)>,
    /// A first name shared by several known people ("Maria" with two stored
    /// Marias): a retraction on it is reported as a conflict, never ended.
    ambiguous: HashSet<(usize, usize)>,
}

/// What a text retraction may end and what it must leave alone: `exclude` is
/// the relation ids this memory just wrote as its own assertion.
struct RetractionSink<'a> {
    exclude: &'a [i64],
    invalidated: &'a mut Vec<i64>,
    conflicts: &'a mut Vec<crate::engine::Conflict>,
}

/// Every name span in the text that names a known entity: this memory's
/// entities, the heuristic proper-noun spans (so a sentence-initial name is
/// not lost), and alias n-grams already in the gazetteer.
fn retraction_spans(db: &Db, content: &str, entities: &[EntityIn]) -> Result<RetractionSpans> {
    let mut names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
    names.extend(
        extract::heuristic_entities(content)
            .into_iter()
            .map(|c| c.name),
    );
    let mut spans = name_spans(content, names);
    for (norm, start, end) in extract::ngrams(content, 4) {
        if norm.chars().count() < 3 {
            continue;
        }
        let known: bool = db.sql().query_row(
            "SELECT EXISTS(SELECT 1 FROM entity_aliases WHERE alias_norm=?1)",
            [&norm],
            |r| r.get(0),
        )?;
        if known {
            spans.push((start, end, norm));
        }
    }
    let extra = first_name_spans(db, content, entities, &spans)?;
    // A resolved first-name span replaces the bare heuristic span for the same
    // surface ("Tom" -> "tom duarte"), so the retraction can match the card.
    spans.retain(|(s, e, _)| !extra.spans.iter().any(|(xs, xe, _)| *xs <= *s && *xe >= *e));
    spans.extend(extra.spans);
    let ambiguous = extra.ambiguous;
    // Longest span first at the same start, so "New York City" beats "New".
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    spans.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
    Ok(RetractionSpans { spans, ambiguous })
}

/// Spans for first names the text uses on their own ("Maria divorced Tom"):
/// a first name that belongs to exactly one known person resolves to that
/// person's canonical norm, so the retraction can match the stored relation.
/// A first name shared by several people stays ambiguous.
fn first_name_spans(
    db: &Db,
    content: &str,
    entities: &[EntityIn],
    spans: &[(usize, usize, String)],
) -> Result<RetractionSpans> {
    let lower = content.to_lowercase();
    let mut st = db
        .sql()
        .prepare("SELECT norm FROM entities WHERE type='person'")?;
    let norms: Vec<String> = st
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut by_first: HashMap<String, Vec<String>> = HashMap::new();
    for norm in norms {
        let mut parts = norm.split_whitespace();
        let Some(first) = parts.next() else { continue };
        if first.chars().count() < 3 || parts.next().is_none() {
            continue;
        }
        by_first.entry(first.to_string()).or_default().push(norm);
    }
    let supplied: HashSet<String> = entities.iter().map(|e| text::norm_name(&e.name)).collect();
    let mut extra = Vec::new();
    let mut ambiguous = HashSet::new();
    for (first, mut candidates) in by_first {
        candidates.sort();
        candidates.dedup();
        for (start, end) in word_occurrences(&lower, &first) {
            // The full name already covers this mention ("Maria Duarte").
            let covered = spans
                .iter()
                .any(|(s, e, _)| *s <= start && *e >= end && *e - *s > end - start);
            if covered {
                continue;
            }
            // Prefer the person this write supplied when the name is shared.
            let norm = candidates
                .iter()
                .find(|n| supplied.contains(n.as_str()))
                .unwrap_or(&candidates[0])
                .clone();
            if candidates.len() > 1 {
                ambiguous.insert((start, end));
            }
            extra.push((start, end, norm));
        }
    }
    Ok(RetractionSpans {
        spans: extra,
        ambiguous,
    })
}

fn word_occurrences(lower: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(needle) {
        let start = from + rel;
        let end = start + needle.len();
        let before = lower[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after = lower[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before && after {
            out.push((start, end));
        }
        from = end;
    }
    out
}

/// Retractions stated by a text-only memory. The object is the first named
/// entity after the phrase (or "there" points back); the subject is the
/// nearest named entity before it, falling back to the subject of an earlier
/// clause whose object it was ("Alice moved out of Austin and broke up with
/// Bob" shares Alice).
fn text_retractions(content: &str, spans: &RetractionSpans) -> Vec<TextRetraction> {
    let lower = content.to_lowercase();
    let words: Vec<(usize, usize, &str)> = {
        let mut out = Vec::new();
        let mut start = None;
        for (i, ch) in lower.char_indices() {
            if ch.is_alphanumeric() {
                start.get_or_insert(i);
            } else if let Some(s) = start.take() {
                out.push((s, i, &lower[s..i]));
            }
        }
        if let Some(s) = start {
            out.push((s, lower.len(), &lower[s..]));
        }
        out
    };
    let mut found: Vec<(usize, TextRetraction)> = Vec::new();
    for (phrase, group) in RETRACTIONS {
        let plen = phrase.len();
        if plen == 0 || words.len() < plen {
            continue;
        }
        for p in 0..=words.len() - plen {
            if !words[p..p + plen]
                .iter()
                .map(|w| w.2)
                .eq(phrase.iter().copied())
            {
                continue;
            }
            let phrase_start = words[p].0;
            let phrase_end = words[p + plen - 1].1;
            let max_end = words.get(p + plen + 2).map(|w| w.1).unwrap_or(usize::MAX);
            let mut object_span = spans
                .spans
                .iter()
                .find(|(s, _, _)| *s >= phrase_end && *s <= max_end)
                .cloned();
            if object_span.is_none() && words.iter().skip(p + plen).take(4).any(|w| w.2 == "there")
            {
                object_span = spans
                    .spans
                    .iter()
                    .rev()
                    .find(|(_, e, _)| *e <= phrase_start)
                    .cloned();
            }
            let Some((object_start, object_end, object)) = object_span else {
                continue;
            };
            let mut candidate = spans.ambiguous.contains(&(object_start, object_end));
            if object_start >= phrase_end {
                // A span may start before the name it normalises to ("the
                // Acme" from the alias n-grams), so the gap and the tail are
                // measured from different edges.
                let gap_end = words.partition_point(|w| w.0 < object_start);
                let next_word = words.partition_point(|w| w.0 < object_end);
                // Rule 1: a determiner between the cue and the name means the
                // name is inside a longer noun phrase, not the object. The
                // determiner that immediately precedes the name belongs to the
                // object phrase itself ("the core team"), so only an earlier
                // one rejects the match.
                let gap = &words[p + plen..gap_end];
                let earlier = gap.split_last().map_or(gap, |(_, rest)| rest);
                if earlier.iter().any(|w| OBJECT_DETERMINERS.contains(&w.2)) {
                    continue;
                }
                // Rules 1+2: the object must be the whole object. A bare
                // "left" also means "departed from" or "left behind", so it
                // ends nothing unless a clause boundary, a time expression or
                // an employment tail follows (rule 4).
                if object_tail(&lower, object_end, &words, next_word, group, &spans.spans)
                    == ObjectTail::Noun
                {
                    continue;
                }
                // Rule 3: a hedge or report before the cue ("almost quit",
                // "thinks Bob left") is a candidate, not an end.
                let clause = lower[..phrase_start]
                    .char_indices()
                    .rev()
                    .find(|(_, c)| matches!(c, '.' | ';' | '!' | '?' | '\n'))
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                let from = words.partition_point(|w| w.0 < clause);
                candidate |= hedged(&words, from, p);
            }
            let min_start = words.get(p.saturating_sub(10)).map(|w| w.0).unwrap_or(0);
            let Some((subject_start, subject_end, subject)) = spans
                .spans
                .iter()
                .rev()
                .find(|(s, e, name)| *e <= phrase_start && *s >= min_start && *name != object)
                .cloned()
            else {
                continue;
            };
            candidate |= spans.ambiguous.contains(&(subject_start, subject_end));
            found.push((
                phrase_start,
                TextRetraction {
                    subject,
                    object,
                    group,
                    noun: None,
                    candidate,
                },
            ));
        }
    }
    found.extend(pair_retractions(&lower, &words, spans));
    found.sort_by_key(|(pos, _)| *pos);
    let mut out: Vec<TextRetraction> = Vec::new();
    let mut subject_of_object: HashMap<String, String> = HashMap::new();
    for (_, mut retraction) in found {
        // An elided subject after "and" is the subject of the clause whose
        // object it repeats: "Alice left Acme and broke up with Bob".
        if let Some(shared) = subject_of_object.get(&retraction.subject) {
            if *shared != retraction.object {
                retraction.subject = shared.clone();
            }
        }
        if retraction.subject == retraction.object {
            continue;
        }
        subject_of_object.insert(retraction.object.clone(), retraction.subject.clone());
        if let Some(seen) = out.iter_mut().find(|r| {
            r.subject == retraction.subject
                && r.object == retraction.object
                && r.group == retraction.group
        }) {
            // A plain statement of the same retraction outranks a hedged one.
            seen.candidate &= retraction.candidate;
        } else {
            out.push(retraction);
        }
    }
    out
}

/// Relation nouns a coordinated pair can be retracted by ("A and B are no
/// longer partners"), with the predicate group each names ("" = any group,
/// narrowed by the noun in the fact).
const PAIR_NOUNS: &[(&str, &str)] = &[
    ("partners", ""),
    ("partner", ""),
    ("married", "married to"),
    ("friends", "friend with"),
    ("friend", "friend with"),
];

/// "A and B are no longer <relation noun>": the pair before the cue is the
/// subject and the object, so the retraction needs no object after the cue.
fn pair_retractions(
    lower: &str,
    words: &[(usize, usize, &str)],
    spans: &RetractionSpans,
) -> Vec<(usize, TextRetraction)> {
    const CUES: &[&[&str]] = &[
        &["is", "no", "longer"],
        &["are", "no", "longer"],
        &["was", "no", "longer"],
        &["were", "no", "longer"],
        &["no", "longer"],
    ];
    let mut out = Vec::new();
    for cue in CUES {
        if words.len() < cue.len() {
            continue;
        }
        for p in 0..=words.len() - cue.len() {
            if !words[p..p + cue.len()]
                .iter()
                .map(|w| w.2)
                .eq(cue.iter().copied())
            {
                continue;
            }
            let phrase_start = words[p].0;
            let mut q = p + cue.len();
            while q < words.len()
                && matches!(
                    words[q].2,
                    "be" | "been" | "being" | "still" | "a" | "an" | "the"
                )
            {
                q += 1;
            }
            let Some(&(_, _, seen)) = words.get(q) else {
                continue;
            };
            let Some(&(noun, group)) = PAIR_NOUNS.iter().find(|(n, _)| *n == seen) else {
                continue;
            };
            // The two nearest name spans before the cue, joined by "and".
            let before: Vec<&(usize, usize, String)> = spans
                .spans
                .iter()
                .filter(|(_, e, _)| *e <= phrase_start)
                .collect();
            let (Some(a), Some(b)) = (before.iter().rev().nth(1).copied(), before.last().copied())
            else {
                continue;
            };
            if a.1 > b.0
                || !words
                    .iter()
                    .any(|w| w.2 == "and" && w.0 >= a.1 && w.1 <= b.0)
            {
                continue;
            }
            // The pair must be the clause's subject: only a short run of
            // auxiliaries/modal words may separate it from the cue.
            if words.partition_point(|w| w.0 < b.1) + 4 < p {
                continue;
            }
            let clause = lower[..phrase_start]
                .char_indices()
                .rev()
                .find(|(_, c)| matches!(c, '.' | ';' | '!' | '?' | '\n'))
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            let from = words.partition_point(|w| w.0 < clause);
            let mut candidate = hedged(words, from, p);
            candidate |= spans.ambiguous.contains(&(a.0, a.1));
            candidate |= spans.ambiguous.contains(&(b.0, b.1));
            if a.2 == b.2 {
                continue;
            }
            out.push((
                phrase_start,
                TextRetraction {
                    subject: a.2.clone(),
                    object: b.2.clone(),
                    group,
                    // Only the generic group needs the fact-noun filter.
                    noun: group.is_empty().then_some(noun),
                    candidate,
                },
            ));
        }
    }
    out
}

fn end_text_retractions(
    db: &Db,
    content: &str,
    entities: &[EntityIn],
    at: i64,
    now: i64,
    sink: &mut RetractionSink<'_>,
) -> Result<()> {
    // Most text-only memories state nothing retracted; skip the alias scan.
    let words = words_of(content);
    let has_cue = RETRACTIONS.iter().any(|(phrase, _)| {
        phrase.len() <= words.len()
            && words
                .windows(phrase.len())
                .any(|window| window.iter().map(String::as_str).eq(phrase.iter().copied()))
    });
    if !has_cue {
        return Ok(());
    }
    let spans = retraction_spans(db, content, entities)?;
    for retraction in text_retractions(content, &spans) {
        let group_id: Option<i64> = if retraction.group.is_empty() {
            None
        } else {
            let id = db
                .sql()
                .query_row(
                    "SELECT id FROM predicates WHERE canonical=?1",
                    [retraction.group],
                    |r| r.get(0),
                )
                .optional()?;
            match id {
                Some(id) => Some(id),
                // The group is not in this database's ontology.
                None => continue,
            }
        };
        // A live fact recorded at ingest time may start after the retraction's
        // inferred date ("divorced ... in 2024"): match it and let the
        // correction branch retire it. Ended rows still need to contain the
        // retraction date.
        let mut st = db.sql().prepare(
            "SELECT r.id,r.fact,r.valid_from FROM relations r
             JOIN predicates p ON p.id=r.predicate_id
             JOIN entities s ON s.id=r.src
             JOIN entities d ON d.id=r.dst
             WHERE r.expired_at IS NULL AND r.polarity=1
               AND ((s.norm=?1 AND d.norm=?2) OR (s.norm=?2 AND d.norm=?1))
               AND (r.valid_to IS NULL OR (r.valid_from<=?3 AND r.valid_to>?3))
               AND (?4 IS NULL OR COALESCE(NULLIF(p.group_id,0),p.id)=?4)",
        )?;
        let rows: Vec<(i64, String, i64)> = st
            .query_map(
                params![retraction.subject, retraction.object, at, group_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?
            .collect::<rusqlite::Result<_>>()?;
        let rows: Vec<(i64, String, i64)> = rows
            .into_iter()
            .filter(|(rid, _, _)| !sink.exclude.contains(rid))
            .filter(|(_, fact, _)| {
                retraction.noun.is_none_or(|noun| {
                    words_of(fact)
                        .iter()
                        .any(|w| w == noun || text::singularize(w) == noun)
                })
            })
            .collect();
        match rows.as_slice() {
            [(rid, _, old_from)] if !retraction.candidate => {
                if *old_from >= at {
                    // The recorded start is at or after the retraction date
                    // (ingest-time recording): retire the row as a correction
                    // rather than write a zero-length interval.
                    db.sql().execute(
                        "UPDATE relations SET valid_to=?1, expired_at=?2 WHERE id=?3",
                        params![old_from, now, rid],
                    )?;
                } else {
                    db.sql().execute(
                        "UPDATE relations SET valid_to=?1 WHERE id=?2",
                        params![at, rid],
                    )?;
                }
                if !sink.invalidated.contains(rid) {
                    sink.invalidated.push(*rid);
                }
            }
            [] => {}
            rows => {
                for (rid, fact, valid_from) in rows {
                    if !sink.conflicts.iter().any(|c| c.relation_id == *rid) {
                        sink.conflicts.push(crate::engine::Conflict {
                            relation_id: *rid,
                            fact: fact.clone(),
                            valid_from: *valid_from,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn check_embedder(db: &Db, emb: &dyn Embedder) -> Result<()> {
    let found: Option<String> = db
        .sql()
        .query_row("SELECT value FROM meta WHERE key='embedder'", [], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(found) = found {
        if found != emb.name() {
            return Err(crate::err!("this database was built with embedder '{}' but '{}' is active; use the same embedder or another --db", found, emb.name()));
        }
    }
    Ok(())
}

/// Inputs per `remember_batch` chunk: one embed call and one transaction.
pub const BATCH_SIZE: usize = 256;

/// A validated input with its embedder texts laid out and its vectors filled
/// in. One `Prepared` is exactly one memory: the single and the batch path
/// share `prepare` + `write_prepared`, so they cannot drift apart.
struct Prepared {
    content: String,
    kind: String,
    tags: String,
    importance: f32,
    scope: String,
    source: String,
    session: String,
    valid_from: i64,
    valid_to: Option<i64>,
    when_inferred: Option<String>,
    entities: Vec<EntityIn>,
    relations: Vec<RelationIn>,
    facts: Vec<String>,
    extra_index: HashMap<String, usize>,
    hash: String,
    supersedes: Vec<i64>,
    /// content, entity cards, fact sentences, relation endpoints not in
    /// entities, one phrase per distinct predicate
    texts: Vec<String>,
    vectors: Vec<Vec<f32>>,
    fact_base: usize,
    extra_base: usize,
    pred_base: usize,
    pred_index: HashMap<String, usize>,
}

/// The content vector is always index 0 and entity cards start at 1.
const CARD_BASE: usize = 1;

pub fn remember(
    db: &Db,
    emb: &dyn Embedder,
    cfg: &Config,
    clock: Clock,
    inp: RememberInput,
) -> Result<RememberOutput> {
    check_embedder(db, emb)?;
    let mut prepared = prepare(db, cfg, clock, inp)?;
    embed_prepared(emb, std::slice::from_mut(&mut prepared))?;
    db.write(|db| write_prepared(db, cfg, clock, emb.name(), prepared))
}

/// Store many memories with the semantics of calling `remember` in order.
///
/// All texts of a chunk (up to `BATCH_SIZE` inputs) go through one embed call
/// in `Document` mode, then the chunk's writes run in one transaction with a
/// SAVEPOINT per item: a failing input is reported in its slot and leaves no
/// partial state, and the items after it still run.
pub fn remember_batch(
    db: &Db,
    emb: &dyn Embedder,
    cfg: &Config,
    clock: Clock,
    inputs: Vec<RememberInput>,
) -> Vec<Result<RememberOutput>> {
    let n = inputs.len();
    let mut slots: Vec<Option<Result<RememberOutput>>> = (0..n).map(|_| None).collect();
    if let Err(e) = check_embedder(db, emb) {
        return (0..n).map(|_| Err(crate::err!("{e}"))).collect();
    }
    let mut base = 0;
    for chunk in inputs.chunks(BATCH_SIZE) {
        let prepare_start = std::time::Instant::now();
        let mut prepared: Vec<(usize, Prepared)> = Vec::with_capacity(chunk.len());
        for (offset, inp) in chunk.iter().enumerate() {
            match prepare(db, cfg, clock, inp.clone()) {
                Ok(item) => prepared.push((base + offset, item)),
                Err(e) => slots[base + offset] = Some(Err(e)),
            }
        }
        base += chunk.len();
        if prepared.is_empty() {
            continue;
        }
        let prepare_ms = prepare_start.elapsed().as_secs_f64() * 1e3;
        let indices: Vec<usize> = prepared.iter().map(|(i, _)| *i).collect();
        let mut items: Vec<Prepared> = prepared.into_iter().map(|(_, p)| p).collect();
        let embed_start = std::time::Instant::now();
        if let Err(e) = embed_prepared(emb, &mut items) {
            for i in indices {
                slots[i] = Some(Err(crate::err!("{e}")));
            }
            continue;
        }
        let embed_ms = embed_start.elapsed().as_secs_f64() * 1e3;
        let mut written: Vec<(usize, Result<RememberOutput>)> = Vec::with_capacity(items.len());
        let items_len = items.len();
        let write_start = std::time::Instant::now();
        let mut savepoint_us = 0f64;
        let mut savepoint_begin_us = 0f64;
        let transaction = db.write(|db| {
            for (i, item) in indices.iter().zip(items) {
                let sp = std::time::Instant::now();
                db.sql().execute_batch("SAVEPOINT remember_item")?;
                savepoint_begin_us += sp.elapsed().as_secs_f64() * 1e6;
                let result = match write_prepared(db, cfg, clock, emb.name(), item) {
                    Ok(out) => {
                        db.sql().execute_batch("RELEASE remember_item")?;
                        Ok(out)
                    }
                    Err(e) => {
                        db.sql()
                            .execute_batch("ROLLBACK TO remember_item; RELEASE remember_item")?;
                        Err(e)
                    }
                };
                savepoint_us += sp.elapsed().as_secs_f64() * 1e6;
                written.push((*i, result));
            }
            Ok(())
        });
        match transaction {
            Ok(()) => {
                for (i, result) in written {
                    slots[i] = Some(result);
                }
            }
            Err(e) => {
                for i in indices {
                    slots[i] = Some(Err(crate::err!("{e}")));
                }
            }
        }
        crate::wtiming::chunk(
            items_len,
            prepare_ms,
            embed_ms,
            write_start.elapsed().as_secs_f64() * 1e3,
            savepoint_us / 1e3,
            savepoint_begin_us / 1e3,
        );
    }
    slots
        .into_iter()
        .map(|slot| slot.unwrap_or_else(|| Err(crate::err!("remember_batch lost an input"))))
        .collect()
}

/// Validate one input and lay out the texts that must be embedded for it.
fn prepare(db: &Db, cfg: &Config, clock: Clock, mut inp: RememberInput) -> Result<Prepared> {
    let content = inp.content.trim().to_string();
    if content.is_empty() {
        return Err(crate::err!("content must not be empty"));
    }
    // Opt-in rules extractor for memories that carry no structure; dates in
    // the text are handled by the text-date inference below.
    if std::env::var("NEOCORTEX_EXTRACTOR").as_deref() == Ok("rules")
        && inp.entities.is_empty()
        && inp.relations.is_empty()
    {
        let extracted = crate::extract_model::extract_text(&content, &clock);
        inp.entities = extracted.entities;
        inp.relations = extracted.relations;
    }
    // A blank fact would embed as a zero vector, whose cosine distance is NULL.
    for r in &mut inp.relations {
        r.fact = r.fact.take().filter(|f| !f.trim().is_empty());
    }
    let kind = inp.kind.unwrap_or_else(|| "note".into());
    if !crate::engine::KINDS.contains(&kind.as_str()) {
        return Err(crate::err!(
            "invalid kind '{}'; valid kinds: {}",
            kind,
            crate::engine::KINDS.join(", ")
        ));
    }
    // Future tense and recurrence are properties of the sentence, not of the
    // clock: a plan gets a future start (or a `planned` tag), and a rule keeps
    // its wording with a `recurring` tag.
    let plan = temporal::plan_intent(&content, &clock);
    let recurring = temporal::recurrence(&content);
    let future_plan = if plan {
        temporal::future_window(&content, &clock)
    } else {
        None
    };
    // Without --when, a date written in the content is the start (one only).
    let (valid_from, span_end, when_inferred) = match inp.valid_from.as_deref() {
        Some(s) => match temporal::parse_span(s, &clock) {
            Some((start, end)) => (start, Some(end), None),
            None => {
                let start = temporal::parse_instant(s, &clock).ok_or_else(|| {
                    crate::err!(
                        "invalid valid_from '{s}'; use an ISO date (2025-03-05), a month (March 2025), \
                         a year, unix seconds, or a phrase like yesterday, last week, last March, \
                         3 days ago, 2 weeks ago, next Monday, or a span 'from X to Y'"
                    )
                })?;
                (start, None, None)
            }
        },
        None if plan => match future_plan {
            Some(w) => (w.start, None, Some(w.matched)),
            // A plan with no resolvable date is not a past event either.
            None => (clock.now, None, None),
        },
        // A future date in a decision/note is the subject's date, not when the
        // memory became true; only events are dated in the future.
        None => match temporal::text_date(&content, &clock) {
            Some(w) if w.start <= clock.now || kind == "event" => (w.start, None, Some(w.matched)),
            _ => (clock.now, None, None),
        },
    };
    // A plan stated in the text, or anything dated after now, has not happened
    // yet: recall keeps it out of current answers until its date arrives.
    let planned = (plan && inp.valid_from.is_none()) || valid_from > clock.now;
    let valid_to = inp
        .valid_to
        .as_deref()
        .map(|s| {
            temporal::parse_instant(s, &clock).ok_or_else(|| {
                crate::err!(
                    "invalid valid_to '{}'; use ISO date, unix seconds, or 'tomorrow'",
                    s
                )
            })
        })
        .transpose()?;
    if span_end.is_some() && valid_to.is_some() {
        return Err(crate::err!(
            "give a span in --when or an end in --until, not both"
        ));
    }
    let valid_to = valid_to.or(span_end);
    if valid_to.is_some_and(|v| v <= valid_from) {
        return Err(crate::err!("valid_to must be after valid_from"));
    }
    let importance = inp
        .importance
        .filter(|x| !x.is_nan())
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    let mut tags: Vec<String> = inp
        .tags
        .iter()
        .chain(inp.keywords.iter())
        .map(|x| text::fold(x))
        .filter(|x| !x.is_empty())
        .collect();
    if planned {
        tags.push("planned".into());
    }
    if recurring {
        tags.push("recurring".into());
    }
    tags.sort();
    tags.dedup();
    let tags = tags.join(" ");
    let mut entities = inp.entities.clone();
    if entities.is_empty() {
        let known = |name: &str| resolve::alias_known(db, name).unwrap_or(false);
        entities = extract::heuristic_entities_with(&content, &known)
            .into_iter()
            .map(|c| EntityIn {
                name: c.name,
                kind: None,
                description: None,
                aliases: Vec::new(),
                same_as: None,
                distinct_from: Vec::new(),
            })
            .collect();
    }
    // E4b: conservative text cue ("A because B") when the agent supplied no
    // facts. Both clause sides must name exactly one known entity.
    if cfg.causal_extract && inp.relations.is_empty() {
        let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
        if let Some((cause, effect)) = crate::extract_model::causal_cue(&content, &names) {
            inp.relations.push(RelationIn {
                source: cause.clone(),
                predicate: "causes".into(),
                target: effect.clone(),
                fact: Some(format!("{cause} causes {effect}")),
                exclusive: None,
                negated: None,
                valid_from: None,
                valid_to: None,
            });
        }
    }
    let cards: Vec<String> = entities
        .iter()
        .map(|e| {
            format!(
                "{} ({}): {}",
                e.name,
                e.kind.as_deref().unwrap_or(""),
                e.description.as_deref().unwrap_or("")
            )
        })
        .collect();
    let facts: Vec<String> = inp
        .relations
        .iter()
        .map(|r| {
            r.fact
                .clone()
                .unwrap_or_else(|| format!("{} {} {}", r.source, r.predicate, r.target))
        })
        .collect();
    // Every relation endpoint that is not one of this memory's entities gets
    // its own bare-name vector. Indices are explicit so no vector is ever
    // reused for another text.
    let entity_norms: HashSet<String> = entities.iter().map(|e| text::norm_name(&e.name)).collect();
    let mut extra_names: Vec<String> = Vec::new();
    let mut extra_index: HashMap<String, usize> = HashMap::new();
    for relation in &inp.relations {
        for name in [&relation.source, &relation.target] {
            let norm = text::norm_name(name);
            if !entity_norms.contains(&norm) && !extra_index.contains_key(&norm) {
                extra_index.insert(norm, extra_names.len());
                extra_names.push(name.clone());
            }
        }
    }
    let mut pred_index: HashMap<String, usize> = HashMap::new();
    let mut pred_texts: Vec<String> = Vec::new();
    for relation in &inp.relations {
        let norm = text::norm_predicate(&relation.predicate);
        if norm.is_empty() {
            return Err(crate::err!(
                "relation predicate cannot be empty; give a verb phrase such as \"works at\""
            ));
        }
        let next = pred_texts.len();
        if let std::collections::hash_map::Entry::Vacant(slot) = pred_index.entry(norm) {
            slot.insert(next);
            pred_texts.push(relation.predicate.trim().to_string());
        }
    }
    let mut texts: Vec<String> =
        Vec::with_capacity(1 + cards.len() + facts.len() + extra_names.len() + pred_texts.len());
    texts.push(content.clone());
    texts.extend(cards);
    let fact_base = CARD_BASE + entities.len();
    texts.extend(facts.iter().cloned());
    let extra_base = fact_base + facts.len();
    texts.extend(extra_names.iter().cloned());
    let pred_base = extra_base + extra_names.len();
    texts.extend(pred_texts);
    let supersedes: Vec<i64> = inp
        .supersedes
        .iter()
        .map(|s| {
            crate::engine::parse_id(s, 'm')
                .ok_or_else(|| crate::err!("invalid supersedes id '{s}'"))
        })
        .collect::<Result<_>>()?;
    Ok(Prepared {
        hash: text::content_hash(&kind, &content),
        content,
        kind,
        tags,
        importance,
        scope: inp.scope.unwrap_or_default(),
        source: inp.source.unwrap_or_default(),
        session: inp.session.unwrap_or_default(),
        valid_from,
        valid_to,
        when_inferred,
        entities,
        relations: inp.relations,
        facts,
        extra_index,
        supersedes,
        texts,
        vectors: Vec::new(),
        fact_base,
        extra_base,
        pred_base,
        pred_index,
    })
}

/// One embed call for every text of `items` (a chunk), in `Document` mode.
fn embed_prepared(emb: &dyn Embedder, items: &mut [Prepared]) -> Result<()> {
    let mut texts: Vec<&str> = Vec::new();
    for item in items.iter() {
        texts.extend(item.texts.iter().map(String::as_str));
    }
    let vectors = emb.embed(&texts, Mode::Document)?;
    if vectors.len() != texts.len() {
        return Err(crate::err!(
            "embedder returned {} vectors for {} texts",
            vectors.len(),
            texts.len()
        ));
    }
    let mut vectors = vectors.into_iter();
    for item in items.iter_mut() {
        item.vectors = item.texts.iter().map(|_| vectors.next().unwrap()).collect();
    }
    Ok(())
}

/// Write one prepared memory. The caller owns the transaction (the single path
/// opens one, the batch path wraps each item in a SAVEPOINT), so a failure
/// here can be rolled back to that savepoint without touching its neighbours.
fn write_prepared(
    db: &Db,
    cfg: &Config,
    clock: Clock,
    embedder: &str,
    p: Prepared,
) -> Result<RememberOutput> {
    // Only a semantic embedder can judge description conflicts; the hash
    // embedder (tests) reports everything as unrelated.
    let semantic = embedder != "hash";
    let Prepared {
        content,
        kind,
        tags,
        importance,
        scope,
        source,
        session,
        valid_from,
        valid_to,
        when_inferred,
        entities,
        relations,
        facts,
        extra_index,
        hash,
        supersedes,
        texts: _,
        vectors,
        fact_base,
        extra_base,
        pred_base,
        pred_index,
    } = p;
    // Every entity name this write supplies, entity cards and relation
    // endpoints alike: the neighbour evidence a near-name merge needs (ER-08).
    let mut write_names: Vec<String> = Vec::new();
    for name in entities
        .iter()
        .map(|e| &e.name)
        .chain(relations.iter().flat_map(|r| [&r.source, &r.target]))
    {
        let norm = text::norm_name(name);
        if !norm.is_empty() && !write_names.contains(&norm) {
            write_names.push(norm);
        }
    }
    crate::wtiming::begin_item();
    let endpoint_vector = |name: &str| -> &[f32] {
        let norm = text::norm_name(name);
        if let Some(pos) = entities
            .iter()
            .position(|e| text::norm_name(&e.name) == norm)
        {
            &vectors[CARD_BASE + pos]
        } else {
            let index = extra_index[&norm];
            &vectors[extra_base + index]
        }
    };
    // Both checks read the database, so they belong after the earlier items of
    // this chunk were written: a later input must see them. They run after
    // the mentions are written, because the candidate set is entity-driven.
    for id in &supersedes {
        let exists: bool = db.sql().query_row(
            "SELECT EXISTS(SELECT 1 FROM memories WHERE id=?1 AND expired_at IS NULL)",
            [id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(crate::err!(
                "supersedes target m{} is unknown or already expired",
                id
            ));
        }
    }
    let now = clock.now;
    // The duplicate decision is made under the write lock: a read outside
    // it races with concurrent writers (the unique index is the backstop).
    let mut duplicate_of = live_duplicate(db, &hash, &kind, &content)?;
    let stored_hash = if duplicate_of.is_some() {
        hash.clone()
    } else {
        // A live row with this hash but different text is a hash collision;
        // salt the key so both memories can be stored.
        free_hash(db, &hash)?
    };
    db.sql().execute(
        "INSERT OR IGNORE INTO meta(key,value) VALUES('embedder',?1)",
        [embedder],
    )?;
    let id = if let Some(id) = duplicate_of {
        touch_memory(db, id, now)?;
        id
    } else {
        crate::wtiming::begin(crate::wtiming::MEM_INSERT);
        let inserted = db.sql().execute("INSERT INTO memories(content,kind,tags,importance,scope,source,session,created_at,valid_from,valid_to,content_hash) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![content,kind,tags,importance,scope,source,session,now,valid_from,valid_to,stored_hash]);
        crate::wtiming::end(crate::wtiming::MEM_INSERT);
        match inserted {
            Ok(_) => {
                let id = db.sql().last_insert_rowid();
                crate::wtiming::begin(crate::wtiming::VEC_INSERT);
                db.sql().execute(
                    &format!(
                        "INSERT INTO memories_vec(rowid,embedding,live) VALUES (?1,{},1)",
                        db.vec_sql("?2")
                    ),
                    params![id, db.vec_blob(&vectors[0])],
                )?;
                crate::wtiming::end(crate::wtiming::VEC_INSERT);
                crate::wtiming::begin(crate::wtiming::GRAPH);
                kg::project_memory(db, id, &kind, &content)?;
                crate::wtiming::end(crate::wtiming::GRAPH);
                id
            }
            Err(e) if crate::db::is_unique_violation(&e) => {
                // An identical writer won the race; reuse its row.
                let id: i64 = db.sql().query_row(
                    "SELECT id FROM memories WHERE content_hash=?1 AND expired_at IS NULL",
                    [&stored_hash],
                    |r| r.get(0),
                )?;
                touch_memory(db, id, now)?;
                duplicate_of = Some(id);
                id
            }
            Err(e) => return Err(e.into()),
        }
    };
    let mut resolutions = Vec::new();
    let mut ids: HashMap<String, i64> = HashMap::new();
    for (i, entity) in entities.iter().enumerate() {
        crate::wtiming::begin(crate::wtiming::ENTITIES);
        let resolved = resolve::resolve_entity(
            db,
            entity,
            &vectors[CARD_BASE + i],
            &vectors[0],
            now,
            cfg,
            semantic,
            &write_names,
        )?;
        crate::wtiming::end(crate::wtiming::ENTITIES);
        let eid = resolved.id;
        ids.insert(text::norm_name(&entity.name), eid);
        crate::wtiming::begin(crate::wtiming::GRAPH);
        kg::project_entity_from_db(db, eid)?;
        kg::project_name_parts(db, eid)?;
        crate::wtiming::end(crate::wtiming::GRAPH);
        add_mention(db, id, eid, now)?;
        for (other, _name, score) in &resolved.similar {
            write_similar(db, eid, *other, *score)?;
        }
        resolutions.push(EntityResolution {
            name: entity.name.clone(),
            id: eid,
            status: resolved.status,
            similar: resolved.similar,
            candidates: resolved.candidates,
        });
    }
    // Gazetteer: link known aliases that appear as whole-word n-grams in
    // the content (never agent-supplied entities, which are already in).
    let mut seen: HashSet<String> = HashSet::new();
    crate::wtiming::begin(crate::wtiming::GAZETTEER);
    for (norm, _, _) in extract::ngrams(&content, 4) {
        if norm.chars().count() < 3 || !seen.insert(norm.clone()) {
            continue;
        }
        let mut st = db
            .sql()
            .prepare("SELECT DISTINCT entity_id FROM entity_aliases WHERE alias_norm=?1")?;
        let matches: Vec<i64> = st
            .query_map([&norm], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<_>>()?;
        if matches.len() == 1 {
            let eid = matches[0];
            kg::project_entity_from_db(db, eid)?;
            add_mention(db, id, eid, now)?;
        }
    }
    crate::wtiming::end(crate::wtiming::GAZETTEER);
    // Near-duplicate and related checks share one bounded candidate set: FTS
    // hits on this text, the newest memories per mentioned entity and the
    // newest memories overall. The vec0 KNN they replace compared every
    // stored vector, so the write cost followed the corpus.
    crate::wtiming::begin(crate::wtiming::KNN);
    // Sorted: a hash map's order is per-process, and equal cosines keep the
    // candidate order in the sort.
    let mut entity_ids: Vec<i64> = ids.values().copied().collect();
    entity_ids.sort_unstable();
    let candidates = neighbour_candidates(db, &content, &entity_ids)?;
    let neighbours = knn_among(
        db,
        &vectors[0],
        &candidates,
        if relations.is_empty() { 10 } else { 1 },
        id,
    )?;
    crate::wtiming::end(crate::wtiming::KNN);
    let near = neighbours
        .first()
        .copied()
        .filter(|(_, s)| *s >= cfg.near_dup);
    // E4a: memories that share an entity and sit close in world time are
    // neighbours ("what else happened around the move"). The fan-out cap is
    // enforced on both endpoints, so no memory can become a hub.
    if duplicate_of.is_none() {
        crate::wtiming::begin(crate::wtiming::TLINKS);
        write_temporal_links(db, id, valid_from, cfg)?;
        crate::wtiming::end(crate::wtiming::TLINKS);
    }
    let mut rel_results = Vec::new();
    let mut invalidated = Vec::new();
    let mut conflicts = Vec::new();
    let mut related = Vec::new();
    if relations.is_empty() && !entity_ids.is_empty() {
        for &(mid, cosine) in neighbours.iter() {
            if cosine < cfg.update_candidate || cosine >= cfg.near_dup || mid == id {
                continue;
            }
            let shared: bool = db.sql().query_row(
                "SELECT EXISTS(SELECT 1 FROM mentions WHERE memory_id=?1 AND entity_id IN (SELECT value FROM json_each(?2)))",
                rusqlite::params![mid, serde_json::to_string(&entity_ids)?],
                |r| r.get(0),
            )?;
            if shared {
                let snippet: String =
                    db.sql()
                        .query_row("SELECT content FROM memories WHERE id=?1", [mid], |r| {
                            r.get(0)
                        })?;
                related.push(crate::engine::RelatedMemory {
                    memory_id: mid,
                    cosine,
                    snippet,
                });
                if related.len() == 3 {
                    break;
                }
            }
        }
    }
    for (ri, relation) in relations.iter().enumerate() {
        crate::wtiming::begin(crate::wtiming::RELATIONS);
        let pred_norm = text::norm_predicate(&relation.predicate);
        let phrase_vector = &vectors[pred_base + pred_index[&pred_norm]];
        let resolved = resolve::resolve_predicate(
            db,
            &relation.predicate,
            phrase_vector,
            relation.exclusive,
            cfg,
        )?;
        // Inverse writes are stored in the canonical direction (lower id), so
        // "B reports to A" lives as "A manages B" and is rendered both ways.
        let mut pred = resolved.clone();
        let swap = match resolved.inverse {
            Some(inverse) if inverse < resolved.id => {
                pred = resolve::predicate_row(db, inverse)?;
                true
            }
            _ => false,
        };
        let (src_name, dst_name) = if swap {
            (&relation.target, &relation.source)
        } else {
            (&relation.source, &relation.target)
        };
        let mut src = resolve_endpoint(
            db,
            id,
            src_name,
            &mut ids,
            now,
            cfg,
            endpoint_vector(src_name),
            semantic,
            &write_names,
        )?;
        let mut dst = resolve_endpoint(
            db,
            id,
            dst_name,
            &mut ids,
            now,
            cfg,
            endpoint_vector(dst_name),
            semantic,
            &write_names,
        )?;
        if pred.symmetric && src > dst {
            std::mem::swap(&mut src, &mut dst);
        }
        // Exclusivity is per semantic subject: src normally, the original
        // source for an inverse write (stored as dst), min(src,dst) when
        // symmetric.
        let subj = if swap { dst } else { src };
        let group_id = pred.group_id;
        let predicate_text = if swap {
            pred.display.clone()
        } else {
            relation.predicate.trim().to_string()
        };
        let rf = relation
            .valid_from
            .as_deref()
            .map(|s| {
                temporal::parse_instant(s, &clock)
                    .ok_or_else(|| crate::err!("invalid relation valid_from '{s}'"))
            })
            .transpose()?
            .unwrap_or(valid_from);
        let mut new_valid_to = relation
            .valid_to
            .as_deref()
            .map(|x| {
                temporal::parse_instant(x, &clock)
                    .ok_or_else(|| crate::err!("invalid relation valid_to '{x}'"))
            })
            .transpose()?;
        let mut fact = facts[ri].clone();
        let polarity = if relation.negated.unwrap_or_else(|| {
            infer_negated(
                relation.fact.as_deref().unwrap_or(&content),
                &resolved.canonical,
                &resolved.group,
                &relation.target,
                &relation.source,
            )
        }) {
            0
        } else {
            1
        };
        if relation.fact.is_none() && (swap || polarity == 0) {
            fact = format!(
                "{}{} {} {}",
                src_name,
                if polarity == 0 { " NOT" } else { "" },
                pred.display,
                dst_name
            );
        }
        // Relations this one ends; each gets ended_by = this relation.
        let mut ended_here: Vec<i64> = Vec::new();
        // The later relation this backfilled one is history up to.
        let mut ended_by_later: Option<i64> = None;
        if new_valid_to.is_some_and(|to| to <= rf) {
            return Err(crate::err!(
                "relation '{fact}': valid_to must be after valid_from"
            ));
        }
        if polarity == 1
            && relation.exclusive != Some(false)
            && (relation.exclusive == Some(true) || resolved.exclusive)
        {
            let mut st = db.sql().prepare(
                "SELECT r.id,r.valid_from,r.valid_to,r.src,r.dst,es.norm,ed.norm,r.fact
                 FROM relations r
                 JOIN predicates p ON p.id=r.predicate_id
                 JOIN entities es ON es.id=r.src
                 JOIN entities ed ON ed.id=r.dst
                 WHERE r.subj=?1 AND COALESCE(NULLIF(p.group_id,0),p.id)=?2
                   AND r.polarity=1 AND r.expired_at IS NULL",
            )?;
            /// One live exclusive relation: (id, valid_from, valid_to, src,
            /// dst, src norm, dst norm, fact).
            type OldRel = (i64, i64, Option<i64>, i64, i64, String, String, String);
            let olds: Vec<OldRel> = st
                .query_map(params![subj, group_id], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                })?
                .collect::<rusqlite::Result<_>>()?;
            // The exclusive value is the endpoint that is not the subject; an
            // inverted predicate (e.g. "reports to", stored as its inverse
            // "manages") keeps it on the src side. REL-19: a near-duplicate
            // new value ("Bergn") never ends the correct one; near targets
            // that hit a blocker or sit over the typo-edit allowance are
            // reported as conflict candidates instead.
            let value_new = if swap { src } else { dst };
            let value_norm = text::norm_name(if swap { src_name } else { dst_name });
            let value_vector = if swap {
                endpoint_vector(src_name)
            } else {
                endpoint_vector(dst_name)
            };
            let mut later: Option<(i64, i64)> = None;
            for (old, old_from, old_to, o_src, o_dst, s_norm, d_norm, old_fact) in olds {
                let (value_old, value_old_norm) = if o_dst == subj {
                    (o_src, s_norm)
                } else {
                    (o_dst, d_norm)
                };
                if value_old == value_new {
                    // Re-assertion of the same value: the evidence path dedupes it.
                    continue;
                }
                let judge = resolve::NearJudge {
                    embedding: value_vector,
                    context: &vectors[0],
                    cfg,
                    semantic,
                };
                let verdict =
                    resolve::near_name(db, &value_norm, &value_old_norm, value_old, &judge);
                if !matches!(verdict, resolve::Verdict::Below) {
                    if !matches!(verdict, resolve::Verdict::Merge) {
                        conflicts.push(crate::engine::Conflict {
                            relation_id: old,
                            fact: old_fact,
                            valid_from: old_from,
                        });
                    }
                    continue;
                }
                if old_from == rf {
                    // Equal start: the newer write wins.
                    db.sql().execute(
                        "UPDATE relations SET valid_to=?1, expired_at=?2 WHERE id=?3",
                        params![rf, now, old],
                    )?;
                    ended_here.push(old);
                } else if old_from < rf && old_to.is_none_or(|to| to > rf) {
                    // Older and still true at the new start: it ends there.
                    db.sql().execute(
                        "UPDATE relations SET valid_to=?1 WHERE id=?2",
                        params![rf, old],
                    )?;
                    ended_here.push(old);
                } else if old_from > rf {
                    // Backfill: the new fact is history up to the later one.
                    if later.is_none_or(|(from, _)| old_from < from) {
                        later = Some((old_from, old));
                    }
                }
            }
            if let Some((later, by)) = later {
                if new_valid_to.is_none_or(|to| to > later) {
                    new_valid_to = Some(later);
                    ended_by_later = Some(by);
                }
            }
        }
        if relation.exclusive.is_none() && !resolved.exclusive && polarity == 1 {
            let mut st = db.sql().prepare(
                "SELECT r.id,r.fact,r.valid_from FROM relations r
                 JOIN predicates p ON p.id=r.predicate_id
                 WHERE r.subj=?1 AND COALESCE(NULLIF(p.group_id,0),p.id)=?2
                   AND r.dst!=?3 AND r.polarity=1 AND r.expired_at IS NULL
                   AND (r.valid_to IS NULL OR r.valid_to>?4) LIMIT 5",
            )?;
            for row in st.query_map(params![subj, group_id, dst, rf], |r| {
                Ok(crate::engine::Conflict {
                    relation_id: r.get(0)?,
                    fact: r.get(1)?,
                    valid_from: r.get(2)?,
                })
            })? {
                conflicts.push(row?);
            }
        }
        let existing: Option<(i64, i64, Option<i64>)> = db
            .sql()
            .query_row(
                "SELECT r.id,r.evidence_count,r.valid_to FROM relations r
                 JOIN predicates p ON p.id=r.predicate_id
                 WHERE r.src=?1 AND r.dst=?2
                   AND COALESCE(NULLIF(p.group_id,0),p.id)=?3 AND r.polarity=?4
                   AND r.expired_at IS NULL
                   AND (r.valid_to IS NULL OR r.valid_to>=?5)
                   AND r.valid_from<=COALESCE(?6,r.valid_from) LIMIT 1",
                params![src, dst, group_id, polarity, rf, new_valid_to],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let rid = if let Some((rid, _count, _old_to)) = existing {
            db.sql().execute(
                "UPDATE relations SET evidence_count=evidence_count+1,
                     weight=evidence_count+1, valid_from=MIN(valid_from,?1),
                     valid_to=CASE WHEN valid_to IS NULL THEN NULL
                                   WHEN ?2 IS NULL THEN CASE WHEN ?1>=valid_to THEN NULL ELSE valid_to END
                                   ELSE MAX(valid_to,?2) END
                     WHERE id=?3",
                params![rf, new_valid_to, rid],
            )?;
            db.sql().execute(
                "INSERT OR IGNORE INTO relation_evidence(relation_id,memory_id) VALUES(?1,?2)",
                params![rid, id],
            )?;
            rid
        } else {
            db.sql().execute(
                "INSERT INTO relations(src,dst,predicate_id,predicate_text,subj,fact,weight,evidence_count,valid_from,valid_to,created_at,polarity)
                     VALUES (?1,?2,?3,?4,?5,?6,1,1,?7,?8,?9,?10)",
                params![src, dst, pred.id, predicate_text, subj, fact, rf, new_valid_to, now, polarity],
            )?;
            let rid = db.sql().last_insert_rowid();
            db.sql().execute(
                "INSERT INTO relation_evidence(relation_id,memory_id) VALUES(?1,?2)",
                params![rid, id],
            )?;
            db.sql().execute(
                "INSERT INTO relations_fts(rowid,fact,predicate) VALUES (?1,?2,?3)",
                params![rid, fact, predicate_text],
            )?;
            db.sql().execute(
                &format!(
                    "INSERT INTO relations_vec(rowid,embedding) VALUES (?1,{})",
                    db.vec_sql("?2")
                ),
                params![rid, db.vec_blob(&vectors[fact_base + ri])],
            )?;
            // Polarity flip: the same stored triple asserted with the opposite
            // polarity ends the older live assertion (both stay history). An
            // equal start is a same-day correction: the older row is retired.
            let opposite: Vec<(i64, i64)> = {
                let mut st = db.sql().prepare(
                    "SELECT r.id,r.valid_from FROM relations r
                     JOIN predicates p ON p.id=r.predicate_id
                     WHERE r.src=?1 AND r.dst=?2
                       AND COALESCE(NULLIF(p.group_id,0),p.id)=?3
                       AND r.polarity=?4 AND r.expired_at IS NULL
                       AND r.valid_from<=?5 AND (r.valid_to IS NULL OR r.valid_to>?5)",
                )?;
                let rows = st.query_map(params![src, dst, group_id, 1 - polarity, rf], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            for (old, old_from) in opposite {
                if old_from == rf {
                    db.sql().execute(
                        "UPDATE relations SET valid_to=?1, expired_at=?2 WHERE id=?3",
                        params![rf, now, old],
                    )?;
                } else {
                    db.sql().execute(
                        "UPDATE relations SET valid_to=?1 WHERE id=?2",
                        params![rf, old],
                    )?;
                }
                ended_here.push(old);
            }
            rid
        };
        for old in &ended_here {
            db.sql().execute(
                "UPDATE relations SET ended_by=?1 WHERE id=?2",
                params![rid, old],
            )?;
        }
        if let Some(by) = ended_by_later {
            db.sql().execute(
                "UPDATE relations SET ended_by=?1 WHERE id=?2 AND valid_to IS NOT NULL",
                params![by, rid],
            )?;
        }
        invalidated.extend(ended_here);
        crate::wtiming::begin(crate::wtiming::GRAPH);
        kg::project_relation(db, rid, src, dst, &predicate_text)?;
        crate::wtiming::end(crate::wtiming::GRAPH);
        rel_results.push(RelationResult {
            id: rid,
            fact,
            status: if existing.is_some() {
                "merged".into()
            } else {
                "new".into()
            },
        });
        crate::wtiming::end(crate::wtiming::RELATIONS);
    }
    // Text can still retract a different fact ("Alice no longer leads the
    // team") when the agent supplied relations of its own: end the one live
    // relation the sentence names, or report candidates when the pair is
    // ambiguous. The relations this memory just evidenced are its own
    // assertion and are never retracted by its prose.
    if duplicate_of.is_none() {
        crate::wtiming::begin(crate::wtiming::RETRACTIONS);
        let own: Vec<i64> = rel_results.iter().map(|r: &RelationResult| r.id).collect();
        let mut sink = RetractionSink {
            exclude: &own,
            invalidated: &mut invalidated,
            conflicts: &mut conflicts,
        };
        end_text_retractions(db, &content, &entities, valid_from, now, &mut sink)?;
        crate::wtiming::end(crate::wtiming::RETRACTIONS);
    }
    // A memory with an end (--until) ends the relations only it keeps current.
    if let (None, Some(to)) = (duplicate_of, valid_to) {
        end_relations_losing_evidence(db, id, to)?;
    }
    for target in &supersedes {
        // The superseded memory's relations end where the new memory starts.
        end_relations_losing_evidence(db, *target, valid_from)?;
        db.sql().execute(
            "UPDATE memories SET valid_to=COALESCE(valid_to,?1),expired_at=?2,superseded_by=?3 WHERE id=?4",
            params![valid_from, now, id, target],
        )?;
        db.sql()
            .execute("UPDATE memories_vec SET live=0 WHERE rowid=?1", [target])?;
    }
    let touched: Vec<i64> = invalidated
        .iter()
        .copied()
        .chain(rel_results.iter().map(|r| r.id))
        .collect();
    crate::wtiming::begin(crate::wtiming::FINALIZE);
    end_memories_of_ended_facts(db, &touched)?;
    kg::bump_gen(db)?;
    crate::wtiming::end(crate::wtiming::FINALIZE);
    let mut hints = if content.len() > 2000 {
        vec!["long content: consider splitting into atomic memories".into()]
    } else {
        Vec::new()
    };
    if !conflicts.is_empty() {
        let c = &conflicts[0];
        hints.push(format!("r{} ({}) is also current; if the new fact replaces it call invalidate r{}, or re-send with exclusive=true", c.relation_id, c.fact, c.relation_id));
    }
    if !related.is_empty() {
        hints.push(format!(
            "if this updates m{}, pass supersedes=[m{}]",
            related[0].memory_id, related[0].memory_id
        ));
    }
    // Text alone yields no graph facts (no CPU-sized extractor is reliable
    // here, see docs/research/extraction.md); the calling agent is the extractor.
    if duplicate_of.is_none() && rel_results.is_empty() && resolutions.len() >= 2 {
        hints.push(
            "no facts stored: pass relations [{source, predicate, target}] so this memory joins the knowledge graph"
                .into(),
        );
    }
    crate::wtiming::end_item(id);
    Ok(RememberOutput {
        id,
        duplicate_of,
        near_duplicate: if duplicate_of.is_some() { None } else { near },
        entities: resolutions,
        relations: rel_results,
        invalidated_relations: invalidated,
        superseded_memories: supersedes,
        conflicts,
        related,
        hints,
        when_inferred,
    })
}

fn add_mention(db: &Db, memory: i64, entity: i64, now: i64) -> Result<bool> {
    crate::wtiming::begin(crate::wtiming::MENTIONS);
    let inserted = db.sql().execute(
        "INSERT OR IGNORE INTO mentions(memory_id,entity_id) VALUES (?1,?2)",
        params![memory, entity],
    )? > 0;
    if inserted {
        db.sql().execute(
            "UPDATE entities SET mention_count=mention_count+1, updated_at=?1 WHERE id=?2",
            params![now, entity],
        )?;
        crate::wtiming::end(crate::wtiming::MENTIONS);
        crate::wtiming::begin(crate::wtiming::GRAPH);
        kg::project_mention(db, memory, entity)?;
        crate::wtiming::end(crate::wtiming::GRAPH);
    } else {
        crate::wtiming::end(crate::wtiming::MENTIONS);
    }
    Ok(inserted)
}

fn write_similar(db: &Db, entity: i64, other: i64, score: f32) -> Result<()> {
    if entity == other {
        return Ok(());
    }
    let (a, b) = if entity < other {
        (entity, other)
    } else {
        (other, entity)
    };
    db.sql().execute(
        "INSERT OR REPLACE INTO similar(a,b,score) VALUES (?1,?2,?3)",
        params![a, b, score],
    )?;
    kg::project_similar(db, a, b, score)
}

/// Largest temporal-link weight gap the brief allows: a pair further apart
/// than a day still gets the floor weight only if it is inside the window.
const TLINK_DAY: f64 = 86_400.0;
const TLINK_FLOOR: f64 = 0.3;

/// E4a: link a new memory to the closest live memories that share one of its
/// entities. Weight is `max(0.3, 1 - gap/24h)` (Hindsight); the window scan
/// is driven by `idx_memories_valid` and each candidate probes the
/// `mentions(memory_id, entity_id)` primary key, so the write cost tracks the
/// window, not the corpus.
fn write_temporal_links(db: &Db, memory: i64, valid_from: i64, cfg: &Config) -> Result<()> {
    let cap = cfg.tlink_cap;
    let window = (cfg.tlink_window_hours.max(0.0) * 3_600.0).round() as i64;
    if cap == 0 || window <= 0 {
        return Ok(());
    }
    let entity_ids: Vec<i64> = {
        let mut st = db
            .sql()
            .prepare("SELECT entity_id FROM mentions WHERE memory_id=?1")?;
        let mut ids = Vec::new();
        for row in st.query_map([memory], |r| r.get(0))? {
            ids.push(row?);
        }
        ids
    };
    if entity_ids.is_empty() {
        return Ok(());
    }
    let marks = (1..=entity_ids.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT m.id,m.valid_from FROM memories m
         WHERE m.valid_from BETWEEN ?{lo} AND ?{hi}
           AND m.id!=?{a} AND m.expired_at IS NULL
           AND EXISTS (SELECT 1 FROM mentions me
                       WHERE me.memory_id=m.id AND me.entity_id IN ({marks}))
         ORDER BY ABS(m.valid_from-?{at}), m.id
         LIMIT ?{limit}",
        a = entity_ids.len() + 1,
        lo = entity_ids.len() + 2,
        hi = entity_ids.len() + 3,
        at = entity_ids.len() + 4,
        limit = entity_ids.len() + 5,
    );
    let mut st = db.sql().prepare(&sql)?;
    let mut p: Vec<Box<dyn rusqlite::ToSql>> = entity_ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::ToSql>)
        .collect();
    for value in [
        memory,
        valid_from - window,
        valid_from + window,
        valid_from,
        cap as i64,
    ] {
        p.push(Box::new(value));
    }
    let candidates: Vec<(i64, i64)> = st
        .query_map(
            rusqlite::params_from_iter(p.iter().map(|x| x.as_ref())),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
        .collect::<rusqlite::Result<_>>()?;
    for (other, other_from) in candidates {
        let gap = (valid_from - other_from).abs() as f64;
        let weight = (1.0 - gap / TLINK_DAY).max(TLINK_FLOOR);
        let (a, b) = if memory < other {
            (memory, other)
        } else {
            (other, memory)
        };
        db.sql().execute(
            "INSERT INTO memory_links(a,b,weight) VALUES (?1,?2,?3)
             ON CONFLICT(a,b) DO UPDATE SET weight=MAX(memory_links.weight,excluded.weight)",
            params![a, b, weight],
        )?;
        trim_temporal_links(db, memory, cap)?;
        trim_temporal_links(db, other, cap)?;
    }
    Ok(())
}

/// Keep the `cap` strongest links incident to one memory.
fn trim_temporal_links(db: &Db, memory: i64, cap: usize) -> Result<()> {
    db.sql().execute(
        "DELETE FROM memory_links WHERE rowid IN (
           SELECT rowid FROM memory_links WHERE a=?1 OR b=?1
           ORDER BY weight DESC, a, b LIMIT -1 OFFSET ?2)",
        params![memory, cap as i64],
    )?;
    Ok(())
}

/// A memory whose every evidenced relation has ended describes a past state:
/// give it valid_to = the latest relation end (if still open), so recall treats
/// it as history ("lives in Austin" once "lives in Denver" was written).
pub fn end_memories_of_ended_facts(db: &Db, relation_ids: &[i64]) -> Result<()> {
    for rid in relation_ids {
        db.sql().execute(
            "UPDATE memories SET valid_to = (
                 SELECT MAX(r.valid_to) FROM relation_evidence e JOIN relations r ON r.id = e.relation_id
                 WHERE e.memory_id = memories.id)
             WHERE valid_to IS NULL
               AND id IN (SELECT memory_id FROM relation_evidence WHERE relation_id = ?1)
               AND NOT EXISTS (
                 SELECT 1 FROM relation_evidence e JOIN relations r ON r.id = e.relation_id
                 WHERE e.memory_id = memories.id AND r.valid_to IS NULL)
               AND (SELECT MAX(r.valid_to) FROM relation_evidence e JOIN relations r ON r.id = e.relation_id
                    WHERE e.memory_id = memories.id) >= valid_from",
            [rid],
        )?;
    }
    Ok(())
}

/// Resolve a relation endpoint and record that `memory` mentions it.
#[allow(clippy::too_many_arguments)]
fn resolve_endpoint(
    db: &Db,
    memory: i64,
    name: &str,
    ids: &mut HashMap<String, i64>,
    now: i64,
    cfg: &Config,
    embedding: &[f32],
    semantic: bool,
    write_names: &[String],
) -> Result<i64> {
    let norm = text::norm_name(name);
    if let Some(id) = ids.get(&norm) {
        add_mention(db, memory, *id, now)?;
        return Ok(*id);
    }
    let resolved = resolve::resolve_entity(
        db,
        &EntityIn {
            name: name.into(),
            ..Default::default()
        },
        embedding,
        embedding,
        now,
        cfg,
        semantic,
        write_names,
    )?;
    ids.insert(norm, resolved.id);
    kg::project_entity_from_db(db, resolved.id)?;
    kg::project_name_parts(db, resolved.id)?;
    add_mention(db, memory, resolved.id, now)?;
    for (other, _name, score) in &resolved.similar {
        write_similar(db, resolved.id, *other, *score)?;
    }
    Ok(resolved.id)
}

/// The live memory whose kind and folded text equal `kind` + `content`, if any.
/// A hash hit with different text is a collision, not a duplicate.
fn live_duplicate(db: &Db, hash: &str, kind: &str, content: &str) -> Result<Option<i64>> {
    let row: Option<(i64, String, String)> = db
        .sql()
        .query_row(
            "SELECT id,kind,content FROM memories
             WHERE content_hash=?1 AND expired_at IS NULL LIMIT 1",
            [hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let folded = text::fold(content);
    Ok(row
        .filter(|(_, k, c)| k == kind && text::fold(c) == folded)
        .map(|(id, _, _)| id))
}

/// `hash`, or a salted variant when a live row already owns it with other text.
fn free_hash(db: &Db, hash: &str) -> Result<String> {
    let mut candidate = hash.to_string();
    loop {
        let taken: Option<i64> = db
            .sql()
            .query_row(
                "SELECT 1 FROM memories WHERE content_hash=?1 AND expired_at IS NULL",
                [&candidate],
                |r| r.get(0),
            )
            .optional()?;
        if taken.is_none() {
            return Ok(candidate);
        }
        candidate.push('#');
    }
}

fn touch_memory(db: &Db, id: i64, now: i64) -> Result<()> {
    db.sql().execute(
        "UPDATE memories SET access_count=access_count+1,last_access=?1 WHERE id=?2",
        params![now, id],
    )?;
    Ok(())
}

/// Relations evidenced by `memory` for which no *other live* memory is evidence.
pub fn relations_losing_their_last_evidence(db: &Db, memory: i64) -> Result<Vec<i64>> {
    let mut st = db.sql().prepare(
        "SELECT DISTINCT e.relation_id FROM relation_evidence e
         WHERE e.memory_id=?1
           AND NOT EXISTS (
             SELECT 1 FROM relation_evidence e2 JOIN memories m ON m.id=e2.memory_id
             WHERE e2.relation_id=e.relation_id AND e2.memory_id<>?1
               AND m.expired_at IS NULL)",
    )?;
    let rows = st.query_map([memory], |r| r.get::<_, i64>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Relations that `memory` alone keeps current end at `at` (never later than
/// their current end). Callers pick `at`: the memory's own end, or the start of a
/// memory that supersedes it.
pub fn end_relations_losing_evidence(db: &Db, memory: i64, at: i64) -> Result<()> {
    for rid in relations_losing_their_last_evidence(db, memory)? {
        db.sql().execute(
            "UPDATE relations SET valid_to=MIN(COALESCE(valid_to,?1),?1)
             WHERE id=?2 AND expired_at IS NULL",
            params![at, rid],
        )?;
    }
    Ok(())
}

/// Retract relations (system time): `expired_at = now`, world validity untouched.
pub fn retract_relations(db: &Db, ids: &[i64], now: i64) -> Result<()> {
    for rid in ids {
        db.sql().execute(
            "UPDATE relations SET expired_at=?1 WHERE id=?2 AND expired_at IS NULL",
            params![now, rid],
        )?;
    }
    Ok(())
}

/// Bounds of the write-path neighbour search. The candidate set is lexical
/// and entity overlap plus the newest memories, so a repeat of recent work
/// is found without comparing every stored vector.
const NEIGHBOUR_FTS: usize = 128;
const NEIGHBOUR_PER_ENTITY: usize = 256;
const NEIGHBOUR_RECENT: usize = 512;
const NEIGHBOUR_CAP: usize = 1024;

fn push_candidate(out: &mut Vec<i64>, seen: &mut HashSet<i64>, id: i64) {
    if out.len() < NEIGHBOUR_CAP && seen.insert(id) {
        out.push(id);
    }
}

/// FTS expression for the candidate lookup: the longest exact content tokens
/// (no prefix arms; a near duplicate shares whole words). Ranking by bm25
/// would score every match; newest-first stops after `LIMIT`.
fn candidate_fts_query(content: &str) -> Option<String> {
    let mut toks: Vec<String> = text::tokens(content)
        .into_iter()
        .filter(|t| t.chars().count() >= 4 && !text::is_stopword(t))
        .collect();
    toks.sort_by_key(|t| std::cmp::Reverse(t.chars().count()));
    toks.dedup();
    toks.truncate(8);
    if toks.is_empty() {
        return None;
    }
    Some(
        toks.iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

/// Candidate memories for the near-duplicate and related checks: the newest
/// memories whose text shares words with this one, the newest memories per
/// mentioned entity (the `related` block needs shared entities anyway) and
/// the newest memories overall (agents repeat what they just wrote).
fn neighbour_candidates(db: &Db, content: &str, entity_ids: &[i64]) -> Result<Vec<i64>> {
    let mut out: Vec<i64> = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    if let Some(q) = candidate_fts_query(content) {
        let mut st = db.sql().prepare(
            "SELECT rowid FROM memories_fts WHERE memories_fts MATCH ?1
             ORDER BY rowid DESC LIMIT ?2",
        )?;
        let rows = st.query_map(params![q, NEIGHBOUR_FTS as i64], |r| r.get::<_, i64>(0))?;
        for row in rows {
            push_candidate(&mut out, &mut seen, row?);
        }
    }
    {
        // The index walks backwards, so this is the newest `LIMIT` mentions
        // of the entity, not a scan of all of them.
        let mut st = db.sql().prepare(
            "SELECT memory_id FROM mentions WHERE entity_id=?1
             ORDER BY memory_id DESC LIMIT ?2",
        )?;
        for eid in entity_ids {
            let rows = st.query_map(params![eid, NEIGHBOUR_PER_ENTITY as i64], |r| {
                r.get::<_, i64>(0)
            })?;
            for row in rows {
                push_candidate(&mut out, &mut seen, row?);
            }
        }
    }
    {
        let mut st = db
            .sql()
            .prepare("SELECT id FROM memories ORDER BY rowid DESC LIMIT ?1")?;
        let rows = st.query_map([NEIGHBOUR_RECENT as i64], |r| r.get::<_, i64>(0))?;
        for row in rows {
            push_candidate(&mut out, &mut seen, row?);
        }
    }
    Ok(out)
}

/// Cosine between `v` and the stored vectors of `candidates`, nearest first.
/// Point reads keep the cost tied to the candidate count, not the corpus.
fn knn_among(
    db: &Db,
    v: &[f32],
    candidates: &[i64],
    k: usize,
    skip: i64,
) -> Result<Vec<(i64, f32)>> {
    let mut st = db
        .sql()
        .prepare("SELECT embedding FROM memories_vec WHERE rowid=?1 AND live=1")?;
    let mut scored: Vec<(i64, f32)> = Vec::with_capacity(candidates.len());
    for id in candidates {
        if *id == skip {
            continue;
        }
        match st.query_row([id], |r| r.get::<_, Vec<u8>>(0)) {
            Ok(blob) => {
                if let Some(other) = db.blob_to_vec(&blob) {
                    scored.push((*id, crate::rank::cosine(v, &other)));
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(e.into()),
        }
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(k);
    Ok(scored)
}

#[cfg(test)]
mod negation_tests {
    use super::infer_negated;

    #[test]
    fn negation_cues_are_anchored_on_the_target() {
        // (text, predicate canonical, predicate group, target, source, negated?)
        let cases = [
            (
                "Alice no longer likes jazz.",
                "like",
                "like",
                "jazz",
                "Alice",
                true,
            ),
            (
                "Alice doesn't like jazz",
                "like",
                "like",
                "jazz",
                "Alice",
                true,
            ),
            (
                "Alice does not like jazz",
                "like",
                "like",
                "jazz",
                "Alice",
                true,
            ),
            (
                "Bob never worked at Acme",
                "work at",
                "work at",
                "Acme",
                "Bob",
                true,
            ),
            (
                "Bob stopped working at Acme in May",
                "work at",
                "work at",
                "Acme",
                "Bob",
                true,
            ),
            (
                "Carol isn't living in Pune anymore",
                "live in",
                "live in",
                "Pune",
                "Carol",
                true,
            ),
            ("Alice likes jazz.", "like", "like", "jazz", "Alice", false),
            (
                "Bob works at Acme, not Globex.",
                "work at",
                "work at",
                "Acme",
                "Bob",
                false,
            ),
            (
                "Alice moved to Denver and no longer lives in Austin.",
                "live in",
                "live in",
                "Denver",
                "Alice",
                false,
            ),
            (
                "Alice moved to Denver and no longer lives in Austin.",
                "live in",
                "live in",
                "Austin",
                "Alice",
                true,
            ),
            (
                "Alice not only likes jazz but also blues",
                "like",
                "like",
                "jazz",
                "Alice",
                false,
            ),
            (
                "Alice left for Paris",
                "live in",
                "live in",
                "Paris",
                "Alice",
                false,
            ),
            (
                "I have no doubt Alice likes jazz",
                "like",
                "like",
                "jazz",
                "Alice",
                false,
            ),
            ("Jazz is great", "like", "like", "jazz", "Jazz", false),
            // ER-03 A: the cue belongs to the contrastive subject, not the verb.
            (
                "Alice, not Bob, lives in Denver.",
                "live in",
                "live in",
                "Denver",
                "Alice",
                false,
            ),
            // ER-03 B: the negated clause is the earlier target; Denver stays
            // positive, Austin is retracted.
            (
                "Alice no longer lives in Austin; she moved to Denver.",
                "live in",
                "live in",
                "Denver",
                "Alice",
                false,
            ),
            (
                "Alice no longer lives in Austin; she moved to Denver.",
                "live in",
                "live in",
                "Austin",
                "Alice",
                true,
            ),
            // ER-07: retraction phrasings, matched per predicate group.
            (
                "Bob resigned from Acme in June.",
                "work at",
                "work at",
                "Acme",
                "Bob",
                true,
            ),
            (
                "Bob is no longer at Acme.",
                "work at",
                "work at",
                "Acme",
                "Bob",
                true,
            ),
            ("Bob quit Acme.", "work at", "work at", "Acme", "Bob", true),
            (
                "Bob left the job at Acme.",
                "work at",
                "work at",
                "Acme",
                "Bob",
                true,
            ),
            (
                "Alice moved out of the Austin flat.",
                "live in",
                "live in",
                "Austin",
                "Alice",
                true,
            ),
            (
                "Alice broke up with Bob.",
                "married to",
                "married to",
                "Bob",
                "Alice",
                true,
            ),
            (
                "Alice divorced Bob.",
                "married to",
                "married to",
                "Bob",
                "Alice",
                true,
            ),
            ("Alice sold the boat.", "own", "own", "boat", "Alice", true),
            ("Bob stopped using Vim.", "use", "use", "Vim", "Bob", true),
            ("Bob gave up Vim.", "use", "use", "Vim", "Bob", true),
            // A retraction only retracts its own group.
            (
                "Bob resigned from Acme.",
                "live in",
                "live in",
                "Acme",
                "Bob",
                false,
            ),
            (
                "Alice moved out of Austin and Bob quit Acme.",
                "work at",
                "work at",
                "Acme",
                "Bob",
                true,
            ),
            // ER-14: Spanish "ya no".
            (
                "Alicia ya no vive en Madrid.",
                "vive en",
                "vive en",
                "Madrid",
                "Alicia",
                true,
            ),
            // Double negatives stay positive.
            (
                "Alice never stopped liking jazz",
                "like",
                "like",
                "jazz",
                "Alice",
                false,
            ),
            // H2-04: a cue about another subject never negates this relation.
            (
                "Alice no longer leads the core team; Bob took over.",
                "lead",
                "lead",
                "core team",
                "Bob",
                false,
            ),
            (
                "Alice no longer leads the core team; Bob took over.",
                "lead",
                "lead",
                "core team",
                "Alice",
                true,
            ),
        ];
        for (text, predicate, group, target, source, want) in cases {
            assert_eq!(
                infer_negated(text, predicate, group, target, source),
                want,
                "{text} / {target} / {source}"
            );
        }
    }
}
