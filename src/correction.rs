//! Correction hints (H3-04/H3-05): a strict text rule that says a newer
//! memory may correct an older one. It only proposes; no memory is changed.
//!
//! Rule (`is_correction`): the new text has an explicit correction cue, the
//! old text's numeric value is restated in the new text together with a value
//! the old text lacks, and the two share subject words (>= 2 and >= 40% of
//! the smaller content set). Years, ids and build/version numbers are not
//! values. Known misses by design: corrections without a number ("Ines, not
//! Dima") and without a cue ("moved to").

use crate::db::Db;
use crate::engine::PossibleSupersede;
use crate::Result;
use rusqlite::params;

/// Words after which a number is an id, build or version, not a value.
const ID_WORDS: &[&str] = &[
    "build", "version", "v", "release", "rc", "ticket", "issue", "pr", "bug", "commit", "rev",
    "revision", "id", "rfc", "cve", "sprint", "incident", "step",
];

/// Cue words that are never subject words.
const CUE_WORDS: &[&str] = &["correction", "actually", "instead", "changed", "longer"];

/// True when `new` corrects `old`. See the module docs for the rule.
pub fn is_correction(new: &str, old: &str) -> bool {
    let nt = words(new);
    let ot = words(old);
    if !has_cue(&nt) {
        return false;
    }
    let nv = values(&nt);
    let ov = values(&ot);
    if !ov.iter().any(|v| nv.contains(v)) || nv.iter().all(|v| ov.contains(v)) {
        return false;
    }
    // Two texts that each name a different entity ("Lisbon office" vs "Porto
    // office") are about different subjects, whatever number they share.
    let (nn, on) = (names(new), names(old));
    if !nn.is_empty() && !on.is_empty() && !nn.iter().any(|n| on.contains(n)) {
        return false;
    }
    let nc = content(&nt);
    let oc = content(&ot);
    let shared = nc.iter().filter(|w| oc.contains(w)).count();
    shared >= 2 && shared * 5 >= nc.len().min(oc.len()) * 2
}

/// Capitalised words, lowercased, possessive `'s` removed. A sentence-initial
/// word counts unless it is a common opener; cue words are not names.
fn names(s: &str) -> Vec<String> {
    const OPENERS: &[&str] = &[
        "the", "a", "an", "this", "that", "these", "those", "we", "our", "it", "its", "there",
        "on", "at", "in", "for", "if", "with", "after", "before", "when", "no", "not", "also",
    ];
    let mut out: Vec<String> = Vec::new();
    for (i, raw) in s.split_whitespace().enumerate() {
        let w = raw.trim_matches(|c: char| !c.is_alphanumeric());
        let w = w
            .strip_suffix("'s")
            .or_else(|| w.strip_suffix("\u{2019}s"))
            .unwrap_or(w);
        let lower = w.to_lowercase();
        let capital = w.chars().next().is_some_and(|c| c.is_uppercase());
        let skip =
            CUE_WORDS.contains(&lower.as_str()) || (i == 0 && OPENERS.contains(&lower.as_str()));
        if capital && w.chars().count() >= 2 && !skip && !out.contains(&lower) {
            out.push(lower);
        }
    }
    out
}

/// Lowercased tokens with edge punctuation removed; `#` is kept so "#123" stays an id.
fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '#')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn has_cue(t: &[String]) -> bool {
    t.iter().any(|w| w == "correction" || w == "actually")
        || t.windows(2).any(|p| {
            (p[0] == "instead" && p[1] == "of")
                || (p[0] == "changed" && p[1] == "from")
                || (p[0] == "no" && p[1] == "longer")
        })
        || t.iter()
            .enumerate()
            .any(|(i, w)| w == "not" && t[i + 1..(i + 6).min(t.len())].iter().any(|x| x == "but"))
}

/// The numeric value of a token: a leading run of digits and `.:-` ending in
/// a digit, followed by nothing or letters ("8mb", "99.9", "01:00").
fn numeric_run(w: &str) -> Option<&str> {
    let run_end = w
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ':' | '-')))
        .unwrap_or(w.len());
    let (run, rest) = w.split_at(run_end);
    let ok_rest = rest.chars().all(|c| c.is_ascii_alphabetic());
    (run.ends_with(|c: char| c.is_ascii_digit()) && ok_rest).then_some(run)
}

