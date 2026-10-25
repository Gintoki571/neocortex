# DOX framework

- DOX is highly performant AGENTS.md hierarchy installed here
- Agent must follow DOX instructions across any edits

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees
- Work products, source materials, instructions, records, assets, and durable docs must stay understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it

## Read Before Editing

1. Read the root AGENTS.md
2. Identify every file or folder you expect to touch
3. Walk from the repository root to each target path
4. Read every AGENTS.md found along each route
5. If a parent AGENTS.md lists a child AGENTS.md whose scope contains the path, read that child and continue from there
6. Use the nearest AGENTS.md as the local contract and parent docs for repo-wide rules
7. If docs conflict, the closer doc controls local work details, but no child doc may weaken DOX

Do not rely on memory. Re-read the applicable DOX chain in the current session before editing.

## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning AGENTS.md when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or quality
- AGENTS.md creation, deletion, move, rename, or index contents

Update parent docs when parent-level structure, ownership, workflow, or child index changes. Update child docs when parent changes alter local rules. Remove stale or contradictory text immediately. Small edits that do not change behavior or contracts may leave docs unchanged, but the DOX pass still must happen.

## Hierarchy

- Root AGENTS.md is the DOX rail: project-wide instructions, global preferences, durable workflow rules, and the top-level Child DOX Index
- Child AGENTS.md files own domain-specific instructions and their own Child DOX Index
- Each parent explains what its direct children cover and what stays owned by the parent
- The closer a doc is to the work, the more specific and practical it must be

## Child Doc Shape

- Create a child AGENTS.md when a folder becomes a durable boundary with its own purpose, rules, responsibilities, workflow, materials, or quality standards
- Work Guidance must reflect the current standards of the project or user instructions; if there are no specific standards or instructions yet, leave it empty
- Verification must reflect an existing check; if no verification framework exists yet, leave it empty and update it when one exists

Default section order:
- Purpose
- Ownership
- Local Contracts
- Work Guidance
- Verification
- Child DOX Index

## Style

- Keep docs concise, current, and operational
- Document stable contracts, not diary entries
- Put broad rules in parent docs and concrete details in child docs
- Prefer direct bullets with explicit names
- Do not duplicate rules across many files unless each scope needs a local version
- Delete stale notes instead of explaining history
- Trim obvious statements, repeated rules, misplaced detail, and warnings for risks that no longer exist

## Closeout

1. Re-check changed paths against the DOX chain
2. Update nearest owning docs and any affected parents or children
3. Refresh every affected Child DOX Index
4. Remove stale or contradictory text
5. Run existing verification when relevant
6. Report any docs intentionally left unchanged and why

## User Preferences

When the user requests a durable behavior change, record it here or in the relevant child AGENTS.md

## Child DOX Index

- No child AGENTS.md yet. `src/` modules are owned by this file (see Layout).

# neocortex AGENTS.md

## Purpose

Long-term memory for AI agents, v0.1.4. Rust, one crate, one binary `neocortex` (CLI + MCP stdio server). One SQLite file holds memories (FTS5 BM25 + sqlite-vec), an entity/relation knowledge graph (graphqlite projection) and bi-temporal facts. Embeddings: UltraBERT-Embed-150M on candle (pure Rust, CPU). No LLM inside, no Python at runtime (offline tooling only, in `scripts/` and `eval/extract/reference/`), no network.

Design (binding): `docs/superpowers/specs/2026-10-22-neocortex-design.md`. Plan: `docs/superpowers/plans/2026-10-23-neocortex-plan.md`. Pre-rewrite ReMem code: git tag `remem-final`.

## Ownership

- Orchestrator/validator: Claude (merges lanes, runs gates, owns this file and the design doc).
- Lanes (external harness agents) own only the files named in their brief, on branch `lane-<name>` in a git worktree under `../neocortex-lanes/<name>`.

## Local Contracts

