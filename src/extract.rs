//! Heuristic entity extraction for memories written without structure, and
//! n-gram generation for alias (gazetteer) lookup. Pure functions.

use crate::text::{fold, is_stopword, norm_name, tokens};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Surface text as written.
    pub name: String,
    /// "proper" | "quoted" | "handle" | "hashtag" | "url" | "path" | "identifier" | "version"
    pub hint: &'static str,
    /// Byte offsets into the input.
    pub start: usize,
    pub end: usize,
}

/// Candidate entities: capitalised multi-word spans (not just a
/// sentence-initial common word), quoted spans, @handles, #hashtags, URLs,
/// file paths, code identifiers (snake_case, camelCase, dotted.names),
/// versioned names ("Postgres 16"). Deduped by norm_name, document order.
pub fn heuristic_entities(text: &str) -> Vec<Candidate> {
    heuristic_entities_with(text, &|_| false)
}

/// As `heuristic_entities`, with `known` telling whether a name is already an
/// entity or alias (EX-03: "Cypher Memory nodes" keeps "Cypher" when "Cypher"
/// is known and the compound never was).
pub fn heuristic_entities_with(text: &str, known: &dyn Fn(&str) -> bool) -> Vec<Candidate> {
    let (mut cands, consumed) = scan_specials(text);
    cands.extend(word_candidates(text, &consumed, known));
    cands.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| hint_rank(a.hint).cmp(&hint_rank(b.hint)))
    });
    let mut seen: Vec<String> = Vec::new();
    cands.retain(|c| {
        if !c.name.chars().any(char::is_alphabetic) {
            return false;
        }
        if c.name.chars().count() <= 1 {
            return false;
        }
        if is_all_caps_status_word(&c.name) {
            return false;
        }
        if is_date_like_name(&c.name) {
            return false;
        }
        let norm = norm_name(&c.name);
        if norm.is_empty() || seen.contains(&norm) {
            return false;
        }
        seen.push(norm);
        true
    });
    cands
}

fn hint_rank(hint: &str) -> u8 {
    match hint {
        "url" => 0,
        "quoted" => 1,
        "handle" => 2,
        "hashtag" => 3,
        "path" => 4,
        "identifier" => 5,
        "version" => 6,
        _ => 7,
    }
}

/// URLs, paths, @handles, #hashtags and quoted spans found by raw text scan.
/// Returns the candidates plus the ranges whose text must not also become a
/// word-level candidate (a URL's "example.com" is not a dotted identifier).
fn scan_specials(text: &str) -> (Vec<Candidate>, Vec<(usize, usize)>) {
    let mut cands = Vec::new();
    let mut consumed = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let c = rest.chars().next().expect("char boundary");
        let prev = text[..i].chars().next_back();
        let at_boundary = prev.is_none_or(|p| !p.is_alphanumeric());

        if starts_ignore_case(rest, "https://") || starts_ignore_case(rest, "http://") {
            let end = url_end(text, i);
            cands.push(Candidate {
                name: text[i..end].to_string(),
                hint: "url",
                start: i,
                end,
            });
            consumed.push((i, end));
            i = end;
            continue;
        }

        if c == '@' && at_boundary {
            let mut end = i + 1;
            while let Some(ch) = text[end..].chars().next() {
                if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-') {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            while end > i + 1 {
                let last = text[i + 1..end].chars().next_back().expect("non-empty");
                if matches!(last, '.' | '-') {
                    end -= last.len_utf8();
                } else {
                    break;
                }
            }
            let tag = &text[i + 1..end];
            if tag.chars().any(|ch| ch.is_ascii_alphanumeric()) {
                cands.push(Candidate {
                    name: text[i..end].to_string(),
                    hint: "handle",
                    start: i,
                    end,
                });
                consumed.push((i, end));
            }
            i = end.max(i + 1);
            continue;
        }

        if c == '#' && at_boundary {
            let mut end = i + 1;
            while let Some(ch) = text[end..].chars().next() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            let tag = &text[i + 1..end];
            if tag
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric())
            {
                cands.push(Candidate {
                    name: tag.to_string(),
                    hint: "hashtag",
                    start: i + 1,
                    end,
                });
                consumed.push((i, end));
            }
            i = end.max(i + 1);
            continue;
        }

        if at_boundary {
            if let Some(end) = path_end(text, i) {
                cands.push(Candidate {
                    name: text[i..end].to_string(),
                    hint: "path",
                    start: i,
                    end,
                });
                consumed.push((i, end));
                i = end;
                continue;
            }
        }

        if matches!(c, '"' | '\'' | '`') && at_boundary {
            if let Some((name_start, name_end)) = quoted_span(text, i, c) {
                cands.push(Candidate {
                    name: text[name_start..name_end].to_string(),
                    hint: "quoted",
                    start: name_start,
                    end: name_end,
                });
            }
        }
        i += c.len_utf8();
    }
    (cands, consumed)
}

fn starts_ignore_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn url_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for (off, ch) in text[start..].char_indices() {
        if ch.is_whitespace() {
            break;
        }
        end = start + off + ch.len_utf8();
    }
    trim_trailing_punct(text, start, end)
}

fn trim_trailing_punct(text: &str, start: usize, mut end: usize) -> usize {
    while end > start {
        let last = text[start..end].chars().next_back().expect("non-empty");
        if matches!(
            last,
            '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '\'' | '"' | '`'
        ) {
            end -= last.len_utf8();
        } else {
            break;
        }
    }
    end
}

