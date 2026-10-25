# Mechanism card: Hindsight (vectorize-io/hindsight)

Local clone: `/run/media/bindesh/E/New Folder/rag/hindsight/`. Core engine: `hindsight-api-slim/hindsight_api/engine/`.
Citations are `path:line` relative to `hindsight-api-slim/hindsight_api/engine/` unless the path is absolute-ish (e.g. `skills/hindsight-docs/...` relative to the clone root).

Caveats about this checkout (verified):
- `hindsight_api/config.py` is **not present** in the slim package (the modules import `...config.get_config`). Env defaults below come from the docs copy `skills/hindsight-docs/references/developer/configuration.md` (cited as `CFG:line`). Treat them as documented defaults, not read-from-code.
- The `.git` directory is a stub (no HEAD), so no commit id can be cited.
- No committed LongMemEval / LoCoMo result files exist in the repo. Published numbers in section 10 come from a web search (secondary), not from the repo.
- Design docs exist in two copies: `skills/hindsight-docs/references/developer/*.md` (used here, the most current) and `hindsight-docs/versioned_docs/version-0.9/`. They largely agree.

---

## 1. Problem solved

Agent long-term memory as a "memory bank" with three layers: raw **world/experience facts** (LLM-extracted from text), **observations** (LLM-consolidated beliefs with proof counts), and **mental models** (user-declared standing questions whose answers are re-written in the background). Retrieval is 4-way (semantic, BM25, graph, temporal) fused by RRF, optionally cross-encoder reranked, then combined with recency/temporal/evidence multipliers and packed into a token budget. Reflect is an agentic LLM loop over the same layers.

## 2. Data model

- `memory_units` rows: `text`, `context`, `fact_type` in {`world`, `experience`, `observation`}, `occurred_start`, `occurred_end`, `mentioned_at`, `event_date`, `embedding`, `proof_count`, `source_memory_ids`, `tags`, `updated_at`. (`memories/pg/writes.py:62-128`; `search/retrieval.py:561-564`; `consolidation/consolidator.py:2713-2723`.)
- `event_date` = `occurred_start` if set, else `mentioned_at` (`memories/pg/writes.py:81-83`). `mentioned_at` = the source document's date.
- `memory_links(from_unit_id, to_unit_id, link_type, weight, entity_id, bank_id)`; `link_type` ∈ {`temporal`, `semantic`, `caused_by`} written at retain; read also as `causes`, `enables`, `prevents` (legacy, `causal_links.py:11-16`; read in `search/retrieval.py:717`, `db/ops_postgresql.py:883`).
- `unit_entities` (fact↔entity), entities table with `canonical_name`, `entity_kind` (`entity_resolver.py`), co-occurrence table `entity_cooccurrences` (`engine/retain/link_utils.py:375` area, `entity_resolver.py:153-186`).
- `mental_models`: `source_query`, `content`, `trigger` (JSON: mode, refresh_cron, fact_types, tags), `structured_content`, `last_refreshed_source_query`; `mental_model_history` (capped per model, `CFG:1416`).
- Observation history table (capped, `CFG:2080`).
- Documents and chunks (raw text kept; `chunk_id = {bank}_{doc}_{index}`, `orchestrator.py:1232-1238`).

## 3. Write path (retain)

Entry: `retain/orchestrator.py:1194` `retain_batch` → `_extract_and_embed` (`:1054`) → `_process_extracted_facts` (`:1145`) → `_insert_facts_and_links` (`:536`, one DB transaction).

**Step W1, chunking (no LLM).** `RETAIN_CHUNK_SIZE` default 3000 chars (`CFG:1520`). Chunkers in `retain/fact_extraction.py:553-1035` (recursive separators, conversation turns, JSONL). Chunk mode skips the LLM entirely (`CFG` retain_extraction_mode `chunks`).

**Step W2, fact extraction (LLM, per chunk).** `fact_extraction.py:1852` `_extract_facts_from_chunk` with a structured-output schema. Default mode `concise` (`CFG`; prompt `_BASE_FACT_EXTRACTION_PROMPT` at `:1041`). Output per fact (`ExtractedFact`, `:255-295`): `what`, `when`, `where`, `who`, `why` (each `N/A` if absent), `occurred_start`/`occurred_end` (ISO), `fact_type` ∈ {`world`, `assistant`}, `entities: list[str]`, `causal_relations` (max 2 per fact, `target_index` must be an earlier fact index, `relation_type` only `caused_by`, `CAUSAL_RELATIONSHIPS_SECTION` at `:1316-1327`), `from_attachments`.
- The stored fact text is `what | when | where | who | why`-style (docstring `:160`); the fact-type split is by speaker, not grammar (`skills/hindsight-docs/references/developer/retain.md`, "Two Types of Facts").
- Causal extraction can be switched off with `RETAIN_EXTRACT_CAUSAL_LINKS` (default true, `CFG:1528`).
- Temporal fallback (LLM-free, regex): `_infer_temporal_date` (`fact_extraction.py:83-119`) maps "yesterday" → -1 day, "last week" → -7, "next month" → +30, etc., relative to `event_date`, applied only when the LLM left `occurred_start` empty (`:2099`, `:2977`).
- Speaker/context steering is a caller concern (`context` field), not code.