fn values(t: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, w) in t.iter().enumerate() {
        let Some(run) = numeric_run(w) else { continue };
        let year = run.len() == 4 && run.chars().all(|c| c.is_ascii_digit()) && {
            let y: u32 = run.parse().unwrap_or(0);
            (1900..=2100).contains(&y)
        };
        let id = i > 0 && ID_WORDS.contains(&t[i - 1].as_str());
        if !year && !id {
            out.push(run.to_string());
        }
    }
    out
}

/// Distinct alphabetic words of length >= 3 that are not stopwords or cues.
fn content(t: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in t {
        if w.len() >= 3
            && w.chars().all(|c| c.is_ascii_alphabetic())
            && !crate::text::is_stopword(w)
            && !CUE_WORDS.contains(&w.as_str())
            && !out.contains(w)
        {
            out.push(w.clone());
        }
    }
    out
}

/// Whether `old` may be proposed as superseded: live, text-only (no relation
/// evidence) and not dated after `new_from`.
fn proposable(db: &Db, old: i64, new_from: i64) -> Result<Option<String>> {
    let row: Option<(String, i64)> = db
        .sql()
        .query_row(
            "SELECT content, valid_from FROM memories m
             WHERE m.id=?1 AND m.expired_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM relation_evidence e WHERE e.memory_id=m.id)",
            [old],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    Ok(row.and_then(|(c, from)| (from <= new_from).then_some(c)))
}

/// Older live memories that the new memory `new` corrects, newest first, at
/// most three. `new` itself is excluded. Changes nothing.
pub fn possible_supersedes(
    db: &Db,
    new: i64,
    content: &str,
    valid_from: i64,
) -> Result<Vec<PossibleSupersede>> {
    let mut out = Vec::new();
    for id in crate::remember::neighbour_candidates(db, content, &[])? {
        if id == new || out.len() == 3 {
            continue;
        }
        if let Some(old) = proposable(db, id, valid_from)? {
            if is_correction(content, &old) {
                out.push(PossibleSupersede {
                    memory_id: id,
                    snippet: snippet(&old),
                });
            }
        }
    }
    Ok(out)
}