/// End offset of a path starting at `start` (`/a/b`, `./x`, `~/x`, `C:\x`).
fn path_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    let prefix = if rest.starts_with("../") {
        3
    } else if rest.starts_with("./") || rest.starts_with("~/") {
        2
    } else if rest.starts_with('/') {
        1
    } else {
        let bytes = rest.as_bytes();
        if bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/')
        {
            3
        } else {
            return None;
        }
    };
    if !rest[prefix..]
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '~'))
    {
        return None;
    }
    let mut end = start;
    for (off, ch) in rest.char_indices() {
        if ch.is_alphanumeric()
            || matches!(
                ch,
                '/' | '\\' | '.' | '-' | '_' | '~' | '@' | '+' | '%' | ':'
            )
        {
            end = start + off + ch.len_utf8();
        } else {
            break;
        }
    }
    let end = trim_trailing_punct(text, start, end);
    let path = &text[start..end];
    let has_extension = path.contains('/')
        && path
            .rsplit('/')
            .next()
            .is_some_and(|part| part.contains('.'));
    let has_segments = path.starts_with('/') && path[1..].matches('/').count() >= 1;
    let explicit_relative =
        path.starts_with("./") || path.starts_with("~/") || path.starts_with("../");
    (end > start + prefix && (has_extension || has_segments || explicit_relative)).then_some(end)
}

fn quoted_span(text: &str, open: usize, quote: char) -> Option<(usize, usize)> {
    let inner_start = open + quote.len_utf8();
    let mut search = inner_start;
    while let Some(rel) = text[search..].find(quote) {
        let close = search + rel;
        let after = text[close + quote.len_utf8()..].chars().next();
        let inner = &text[inner_start..close];
        let words = inner.split_whitespace().count();
        if after.is_none_or(|c| !c.is_alphanumeric())
            && !inner.contains('\n')
            && (1..=6).contains(&words)
        {
            let trimmed = inner.trim();
            if !trimmed.is_empty() {
                let lead = inner.len() - inner.trim_start().len();
                return Some((inner_start + lead, inner_start + lead + trimmed.len()));
            }
        }
        search = close + quote.len_utf8();
    }
    None
}

struct Word<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn is_consumed(ranges: &[(usize, usize)], pos: usize) -> bool {
    ranges.iter().any(|&(start, end)| pos >= start && pos < end)
}

fn word_tokens<'a>(text: &'a str, consumed: &[(usize, usize)]) -> Vec<Word<'a>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut words = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        if is_consumed(consumed, start) {
            i += 1;
            continue;
        }
        if c == '&' {
            words.push(Word {
                text: &text[start..start + 1],
                start,
                end: start + 1,
            });
            i += 1;
            continue;
        }
        if !c.is_alphanumeric() {
            i += 1;
            continue;
        }
        let mut end = start + c.len_utf8();
        let mut j = i + 1;
        while j < chars.len() {
            let (pos, ch) = chars[j];
            if is_consumed(consumed, pos) {
                break;
            }
            if ch.is_alphanumeric() || is_word_joiner(&chars, j) {
                end = pos + ch.len_utf8();
                j += 1;
            } else {
                break;
            }
        }
        words.push(Word {
            text: &text[start..end],
            start,
            end,
        });
        i = j;
    }
    words
}

fn is_word_joiner(chars: &[(usize, char)], j: usize) -> bool {
    let ch = chars[j].1;
    let prev = j.checked_sub(1).and_then(|k| chars.get(k)).map(|&(_, c)| c);
    let next = chars.get(j + 1).map(|&(_, c)| c);
    match ch {
        '.' | '\'' | '’' | '_' => {
            prev.is_some_and(char::is_alphanumeric) && next.is_some_and(char::is_alphanumeric)
        }
        ':' => {
            prev.is_some_and(|p| p.is_alphanumeric() || p == ':')
                && next.is_some_and(|n| n.is_alphanumeric() || n == ':')
        }
        _ => false,
    }
}

pub const STATUS_WORDS: &[&str] = &[
    "PASS", "FAIL", "FAILED", "OK", "DONE", "CLOSED", "OPEN", "TODO", "FIXME", "NOTE", "WARN",
    "WARNING", "ERROR", "NEW", "FULL", "RED", "GREEN", "YES", "NO", "CLEAN", "DIRTY", "READY",
    "WIP", "NA", "TBD", "AND", "OR", "NOT", "THE", "ALL", "ANY",
];

pub fn is_status_word(word: &str) -> bool {
    STATUS_WORDS.contains(&word)
}

fn is_all_caps_status_word(name: &str) -> bool {
    name.split_whitespace().any(|word| {
        is_status_word(word)
            // The >= 5 catch rejects English status shouts (MERGED, BUILD);
            // other cased scripts have no English status words, so their
            // all-caps names (ЕВРОПА, МОСКВА) are kept.
            || (word.is_ascii()
                && word.chars().count() >= 5
                && word.chars().all(|c| !c.is_lowercase())
                && !word.chars().any(|c| c.is_ascii_digit()))
    })
}

const ORDINARY_WORDS: &[&str] = &[
    "replay",
    "merged",
    "merge",
    "full",
    "remaining",
    "remain",
    "starting",
    "start",
    "unpark",
    "reports",
    "report",
    "lane",
    "rule",
    "rules",
    "suite",
    "test",
    "tests",
    "build",
    "builds",
    "check",
    "checked",
    "clean",
    "step",
    "steps",
    "task",
    "tasks",
    "item",
    "items",
    "part",
    "parts",
    "phase",
    "phases",
];

fn is_ordinary_word(word: &str) -> bool {
    let f = fold(word);
    is_stopword(&f)
        || is_weekday_or_month(&f)
        || excluded_word(word)
        || is_status_word(word)
        || ORDINARY_WORDS.contains(&f.as_str())
}

fn is_sentence_start(text: &str, pos: usize) -> bool {
    let before = text[..pos].trim_end_matches([' ', '\t']);
    if before.is_empty() {
        return true;
    }
    let last = before.chars().next_back().unwrap();
    if matches!(last, '.' | '!' | '?' | ':' | ';' | '\n' | '\r') {
        return true;
    }
    if matches!(last, '-' | '*' | '+') {
        let pre = before[..before.len() - last.len_utf8()].trim_end_matches([' ', '\t']);
        if pre.is_empty() || pre.ends_with('\n') || pre.ends_with('\r') {
            return true;
        }
    }
    false
}