**Step W3, offsets (no LLM).** `_add_temporal_offsets` (`fact_extraction.py:3441-3465`) adds `i * SECONDS_PER_FACT` (`SECONDS_PER_FACT = 0.01`, `:2566`) to all temporal fields so same-timestamp facts keep a total order.

**Step W4, embedding (non-LLM model).** Embedding input is augmented, storage text is not (`retain/embedding_processing.py:15-46`):
- `"{fact} (happened in {date})"` or `"(happened from {d1} to {d2})"` when a span is present;
- then `" [{entity1, entity2}]"` appended when entities exist.
Default local embedder `BAAI/bge-small-en-v1.5` (`CFG:769`). `ProcessedFact.from_extracted_fact` (`retain/types.py:449-496`) rejects degenerate text (punctuation-only, `:440-446`).

**Step W5, entity resolution (no LLM; deterministic). `engine/entity_resolver.py`.**
Entities come from the extractor's `entities` list. Each mention is scored against existing candidates from a trigram probe:
- Candidate probe admits names with trigram Jaccard ≥ 0.15 (comment `:1151-1152`); in-memory pg_trgm-equivalent Jaccard (`:80-117`), padding of two spaces + one trailing space per word.
- Gate: `name_trigram_similarity ≥ merge_min_similarity` (constructor default `0.3`, `:369`, check `:1164`); token-wise compatibility (`_tokens_are_compatible`, `:132-149`; per-word `SequenceMatcher ratio ≥ 0.6` or prefix match, `:124`, `:127-129`). Single-word names exempt.
- Identical trigram sets → score 1.0 (`_IDENTICAL_TRIGRAMS = 1.0`, `:72`, `:1172-1178`).
- Otherwise score = `0.5·SeqRatio(name)` + `0.3·Σ_{shared nearby names} w(name) / |nearby|` + `0.2·max(0, 1 − Δdays/7)` (`:1183-1202`), where `w(name) = 1/√degree(name)` (`_cooccurrence_weight`, `:189-199`; degree = number of distinct co-occurring entities, so hubs like "user" are damped).
- Merge iff `round(best, 6) ≥ 0.6` (`:1215-1217`). Otherwise create.
- Label entities (`entity_labels` config) resolve by exact match only (`:1098-1116`).
- In-batch clustering of new names by trigram ≥ threshold (`_find_intrabatch_similar_pairs`, `:202-212`), capped at 250 names (`_INTRABATCH_MAX_NAMES`, `:67`).

**Step W6, insert (no LLM).** `fact_storage.insert_facts_batch` (`orchestrator.py:564`), then `unit_entities` link rows (`:581-589`).

**Step W7, temporal links (no LLM). `retain/link_utils.py:455-570`, entry `link_creation.py:16-33`.**
- Weight: `weight = max(0.3, 1 − time_diff_hours / time_window_hours)` (`link_utils.py:545`; same formula within batch at `:158`); `time_window_hours = 24` default (`:459`).
- Cross-batch neighbours come from `ops_postgresql.py:733-790`: for each new unit, the **20 nearest earlier** and **20 nearest later** units of the same `fact_type` by `event_date` (`ORDER BY event_date DESC/ASC LIMIT 20`, `ROW_NUMBER ... rn <= 20`). **No 24 h window is applied in this path** (see weaknesses).
- Only `(new_unit → neighbour)` rows are inserted here (`link_utils.py:546`), i.e. one direction.
- Within-batch pairs (`_within_batch_temporal_links`, `:101-162`) are bidirectional and **do** break at `time_window_hours` (`:156`).
- Cap: `MAX_TEMPORAL_LINKS_PER_UNIT = 20` per `from_unit_id` by weight (`:63`, `_cap_links_per_unit` `:71-98`, applied at `:554`). Ties at 0.3 (any gap > 16.8 h) are broken by list order.

**Step W8, semantic links (no LLM).** `link_utils.py:573-719` (`compute_semantic_links_ann`) and `:722-785` (within batch).
- Threshold `SEMANTIC_LINK_MIN_SIMILARITY` default **0.7** (`CFG:1406`), passed as `threshold=config.semantic_link_min_similarity` (`orchestrator.py:609`).
- Write-time `top_k=50` (`link_utils.py:579`, `:725`). Final ANN pass after the batch uses `top_k=20` (`orchestrator.py:1928`, comment "Recall uses at most 20 neighbors").
- Weight stored = cosine similarity clamped to [0,1] (`link_utils.py:712`, `:782`). Per-fact ANN uses the partial HNSW index per `fact_type` (`:591-596`).

