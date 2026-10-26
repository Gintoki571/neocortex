# Mechanism card: ReMem (predecessor of neocortex)

## Provenance (read this first)

- `/run/media/bindesh/E/New Folder/rag/remem` is an empty git repo: `git status` reports "No commits yet", there are no refs, no packs and no files. I read nothing from its working tree.
- The ReMem source lives in `/run/media/bindesh/E/New Folder/rag/remem-v31` at tag **`remem-final`** (commit `20911b7`, "docs: add DOX framework header to AGENTS.md (pre-neocortex snapshot)", 2026-10-22). It is the Rust v3 workspace at `crates/`. I read it with `git show remem-final:<path>` (read-only). Nothing was written to any repo.
- Tag `v3.0` = commit `a9d18f6` ("retire v2 TypeScript tree, Rust is main"). The v2 TypeScript source (`src/recall.ts`, which `rank.rs` says it was ported from) is not in any tag or branch I could list, so the v2 behaviour is unavailable here.
- Citation convention: `remem-final:crates/<crate>/src/<file>.rs:LINE`. Line numbers are those of that blob. Scratch copies were written only under the session scratchpad.
- Neocortex (for the comparison in section 9) is `remem-v31` HEAD `367b9a9`, `src/`, SCHEMA_VERSION 4 (`src/db.rs:15`).
- `(inference)` marks my reasoning. Skipped: CUDA probe, bench examples, fuzz and soak reports, MCP wiring, CLI argument plumbing, tests except where a test pins a number.

## 1. What problem it solves

A single-file (SQLite) long-term memory for LLM agents, with no LLM in the write or read path. The agent writes facts, decisions and preferences with `remember`, and reads them back with `recall`. Recall must abstain on junk queries and must not return stale corrections. Crates: `remem-types` (data), `remem-store` (SQLite + FTS5 + sqlite-vec), `remem-graph` (graphqlite), `remem-embed` (local BERT, 768-d), `remem-recall` (ranking and engine), `remem-mcp` (MCP stdio). Design contract: `crates/SPEC.md`.

## 2. Data model

- **MemoryItem** (`remem-types/src/lib.rs:40-60`): `id`, `kind` (enum, `lib.rs:5`), `content`, `tags: Vec<String>`, `agent_id`, `session_id`, `importance: f32` (clamped, `lib.rs:88`), `created_at`, `updated_at`, `occurred_at: Option<i64>` (event time, `lib.rs:53`), `ended: bool` (ended, date unknown, `lib.rs:59`).
- **event_time** = `occurred_at.unwrap_or(created_at)` (`lib.rs:108-110`).
- **RecallQuery** (`lib.rs:114-128`): `text`, `k`, optional `kinds`, `tags`, `agent_id`, `session_id`, `since`, `until`, `max_chars`. **RecallHit** (`lib.rs:131-134`): `item`, `score`, `reasons`.
- **Schema** (`remem-store/schema.sql`):
  - `memories` (`schema.sql:1-22`): as above, plus `content_hash`, `supersedes`, `superseded_at`, `deleted`, `ended`.
  - Partial unique index `idx_memories_content_hash ON memories(content_hash) WHERE deleted = 0 AND superseded_at IS NULL` (`schema.sql:26-27`). A superseded row frees its hash.
  - `forget_events(memory_id, forgotten_at)` (`schema.sql:31`), with `undo_forget` (`store/src/lib.rs:404`).
  - `memories_fts` = FTS5 `fts5(content, tags, tokenize='porter unicode61')` (`schema.sql:42`), with insert, update and delete triggers. Tags are a separate column so they can be weighted.
  - `mem_vec` = `vec0(embedding FLOAT[768])` (`schema.sql:58`).
  - `merges(absorbed_id, survivor_id, absorbed_content, absorbed_at)` plus `merges_fts` (`schema.sql:64-`): absorbed phrasings stay searchable.
  - `suggested_edges(from_id, to_id, query, rank, ...)` with a unique pair index (`schema.sql:83-93`): recall-suggested link candidates, never auto-applied.