fn appears_capitalised_not_at_sentence_start(text: &str, word: &str) -> bool {
    let mut search = 0;
    while let Some(rel) = text[search..].find(word) {
        let idx = search + rel;
        search = idx + word.len();

        let at_word_start = idx == 0 || !text[..idx].chars().next_back().unwrap().is_alphanumeric();
        let at_word_end =
            search == text.len() || !text[search..].chars().next().unwrap().is_alphanumeric();

        if at_word_start && at_word_end && !is_sentence_start(text, idx) {
            return true;
        }
    }
    false
}

fn has_inner_capital_or_digit(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 2 {
        return false;
    }
    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let curr = chars[i];
        if curr.is_uppercase() && prev.is_lowercase() {
            return true;
        }
        if curr.is_ascii_digit()
            && prev.is_alphabetic()
            && (i + 1 < chars.len() && chars[i + 1].is_alphabetic())
        {
            return true;
        }
    }
    false
}

fn is_acronym(word: &str) -> bool {
    let count = word.chars().count();
    (2..=6).contains(&count) && word.chars().all(|c| c.is_uppercase()) && !is_status_word(word)
}

fn is_single_capital_letter(word: &str) -> bool {
    word.chars().count() == 1 && word.chars().next().unwrap().is_uppercase()
}

/// Non-ASCII title-case word (Latin with diacritics, Greek, Cyrillic,
/// Armenian): a borrowed name the stopword/openers lists cannot contain, so
/// sentence position alone must not reject it. Caseless scripts (CJK, Arabic)
/// stay a documented limit - they carry no capitalisation to detect.
fn is_foreign_title_case(word: &str) -> bool {
    let cs: Vec<char> = word.chars().collect();
    cs.len() >= 2
        && cs[0].is_uppercase()
        && cs[1..].iter().all(|c| !c.is_uppercase())
        && cs.iter().any(|c| !c.is_ascii())
}

fn word_candidates(
    text: &str,
    consumed: &[(usize, usize)],
    known: &dyn Fn(&str) -> bool,
) -> Vec<Candidate> {
    let words = word_tokens(text, consumed);
    let mut cands = Vec::new();
    for (idx, word) in words.iter().enumerate() {
        if is_unit_after_number(&words, text, idx) {
            continue;
        }
        if let Some(hint) = word_hint(word.text) {
            cands.push(Candidate {
                name: word.text.to_string(),
                hint,
                start: word.start,
                end: word.end,
            });
        }
    }
    cands.extend(unit_label_candidates(text, &words));

    let mut i = 0;
    while i < words.len() {
        if !is_span_start(words[i].text)
            || is_unit_after_number(&words, text, i)
            || unit_label_follows(&words, text, i)
        {
            i += 1;
            continue;
        }
        let mut last = i;
        let mut k = i + 1;
        while k < words.len() {
            if !gap_is_space(text, &words[k - 1], &words[k]) {
                break;
            }
            let w = words[k].text;
            if is_acronym(words[k - 1].text) && is_acronym(w) {
                break;
            }
            if unit_label_follows(&words, text, k) {
                break;
            }
            if is_proper_word(w) && !is_unit_after_number(&words, text, k) {
                last = k;
                k += 1;
            } else if is_connector(w)
                && k + 1 < words.len()
                && is_proper_word(words[k + 1].text)
                && gap_is_space(text, &words[k], &words[k + 1])
            {
                k += 1;
            } else if is_number(w) && (k + 1 >= words.len() || !is_proper_word(words[k + 1].text)) {
                last = k;
                break;
            } else {
                break;
            }
        }

        // Rule 3: leading/trailing status words are trimmed from spans
        let mut s_idx = i;
        let mut e_idx = last;
        while s_idx <= e_idx && is_status_word(words[s_idx].text) {
            s_idx += 1;
        }
        while e_idx >= s_idx && is_status_word(words[e_idx].text) {
            e_idx -= 1;
        }
        if s_idx > e_idx {
            i = last + 1;
            continue;
        }

        // Rule 3: a span made only of single letters/numbers/status words is rejected
        let only_trivial = words[s_idx..=e_idx]
            .iter()
            .all(|w| w.text.chars().count() <= 1 || is_number(w.text) || is_status_word(w.text));
        if only_trivial {
            i = last + 1;
            continue;
        }

        if s_idx == e_idx {
            let w = &words[s_idx];
            // Rule 2: A single capitalised word at sentence start is kept only if
            // the same word also appears capitalised NOT at a sentence start elsewhere in the text,
            // or it has an inner capital/digit (ReMem, PostgreSQL, k8s),
            // or it is an acronym (SQL, MCP, CPU).
            // Non-ASCII title-case words (Ålesund, Москва, Νίκος) are kept: the
            // stopword/openers lexicons are English, so position is their only
            // English-word signal.
            let keep = if is_sentence_start(text, w.start) {
                is_acronym(w.text)
                    || has_inner_capital_or_digit(w.text)
                    || is_foreign_title_case(w.text)
                    || appears_capitalised_not_at_sentence_start(text, w.text)
            } else {
                true
            };
            if keep {
                cands.push(Candidate {
                    name: w.text.to_string(),
                    hint: "proper",
                    start: w.start,
                    end: w.end,
                });
            }
        } else {
            // Rule 2: keep spans where the second token is a single capital letter or a number
            // only if the first token is not a sentence-initial ordinary word
            let w0 = &words[s_idx];
            let w1 = &words[s_idx + 1];
            let second_is_single_cap_or_num =
                is_single_capital_letter(w1.text) || is_number(w1.text);
            let reject_span = second_is_single_cap_or_num
                && is_sentence_start(text, w0.start)
                && (w0.text.chars().count() <= 1 || is_ordinary_word(w0.text));

            if is_known_head_span(text, &words, s_idx, e_idx, known) {
                // EX-03: a known first word stands alone; its compound is not an entity.
                let w = &words[s_idx];
                cands.push(Candidate {
                    name: w.text.to_string(),
                    hint: "proper",
                    start: w.start,
                    end: w.end,
                });
            } else if !reject_span {
                let has_calendar = words[s_idx..=e_idx]
                    .iter()
                    .any(|w| is_weekday_or_month(&fold(w.text)));
                if !has_calendar {
                    let start = words[s_idx].start;
                    let end = words[e_idx].end;
                    let hint = if is_number(words[e_idx].text) {
                        "version"
                    } else {
                        "proper"
                    };
                    cands.push(Candidate {
                        name: text[start..end].to_string(),
                        hint,
                        start,
                        end,
                    });
                }
            }
        }

        i = last + 1;
    }
    cands
}