**Step W9, causal links (LLM-provided, no LLM at write).** `link_creation.py:81-109` → `_write_causal_links_batch` (`link_utils.py:911-975`): for each `causal_relation`, insert `(from=this fact, to=target fact, link_type, weight=1.0)` (`:961`). Only `caused_by` is accepted for new writes (`CANONICAL_CAUSAL_LINK_TYPES`, `:879-886`; `causal_links.py:11-16`). Self-links dropped (`:958`). **Edge direction is effect → cause.**

**Step W10, entity links.** Written to `unit_entities`, not `memory_links`; the graph arm does a query-time self-join through them (`orchestrator.py:615-617`, `link_expansion_retrieval.py:7-9`).

**Step W11, consolidation trigger** after retain (see section 7).

## 4. Read path (recall)

Entry: `memory_engine.py:7044` `recall_async` → `search/retrieval.py:793` `retrieve_all_fact_types_parallel` → store `recall_unified` (`memories/pg/reads.py`) → fusion `memory_engine.py:7943-7958` → pre-cap `:7994-8008` → optional cross-encoder `:8053-8065` → combined scoring `:8114-8134` → min_scores `:8149-8162` → prefer-observation dedup → token packing (docs `retrieval.md`, Stage 4).

### 4.1 Arm budget
Per-arm LIMIT = `recall_budget` by `budget` level (fixed: low 100, mid 300, high 1000; `CFG:1446-1448`; adaptive: ratio × max_tokens clamped to [20, 2000], `CFG:1449-1453`). Semantic/BM25 are one UNION-ALL SQL per fact type (`retrieval.py:123-398`); `semantic_fetch = max(limit, GRAPH_SEED_LIMIT)` when graph seeding is on (`:208`).
- Semantic arm: cosine ≥ `SEMANTIC_MIN_SIMILARITY` (default **0.3**, `CFG:1403`) (`:195`).
- BM25 arm: `ts_rank_cd`-style score ≥ `BM25_MIN_SCORE` (default 0, `CFG:1407`); query tokenisation `tokenize_query` (`retrieval.py:34-40`); backends listed in `skills/hindsight-docs/references/developer/retrieval.md` ("Keyword Search"). This is **not** textbook BM25 on the native backend (docs say TF-IDF via `ts_rank_cd`).

### 4.2 Temporal arm (LLM-free by default)
- Constraint extraction: `search/temporal_extraction.py:64-76` uses `DateparserQueryAnalyzer` by default (rule-based `dateparser` + `temporal_periods.py`). `TransformerQueryAnalyzer` (T5, `query_analyzer.py:459-471`) is optional. Caller `temporal_window` overrides analysis (`retrieval.py:856-862`).
- Entry pool per fact type: top **60** by cosine among units whose time overlaps the window (`_TEMPORAL_POOL_SIZE = 60`, `:402`; SQL `:565-591`; overlap predicate `:574-582`). Similarity floor `temporal_semantic_min_similarity` = **0.1** (`CFG:1405`; passed `:892`).
- Coverage selection: window split into **8** buckets (`_TEMPORAL_COVERAGE_BUCKETS = 8`, `:404`); keep **10** entry points (`_TEMPORAL_ENTRY_POINTS = 10`, `:403`) by round-robin across populated buckets, best-similarity first in each tier (`_select_with_temporal_coverage`, `:412-458`). Degenerate single-bucket windows collapse to similarity order (`:437`, `:429-430`).
- Temporal proximity of a row to the window centre: `prox = 1 − min(|d − mid| / (span/2), 1)` (`:647-648`), where `d` = midpoint of `[occurred_start, occurred_end]` or whichever end exists (`:634-642`). Undated → 0.5 (`:650`).
- Spreading (BFS over links): batch 20 seeds (`:661`), `per_source_limit = 10` neighbours by weight (`:665`), `max_iterations = 5` (`:667`), link types `temporal, causes, caused_by, enables, prevents` with `weight ≥ 0.1` (`:717-718`). Causal boost: `causes/caused_by` ×2.0, `enables/prevents` ×1.5, else ×1.0 (`:766-771`). Propagated score `parent_temporal × weight × causal_boost × 0.7` (`:773`). Neighbour temporal score `max(neighbour_prox, propagated)` (`:774`); neighbour undated prox = 0.3 (`:763`). Expansion continues only if `combined > 0.2` (`:781`). Budget = recall budget minus entry points (`:660`).
- Spreading is disabled on Oracle (`:689`).