- Version stays `0.1.4` until the user says otherwise.
- Dependencies: rusqlite (bundled), sqlite-vec `=0.1.9` (0.1.10-alpha does not compile), graphqlite `0.12`, candle-core/candle-nn `0.11`, tokenizers (`fancy-regex`, no default features), serde, serde_json. Adding any other crate needs a one-line reason here.
- Relational tables are the source of truth; graphqlite nodes/edges are a projection rebuilt by `neocortex reindex`.
- One connection per process (`db::Db`): sqlite-vec via `sqlite3_auto_extension`, graphqlite via `Connection::from_rusqlite`. Writes go through `Db::write` (BEGIN IMMEDIATE). Cypher runs inside the same transaction.
- Validity is half-open `[valid_from, valid_to)`; system retirement is `expired_at`. Times are unix seconds UTC.
- One live memory per `content_hash` (unique partial index); a hash hit only counts as a duplicate when kind + folded text match. A memory's end (`--until`, `invalidate m`, supersede) ends the relations it alone still evidenced; soft forget retracts them (`expired_at`, no end date); hard forget and merge patch the graph projection incrementally, never `rebuild_all`.
- ER-07 near-name dedup is layered: JW on the comparison key only (`text::translit_norm` is never stored), then hard blockers (`text::name_blocker`: differing digit runs, whole names < 4 chars, strict token-set subset, differing-token short labels < 4 chars, single-token prefix extension with a >= 2-letter tail, differing multi-token tokens both >= 4 chars and JW < 0.85), then the typo-edit gate (`text::name_typo_ok`: the differing token keys must be within a Damerau-Levenshtein allowance, <= 1 edit up to 8 chars and <= 2 beyond; "Wisniewski"/"Wisniowski" (1 edit, 10 chars) stays mergeable - see Config doc), and only then the entity-cosine gate reuses `entity_conflict` as a veto (semantic embedder only; hash embedders decide on the name layer alone; an agreeing cosine never causes a merge - "Bergen Airport South" is not "Bergen Airport" even when every vector agrees). At `name_merge = 0.92` (2026-10-25 measured, 51 labelled pairs in `tests/data/name_pairs.json` via `examples/name_eval`): recall 0.964, precision 0.964 - the one recall miss is "Aleksandr Ivanov"/"Alexander Ivanov" (4 key edits on 9-char tokens) which lands Near; the one false merge is "Wisniewski"/"Wisniowski" (1 edit on 10-char keys, name-layer isomorphic to Достоевский/Dostoevsky). `name_near = 0.85` splits Near (reported, never merged; over-allowance pairs land here even above `name_merge`) from Below (silent). Merge resolves with status "near" (alias origin "near"); Near/Blocked/Unsure create a new entity with status "ambiguous" + candidates, never silent.
- Q-08: `lookup_entity_fuzzy` gained a JW stage (after prefix, before vector) on the same `name_merge` threshold and blockers; it requires >= 4 chars and falls through when two entities win within 0.005.
- Embedding-only similarity never merges entities; it only adds `similar` soft edges.
- Ids shown to agents: `m<id>`, `e<id>`, `r<id>`; tools accept both forms.
- `recall` adds a query-directed path channel (`paths::search`, beam 24, depth 3, `PATH_WEIGHT` 2.5) plus a semantic multi-hop channel (`paths::search_semantic`: relation KNN k=100, residual-question hop scoring, >= 2-hop chains, weight `w_sempath` 0.25) and renders the path facts; it is skipped on negation and history-intent questions, and an empty recall is returned when no lexical, entity, relation or path support exists and the best cosine is below `ABSTAIN_COSINE` (0.18).
- Two optional channels are measured experiments and default to 0: `tlink` (`w_tlink`; temporal links are always written between memories within `tlink_window_hours` that share an entity, weight `max(0.3, 1-gap/24h)`, at most `tlink_cap` per endpoint) and `causal` (`w_causal`; only on why-intent questions, over the seeded `cause` group; `causal_extract` optionally adds one conservative in-memory text-cue relation when the agent supplied none).
- Access is recorded for every memory `recall` returns, in one `Db::write` transaction (`access_count`/`last_access` totals plus `memory_access` timestamps, capped by `access_keep`; `0` keeps no history). `w_retention` (default 0.05: neutral on fixtures/world/multihop-holdout2, +0.007 MRR on holdout) blends `rank::retention` into ranking; `stats.cold_memories` is a report-only hint (retention < 0.15).
- Communities are deterministic Leiden (`graph::leiden`, seed 7, resolution 1.0) over the live weighted snapshot (memories, entities, mentions, live relations, similar). Ids are remapped to the previous run by greedy member overlap, so `[cN]` stays put while the graph only grows. `overview` and the `entity` card read the table; recall scoring does not, so communities never move retrieval scores.
- `Config.hop_scorer` (`off` default | `on`; `eval --set hop_scorer=on`) replaces the semantic beam's residual cosine with the learned logistic hop scorer (`HOP_WEIGHTS`/`HOP_BIAS` in `src/paths.rs`, retrained only by `scripts/train_hop_scorer.py`). Kept off: no multihop gain (2026-10-26, `docs/research/multihop.md` section 8). Hop features are built only when the scorer or the dump is on.
- Text-only retraction: when a write has no relations and its text carries a `RETRACTIONS` cue, `remember` matches names (memory entities, heuristic candidates, alias n-grams) against live relations of that group and ends the single match at `valid_from` (reported in `invalidated`); several matches return `conflicts` and end nothing. The name must be the whole object of the cue: a determiner between cue and name, a possessive, a ditransitive object or a following noun rejects it, and only a clause boundary, a time expression or an employment tail ("to join", "for <Entity>") may follow. A hedge or report in the clause (`almost`, `might`, `thinks`, `said that`, ...) returns the match as `conflicts` and ends nothing. Plans (`will`, `going to`, `plan to`, `next <unit>`, `in N <unit>`) resolve a future `valid_from` via `temporal::future_window` or get the tag `planned`; `recall` drops `planned`/future rows unless the query asks for plans or its window starts in the future. Recurrence cues tag `recurring` and keep the rule text. Ordering/duration intent sorts memories and facts by `valid_from` and fills `duration` (calendar units, "ongoing since …").
- The embedder sets `RAYON_NUM_THREADS` to candle's thread count at model load when unset (gemm and rayon pools must agree; 2x faster).
- Env: `NEOCORTEX_EMBEDDER=hash` (tests only; a DB records its embedder and refuses another), `NEOCORTEX_DB` (default `~/.neocortex/neocortex.db`), `NEOCORTEX_MODEL_DIR` (default `~/.neocortex/models/UltraBERT-Embed-150M`), `NEOCORTEX_TZ` (`+HH:MM`, default system local offset), `NEOCORTEX_MAX_TOKENS` (default 1024), `NEOCORTEX_EXTRACTOR` (unset = off; `rules` = `src/extract_model.rs` relation/date tier, used only when a memory has no agent-supplied entities or relations), `NEOCORTEX_DUMP_HOPS=<file>` (appends one JSON line per candidate hop the semantic beam scores: features, evidence memory ids; training data only).