/// The newest live memory that corrects the recalled memory `old`, if any:
/// the recall-side mirror of `possible_supersedes`.
pub fn later_correction(
    db: &Db,
    old: i64,
    old_content: &str,
    old_from: i64,
) -> Result<Option<i64>> {
    if proposable(db, old, old_from)?.is_none() {
        return Ok(None);
    }
    let mut best: Option<i64> = None;
    for id in crate::remember::neighbour_candidates(db, old_content, &[])? {
        if id <= old || best.is_some_and(|b| b >= id) {
            continue;
        }
        let row: Option<(String, i64)> = db
            .sql()
            .query_row(
                "SELECT content, valid_from FROM memories WHERE id=?1 AND expired_at IS NULL",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        if let Some((new_content, from)) = row {
            if from >= old_from && is_correction(&new_content, old_content) {
                best = Some(id);
            }
        }
    }
    Ok(best)
}

fn snippet(content: &str) -> String {
    const MAX: usize = 120;
    if content.chars().count() <= MAX {
        return content.to_string();
    }
    let mut s: String = content.chars().take(MAX - 1).collect();
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_needs_a_real_cue() {
        assert!(has_cue(&words("Correction: x")));
        assert!(has_cue(&words("not 500 but 400")));
        assert!(!has_cue(&words("not 500 at all, 400 now")));
        assert!(has_cue(&words("instead of 3 use 5")));
        assert!(!has_cue(&words("the limit is now 5")));
    }

    #[test]
    fn values_skip_years_ids_and_builds() {
        assert_eq!(
            values(&words("Build 13 now passes in 2026")),
            Vec::<String>::new()
        );
        assert_eq!(values(&words("ticket 4412 and m12")), Vec::<String>::new());
        assert_eq!(
            values(&words("it is 400 and 01:00 and 2026-11-20")),
            ["400", "01:00", "2026-11-20"]
        );
        assert_eq!(values(&words("8MB and 99.9 percent")), ["8", "99.9"]);
    }

    #[test]
    fn h3_04_a_and_h3_05_produce_the_candidate() {
        // hunt3 scen9 m18 over m17; scen11 m22 over m21 and m21 over m20.
        assert!(is_correction(
            "Correction to my June note: the Beacon API rate limit is 400 requests per minute, not 500.",
            "The Beacon API rate limit is 500 requests per minute per key after the second partner onboarding."
        ));
        assert!(is_correction(
            "Correction: the 8 MB figure was also wrong; production enforces 6 MB per request.",
            "Correction: the Lantern ingest API limit is 8 MB per request, the 4 MB figure was an old staging value."
        ));
        assert!(is_correction(
            "Correction: the Lantern ingest API limit is 8 MB per request, the 4 MB figure was an old staging value.",
            "Lantern's ingest API limit is now 4 MB per request."
        ));
    }

    #[test]
    fn h3_04_b_c_d_e_f_are_documented_misses() {
        // No old value in the new text, or no cue: outside the rule by design.
        assert!(!is_correction(
            "Correction again on 2026-06-11: the audit retention is 400 days to cover the leap-year rounding in the policy.",
            "Correction: the audit retention is 365 days, not 180. Ops confirmed on 2026-05-02."
        ));
        assert!(!is_correction(
            "The Harbor batch window moved to 22:00 to 00:00 UTC after the EU load grew.",
            "The Harbor batch window is 01:00 to 03:00 UTC."
        ));
        assert!(!is_correction(
            "The Lantern schema migration freeze was moved to 2026-11-27 for the holiday.",
            "The Lantern schema migration freeze begins 2026-11-20."
        ));
        assert!(!is_correction(
            "Correction: the portal launch date in my June note was wrong. The real date is 2026-12-05, after the second audit.",
            "The portal launch is now 2026-11-28 after the accessibility audit slipped."
        ));
        assert!(!is_correction(
            "Jo is back on the data team as a contractor, helping with schema migrations from 2026-10-01.",
            "Jo left the data team on 2026-07-15 to work on the mobile SDK."
        ));
    }

    #[test]
    fn different_named_subjects_are_not_corrections() {
        assert!(!is_correction(
            "Correction: the Lisbon office has 40 desks, not 30.",
            "Porto office opened with 30 desks."
        ));
        assert!(is_correction(
            "Correction: the Lisbon office has 40 desks, not 30.",
            "The Lisbon office opened with 30 desks."
        ));
    }

    #[test]
    fn build_and_version_controls_produce_none() {
        assert!(!is_correction(
            "Correction: build 13 now passes.",
            "Build 12 now passes."
        ));
        assert!(!is_correction(
            "Correction: we shipped version 2.4, not version 2.3.",
            "We shipped version 2.3 last week."
        ));
    }

    /// Precision and recall on tests/fixtures/correction_pairs.json. Recall is
    /// expected to miss the designed misses (no value, no cue).
    #[test]
    fn fixture_precision_and_recall() {
        let raw = include_str!("../tests/fixtures/correction_pairs.json");
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        let pairs = |key: &str| -> Vec<(String, String)> {
            v[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p[0].as_str().unwrap().into(), p[1].as_str().unwrap().into()))
                .collect()
        };
        let (tp, fn_) = pairs("true").iter().fold((0, 0), |(tp, fneg), (n, o)| {
            if is_correction(n, o) {
                (tp + 1, fneg)
            } else {
                (tp, fneg + 1)
            }
        });
        let fp = pairs("controls")
            .iter()
            .filter(|(n, o)| is_correction(n, o))
            .count();
        let precision = tp as f64 / (tp + fp).max(1) as f64;
        let recall = tp as f64 / (tp + fn_) as f64;
        for (n, o) in pairs("controls")
            .iter()
            .filter(|(n, o)| is_correction(n, o))
        {
            eprintln!("false positive: {n} <- {o}");
        }
        eprintln!("correction rule on fixture: tp={tp} fp={fp} fn={fn_} precision={precision:.3} recall={recall:.3}");
        assert!(
            precision >= 0.9,
            "precision {precision:.3} below the 0.9 kill line"
        );
        assert!(recall >= 0.6, "recall {recall:.3} below 0.6");
    }
}