### 4.3 Graph arm (LLM-free)
Default retriever `link_expansion` (`CFG:1392`; `search/link_expansion_retrieval.py`).
- Seeds: top `GRAPH_SEED_LIMIT = 20` semantic rows with cosine ≥ `GRAPH_SEED_MIN_SIMILARITY` (default **0.3**, `CFG:1404`) (`link_expansion_retrieval.py:46`, `:49-105`; reused from the semantic arm at `retrieval.py:203-207, 391-396`).
- Entity expansion: shared-entity count per candidate via `unit_entities` self-join with per-entity LATERAL cap `LINK_EXPANSION_PER_ENTITY_LIMIT = 200` (`CFG:1393`); timeout 10 s (`CFG:1394`).
- Semantic expansion: `MAX(weight)` over `memory_links` with `link_type='semantic'`, **both directions** (`db/ops_postgresql.py:843-874`).
- Causal expansion: `link_type IN ('causes','caused_by','enables','prevents')`, **from seed to target only** (`ops_postgresql.py:875-888`).
- Merge, additive: `entity = tanh(0.5 · count)` (`link_expansion_retrieval.py:247`; values 0.46/0.76/0.91/0.96 for 1–4 shared entities, `:233-234`); `semantic = max weight` (`:252`); `causal = max weight` (`:257`); `score = entity + semantic + causal ∈ [0,3]` (`:260-262`); top `budget` by score (`:266`). The outer RRF uses this order.

### 4.4 Fusion (LLM-free). `search/fusion.py`
Plain **unweighted RRF**, `k = 60`:
```
score(d) = Σ_arms 1 / (60 + rank_arm(d))      # rank 1-indexed; arms: semantic, bm25, graph, temporal
```
(`fusion.py:29-85`, `k` default `:29`, `:85`; invoked `memory_engine.py:7957-7958`.) Raw arm scores are kept only for the trace (`:88-93`). Arms are **not** weighted in RRF; the docs say so (`retrieval.md`, "Within RRF, all four strategies are weighted equally").

### 4.5 Per-strategy boost (LLM-free). `search/recall_boost.py`
Config `RECALL_STRATEGY_BOOSTS` (empty by default, `CFG:1409`). Levels:

| level | rank_divisor | additive |
|---|---|---|
| low | 2 | 0.05 |
| medium | 4 | 0.2 |
| high | 8 | 0.5 |
(`recall_boost.py:97-100`.)
- Pre-cap (rank space): contribution of a boosted arm becomes `1/(k + rank/divisor)` instead of `1/(k+rank)` (`boosted_rrf_score`, `:104-128`); used to sort before the reranker cap (`memory_engine.py:8005-8006`). Authors' reason (`:27-47`): multiplying the RRF term by a weight breaks the head of the ranking.
- Post-scoring: `weight += Σ additive` for boosted arms that surfaced the candidate (`additive_strategy_boost`, `:131-147`; applied `memory_engine.py:8129-8134`).

### 4.6 Reranker pre-filter and cross-encoder (ML, not LLM)
- Cut to `RERANKER_MAX_CANDIDATES` = **300** by (boosted) RRF (`CFG:1399`; `memory_engine.py:7999-8008`). Budget-specific overrides exist (`CFG:1400-1402`).
- Pair text: `"[context: ]text"`, with `"[Date: {June 5, 2022} ({2022-06-05})] "` prepended when `occurred_start` is set (`reranking.py:359-383`).
- Default model: local cross-encoder `cross-encoder/ms-marco-MiniLM-L-6-v2` (`CFG:1099`). Batch 32 local / 128 TEI (`skills/.../retrieval.md`, "Stage 2").
- Normalisation (`reranking.py:388-406`): if all scores lie in [0,1] pass through (calibrated API rerankers); else sigmoid `1/(1+e^{-x})`. NaN → 0 (`:414-419`).
- Passthrough mode (`rrf`, `interleave`, or slim image): CE seeded from RRF rank as `1 − 0.9·rank/(n−1)` (`reranking.py:231-242`); CE is skipped entirely for `interleave` (`memory_engine.py:8066-8081`).

### 4.7 Combined scoring (LLM-free). `search/reranking.py:155-285`
```
recency_boost     = 1 + 0.2 · (recency − 0.5)           # α = 0.2   (:16, :281)
temporal_boost    = 1 + 0.2 · (temporal − 0.5)          # α = 0.2   (:17, :282)
proof_count_boost = 1 + 0.1 · (proof_norm − 0.5)        # α = 0.1   (:18, :283)
final = CE_norm × recency_boost × temporal_boost × proof_count_boost   (:284)
```
Each boost lies in [1 − α/2, 1 + α/2] (`reranking.py:14-15`, `:176-178`). Maximum combined multiplier with all three at their top: 1.10 × 1.10 × 1.05 ≈ 1.27 (+27 %); minimum: 0.90 × 0.90 × 0.95 ≈ 0.77 (−23 %), matching the docs (`skills/hindsight-docs/references/developer/retrieval.md`, "Maximum combined range"). The code comment at `:14-15` only covers the two α = 0.2 boosts (≈ +21 %).