- **Graph** (`remem-graph/src/lib.rs`): `Memory` nodes keyed by `mid`, hub nodes `Agent` and `Session` (ids namespaced `agent:` and `session:`, `lib.rs:87-89`). Default edge `RELATES_TO` (`lib.rs:81`). Every edge has a `provenance` property from `EdgeProvenance` (`lib.rs:20-60`: manual, recall-suggested, correction-chain, extracted, temporal, auto-link). Graph is `graphqlite 0.8` on the same file (SPEC: must coexist with sqlite-vec).
- **Embedder** (`remem-embed/src/lib.rs:1-16`): BERT via candle, CPU or CUDA. Mean-pool over real tokens, then L2-normalize. `MAX_LEN = 512`. Default model dir `cadet-embed-base-v1` (768-d).
- Vectors are L2-normalized, so `cos = 1 - d²/2` for knn distance `d` (stated at `recall/src/lib.rs:~785` and `:~490`).

## 3. Write path (`remember`, `remem-recall/src/lib.rs:274-342`)

Steps in order:

1. **Embed** the content (`embed_one`, `lib.rs:274-276`).
2. **Exact dup check**: `find_by_hash(content_hash(kind, content))` (`lib.rs:279`). `content_hash` = SHA-256 over `kind`, a newline, and the content with whitespace collapsed and lowercased (`store/src/lib.rs:705-717`). A dup skips the probe below.
3. **Near-dup probe**: `knn(vec, SIMILAR_PROBE_K = 3)` (`lib.rs:84`, `:286-290`), keep neighbours with L2 distance `<= SIMILAR_MAX_DISTANCE = 0.48` (`lib.rs:79`). That is cosine about 0.885. The calibration comment (`lib.rs:60-78`) reports 4 paraphrase pairs at 0.42-0.63 L2 and the closest non-paraphrase pair at 0.518. The result is returned to the writer, not applied.
4. **Corroboration merge gate** (`lib.rs:296-327`): a neighbour absorbs the write only when it is the **unique** near neighbour whose distinctive objects are a subset of the new item's objects. Distinctive objects (`lib.rs:25-40`) are the tags, digit-only tokens, and capitalized tokens after position 0. Zero or two-plus corroborators → a new row. A value correction (`8080` → `9090`) never reproduces the old object, so it always splits.
   On merge: `insert_merge(survivor, content, now)` (`lib.rs:317-323`, store `:647`). No new row, no new node. Return the survivor id.
5. **Insert** the row (`store.insert`), `set_embedding` (`lib.rs:330-333`), graph `attach` (`lib.rs:334`).
6. **Reference extraction** (`extract_references`, `lib.rs:346-390`): `[[uuid]]` or `remem://<uuid>` in content → one `references` edge per live target, provenance `extracted`. Unresolvable ids are ignored.
7. **Temporal links** (`link_temporal_near`, `lib.rs:392-484`): if `occurred_at` is set, link to other live memories whose `occurred_at` is within `WINDOW_SECS = 24*3600` (`lib.rs:400`), each node capped at `CAP = 20` TEMPORAL_NEAR edges (`lib.rs:401`). Edge type `TEMPORAL_NEAR`, provenance `temporal`.
8. **Auto-link** (`auto_link`, `lib.rs:485-521`): knn top `AUTO_LINK_PROBE_K = 3` (`lib.rs:92`). Candidates with cosine `>= AUTO_LINK_MIN_COSINE = 0.72` (`lib.rs:89`, cos computed at `lib.rs:~490`). **Exactly one** candidate → auto-link edge (`lib.rs:502`, provenance `auto-link`). More than one → `record_suggestion` rows only ("graphify unique-evidence" gate, comment at `lib.rs:88`).

Batch writes and remember-time LLM calls: none. The engine contains no model except the embedder. The writer is the agent (`docs/inspiration.md` idea 3: "v3 has no extraction LLM").