## Layout

- `src/db.rs` + `src/schema.sql` - connection, extensions, schema. The schema version lives in the `schema_meta` table (v4), never in `PRAGMA user_version`: graphqlite owns that pragma (it stamps 1 and its cypher executor refuses any other value). Migrations are an explicit ladder in `Db::init` (v4 owns every pre-f2 predicate shape: the committed baseline stamps v3 with a three-column `predicates` table); the seeded predicate ontology (21 rows: `predicates.display/group_id/inverse_of/symmetric/seeded`) is re-seeded idempotently, and seeded `exclusive` is not agent-settable. `memory_links` (E4a) and `memory_access` (E5) are additive tables ensured by `Db::ensure_additive_tables` on every v4 open; the schema version stays v4, and a missing `cause` anchor re-seeds the causal family so existing databases gain it. New databases declare `chunk_size=64` on all four vec0 tables (the 1024 default reserves 12.6 MiB on the first insert); existing databases keep the chunk size they were created with, since `CREATE VIRTUAL TABLE IF NOT EXISTS` never rewrites them.
- `src/embed.rs`, `src/ultrabert.rs` - Embedder trait, UltraBERT forward pass, HashEmbedder (tests). Cold CLI recall is dominated by the safetensors loader (`MmapedSafetensors::get` copies every tensor through `Tensor::from_slice`, ~604 MB) plus the tokenizer JSON parse (~155 ms); the fused q/k/v and gate/up tensors cost one more 306 MiB copy at load but save ~45 ms of forward time versus three separate gemms (40 paired CPU runs), so they stay.
- `src/text.rs` - normalisation, singularise, Jaro-Winkler, trigrams, FTS query building, predicate keys, content hash. Greek accents fold (tonos/dialytika, ς→σ, ё→е); `translit_norm` builds the ER-07 comparison key (Cyrillic→Latin, ю deliberately as "u", German ue/oe/ae digraphs, Slavic endings ky→ki/ow→ov); `name_blocker` is the ER-07 hard-blocker list.
- `src/temporal.rs` - civil dates, instant/window parsing, query windows, history intent, plan/recurrence/ordering/duration cues and calendar duration formatting.
- `src/extract.rs` - heuristic entity candidates, n-grams for alias lookup. S-03: Unicode-aware capitalisation (`is_acronym`, single capitals, `is_foreign_title_case`) keeps cased non-Latin scripts (Latin diacritics, Greek, Cyrillic, Armenian); caseless scripts (CJK, Arabic) are a documented limit. Calendar/clock/year/duration/ordinal/relative-time spans are rejected (`is_date_like_name`).
- `src/extract_model.rs` - optional deterministic relation/date tier (off by default). `causal_cue` is the conservative single-cue "A because B" extractor used by E4b. Scored by `examples/extract_eval.rs` on `eval/extract/gold.json` (128 memories; `cargo run --release --example extract_eval -- [gold] [predictions.json] [name]`). Reference predictions and runner scripts: `eval/extract/preds/`, `eval/extract/reference/`. Comparison and candle port plan: `docs/research/extraction.md`.
- `src/graph.rs` - CSR + ACL push personalized PageRank, deterministic Leiden community detection (seeded XorShift visit order, refinement with connected-component splits, aggregation carrying collapsed self-loop weight) and Newman modularity. `src/rank.rs` - RRF, MMR, cosine, packing, and `retention` (E5 score).
- `src/remember.rs`, `src/resolve.rs`, `src/kg.rs` - write pipeline, entity/predicate resolution, and rebuildable graph projection. `remember` and `remember_batch` share `prepare` + `write_prepared`; a batch chunk of <= 256 inputs is one embed call and one transaction with a SAVEPOINT per item. `write_prepared` also writes E4a temporal links. Entities are unique on `(norm, type, homonym)` (types lower-cased); homonym pools resolve by card cosine + memory-context cosine; inverse writes are stored in the canonical predicate direction, symmetric ones once with `src < dst`; negation (`polarity=0`) ends the same `(src, dst, group)` positive interval; exclusivity and conflicts key on the predicate group (`relations.subj`, `predicates.group_id`); the REL-19 exclusive-ending block queries the value side (the non-subject endpoint, so inverted predicates like "reports to" stored as "manages" end correctly) and asks `resolve::near_name`: Below ends the old fact, Merge skips silently, Near/Blocked/Unsure skip and push a `Conflict{relation_id, fact, valid_from}`; relation phrase vectors use `pred_alias = 0.80` with head/tail guards; stale `predicates_vec` is wiped by the v4 migration.
- `src/recall.rs` - ranked multi-channel memory, fact, and entity retrieval (base channels plus optional `tlink`, `causal` and retention blend), `overview` community cards (label = top-degree names, size, key predicates, date span, central facts) and the optional `community` field on `entity` cards; `lookup_entity_fuzzy` has a JW stage on the ER-07 `name_merge` threshold and blockers (Q-08); `src/eval.rs` - fixture-based retrieval evaluation and ablations.
- `src/paths.rs` - LLM-free query-directed path search over relations (lexical channel `path`) and the semantic beam (channel `sem`: residual-question hop scoring over recognised facts, plus text-only memories as prose hops, never passing through a question anchor), plus the agent tools `follow` (bounded traversal, `reached` flag) and `aggregate` (grouped counts with a `negative` bucket). Also owns negation cues (`has_negation_cue`, `negation_intent`), why-intent (`causal_intent`) and causal evidence (`causal_memories`).
- `tests/fixtures/ultrabert_fixture.json` - authoritative embedding reference (sentence-transformers 6.2.0, fp32).
- `src/engine.rs` - public API (Engine, input/output types = MCP tool schema), Config tunables; the measured 2026-10-23 thresholds live here (`pred_alias = 0.80` phrase aliasing, `entity_conflict = 0.73` description conflict: same min 0.7404 / different max 0.7275) plus the ER-07 near-name pair (`name_merge = 0.92`, `name_near = 0.85`, measured 2026-10-25). Also owns the experiment knobs (`w_tlink`, `tlink_window_hours`, `tlink_cap`, `w_causal`, `causal_extract`, `w_retention`, `access_keep`) and `Stats.cold_memories`.
- `src/mcp.rs` - MCP stdio server (JSON-RPC 2.0, 10 tools, LazyEmbedder). `src/render.rs` - compact text output. `src/main.rs` - CLI. `src/import.rs` - one-shot ReMem v3 import.
- `src/export.rs` - JSON Lines `export`/`import` (backup and embedder migration: re-ingest through remember). `src/bench.rs` - synthetic corpus + latency/size report (`neocortex bench --n N`). `src/eval.rs` - `neocortex eval` (ablation, failures, `--set`, `--sweep` on one ingestion).
- `tests/temporal_fixes.rs` - regression tests for the temporal/invalidation hunt (until/invalidate/forget ends, interval overlap, `--when` spans, text-date inference, budget and duplicate flags). `tests/resolution_typos.rs` - S-03/ER-07/Q-08/REL-19 integration tests: non-English candidate extraction, translit merge/near/ambiguous tiers, fuzzy query resolution, and the exclusive near-value ending.
- `tests/ops.rs` - export/import round trip, concurrent writer processes. `tests/world.rs` - batch == sequential on a mixed workload (ids, whole outputs, tables) including a negation, a seeded single-valued change and a non-exclusive conflict, batch error isolation, and the committed world fixture's key/answer integrity. `tests/write_path.rs` - the write-path/body contract: mentions, exclusivity and conflicts, homonyms + context picks + `distinct_from`, inverse/symmetric storage and both-side rendering, `merge_entities` (self-loops kept expired, no zero-length intervals, `force` for types), and the v1/v2/baseline-v3 migration ladder. `tests/conflict.rs` - same-day polarity flips and as-of retirement. `tests/links_access.rs` - E4a/E4b/E5: temporal-link weights and caps, no cross-entity or out-of-window link, the v4 additive migration, canonical `because of` direction, why-intent ordering, text-cue extraction, access recording/cap and concurrent recalls. `tests/communities.rs` - Leiden determinism, stable ids across a recompute and a new member, overview summaries, and the community field on entity cards.
- `tests/temporal_text.rs` - text-only retraction (TM-10), plans/recurrence (TM-11), ordering/duration (TM-12) and the date-literal entity filter (TM-13).
- `eval/fixtures.json` (tuning, 95 memories / 84 queries) + `eval/holdout.json` (independent check, 105 / 88) + `eval/world-300.json` (generated, 300 people / 3264 memories / 600 queries) + `eval/multihop-holdout.json` (independent multi-hop, 155 / 80; `w_sempath` was chosen on it) + `eval/multihop-holdout2.json` (second independent multi-hop set, 220 memories incl. 25% prose-only / 100 queries; written blind to the code) + `eval/README.md` (format, scoring incl. acc@10, tuning protocol: defaults change only if both sets agree, world protocol).
- `eval/extract/holdout.json` - independent held-out extraction set (150 memories, `content` + `entities` + `relations` + `valid_from`), scored by `examples/extract_eval.rs`; never tuned against, never read by extraction code.
- `examples/gen_world.rs` - deterministic (`--seed`) synthetic year of agent memory + ground-truth queries; the only writer of `eval/world-*.json`. Generated worlds over 2 MB stay out of git. `examples/agent_sim.rs` - iterative agent simulation (recall -> follow -> recall-leaf) over a world fixture and an ingested DB, scoring exactly like eval acc@10. `examples/measure_desc.rs` - threshold calibration probe (needs the real model; writes nothing). `examples/community_eval.rs` - Leiden vs graphqlite louvain modularity/communities/runtime on an ingested DB (`cargo run --release --example community_eval -- <db>`). `examples/name_eval.rs` - sweeps the ER-07 near-name threshold over the labelled pairs in `tests/data/name_pairs.json` (51 pairs incl. hard negatives) and prints per-pair JW/blocker/key-edit decisions (merge | near | no) at 0.92.
- `docs/okf/graphrag/` - OKF v0.2 knowledge bundle: every Awesome-GraphRAG paper, survey, benchmark and project as a concept with sources, footnoted claims and a `neocortex_relevance` verdict (adopt/adapt/watch/skip), plus cross-cutting `techniques/`. Machine-generated (`status: draft`); start at `index.md` ("Adopt first"). Regenerate indexes with a frontmatter scan, never by hand.
- `scripts/train_hop_scorer.py` - offline numpy logistic regression for the hop scorer. Inputs per source: an eval JSON, its `NEOCORTEX_DUMP_HOPS` dump (from `neocortex eval <set> --save-db <db>`) and `<db>.keys.json`; label = an evidence memory is an expected memory of the query. Splits by query (20% validation), picks L2 by validation log-loss, prints the Rust consts for `src/paths.rs`. Train only on fixtures and generated worlds (`gen_world` seeds other than 7); never on `eval/holdout.json`, `eval/multihop-holdout*.json` or `eval/world-300.json` (gates).
- `docs/research/multihop.md` - measured multi-hop/aggregation/negation/abstention results: path-channel ablations, fixtures/holdout gate, latency, agent simulation, recommendation.
- `docs/research/agent-memory/` - research program output (2026-10-25/26, machine-written, draft): `failure-analysis.md` (64 classified failures on mh1/mh2/holdout at 367b9a9, read-time vs write-time), `ideas.md` (merged and new ideas with hypothesis, mechanism, experiment and kill criterion; §D ranks them and holds lane briefs), `literature-map.md` + `papers/` (46 paper mechanism cards), `repos/` (hindsight, agentmemory, graphify, lightrag, ReMem cards). Paths inside cite `lanes/` worktrees and line numbers of their commit; re-check against current code before acting.
- `skills/neocortex/SKILL.md` - agent-facing usage protocol; `README.md` - user docs.