- `recency` (`reranking.py:37-59`, `:104-152`): linear default `recency = max(0.1, min(1, 1 − days/365))` (`:33`, `:57-59`); `exponential` = `0.5^(days/90)` (`:34`, `:50-56`); `none` = 0.5. Age measured from `now` = `question_date` or server time (`memory_engine.py:1434-1440`). Missing date → 0.5 (`reranking.py:145-146`).
- **Coarse-date rule** (`:62-74`, `:81-101`, `:104-138`): if `[occurred_start, occurred_end]` spans a whole calendar month or year (±1 day tolerance, `:74`), age is measured from the **end** of the period and recency is **capped at 0.5** (no freshness boost for a period still in progress). Issue #3893 is cited in the code (`:120`).
- `temporal` = `temporal_proximity` from the temporal arm, else 0.5 (`reranking.py:263`).
- `proof_norm = clamp(0.5 + ln(proof_count)/10, 0, 1)` for observations, else 0.5 (`reranking.py:265-272`). Example: proof 150 → 1.0; proof 1 → 0.5.
- Then `min_scores` (opt-in, no default) filters on CE and final (`memory_engine.py:8149-8162`).
- Docs' token packing (`retrieval.md`, "Stage 4"): sort by final, pack by token count, skip items that do not fit, keep top-1 even if oversize. (Read from docs; the packing code is at `memory_engine.py` ~7580-8200 and was not traced line by line.)

### 4.8 Reflect (LLM agent over the same store)
See section 7.3.

## 5. Time handling

- Two clocks per unit: `occurred_*` (when it happened, from LLM output) and `mentioned_at` (document date). `event_date` is their COALESCE (`memories/pg/writes.py:83`).
- Retrieval time windows: temporal arm above; `created_after/created_before` filter on `updated_at`, not creation, so an edited memory re-enters a window (`retrieval.py:243-246`, mental-model delta watermark).
- Temporal links: 24 h weighting with 0.3 floor (section 3, W7).
- Recency boost at rerank time (section 4.7). Note: recency is scored from `occurred_start` for non-coarse units (`reranking.py:140-146`) and the code comment says intervals "arguably should age from its end too" (`:140-143`).
- Date text in embeddings and CE pairs (W4, 4.6): lets a query like "in June" match via the embedding and via the CE.
- Observations carry `occurred_start = LEAST(...)`, `occurred_end = GREATEST(...)`, `mentioned_at = GREATEST(...)` over their sources (`consolidator.py:2713-2723`).
- Trend labels on observations (LLM-free, `reflect/observations.py:115-186`): recent = last 30 d, old = older than 90 d, density ratio recent/older `> 1.5` → strengthening, `< 0.5` → weakening, no recent evidence → stale, all recent → new, else stable.
- Causal/temporal propagation during spreading (4.2).

## 6. Dedup and identity

- Entities: deterministic resolution (W5). Label entities: exact only.
- Observations: consolidation LLM sees top candidate observations and decides UPDATE vs CREATE (`consolidation/prompts.py` PROCESSING RULES 1–3, 9). Post-hoc **dedup adjudication**: a newly created or updated observation whose embedding cosine ≥ `CONSOLIDATION_DEDUP_THRESHOLD` = **0.97** (`CFG:2087`) against top-5 candidates (`consolidator.py:159`, `_DEDUP_PROMPT` `:193`) is reconciled by a 1-by-1 LLM "merge or keep" call (Postgres only). Merge folds source ids (`_apply_dedup_create_fold` `:366`, `_apply_dedup_update_fold` `:454`).
- Facts: no hash-based dedup across retains in the card-relevant path; document-level delta retain skips unchanged chunks by content hash (`orchestrator.py:3433-3461` `_classify_chunk_diff`, `_try_delta_retain`). (Identity of a fact is its unit id; re-extraction of the same sentence creates a new unit unless the chunk is unchanged.)

## 7. Consolidation, reflection, mental models