Not present in v3 (checked by `git grep` over `crates/` for louvain, leiden, communit, entity_extract, extract_entities: no hits):
- entity or relation extraction from text (a memory is one caller-supplied string; entities exist only as graph nodes the caller links);
- community detection or community summaries (these arrive in neocortex, `src/kg.rs:303`);
- chunking (each `remember` is one item; batch size is the caller's).

## 4. Read path (`recall`, `remem-recall/src/lib.rs:695-970`)

### 4a. Constants

| Name | Value | Where |
|---|---|---|
| `DEFAULT_RRF_K` | 30 (not textbook 60) | `rank.rs:11` |
| `DEFAULT_HALF_LIFE_DAYS` | 30 | `rank.rs:13` |
| `DEFAULT_COSINE_FLOOR` | 0.63 | `rank.rs:25` |
| `LIST_DEPTH_FACTOR` | 4 (candidate depth = 4k) | `lib.rs:97` |
| `GRAPH_EXPANSION_SEEDS` | 8 | `lib.rs:50` |
| `Weights` default | fts 1.0, vector 0.5, graph 1.0 | `lib.rs:~110-121` |
| `DEFAULT_MIN_SCORE` | 0.0 (off) | `lib.rs:62` |
| `TAG_MATCH_BOOST` | 2.0 | `rank.rs:154` |
| `TAG_MIN_PREFIX` | 4 | `rank.rs:161` |
| `PREFIX_MIN_LEN` / `PREFIX_LEN` | 5 / 4 | `store/src/lib.rs:774, 777` |

### 4b. Steps

1. **Content-free guard**: trim; empty or all-stopword query returns nothing (`lib.rs:703-707`, `is_content_free` `rank.rs:183`, stopwords `rank.rs:175-181`). No FTS or vector call happens.
2. **Allowed set**: `list(false)` filtered by `matches()` (kinds, tags, agent, session, since, until; `lib.rs:712-721`, `matches` at `lib.rs:1095`). Every list below is filtered to this set.
3. **FTS list**: `fts_search(text, 4k)` (`lib.rs:722-728`). The query is built by `fts_quote` (`store/src/lib.rs:790-815`):
   - split on whitespace, drop stopwords;
   - each kept token becomes `"tok"`, plus `OR "prefix4"*` when the token has at least 5 characters;
   - joined with **OR** (AND was the early bug; `docs/eval.md` is the old measurement).
   - if nothing survives, every token is quoted and OR-joined.
   Merge hits (`fts_search_merges`, `lib.rs:731-735`) are appended after direct hits so they never outrank them.
4. **Vector list** (`lib.rs:742-792`): `knn(qvec, 4k)` filtered to allowed. Two abstention and pruning rules, with `cos = 1 - d²/2`:
   - **R1 abstain**: with no FTS hit at all, and the nearest vector cosine `< 0.63`, return `[]` (`lib.rs:765-771`).
   - **R2 prune**: with a strong lexical anchor (the top FTS item's content has cosine `>= 0.63` to the query, `lib.rs:776-778`), drop vector candidates below 0.63 (`lib.rs:779-786`). A weak anchor prunes nothing, so tag-only or graph-only recall survives.
5. **Graph list** (`lib.rs:800-840`): seeds = top `min(k, 8)` of RRF over fts and vector. For each seed, one hop over `neighbors_detail`, keeping only `Memory`-labelled, allowed nodes, and skipping `auto-link` edges. Graph list gets weight 1.0. Each graph-reached id remembers its seed (`edge_from`) and edge provenance (`edge_prov`).
6. **Fusion**: `fuse(lists, k=30)` (`rank.rs:84-115`). Score = `Σ_list weight_list × 1/(k + rank + 1)` (`rank.rs:70-72`, rank zero-based). Ties keep first-seen order. Reasons `fts#r`, `vector#r`, `graph#r`.
7. **Final score** (`lib.rs:868-900`): `score = fused × (0.9 + 0.1 × recency)`, recency = `0.5 ^ (age_days / 30)` on `event_time()` (`rank.rs:76-82`, `rank.rs:144-146`). Importance is **not** in the formula (`rank.rs:~131-139` explains: 35/37 with it off vs lower with it on). Reasons: `recent` when recency > 0.9, `important` when importance >= 0.8, `prov:<edge>` for graph hits.
8. **Sort, truncate to k, floor**: `apply_floor(min_score)` (`rank.rs:241`). Default 0.0 means off.
9. **Tag boost** (`tag_boost`, `rank.rs:210-228`; applied `lib.rs:925-940`): only for hits with no FTS presence. A query word and a tag match when their first 4 lowercase characters agree and both are at least 4 chars. Multiplier `2.0`, reason `tag`. Capped at one factor per hit.
10. **Re-sort** and **budget pack** (`pack_by_budget`, `rank.rs:120-135`): keep rank order, skip hits that do not fit `max_chars`, always keep the top hit so a matching query is never empty.
11. Return `RecallHit { item, score, reasons }`.

### 4c. Reported numbers (all from the repo docs, fixture-based)

- **Fixture set**: `docs/eval-fixtures.json`, 40 memories across 6 kinds and 7 agents, 37 answerable queries plus 3 adversarial (pure stopwords, `expect: ""`). Metric: recall@1 by content substring. Embedder: CPU 768-d.
- **Weight spike** (`docs/weight-spike.md`): a grid of 36 configs. Top configs tie at **35/37** recall@1, for example RRF k 30, w_fts 1.0, w_vec 0.5, importance, recency and tag off (config #1). The shipped default (1.0/1.0) scored 31/37.
- **Ranking study** (`docs/ranking-study.md`, cited by `RELEASE-v3.0.md`): pure 1:1 RRF scored 34/37 @1, fused with multipliers 31/37 @1. Vector alone 34/37.
- **Release v3.0** (`docs/RELEASE-v3.0.md`): fused 31/37 @1 (84%), 37/37 @5. With floor `--min-score 0.02`: 31/37 @1, 35/37 @5, adversarial 3/3 suppressed.
- **Temporal eval** (`docs/temporal-eval.md`, 17-memory subset): recall@1 default 5/7, recency-flat 6/7, all-old 5/7. recall@5 7/7 in every arm.
- **Latency** (`docs/RELEASE-v3.0.md`, `docs/perf.md`): `remember` avg 248 ms (n=100), `recall --k 5` avg 241 ms (n=50), `stats` 150 ms. Per-process startup is about 150 ms and one CPU embed about 90-100 ms.
- **Early baseline** (`docs/eval.md`, 25 queries, before the OR fix and weight tuning): recall@1 4/25, recall@5 16/25.

Caveat: the weights were chosen by grid search on the same 37 queries they are scored on. `docs/weight-spike.md` and the code comments both say to re-check on a new corpus. The 0.63 floor is calibrated on the same fixture (`rank.rs:17-24`).

## 5. Time handling

- Two clocks: `created_at`/`updated_at` (when stored) and `occurred_at` (when it happened). `event_time()` (`lib.rs:108-110`) drives recency.
- Date window: `since`/`until` filters on `RecallQuery` (`lib.rs:122-124`), applied through `matches()`.
- `ended` bool (`lib.rs:59`, schema `ended` sentinel, "ended, date unknown").
- Temporal link: TEMPORAL_NEAR within 24 h, cap 20 (section 3, step 7).
- Recency band is narrow on purpose: 0.9 to 1.0 multiplier (`rank.rs:~150-152`), because a 178-day-old row should not beat a relevant one (`docs/temporal-eval.md`).

## 6. Dedup and identity

- Exact: content hash over live rows (`schema.sql:26-27`).
- Near: L2 0.48 probe, reported only (`lib.rs:79-84`).
- Corroborated merge: unique neighbour plus distinctive-object subset (`lib.rs:25-40, 296-327`). The absorbed phrasing is kept and searchable (`merges`, `merges_fts`, `lib.rs:647, 676`).
- Correction chain: `supersede(old, replacement)` (`lib.rs:562-586`, store `:293`) sets `superseded_at` on the old row, frees its hash, inserts the replacement, and adds a `SUPERSEDES` edge with provenance `correction-chain`. Superseded rows stay on disk and are hidden from reads.
- `suspect_supersedes` (`lib.rs:589-620`): reports SUPERSEDES edges whose live endpoints share no tags. A correction should share its topic markers.

## 7. Consolidation and forgetting

- **Forget** = soft delete (`store delete`, `store/src/lib.rs:274`; graph `forget`, `remem-graph/src/lib.rs:328`) plus a `forget_events` row (`schema.sql:31`). `undo_forget` (`store/src/lib.rs:404`) reverses the latest event.
- No decay of stored memories. The only time effect at read time is the 0.9 to 1.0 recency band.
- No consolidation job, no summarization.
- Graph hygiene: `graph_gaps` (`lib.rs:972`), `gc_ghost_nodes` (`lib.rs:1011`), `index_audit` (`lib.rs:1049`), `orphan_memories` (`remem-graph/src/lib.rs:635`), impact BFS with depth (`lib.rs:628-680`, default depth 3).

## 8. Cost

- LLM calls: **0**. Grep of `crates/` finds no LLM client. The only "llm" strings are a provenance label `llm-guess` in tests (`remem-graph/tests/provenance.rs:46`, `remem-recall/tests/edge_cli.rs:74`) and comments that say "no LLM".
- Embedder calls: one per write (`lib.rs:275`), one per recall (`lib.rs:~745`), and up to one more in R2 for the FTS anchor content (`lib.rs:~777`). So at most two per recall.
- Latency: about 240 ms per CLI op, dominated by process startup (about 150 ms) and the CPU embed. A persistent MCP process removes startup (`docs/latency-amortized.md`).

## 9. Comparison with neocortex (`remem-v31/src`, HEAD `367b9a9`)

### 9a. Kept, with changed numbers

| Mechanism | ReMem v3 (`remem-final`) | Neocortex (`src/`) |
|---|---|---|
| Exact content-hash dedup | sha256(kind, normalized content), partial unique index (`schema.sql:26`) | `content_hash` unique index (`db.rs:540`), dedup join at `db.rs:515-533` |
| Three ranked lists fused by RRF | fts, vector, graph (`lib.rs:800-870`) | fts, vector, graph, plus entity and relation channels (`recall.rs:748-855`) |
| RRF constant | 30 (`rank.rs:11`) | 5.0 (`engine.rs:98`) |
| Recency band | `0.9 + 0.1 × 2^(-age/half_life)` (`rank.rs:144`) | same form (`recall.rs:769`), half-life 90 d (`engine.rs:103`) |
| Supersession | `superseded_at` + correction-chain edge (`lib.rs:562`) | `valid_from`, `expired_at`, `superseded_by` (`db.rs`, `remember.rs:611`) |
| Temporal near links | 24 h window, cap 20 (`lib.rs:400-401`) | interval logic in `temporal.rs`; `tlink_window_hours = 24.0`, `tlink_cap = 20`, but `w_tlink = 0.0` (`engine.rs` Config defaults) |
| Eval fixtures | `docs/eval-fixtures.json` + `scripts/eval.sh` | `src/eval.rs` (`ACC_K = 10`, `eval.rs:14`) |

### 9b. Dropped in neocortex (absent from `src/`, checked by grep)

- **Tag-prefix boost** (`rank.rs:154-228`). No `tag_boost` or equivalent.
- **Cosine floor 0.63 and the R1/R2 rules** (`rank.rs:25`, `lib.rs:743-792`). Replaced by `ABSTAIN_COSINE = 0.18` (`recall.rs:20`), which is calibrated to UltraBERT, plus a support test that accepts any of lexical, entity, relation, path, or window support (`recall.rs:801-804`).
- **Near-dup report at L2 0.48** (`lib.rs:79`). Replaced by `near_dup = 0.97` cosine (`engine.rs:88`), used in `remember.rs:593` and `:610`.
- **Distinctive-object corroboration merge and `merges` table** (`lib.rs:25-40, 296-327`; `schema.sql:64-75`). Not present.
- **Unique-evidence auto-link at cos 0.72** (`lib.rs:485-521`, `AUTO_LINK_MIN_COSINE`). Not present. Neocortex links through entities instead.
- **suggested_edges** (`schema.sql:83-93`). Not present.
- **Importance excluded from score**. Neocortex re-adds importance as `0.9 + 0.2 × importance` (`recall.rs:770`).
- **min_score floor** (`lib.rs:62`). Not present.
- **Graph hygiene** (`graph_gaps`, `gc_ghost_nodes`, `index_audit`, impact BFS). Not present (grep finds none).
- **Forget audit and undo** (`schema.sql:31`, `store/src/lib.rs:404`). Not present (grep for `forget_event` and `undo` in `src/` finds none).
- **Char-budget packing** (`rank.rs:120`). Neocortex packs by tokens instead: `budget` default 1200 tokens (`recall.rs:410`) with `rank::pack` (`recall.rs:973`).
- **Merge-to-survivor semantics for a near-duplicate** (`lib.rs:315-323`). Neocortex keeps separate rows and links them.

### 9c. Added in neocortex (not in ReMem v3)

- LLM-free extraction of entities and relations (`extract.rs`, `extract_model.rs` patterns). Entity resolution (`resolve.rs`) with thresholds `entity_seed 0.55`, `pred_alias 0.80`, `entity_conflict 0.73`, `sim_edge 0.75` (`engine.rs`, Config defaults).
- Negation and retraction parsing (`remember.rs:19-72`).
- Personalized PageRank for overview (`recall.rs:1502`), Louvain communities (`kg.rs:303-304`), beam path search (`paths.rs:22-30`, BEAM 24, MAX_DEPTH 3, REVERSE_PENALTY 0.75).
- Weighted channels: bm25 1.0, vector 2.0, graph 1.5, temporal 1.0, semantic path 0.25 (`engine.rs` Config defaults; applied `recall.rs:816-821`).
- UltraBERT 150M embedder, 768-d, 6 heads (`ultrabert.rs:10-13`) replacing cadet BERT.
- Dependencies (`Cargo.toml`): graphqlite 0.12, rusqlite 0.40, sqlite-vec =0.1.9, candle-core 0.11, tokenizers 0.22.

## 10. What is weak or wrong in ReMem v3

- **Tuned on the test set.** The 37-query fixture drives weights, the RRF constant, the floor and the tag rule, and it is also the only evaluation. Numbers above are in-sample.
- **Paraphrase is the known gap.** `docs/eval.md` lists "why did we pick sqlite" and "how should the assistant phrase its replies" as misses. The 768-d embedder cannot separate paraphrases from same-domain noise, and the fixed 0.63 floor is "least bad", not separating (`rank.rs:17-24`, which says the junk and real score bands overlap).
- **Importance was removed, not fixed.** The formula drops a writer's importance signal rather than calibrating it.
- **Merge is lossy for the graph.** A corroborated merge records the phrasing in `merges` but does not add a node or edge for it, so graph hops from the absorbed phrasing do not exist.
- **Vector list is a fixed 4k depth** with no adaptive cut (`lib.rs:97` comment: "ponytail: constant depth, no adaptive recall until ranking quality is measured on real data").
- **Recall is per-process cold** in the CLI: about 150 ms of startup per call (section 8).
- **Temporal recall is weak on dates.** Recency is a 10% band, so "what happened in March" depends on the date window filter, not on ranking.
- **The graph hop is one hop** from the top seeds, so multi-hop questions are not reachable through it.

## 11. Steal list (ranked; neocortex files it would touch)

1. **Corroborated-merge gate with distinctive objects** (`remem-final:crates/remem-recall/src/lib.rs:25-40, 296-327`). Absorb a near-duplicate write only when it is the unique near neighbour and its tags, digits and proper nouns all reappear in the new item. A value correction then always splits. Touches `src/remember.rs` (write path, `:593, :610` near-dup logic) and `src/db.rs` (new `absorbed` table, following the `merges` schema at `schema.sql:64-75`). LLM-free.
2. **Abstention on a strong lexical anchor** (`lib.rs:743-792`, R1 and R2). With no lexical hit and a low nearest cosine, return nothing. With a strong lexical hit, drop low-cosine vector-only candidates. Neocortex's abstain is a single 0.18 check (`recall.rs:804`). Touches `src/recall.rs` (around `:801-804`) and `src/engine.rs` (new threshold, calibrated for UltraBERT). LLM-free.
3. **Auto-link only on unique evidence, otherwise suggest** (`lib.rs:485-521`, `AUTO_LINK_MIN_COSINE 0.72`, suggestions table `schema.sql:83-93`). Touches `src/remember.rs` (after entity write) and `src/db.rs` (suggestion table). Gives a human-reviewable candidate list without polluting the graph. LLM-free.
4. **Tag-prefix boost for FTS-absent hits** (`rank.rs:154-228`, factor 2.0, 4-char prefix match). Recovers decisions whose wording appears only in tags (`decide` vs `decision`), which neither FTS nor the embedder sees. Touches `src/recall.rs` near the final sort (`:855-900`) and `src/text.rs` (prefix helper). LLM-free.
5. **Correction chains with a hash-freeing supersede** (`lib.rs:562-586`, store `:293`). The old row keeps its audit trail but frees its content hash, so the same fact can be re-learned later under a new id. Touches `src/db.rs` (`expired_at` and `superseded_by` columns) and `src/remember.rs:611`. LLM-free.
6. **Suspect-supersede report** (`lib.rs:589-620`). Flag supersession edges whose endpoints share no tags, as a mis-link probe. Touches `src/engine.rs` or `src/eval.rs` (a report command beside `stats`). LLM-free.
7. **Forget audit with undo** (`schema.sql:31`, `store/src/lib.rs:404`). Log each forget and allow an undo of the latest. Touches `src/db.rs` (new `forget_events` table) and `src/engine.rs` (forget entry point). LLM-free.
8. **Traceable merges** (`merges`, `merges_fts`, `schema.sql:64-`; `fts_search_merges` `store/src/lib.rs:676`). Keep absorbed phrasings searchable and add them after direct FTS hits so they never outrank them (`lib.rs:731-735`). Touches `src/db.rs` and `src/recall.rs` (FTS list, `:722-735`). LLM-free.
9. **Narrow recency band with event-time** (`rank.rs:76-82, 144-146`; `docs/temporal-eval.md`). Keep the 0.9 to 1.0 band so recency cannot override fusion, and drive it with `occurred_at` when present. Neocortex already has the form (`recall.rs:769`) but a 90-day half-life (`engine.rs:103`). Touches `src/engine.rs` (half-life) and `src/temporal.rs` (event clock). LLM-free.
10. **Calibration discipline** (`lib.rs:60-78`, `rank.rs:17-24`, `docs/weight-spike.md`). Record the calibration fixture, the grid and the held-out queries next to each threshold, and flag which numbers are in-sample. Touches `src/eval.rs` and the threshold comments in `src/engine.rs`. LLM-free.

## Summary of routes

- Write: embed -> exact hash -> near probe (L2 0.48, report only) -> unique-neighbour corroboration merge (distinctive-object subset) or insert -> reference edges (`[[uuid]]`) -> TEMPORAL_NEAR within 24 h (cap 20) -> unique auto-link at cos 0.72 or suggest.
- Read: content-free guard -> FTS (OR-joined, 4-char prefix arm) and merges -> knn 4k with R1 abstain and R2 prune at cos 0.63 -> one-hop graph expansion from top 8 seeds -> RRF k 30 with weights 1.0/0.5/1.0 -> score x (0.9 + 0.1 recency) -> tag boost x2 for FTS-absent hits -> char-budget pack keeping the top hit.
- Correction: supersede (free hash, SUPERSEDES edge, hidden from reads) -> suspect report. Forget: soft delete plus event row, undo of latest.
