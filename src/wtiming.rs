//! Write-path stage timers (`NEOCORTEX_WRITE_TIMING=<path>`): one TSV line per
//! stored memory (`item`) and one per batch chunk (`chunk`), so the cost of
//! each write step can be compared at 10k / 50k / 100k memories. Off by
//! default; when the variable is unset every mark is one thread-local check.
//!
//! Columns of an `item` line: `item id total_ms <STAGES>`; a `chunk` line is
//! `chunk n prepare_ms embed_ms write_ms savepoint_ms savepoint_begin_ms`,
//! where `savepoint_ms` is the summed SAVEPOINT..RELEASE span (it contains
//! the item's stage times) and `write_ms` minus it is the transaction commit.

use std::cell::RefCell;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

pub const STAGES: &[&str] = &[
    "knn",
    "mem_insert",
    "vec_insert",
    "entities",
    "mentions",
    "gazetteer",
    "graph",
    "tlinks",
    "relations",
    "retractions",
    "finalize",
];

pub const KNN: usize = 0;
pub const MEM_INSERT: usize = 1;
pub const VEC_INSERT: usize = 2;
pub const ENTITIES: usize = 3;
pub const MENTIONS: usize = 4;
pub const GAZETTEER: usize = 5;
pub const GRAPH: usize = 6;
pub const TLINKS: usize = 7;
pub const RELATIONS: usize = 8;
pub const RETRACTIONS: usize = 9;
pub const FINALIZE: usize = 10;

struct Acc {
    enabled: bool,
    acc: [f64; STAGES.len()],
    open: [Option<Instant>; STAGES.len()],
    item_start: Instant,
}

thread_local! {
    static ACC: RefCell<Acc> = RefCell::new(Acc {
        enabled: std::env::var_os("NEOCORTEX_WRITE_TIMING").is_some(),
        acc: [0.0; STAGES.len()],
        open: [None; STAGES.len()],
        item_start: Instant::now(),
    });
}

static OUT: Mutex<Option<std::fs::File>> = Mutex::new(None);

fn emit(line: &str) {
    let mut out = OUT.lock().unwrap_or_else(|e| e.into_inner());
    if out.is_none() {
        let Some(path) = std::env::var_os("NEOCORTEX_WRITE_TIMING") else {
            return;
        };
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(f) => *out = Some(f),
            Err(_) => return,
        }
    }
    if let Some(f) = out.as_mut() {
        let _ = writeln!(f, "{line}");
    }
}

pub fn begin_item() {
    ACC.with(|a| {
        let mut a = a.borrow_mut();
        if a.enabled {
            a.item_start = Instant::now();
        }
    });
}

/// Emit the finished item's line and start a fresh accumulator.
pub fn end_item(id: i64) {
    let line = ACC.with(|a| {
        let mut a = a.borrow_mut();
        if !a.enabled {
            return None;
        }
        let total = a.item_start.elapsed().as_secs_f64() * 1e3;
        let mut line = format!("item\t{id}\t{total:.3}");
        for v in a.acc.iter_mut() {
            line.push_str(&format!("\t{v:.3}"));
            *v = 0.0;
        }
        Some(line)
    });
    if let Some(line) = line {
        emit(&line);
    }
}

pub fn begin(stage: usize) {
    ACC.with(|a| {
        let mut a = a.borrow_mut();
        if a.enabled {
            a.open[stage] = Some(Instant::now());
        }
    });
}

pub fn end(stage: usize) {
    ACC.with(|a| {
        let mut a = a.borrow_mut();
        if !a.enabled {
            return;
        }
        if let Some(start) = a.open[stage].take() {
            a.acc[stage] += start.elapsed().as_secs_f64() * 1e3;
        }
    });
}

pub fn chunk(
    n: usize,
    prepare_ms: f64,
    embed_ms: f64,
    write_ms: f64,
    savepoint_ms: f64,
    savepoint_begin_ms: f64,
) {
    if std::env::var_os("NEOCORTEX_WRITE_TIMING").is_none() {
        return;
    }
    emit(&format!(
        "chunk\t{n}\t{prepare_ms:.3}\t{embed_ms:.3}\t{write_ms:.3}\t{savepoint_ms:.3}\t{savepoint_begin_ms:.3}"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_indices_match_names() {
        assert_eq!(STAGES.len(), 11);
        for (i, name) in [
            (KNN, "knn"),
            (MEM_INSERT, "mem_insert"),
            (VEC_INSERT, "vec_insert"),
            (ENTITIES, "entities"),
            (MENTIONS, "mentions"),
            (GAZETTEER, "gazetteer"),
            (GRAPH, "graph"),
            (TLINKS, "tlinks"),
            (RELATIONS, "relations"),
            (RETRACTIONS, "retractions"),
            (FINALIZE, "finalize"),
        ] {
            assert_eq!(STAGES[i], name);
        }
    }
}