### 7.1 Observation consolidation (LLM)
- Trigger: automatic after retain/delete/update (`CFG:2075`, `skills/.../observations.md`).
- Loads unconsolidated facts in batches of 50 memories (`CFG:2082`), rounds capped at 100 memories then re-queued (`CFG:2083`), LLM batch size **8 facts per call** (`CFG:2086`). Parallel tag groups 4 (`CFG:2088`).
- Per batch, recall existing observations with `budget = consolidation_recall_budget` (default `low`, `CFG:2089`), `max_tokens = 1024` (`CFG:2084`), `fact_type=["observation"]`, `tags_match="all_strict"` if scoped, `include_source_facts=True`, `reranking="interleave"` (no CE; `consolidator.py:2926-2968`, reason in comment `:2961-2966`).
- Source facts in the prompt capped at 4096 tokens total and per-observation cap (`CFG:2090-2091`).
- LLM returns `{creates, updates, deletes}` with `source_fact_ids` and `observation_id`; rules: prefer UPDATE over CREATE, one observation per facet, state changes update concisely, never compute numbers, keep history, be conservative with DELETE (`consolidation/prompts.py`, `_PROCESSING_RULES`, `_OUTPUT_SECTION`).
- Writes: `proof_count = len(source_ids)` (`consolidator.py:2777`) or distinct count in dedup fold (`:413`); previous text kept in observation history (`CFG:2079-2080`).
- Scope cap `MAX_OBSERVATIONS_PER_SCOPE` (default -1, `CFG:2093`).

### 7.2 Mental models (LLM refresh, delta or full)
- A mental model = `source_query` + `trigger` + stored `content` (docs `mental-models.md`).
- Refresh triggers: after consolidation (`consolidator.py:2062` `_trigger_mental_model_refreshes`, min interval `CFG:1415`), or cron (`trigger.refresh_cron`, checked every `MENTAL_MODEL_REFRESH_TICK_SECONDS`, `maintenance.py:1-40`, `CFG:2078`).
- Stale check first: only refresh if the model's own scope has new memories since last refresh (`memory_engine.py:16105-16125`; `maintenance.py:21-30`).
- Mode selection (`memory_engine.py:15953-15990`): `delta` requires existing content and unchanged `source_query` (else `source_query_changed` / `no_baseline_content` fallback to full).
- Snapshot bound: refresh reads facts committed before a DB-time cutoff (`memory_engine.py:16012-16015`); the watermark is the newest in-scope `updated_at` seen, not `now()` (`:16018-16023`).
- Reflect is called with the source query, excluding the model being refreshed (`:16030`ff). `has_sources` short-circuit (`:16118-16125`).
- Delta mode: LLM emits structured ops (`reflect/delta_ops.py`: `append_block`, `insert_block`, `replace_block`, `remove_block`, `add_section`, `remove_section`, `replace_section_blocks`, `rename_section`; applied by `apply_operations` `:411`). Untouched text is kept byte-for-byte. Failure falls back to full (`memory_engine.py:16577-16610`).

### 7.3 Reflect agent (LLM loop)
- `reflect/agent.py`: `DEFAULT_MAX_ITERATIONS = 10` (`:74`). Tools in `reflect/tools_schema.py:18-154`: `search_mental_models`, `search_observations`, `recall`, `expand`, `done`.
- Forced hierarchy on the first turns: `search_mental_models` → `search_observations` → `recall`, then `auto` (`agent.py:926-940`). Last iteration forces a text answer with no tools (`:902-906`). Context-budget guard forces synthesis (`:910-918`).
- A fresh mental model short-circuits the forced path (`stop_forcing_from_iteration`, `:934-936`). Freshness is a signal, not a hard rule (docs `mental-models.md`).

### 7.4 Forgetting and decay
- **No fact decay and no access reinforcement.** `memory_engine.py:8557` records `frequency=0.0` for every visited node, while the module docstring claims "Recency and frequency-based importance" (`memory_engine.py:9`). The only access-related counter is in tracing. No `access_count` or `last_accessed` column is read in ranking.
- Recency decay is a read-time boost only (4.7), not deletion.
- Maintenance loop (`maintenance.py:1-40`) deletes audit and LLM-request logs past retention, re-runs stranded consolidation, and refreshes scheduled mental models. It does not delete facts.
- Observations are never deleted by design except supersession (prompt rule 7, "PRESERVE HISTORY").
- Memory Defense (blocked/redacted items at retain, `orchestrator.py:1385-1425`) is an ingest filter, not forgetting.

## 8. LLM steps and cost (write, read)