fn gap_is_space(text: &str, left: &Word<'_>, right: &Word<'_>) -> bool {
    text[left.end..right.start].chars().all(char::is_whitespace)
}

/// Measures that follow a number ("1.2 MB", "120 ms"): not entities (EX-02).
/// A bare "%" never reaches here, since it is not a word token.
const UNITS: &[&str] = &[
    "kb", "mb", "gb", "tb", "kib", "mib", "gib", "ms", "s", "px", "mhz", "ghz", "km", "kg",
];

/// Head nouns of unit, room and gate labels ("Apt 3B", "Suite 210") (EX-01).
const UNIT_HEADS: &[&str] = &[
    "apt",
    "apartment",
    "flat",
    "room",
    "suite",
    "unit",
    "gate",
    "building",
    "bldg",
    "floor",
    "office",
    "bay",
    "wing",
    "lot",
];

/// Lowercase nouns that may close a known-name span (EX-03). A closed list on
/// purpose: a verb ("Alice Smith lives") must never split a real name.
const SPAN_NOUNS: &[&str] = &[
    "nodes",
    "edges",
    "queries",
    "tables",
    "records",
    "rows",
    "files",
    "logs",
    "tests",
    "models",
    "memories",
    "entries",
    "items",
    "pages",
    "indexes",
    "columns",
    "fields",
    "values",
    "keys",
    "functions",
    "modules",
    "crates",
    "packages",
    "servers",
    "services",
    "endpoints",
    "tools",
    "commands",
    "options",
    "settings",
    "rules",
    "errors",
    "events",
    "messages",
    "requests",
    "facts",
    "entities",
    "relations",
    "users",
    "rooms",
    "buildings",
    "devices",
    "tasks",
    "bugs",
    "features",
    "versions",
    "releases",
    "projects",
    "scripts",
    "tickets",
    "reports",
];