## Work Guidance

- TDD: failing test first, then the smallest fix (skills: test-driven-development, systematic-debugging; ponytail style: smallest complete change, no speculative abstraction).
- Write from the agent's perspective: compact outputs, short ids, forgiving inputs, errors that say what to do.
- Lanes never commit outside their branch, never push, never edit files outside their brief.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test            # model-dependent tests skip when the model dir is absent
cargo build --release && ./target/release/neocortex eval eval/fixtures.json --ablation   # gate: fixtures recall >= 0.95 / MRR >= 0.89; holdout (eval/holdout.json) recall >= 0.91 / MRR >= 0.78 (0.92 -> 0.91 on 2026-10-23: prose hops cost one synonym-trap query, +0.10 acc@10 on eval/multihop-holdout2.json; see docs/research/multihop.md section 7); multihop-holdout2 acc@10 >= 0.82
NEOCORTEX_EMBEDDER=hash ./target/release/neocortex bench --n 10000 --db target/b.db      # recall p50 <= 150 ms
./target/release/neocortex eval eval/multihop-holdout.json                                   # acc@10 >= 0.825
./target/release/neocortex eval eval/multihop-holdout2.json                                  # acc@10 >= 0.82
cargo run --release --example gen_world -- --people 300 --seed 7 --out eval/world-300.json   # must match the committed file byte for byte
./target/release/neocortex eval eval/world-300.json --save-db /tmp/world300.db              # per-category acc@10 / recall / MRR
cargo run --release --example agent_sim -- eval/world-300.json target/w300.db              # iterative agent accuracy (recall -> follow -> recall-leaf)
cargo run --release --example community_eval -- target/w300.db                             # Leiden modularity >= graphqlite louvain on the same graph
cargo run --release --example name_eval                                                    # ER-07 threshold sweep on tests/data/name_pairs.json; at 0.92 recall 0.964 / precision 0.964 (known false merge: Wisniewski/Wisniowski)
```