| Step | LLM? | LLM-free equivalent (inference) |
|---|---|---|
| Fact extraction, `concise`/`verbose`/`custom`/`verbatim` (`fact_extraction.py:1852`) | Yes, 1+ call per 3000-char chunk | Sentence/clause splitter + NER (GLiNER or rules) + the regex date resolver already in `_infer_temporal_date`. `chunks` mode is LLM-free already. |
| Causal relations (inside extraction) | Yes | Discourse cues ("because", "so", "after") + pair scoring by embedding similarity and temporal order, weight <1 (inference). |
| Entity names (inside extraction) | Yes | NER model or existing `neocortex` extract.rs. |
| Entity resolution (`entity_resolver.py`) | No | n/a (already deterministic) |
| Temporal, semantic, entity links | No | n/a |
| Temporal query parse | No (dateparser) | n/a; T5 optional |
| Vector, BM25, graph, temporal arms, RRF | No | n/a |
| Cross-encoder rerank | No LLM, but a neural model (MiniLM) | Optional, off by default in neocortex. |
| Consolidation (creates/updates/deletes) | Yes | Deterministic merge: cluster by embedding + entity overlap, union sources, keep text of highest-proof member (inference). |
| Dedup adjudication (≥0.97) | Yes | Deterministic: merge when cosine ≥0.97 and entity sets equal and no number/negation mismatch (inference). |
| Mental model refresh (full) | Yes | Deterministic summary template over PPR-ranked facts per entity; no prose (inference). |
| Mental model delta ops | Yes | Deterministic patch of a section keyed by entity (inference). |
| Reflect agent | Yes | Not replaceable LLM-free; caller agent can do it through the tool interface (matches brief). |

## 9. Reported results

Repo: no committed numbers. Benchmarks in `hindsight-dev/benchmarks/`:
- `longmemeval/longmemeval_benchmark.py`: end-to-end QA with an answer LLM and an LLM judge; judge prompt diff vs the paper in `longmemeval/docs/judge_prompt_comparison.md`.
- `locomo/locomo_benchmark.py` with `locomo/datasets/locomo10.json`; `obs/`, `consolidation/`, `perf/`, `document_evolution/` (`baseline_report.json`: 45 rounds, main branch loses 9 table rows, 6 damaged rounds; branch-v2 loses none), `multimodal_retain/` (`baseline_report.json`, api 0.9.2).

Published numbers (web search, secondary; not re-derived from code):
- arXiv 2512.12818 "Hindsight is 20/20" (search snippet): LongMemEval 83.6 % with GPT-OSS-20B (vs 39.0 % full-context baseline), LoCoMo 83.18 % (OSS-20B) and 89.61 % (Gemini-3 Pro).
- Vectorize launch: 91.4 % LongMemEval. Vectorize benchmark post (2026-03-23) and overview: LoCoMo 92.0 %, LongMemEval-S 94.6 %.
- Third-party (Memanto, arXiv 2604.22085, snippet): Hindsight higher accuracy but at a higher-complexity retrieval; Memanto 87.1 % LoCoMo, 89.8 % LongMemEval.
- These are end-to-end QA scores with an LLM reader and LLM judge. They are **not** retrieval recall. Neocortex's numbers (recall/MRR, acc@10) are a different metric.

Sources (web search): arxiv.org/pdf/2512.12818; vectorize.io/blog/hindsight-building-ai-agents-that-actually-learn; prnewswire "Vectorize Breaks 90% on LongMemEval"; hindsight.vectorize.io/blog/2026/03/23/agent-memory-benchmark; arxiv.org/pdf/2604.22085.

## 10. What neocortex could steal

Ranked. File paths are `remem-v31/src/...`. Each item is LLM-free unless marked.