fn is_unit_after_number(words: &[Word<'_>], text: &str, k: usize) -> bool {
    k > 0
        && is_number(words[k - 1].text)
        && gap_is_space(text, &words[k - 1], &words[k])
        && UNITS.contains(&words[k].text.to_ascii_lowercase().as_str())
}

/// A letter+digit label ("3B", "B12", "5A"). Plain numbers stay with the span rules.
fn is_mixed_label(word: &str) -> bool {
    let letters = word.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let digits = word.chars().filter(|c| c.is_ascii_digit()).count();
    word.is_ascii()
        && (1..=2).contains(&letters)
        && (1..=4).contains(&digits)
        && letters + digits == word.len()
}

/// A label after a unit head: a plain number ("210") or a letter+digit label ("3B").
fn is_unit_label(word: &str) -> bool {
    is_number(word) || is_mixed_label(word)
}

fn is_unit_head(word: &str) -> bool {
    is_proper_word(word) && UNIT_HEADS.contains(&fold(word).as_str())
}

/// True when word `k` is a unit head ("Apt") followed by a mixed label ("3B").
/// The head is then neither a span member nor an entity on its own.
fn unit_label_follows(words: &[Word<'_>], text: &str, k: usize) -> bool {
    k + 1 < words.len()
        && is_unit_head(words[k].text)
        && is_unit_label(words[k + 1].text)
        && gap_is_space(text, &words[k], &words[k + 1])
}

fn unit_label_candidates(text: &str, words: &[Word<'_>]) -> Vec<Candidate> {
    (0..words.len())
        .filter(|&k| unit_label_follows(words, text, k))
        .map(|k| {
            let (start, end) = (words[k].start, words[k + 1].end);
            Candidate {
                name: text[start..end].to_string(),
                hint: "label",
                start,
                end,
            }
        })
        .collect()
}

/// EX-03: "Cypher Memory nodes" is a known first word plus a noun when the
/// first word is already an entity or alias, the compound is not, and the
/// next word is in `SPAN_NOUNS`. Two-word spans only.
fn is_known_head_span(
    text: &str,
    words: &[Word<'_>],
    s_idx: usize,
    e_idx: usize,
    known: &dyn Fn(&str) -> bool,
) -> bool {
    if e_idx != s_idx + 1 || !is_proper_word(words[e_idx].text) {
        return false;
    }
    let Some(next) = words.get(e_idx + 1) else {
        return false;
    };
    SPAN_NOUNS.contains(&next.text)
        && gap_is_space(text, &words[e_idx], next)
        && known(words[s_idx].text)
        && !known(&text[words[s_idx].start..words[e_idx].end])
}

fn word_hint(text: &str) -> Option<&'static str> {
    const EXTENSIONS: [&str; 43] = [
        "rs", "py", "js", "ts", "tsx", "jsx", "md", "json", "toml", "yaml", "yml", "txt", "sh",
        "bash", "zsh", "c", "h", "cc", "cpp", "hpp", "go", "java", "rb", "php", "sql", "db",
        "sqlite", "csv", "tsv", "html", "css", "scss", "conf", "cfg", "ini", "log", "lock", "png",
        "jpg", "jpeg", "gif", "svg", "pdf",
    ];
    if let Some((stem, ext)) = text.rsplit_once('.') {
        if !stem.is_empty() && EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
            return Some("path");
        }
    }
    if !text.chars().any(char::is_alphabetic) {
        return None;
    }
    if text.contains('_') || text.contains("::") || text.matches('.').count() >= 2 {
        return Some("identifier");
    }
    if has_inner_capital_or_digit(text) {
        return Some("identifier");
    }
    None
}

fn is_proper_word(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase)
}

fn is_number(word: &str) -> bool {
    word.chars().any(|c| c.is_ascii_digit()) && word.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn is_connector(word: &str) -> bool {
    ["of", "de", "van", "&"]
        .iter()
        .any(|c| word.eq_ignore_ascii_case(c))
}

fn is_span_start(word: &str) -> bool {
    is_proper_word(word) && (!excluded_word(word) || is_status_word(word))
}

fn excluded_word(word: &str) -> bool {
    const OPENERS: &[&str] = &[
        "yesterday",
        "today",
        "tomorrow",
        "also",
        "then",
        "however",
        "note",
        "please",
        "finally",
        "first",
        "second",
        "third",
        "next",
        "meanwhile",
        "additionally",
        "furthermore",
        "moreover",
        "still",
        "yet",
        "overall",
        "generally",
        "usually",
        "often",
        "sometimes",
        "always",
        "never",
        "let",
        "use",
        "call",
        "see",
        "check",
        "contact",
        "run",
        "try",
        "example",
        "remember",
        "important",
        "warning",
        "tip",
        "ok",
        "okay",
        "thanks",
        "thank",
        "hi",
        "hello",
        "dear",
        "sorry",
    ];
    let norm = fold(word);
    norm == "i"
        || is_stopword(&norm)
        || is_weekday_or_month(&norm)
        || OPENERS.contains(&norm.as_str())
}

fn is_weekday_or_month(word: &str) -> bool {
    const CALENDAR: [&str; 41] = [
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
        // Abbreviations, as written without the full stop.
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
        "mon",
        "tue",
        "tues",
        "wed",
        "thu",
        "thur",
        "thurs",
        "fri",
        "sat",
        "sun",
    ];
    if CALENDAR.contains(&word) {
        return true;
    }
    // "Mondays", "Tuesdays": a plural weekday is still a calendar word.
    word.strip_suffix('s')
        .is_some_and(|base| base.len() >= 4 && CALENDAR.contains(&base))
}

/// True when a candidate is a date, month, weekday, year, quarter, duration
/// or relative-time expression rather than a name. Every token must be
/// date-ish, so "Windows 2000" and "Postgres 16" survive while "Dec 2025",
/// "March 5, 2025", "3 days" and "yesterday" do not.
fn is_date_like_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    if is_iso_date(&lower) || is_clock_time(&lower) {
        return true;
    }
    let toks: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    if toks.is_empty() {
        return false;
    }
    // A trailing month + year is a date even when a verb leads the span
    // ("Ships Dec 2025"); "Windows 2000" keeps its product name.
    if toks.len() >= 2 {
        let last = toks[toks.len() - 1];
        let prev = toks[toks.len() - 2];
        if is_year_token(last) && is_calendar_token(prev) {
            return true;
        }
    }
    let mut saw_date = false;
    for tok in &toks {
        if is_calendar_token(tok)
            || is_year_token(tok)
            || is_quarter_token(tok)
            || is_time_unit(tok)
            || is_relative_time(tok)
        {
            saw_date = true;
            continue;
        }
        if is_number_token(tok) || is_ordinal_token(tok) || is_date_connector(tok) {
            continue;
        }
        return false;
    }
    saw_date
}

fn is_calendar_token(tok: &str) -> bool {
    is_weekday_or_month(tok)
}

fn is_year_token(tok: &str) -> bool {
    tok.len() == 4
        && tok.bytes().all(|b| b.is_ascii_digit())
        && tok.parse::<i64>().is_ok_and(|y| (1900..=2100).contains(&y))
}

fn is_quarter_token(tok: &str) -> bool {
    let mut chars = tok.chars();
    matches!(chars.next(), Some('q'))
        && chars.next().is_some_and(|c| ('1'..='4').contains(&c))
        && chars.next().is_none()
}

fn is_time_unit(tok: &str) -> bool {
    matches!(
        tok,
        "day"
            | "days"
            | "week"
            | "weeks"
            | "month"
            | "months"
            | "year"
            | "years"
            | "hour"
            | "hours"
            | "minute"
            | "minutes"
            | "min"
            | "mins"
            | "second"
            | "seconds"
            | "sec"
            | "secs"
            | "fortnight"
            | "fortnights"
    )
}

fn is_relative_time(tok: &str) -> bool {
    matches!(
        tok,
        "yesterday"
            | "today"
            | "tomorrow"
            | "tonight"
            | "now"
            | "ago"
            | "recently"
            | "lately"
            | "soon"
            | "later"
            | "earlier"
            | "upcoming"
            | "currently"
            | "daily"
            | "weekly"
            | "monthly"
            | "annually"
            | "yearly"
    )
}

fn is_number_token(tok: &str) -> bool {
    if !tok.is_empty() && tok.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return tok.bytes().any(|b| b.is_ascii_digit());
    }
    matches!(
        tok,
        "a" | "an"
            | "one"
            | "two"
            | "three"
            | "four"
            | "five"
            | "six"
            | "seven"
            | "eight"
            | "nine"
            | "ten"
            | "eleven"
            | "twelve"
    )
}

fn is_ordinal_token(tok: &str) -> bool {
    let digits = tok.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    if digits.len() < tok.len()
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && matches!(&tok[digits.len()..], "st" | "nd" | "rd" | "th")
    {
        return true;
    }
    matches!(
        tok,
        "first"
            | "second"
            | "third"
            | "fourth"
            | "fifth"
            | "sixth"
            | "seventh"
            | "eighth"
            | "ninth"
            | "tenth"
    )
}

fn is_date_connector(tok: &str) -> bool {
    matches!(
        tok,
        "of" | "the"
            | "in"
            | "on"
            | "at"
            | "and"
            | "to"
            | "from"
            | "between"
            | "since"
            | "until"
            | "before"
            | "after"
            | "ago"
            | "next"
            | "last"
            | "this"
            | "every"
            | "each"
            | "per"
            | "around"
            | "about"
            | "by"
            | "for"
    )
}

fn is_iso_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split(['-', '/']).collect();
    parts.len() == 3
        && parts[0].len() == 4
        && parts[0].bytes().all(|b| b.is_ascii_digit())
        && (1..=2).contains(&parts[1].len())
        && parts[1].bytes().all(|b| b.is_ascii_digit())
        && (1..=2).contains(&parts[2].len())
        && parts[2].bytes().all(|b| b.is_ascii_digit())
}

fn is_clock_time(s: &str) -> bool {
    if let Some((h, rest)) = s.split_once(':') {
        return !h.is_empty()
            && h.bytes().all(|b| b.is_ascii_digit())
            && rest
                .split(':')
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    }
    // "10am", "9pm"
    let hour = |suffix: &str| {
        s.len() > suffix.len()
            && s.ends_with(suffix)
            && s[..s.len() - suffix.len()]
                .bytes()
                .all(|b| b.is_ascii_digit())
    };
    hour("am") || hour("pm")
}

/// All word n-grams (1..=max_n) of `text` as (norm_name(ngram), start, end)
/// byte offsets, for exact alias lookup. Skips n-grams that are only stopwords.
pub fn ngrams(text: &str, max_n: usize) -> Vec<(String, usize, usize)> {
    if max_n == 0 {
        return Vec::new();
    }
    let words = ngram_words(text);
    let mut out = Vec::new();
    for start in 0..words.len() {
        for n in 1..=max_n {
            if start + n > words.len() {
                break;
            }
            let s = words[start].0;
            let e = words[start + n - 1].1;
            let slice = &text[s..e];
            let toks = tokens(slice);
            if toks.is_empty() || toks.iter().all(|t| is_stopword(t)) {
                continue;
            }
            let norm = norm_name(slice);
            if !norm.is_empty() {
                out.push((norm, s, e));
            }
        }
    }
    out
}