1. **Multiplicative evidence boosts with capped alphas and coarse-date handling** (`reranking.py:155-285`, `:104-138`). Put `final = score × (1+0.2(rec−.5)) × (1+0.2(temp−.5)) × (1+0.1(proof−.5))` into `recall.rs` after `rank::rrf` (`recall.rs:748`). Needs `occurred_start/end` and an observation/proof count per memory. LLM-free. Adopt the period-capped recency so "2026 summit" stored as 2026-01-01 is not aged from its start.
2. **Temporal coverage-spread entry selection** (`retrieval.py:402-458`). When a window is given, take the pool (top 60 by cosine in-window), bucket by time into 8 slices, round-robin best-first, keep 10. Implement in `recall.rs` temporal arm and `temporal.rs`. LLM-free. Fixes clustering on the densest slice of the window.
3. **Temporal links at write** (`link_utils.py:101-162`, `:455-570`, `ops_postgresql.py:733-790`). `weight = max(0.3, 1 − Δh/24)`, top-20 nearest per direction, cap 20 per unit. Implement in `remember.rs` (`write_similar` at `:1084` is the nearest existing writer). LLM-free. Make it bidirectional on purpose; the Hindsight cross-batch path is one-way and unbounded (see weakness 1).
4. **Rank-space arm priority before the cap** (`recall_boost.py:27-47`, `:97-128`). Replace a flat per-channel weight with `1/(k + rank/divisor)` for a boosted channel so a boost cannot invert the head of the ranking. Touch `rank.rs:30` (`rrf`) and `recall.rs:16` channel weights. LLM-free.
5. **Entity resolution score** (`entity_resolver.py:1183-1217`, `:189-199`). `0.5·SeqRatio + 0.3·Σ w(co)/|nearby| + 0.2·recency7d`, `w = 1/√deg`, merge at 0.6, trigram gate 0.3, token-compatibility check (`:132-149`). Compare with `resolve.rs` (`candidates_with_scores` `:152`); the hub damping (`1/√deg`) and per-word compatibility check are the parts to add. LLM-free.
6. **Retention prior from access history** (agentmemory `src/functions/retention.ts:98-122`, `:66-80`: `score = min(1, salience·exp(−0.01·Δdays) + 0.3·Σ 1/Δdays_access)`). Neocortex already writes `access_count` and `last_access` (`remember.rs:1199`, `recall.rs:874`) but never reads them in ranking. Hindsight does not do this. (Source is the agentmemory repo; listed here because it fills a gap in neocortex.) LLM-free.
7. **Hindsight-style dedup folds** (not in the card; see `consolidator.py:366-562`): when the candidate is near-identical (cosine ≥0.97) run a 1-by-1 merge-or-keep decision. Implement as a deterministic gate in `remember.rs` (merge only when entity sets match and no number/negation differs; inference). LLM-free version.
8. **Observation trend labels** (`reflect/observations.py:115-186`): recent 30 d vs older 90 d density ratio → strengthening/weakening/stale/new. Add as a derived field on recall results in `recall.rs`/`render.rs`. LLM-free.
9. **Mental-model stale check + snapshot watermark** (`memory_engine.py:16012-16030`, `:16105-16125`). Only recompute a standing summary when its own scope has newer memories; read facts committed before a snapshot cutoff. Would sit in `engine.rs`/`kg.rs`. The recompute itself needs an LLM, but the staleness gate does not (inference that neocortex summaries exist).
10. **Hierarchical forced tool order for an agent loop** (`reflect/agent.py:926-940`: mental models → observations → raw recall, then auto). Relevant only to `mcp.rs` tool design for the calling agent; no LLM in neocortex.

## 11. What is weak or wrong in it

1. **Cross-batch temporal links are one-way and unbounded in time.** `create_temporal_links_batch_per_fact` inserts only `(new → neighbour)` (`link_utils.py:546`), and the neighbour query has no window (`ops_postgresql.py:760-781`). Within-batch pairs are bidirectional and do stop at 24 h (`link_utils.py:156`). Result (inference from the spreading SQL at `retrieval.py:717-724`): from an older unit you cannot walk temporal edges to newer units. Also any gap beyond ~16.8 h gets the 0.3 floor, so most links are weak ties resolved by list order (`:133-136` docstring says so).
2. **Causal arm is forward-only and weight is constant.** Causal writes are fixed at `1.0` (`link_utils.py:961`); the graph arm reads only `from = seed` (`ops_postgresql.py:882`). An effect is found from its cause, but a cause is not found from an effect (inference). Causal links exist only if the LLM emitted them, max 2 per fact (prompt `:1316-1327`).
3. **Frequency term is a placeholder.** `frequency=0.0` in the trace (`memory_engine.py:8557`) while the header claims frequency-based importance (`:9`). There is no access reinforcement at all.
4. **No fact-level forgetting.** Memory grows unbounded; only logs are pruned (`maintenance.py:1-12`). Observations are only deleted on supersession.
5. **Entity merge can be wrong for short or common names.** The docs admit it ("a short new name ... can be absorbed", `retain.md`, Entity Resolution). The `0.2` recency term can push a borderline match over 0.6 (`entity_resolver.py:1199-1202`; comment `:1208-1214` on rounding).
6. **Recency ages from start of a genuine interval.** `reranking.py:140-146` deliberately does not age a multi-day interval from its end; the code calls this arguably wrong.
7. **Documented constants do not match code paths in some places.** Docs describe RRF over "all four strategies" (true), the graph seeds as "semantic 0.3" (true), but BM25 is documented as BM25 while the native backend is `ts_rank_cd` TF-IDF (`retrieval.md`, "Keyword Search" table). Calling it BM25 overstates the arm.
8. **Config defaults are not verifiable from this checkout** (`hindsight_api/config.py` missing). Two docs copies may drift.
9. **Eval is LLM-dependent.** LongMemEval and LoCoMo numbers are end-to-end with an LLM reader and judge. No retrieval-only recall or MRR is reported in-repo, so the numbers cannot be compared with neocortex's recall/MRR.
10. **The cross-encoder only sees the top 300 by RRF**, so a strong graph hit outside the top 300 never gets a CE score (`memory_engine.py:7999-8008`). The rank-space boost mitigates this for boosted arms only.