/// Word spans for n-grams: alphanumeric runs plus inner apostrophes, hyphens
/// and dots.
fn ngram_words(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut words = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        if !c.is_alphanumeric() {
            i += 1;
            continue;
        }
        let mut end = start + c.len_utf8();
        let mut j = i + 1;
        while j < chars.len() {
            let (pos, ch) = chars[j];
            if ch.is_alphanumeric()
                || (matches!(ch, '\'' | '’' | '-' | '.')
                    && chars
                        .get(j + 1)
                        .is_some_and(|&(_, next)| next.is_alphanumeric()))
            {
                end = pos + ch.len_utf8();
                j += 1;
            } else {
                break;
            }
        }
        words.push((start, end));
        i = j;
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::norm_name;

    const PARAGRAPH: &str = "Yesterday Alice and Bob met at New York City. They discussed the \
        \"Old Bridge\" project. Also, ping @alice_dev or follow #rustlang. See \
        https://example.com/docs/v1 for details. The config lives in Cargo.toml and the tool \
        sits in /usr/local/bin/tool.sh; start it with ./run.sh. Call get_user_name and \
        std::fs::read, or getUserName. We deployed Postgres 16 on Monday in January. I checked \
        the API.";

    fn find<'a>(cands: &'a [Candidate], name: &str) -> &'a Candidate {
        cands
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("missing {name:?} in {cands:#?}"))
    }

    #[test]
    fn heuristic_entities_finds_every_kind() {
        let c = heuristic_entities(PARAGRAPH);
        for (name, hint) in [
            ("Alice", "proper"),
            ("Bob", "proper"),
            ("New York City", "proper"),
            ("Old Bridge", "quoted"),
            ("@alice_dev", "handle"),
            ("rustlang", "hashtag"),
            ("https://example.com/docs/v1", "url"),
            ("Cargo.toml", "path"),
            ("/usr/local/bin/tool.sh", "path"),
            ("./run.sh", "path"),
            ("get_user_name", "identifier"),
            ("std::fs::read", "identifier"),
            ("getUserName", "identifier"),
            ("Postgres 16", "version"),
            ("API", "proper"),
        ] {
            assert_eq!(find(&c, name).hint, hint, "hint for {name:?}");
        }
    }

    #[test]
    fn heuristic_entities_offsets_slice_the_input() {
        for cand in heuristic_entities(PARAGRAPH) {
            assert_eq!(
                &PARAGRAPH[cand.start..cand.end],
                cand.name,
                "offsets for {:?}",
                cand.name
            );
        }
    }

    #[test]
    fn heuristic_entities_dedupes_and_orders() {
        let c = heuristic_entities(PARAGRAPH);
        let old_bridge = c
            .iter()
            .filter(|x| norm_name(&x.name) == "old bridge")
            .count();
        assert_eq!(old_bridge, 1, "quoted and proper span must dedupe");
        assert!(c.windows(2).all(|w| w[0].start <= w[1].start));
        assert_eq!(c.first().map(|x| x.name.as_str()), Some("Alice"));
        assert_eq!(find(&c, "Old Bridge").hint, "quoted");
    }

    #[test]
    fn heuristic_entities_skips_common_words_and_calendar_names() {
        let c = heuristic_entities(PARAGRAPH);
        for bad in [
            "Yesterday",
            "They",
            "Also",
            "See",
            "The",
            "Call",
            "We",
            "I",
            "Monday",
            "January",
            "project",
            "details",
            "config",
        ] {
            let n = norm_name(bad);
            assert!(
                c.iter().all(|x| norm_name(&x.name) != n),
                "{bad:?} leaked into {c:#?}"
            );
        }
    }

    // S-03: cased scripts beyond plain ASCII-Latin must yield entities.
    #[test]
    fn heuristic_entities_yields_non_english_names() {
        for (text, kept, rejected) in [
            // Sentence-initial and mid-sentence Cyrillic names.
            ("Мюллер позвонил Астрид.", &["Мюллер", "Астрид"][..], &[]),
            (
                "Борис Жуков приехал в Москву.",
                &["Борис Жуков", "Москву"][..],
                &[],
            ),
            // Greek: tonos on alpha, sentence-initial and mid-sentence.
            ("Νίκος μένει στην Αθήνα.", &["Νίκος", "Αθήνα"][..], &[]),
            // Armenian sentence-initial.
            ("Արամ ապրում է Երևանում.", &["Արամ"][..], &[]),
            // Latin with diacritics.
            ("Ålesund er vakker.", &["Ålesund"][..], &[]),
            ("Zoë werkt in Łódź.", &["Zoë", "Łódź"][..], &[]),
        ] {
            let cands = heuristic_entities(text);
            let names: Vec<&str> = cands.iter().map(|c| c.name.as_str()).collect();
            for want in kept {
                assert!(
                    names.iter().any(|n| n == want),
                    "{want:?} missing from {names:?} in {text:?}"
                );
            }
            for bad in rejected {
                assert!(!names.contains(bad), "{bad:?} present in {names:?}");
            }
        }
    }

    #[test]
    fn heuristic_entities_keeps_all_caps_non_english_names() {
        // All-caps Cyrillic/Greek names are not English status shouts.
        let cands = heuristic_entities("ЕВРОПА продала ПОРТ.");
        let names: Vec<&str> = cands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ЕВРОПА"), "got {names:?}");
        // English all-caps status words stay rejected.
        let cands = heuristic_entities("PRINT MERGED STATUS BUILD42");
        let names: Vec<&str> = cands.iter().map(|c| c.name.as_str()).collect();
        for bad in ["MERGED", "STATUS", "PRINT"] {
            assert!(!names.contains(&bad), "{bad:?} present in {names:?}");
        }
    }

    #[test]
    fn ngrams_skip_stopword_only_spans() {
        let text = "New York City is in the United States of America";
        let g = ngrams(text, 4);
        for (norm, s, e) in &g {
            assert_eq!(norm_name(&text[*s..*e]), *norm);
            assert!(text.is_char_boundary(*s) && text.is_char_boundary(*e));
        }
        let has = |n: &str| g.iter().any(|(x, _, _)| x == n);
        assert!(has("new york city"));
        assert!(has("united states of america"));
        assert!(has("york city"));
        assert!(!has("is in"));
        assert!(!has("in the"));
        assert!(!has("the"));
        let (_, s, e) = g
            .iter()
            .find(|(x, _, _)| x == "united states of america")
            .unwrap();
        assert_eq!(&text[*s..*e], "United States of America");
    }

    #[test]
    fn ngrams_keep_inner_apostrophes_in_offsets() {
        let text = "Mom's house";
        let g = ngrams(text, 2);
        assert!(g
            .iter()
            .any(|(n, s, e)| n == "mom" && &text[*s..*e] == "Mom's"));
        assert!(g.iter().any(|(n, _, _)| n == "moms house"));
        assert!(g
            .iter()
            .any(|(n, s, e)| n == "house" && &text[*s..*e] == "house"));
    }

    fn names(text: &str, known: &dyn Fn(&str) -> bool) -> Vec<String> {
        heuristic_entities_with(text, known)
            .into_iter()
            .map(|c| c.name)
            .collect()
    }

    #[test]
    fn unit_labels_with_head_nouns_are_entities() {
        // EX-01: head noun + letter/digit label; plain numbers keep the span rules.
        for (text, label) in [
            (
                "Apt 3B in the Harbor Building has a leaking pipe.",
                "Apt 3B",
            ),
            ("We met in Unit 5A today.", "Unit 5A"),
            ("Take the Gate B12 shuttle.", "Gate B12"),
            ("Room 101 is booked.", "Room 101"),
            ("Flat 4 needs paint.", "Flat 4"),
            ("Suite 210 is quiet.", "Suite 210"),
            ("Building 7 closed.", "Building 7"),
            ("Version 2 shipped.", "Version 2"),
        ] {
            let got = names(text, &|_| false);
            assert!(got.iter().any(|n| n == label), "{label:?} in {got:?}");
        }
        let got = names("Apt 3B in the Harbor Building has a leaking pipe.", &|_| {
            false
        });
        assert!(got.contains(&"Harbor Building".to_string()), "{got:?}");
        assert!(!got.contains(&"Apt".to_string()), "{got:?}");
        // Mid-sentence "Visited Apt 3B" must not glue the head onto a span.
        let got = names("We visited Apt 3B.", &|_| false);
        assert!(got.contains(&"Apt 3B".to_string()), "{got:?}");
        assert!(!got.contains(&"Visited Apt".to_string()), "{got:?}");
    }

    #[test]
    fn units_after_numbers_are_never_entities() {
        // EX-02: a unit after a number is a measure; acronyms stay.
        let got = names("The fresh DB is 1.2 MB and recall takes 120 ms.", &|_| {
            false
        });
        assert!(!got.iter().any(|n| n == "MB" || n == "ms"), "{got:?}");
        assert!(got.contains(&"DB".to_string()), "{got:?}");
        for unit in [
            "KB", "GB", "TB", "KiB", "MiB", "GiB", "MHz", "GHz", "km", "kg",
        ] {
            let text = format!("The cache is 512 {unit} in size.");
            let got = names(&text, &|_| false);
            assert!(!got.iter().any(|n| n == unit), "{unit} in {got:?}");
        }
        let got = names("Use SQL and MCP on the CPU with 2 GB RAM.", &|_| false);
        for kept in ["SQL", "MCP", "CPU", "RAM"] {
            assert!(got.contains(&kept.to_string()), "{kept} in {got:?}");
        }
        assert!(!got.iter().any(|n| n == "GB"), "{got:?}");
    }

    #[test]
    fn known_head_before_plural_noun_drops_the_compound() {
        // EX-03: a known first word + a never-seen compound + a plural noun.
        let cypher = |n: &str| n.eq_ignore_ascii_case("cypher");
        let got = names("Cypher Memory nodes are readable now.", &cypher);
        assert!(got.contains(&"Cypher".to_string()), "{got:?}");
        assert!(!got.contains(&"Cypher Memory".to_string()), "{got:?}");
        // Unknown head, or a compound already known, keeps the span.
        let got = names("Cypher Memory nodes are readable now.", &|_| false);
        assert!(got.contains(&"Cypher Memory".to_string()), "{got:?}");
        let both =
            |n: &str| n.eq_ignore_ascii_case("cypher") || n.eq_ignore_ascii_case("cypher memory");
        let got = names("Cypher Memory nodes are readable now.", &both);
        assert!(got.contains(&"Cypher Memory".to_string()), "{got:?}");
        // Real two-word names survive: a verb or other non-noun never splits them.
        let known = |n: &str| {
            ["bergen", "harbor", "alice"]
                .iter()
                .any(|k| n.eq_ignore_ascii_case(k))
        };
        for text in [
            "Bergen Airport opened in 1992.",
            "Harbor Building has a leak.",
            "Alice Smith lives in Oslo.",
        ] {
            let got = names(text, &known);
            let span = text.split(' ').take(2).collect::<Vec<_>>().join(" ");
            assert!(got.contains(&span), "{span:?} in {got:?}");
        }
    }

    #[test]
    fn calendar_and_duration_literals_are_not_entities() {
        // Each entry must never surface as an entity, either alone or inside
        // a sentence that also carries a real name.
        for literal in [
            "2025-03-05",
            "March 2025",
            "March 5, 2025",
            "Dec 2025",
            "Jan 3",
            "Monday",
            "Mondays",
            "Tuesdays",
            "Q1 2026",
            "2025",
            "in 2019",
            "3 days ago",
            "two weeks",
            "15 minutes",
            "3 years",
            "last week",
            "next month",
            "yesterday",
            "10:30",
            "every Monday",
            "September 2026",
        ] {
            for text in [literal.to_string(), format!("Alice met Bob on {literal}.")] {
                let names: Vec<String> = heuristic_entities(&text)
                    .into_iter()
                    .map(|c| c.name)
                    .collect();
                let norm = norm_name(literal);
                assert!(
                    names.iter().all(|n| norm_name(n) != norm),
                    "{literal:?} became an entity in {text:?}: {names:?}"
                );
                assert!(
                    !names.iter().any(|n| is_date_like_name(n)),
                    "{literal:?} leaked a date-like entity in {text:?}: {names:?}"
                );
            }
        }
        // Product names and people with numbers survive.
        let names: Vec<String> = heuristic_entities("Windows 2000 and Postgres 16 run here.")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert!(names.iter().any(|n| n == "Windows 2000"), "{names:?}");
        assert!(names.iter().any(|n| n == "Postgres 16"), "{names:?}");
    }

    #[test]
    fn table_test_heuristic_entities_kept_vs_rejected() {
        struct Case {
            input: &'static str,
            kept: &'static [&'static str],
            rejected: &'static [&'static str],
        }

        let cases = [
            Case {
                input: "Replay lane E: CLOSED. F PASS 7, D 11 merged.",
                kept: &[],
                rejected: &[
                    "Replay", "CLOSED", "F", "PASS", "7", "D", "11", "merged", "F PASS 7", "D 11",
                ],
            },
            Case {
                input: "Merged lane-a into main; Full suite PASS.",
                kept: &[],
                rejected: &["Merged", "Full", "PASS"],
            },
            Case {
                input: "Remaining: fix live_filter test in remem-store",
                kept: &["live_filter"],
                rejected: &["Remaining"],
            },
            Case {
                input: "Unpark rule: check dependencies.",
                kept: &[],
                rejected: &["Unpark"],
            },
            Case {
                input: "PASS FAIL FAILED OK DONE CLOSED OPEN TODO FIXME NOTE WARN WARNING ERROR \
                        NEW FULL RED GREEN YES NO CLEAN DIRTY READY WIP NA TBD AND OR NOT THE ALL ANY \
                        SQL MCP RRF CPU GPU API DB NYC AWS",
                kept: &["SQL", "MCP", "RRF", "CPU", "GPU", "API", "DB", "NYC", "AWS"],
                rejected: &[
                    "PASS", "FAIL", "FAILED", "OK", "DONE", "CLOSED", "OPEN", "TODO", "FIXME",
                    "NOTE", "WARN", "WARNING", "ERROR", "NEW", "FULL", "RED", "GREEN", "YES",
                    "NO", "CLEAN", "DIRTY", "READY", "WIP", "NA", "TBD", "AND", "OR", "NOT",
                    "THE", "ALL", "ANY",
                ],
            },
            Case {
                input: "Alice called Bob. Later Alice returned.",
                kept: &["Alice", "Bob"],
                rejected: &[],
            },
            Case {
                input: "Reports indicated starting was full.",
                kept: &[],
                rejected: &["Reports", "Starting", "Full"],
            },
            Case {
                input: "ReMem and PostgreSQL run on k8s.",
                kept: &["ReMem", "PostgreSQL", "k8s"],
                rejected: &[],
            },
            Case {
                input: "Postgres 16 is deployed. We love Postgres 16.",
                kept: &["Postgres 16"],
                rejected: &[],
            },
            Case {
                input: "We tested Lane E. Lane E was closed.",
                kept: &["Lane E"],
                rejected: &[],
            },
            Case {
                input: "We deployed to CLOSED Lane E today.",
                kept: &["Lane E"],
                rejected: &["CLOSED", "CLOSED Lane E"],
            },
            Case {
                input: "Report: F PASS 7 and D 11.",
                kept: &[],
                rejected: &["Report", "F PASS 7", "D 11", "F", "PASS", "7", "D", "11"],
            },
            Case {
                input: "Alice Johnson met SQLite in New York City.",
                kept: &["Alice Johnson", "SQLite", "New York City"],
                rejected: &[],
            },
            Case {
                input: "LANES MERGED POSTGRES /session model ./run.sh /var/log/app.log BUILD42",
                kept: &["./run.sh", "/var/log/app.log", "BUILD42"],
                rejected: &["LANES", "MERGED", "POSTGRES", "/session", "model"],
            },
        ];

        for case in cases {
            let actual = heuristic_entities(case.input);
            let actual_names: Vec<&str> = actual.iter().map(|c| c.name.as_str()).collect();
            for expected in case.kept {
                assert!(
                    actual_names.contains(expected),
                    "expected kept entity {:?} missing from {:?} in input {:?}",
                    expected,
                    actual_names,
                    case.input
                );
            }
            for rejected in case.rejected {
                assert!(
                    !actual_names.contains(rejected),
                    "rejected entity {:?} unexpectedly present in {:?} in input {:?}",
                    rejected,
                    actual_names,
                    case.input
                );
            }
        }
    }
}
