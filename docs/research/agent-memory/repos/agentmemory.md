# Mechanism card: agentmemory

Local clone: `/run/media/bindesh/E/New Folder/rag/agentmemory/` (TypeScript, iii-engine workers). Paths below are relative to that root. Citations are `path:line`.

Caveat: there is no git metadata in the clone, so no commit id can be cited. Version per `AGENTS.md` "Current Stats (v0.9.30)".

---

## 1. Problem solved

Persistent memory for coding agents (Claude Code, Codex, etc.). Hooks capture tool-use events, the server turns them into observations, and later sessions retrieve them through MCP / REST. Memory has explicit `remember` items, session summaries, semantic facts and procedures, graph nodes/edges, insights and lessons. Everything is stored in iii-engine KV scopes (`src/state/schema.ts`).

## 2. Data model

KV scopes (`src/state/schema.ts`, e.g. `retentionScores: "mem:retention"` at `:57`):
- `observations(sessionId)`: `CompressedObservation` {id, sessionId, timestamp, type, title, subtitle, facts[], narrative, concepts[], files[], importance 1–10, compression method} (`src/functions/compress-synthetic.ts:94-106`; `src/types.ts`).
- `memories`: `Memory` {id, type ∈ {pattern, preference, architecture, bug, workflow, fact}, title ≤80, content, concepts, files, `strength` (init 7), `version`, `parentId`, `supersedes[]`, `isLatest`, `forgetAfter` (TTL), `sourceObservationIds`, `project`, `agentId`} (`src/functions/remember.ts:156-181`).
- `summaries`, `semantic` (`SemanticMemory`: fact, confidence, accessCount, lastAccessedAt, strength), `procedural` (name, steps, frequency, strength).
- `accessLog` (per memory: count, lastAt, recent[≤20] timestamps) (`src/functions/access-tracker.ts:6-17`).
- `graphNodes`, `graphEdges`, `graphEdgeHistory`: edges carry `weight`, `tvalid`, `tvalidEnd`, `tcommit`, `version`, `isLatest`, `supersededBy`, `context` {reasoning, sentiment, alternatives, confidence} (`src/types.ts:468-484`, `src/functions/temporal-graph.ts:135-151`).
- `insights` (reflect output), `lessons`, `crystals`, `sketches`, `retentionScores`, `audit`.
- Search indexes are in-memory (BM25 `SearchIndex`, brute-force `VectorIndex`) rebuilt from KV at boot and persisted (`src/state/search-index.ts`, `src/state/vector-index.ts`, `src/state/index-persistence.ts`).

## 3. Write path

### 3.1 Capture and observation (default: no LLM)
- Hook events → `observe` (`src/functions/observe.ts`).
- **Dedup (exact, time-windowed, no LLM):** `DedupMap.computeHash` = SHA-256 of `sessionId:toolName:input[:500]:sha256(output)` (`src/functions/dedup.ts:38-46`); entries expire after `TTL_MS = 5 min` (`:3`, `:58-60`). Duplicate events inside the window are dropped (`src/functions/observe.ts:94-122`).
- **Compression:**
  - If `AGENTMEMORY_AUTO_COMPRESS=true` → `provider.compress(...)` LLM call (`observe.ts:245`, `src/config.ts:517-519`).
  - Else **synthetic** (default, LLM-free): `buildSyntheticCompression` (`src/functions/compress-synthetic.ts:76-106`): title = tool name ≤80 chars; `narrative = truncate(prompt | input | output, 400)` joined by " | "; `concepts = []`, `facts = []`, `importance = 5`; files extracted from tool input (`:105`).
  - The observation is written to KV, added to BM25 and (if an embedding provider exists) to the vector index (`observe.ts:287-288`, `:374-380`).

### 3.2 Explicit remember (no LLM; the recommended write path)
`mem::remember` (`src/functions/remember.ts:29-240`):
1. Type validated, default `fact` (`:57-67`).
2. **Supersession** under a keyed lock: candidates = top 50 BM25 hits for the new content, filtered to `mem_` ids (`:100-102`). For each live candidate of the same agent and compatible project (`:128-137`), `similarity = Jaccard(token sets)`. Tokens: whitespace-split, length > 2, NFC-normalised (`src/state/schema.ts:96-98`, `:126-…`). If `similarity > 0.7` → mark the old one `isLatest=false`, new `version = old+1`, `parentId`, `supersedes=[old]` (`remember.ts:142-147`, `:156-177`, `:183-185`). Old version is removed from BM25/vector (`:190-193`). If `0.4 < similarity ≤ 0.7` the closest is returned as `nearMatch` **hint only** (`:148-153`, `:126`).
3. `ttlDays` sets `forgetAfter` (`:179-181`).
4. Write KV, BM25 add (`:195-205`); vector add happens after, guarded (`:209-226`).
5. `mem::cascade-update` for superseded items (`:228-236`).

### 3.3 Consolidation into memories (LLM, background)
`mem::consolidate` (`src/functions/consolidate.ts`):
- Input: observations with `importance ≥ 5` and a title (`:91-93`); minimum count (`minObservations` default 10, `:71-72`).
- Concept grouping: group by lower-cased `concepts[]`; keep groups with ≥ 3 observations (`:123`), sorted by size; top 8 per group by importance (`.slice(0, 8)`), max **10 LLM calls** per run (`:119`), 30 s timeout per call (`:143-155`).
- LLM returns XML `<memory>` (type, title, content, concepts, files, strength 1–10) (prompt constant near the top of `consolidate.ts`; parser `parseMemoryXml`).
- Evolution: if a latest memory with the same title exists (same project), the old one gets `isLatest=false` and a new version is written with `parentId`, `supersedes`, `version+1` (`:170-200`). Audit records `evolve`.

`mem::consolidate-pipeline` (`src/functions/consolidation-pipeline.ts`), runs on a timer (`src/index.ts:741-748`; interval `CONSOLIDATION_INTERVAL_MS`, `src/config.ts:536-539`):
- Semantic tier: from ≥5 session summaries (`:99`), newest 20 (`:106`), LLM `provider.summarize(SEMANTIC_MERGE_SYSTEM, …)` returns `<fact confidence="…">`. Existing fact (exact lower-case match) → `accessCount++`, `confidence = max(old, new)`; else new semantic record with `strength = confidence` (`:121-154`).
- Procedural tier: LLM extracts named procedures; existing (same name) → `frequency++`, `strength = min(1, strength + 0.1)` (`:232`); new → `strength = 0.5` (`:242`).
- Decay (LLM-free, see §7).

### 3.4 Knowledge graph
- **Heuristic graph (default when graph extraction is not enabled; LLM-free):** `extractGraphHeuristics` (`src/functions/graph.ts:372-460`). For each observation: file nodes, concept nodes; edges (type `related_to`, weight **0.4**, `:369`) between each concept and each file, between consecutive concepts, and between consecutive files; max **12** edges per observation (`:370`, `:…budget`). Edges are de-duplicated by unordered node pair across observations (`edgeByPair` map).
- **LLM graph extraction:** only when `GRAPH_EXTRACTION_ENABLED=true` (`src/config.ts:460-462`, gate `src/functions/graph.ts:743-744`). Prompted XML parse (`parseGraphXml`, `:299`).
- **Temporal graph (LLM):** `mem::temporal-graph-extract` (`src/functions/temporal-graph.ts:163-300`). Prompt (`:20-52`) asks for entities and relationships with `valid_from`, `valid_to`, `weight` 0.1–1.0 (1.0 explicit, 0.5 inferred, 0.1 speculative), reasoning, sentiment, alternatives. Parser `:54-156`: entity regex `:62-63`, relation regex `:94-95`, `confidence = weight` (`:133`).
- Versioning, never overwrite: an existing latest edge with the same (source, target, type) gets `isLatest=false`, `tvalidEnd = now` if unset, `supersededBy = new id`; the new edge gets `version = old + 1` (`:268-281`). Node merge by normalised name+type with alias union (`:196-234`).

### 3.5 Relations and lessons
- `mem::relate` / `mem::evolve` take agent-supplied relations with a heuristic confidence `0.5 + min(0.3, 0.1·sharedSessions) ±0.1 (recency) + 0.1 (supersedes) − 0.05 (contradicts)`, clamped to [0,1] (`src/functions/relations.ts:28-52`). No LLM.
- Lessons: saved with `decayRate: 0.05` (`src/functions/lessons.ts:159`).

## 4. Read path (retrieval)

Entry: `mem::search` (`src/functions/search.ts:728-…`) → hybrid ranker when wired (`src/index.ts:427-442`) → `HybridSearch.search` (`src/state/hybrid-search.ts:44-46`) → `tripleStreamSearch` (`:91-242`). Keyword-only fallback if the hybrid call throws (`search.ts:864-869`) or no hybrid ranker (`:866-867`). Default `limit = 20`, max 100 (`search.ts:752-758`).

### 4.1 Three streams per query (`hybrid-search.ts:99-125`)
- **BM25**: `bm25.search(query, 2·limit)` (`:100`). Scoring in `src/state/search-index.ts:172-261`:
  ```
  idf = ln((N − df + 0.5)/(df + 0.5) + 1)                              (:206)
  bm25 = idf · tf·(k1+1) / (tf + k1·(1 − b + b·docLen/avgDocLen)) · w    (:215-218)
  k1 = 1.2, b = 0.75                                                    (:29-30)
  ```
  Query synonyms enter with `w = 0.7`, the original term with `w = 1.0` (`:184-196`, `synonyms.ts`). Prefix expansion over the sorted term list, `idf_prefix = 0.5·ln(...)` (`:233-234`), scored with the same BM25 shape, max over prefixes (`:235-248`). Tokens: lower-case, unicode letters/digits, length ≥ 2, Porter-style `stem()` (`:356-370`); CJK uses a segmenter (`:361-364`). Indexed text = title + subtitle + narrative + facts + concepts + files + type (`:343-354`).
- **Vector**: query embedding, then brute-force cosine over all vectors (`src/state/vector-index.ts:18-29`, `:99-128`), top `2·limit`. Skipped if no embedding provider or empty index (`hybrid-search.ts:109-116`). Embedding default: none unless a key exists (`src/providers/embedding/index.ts:30-52`; `src/config.ts:350-365`: `EMBEDDING_PROVIDER` env, else GEMINI/OPENAI/VOYAGE/COHERE/OPENROUTER keys). `local` provider = `Xenova/all-MiniLM-L6-v2` (`src/providers/embedding/local.ts:42`).
- **Graph**: `searchByEntities(entities, depth 2, limit)` for entities extracted from the query, plus `expandFromChunks(top-5 vector hits, depth 1, 5)` (`hybrid-search.ts:244-257`). Time-boxed to `GRAPH_BUDGET_MS = 500` (`:25`, `:258-270`). Graph weight default **0.3** (`src/index.ts:428`). Query entities: quoted phrases plus capitalised tokens minus stopwords (`src/functions/query-expansion.ts:132-…`). No LLM.

### 4.2 Weighted RRF with agreement bonus (`hybrid-search.ts:127-226`)
Per observation, rank in each stream (1-based; missing = ∞):
```
RRF_K = 60                                                    (:24)
weighted = wB/(60+rB) + wV/(60+rV) + wG/(60+rG)               (:208-211)
wB = bm25Weight (0.4), wV = vectorWeight (0.6), wG = graphWeight (0.3)  (:36-38; env BM25_WEIGHT / VECTOR_WEIGHT in src/config.ts:336-341)
maxAttainable = activeWeight · 1/61                           (:201)   activeWeight = sum of weights of streams that returned hits
rrf = weighted / maxAttainable                                (:212)   (so a rank-1 hit in every active stream scores 1.0)
combined = rrf · (1 + 0.05 · (matchedStreams − 1))            (:196, :216)
```
Sort by `combined`, then `minRank`, then id (`:221-226`). Note: unlike neocortex's RRF it is **weighted** and normalised by the best attainable score, so a single-stream hit keeps its stream weight.

### 4.3 Session diversification and enrichment (`:237-239`, `:293-327`)
- Keep at most **3** observations per session in the first pass (`maxPerSession = 3`, `:304`), then backfill to the limit (`:317-324`). Retrieval depth = `max(limit, 20)` (`:237`).
- Enrichment loads the observation from KV or falls back to a `Memory` record (`:329-375`).

### 4.4 Optional cross-encoder rerank (off by default)
`RERANK_ENABLED=true` (`hybrid-search.ts:39`). `rerank` (`src/state/reranker.ts:67-95`) scores the top 20 with `Xenova/ms-marco-MiniLM-L-6-v2` (`:3`), input `[CLS] q [SEP] doc [SEP]`, max 512 tokens (`:4`, `:34-48`), document = title + narrative. **Sort by the raw CE logit only**; the RRF score is discarded for the top 20 (`:88-89`, `:…`). Any failure returns the RRF order (`:57-62`, `:78-85`). Also used in `searchWithExpansion` (`hybrid-search.ts:48-89`).

### 4.5 Query expansion (LLM, optional path)
`mem::expand-query` asks the LLM for reformulations and temporal concretisations (`src/functions/query-expansion.ts:69-…`, `provider.compress`). `searchWithExpansion` runs the triple stream on each reformulation and merges by max combined score (`hybrid-search.ts:55-80`). Entry point is only used by callers that pass an expansion (`smart-search` path); the plain `mem::search` does not call it.

### 4.6 Access logging (no effect on ranking)
Results that are returned call `recordAccessBatch` (`src/functions/search.ts:947`, `context.ts:283`, `file-index.ts:124`, `working-memory.ts:155`, `relations.ts:341`). It appends a timestamp to `accessLog` (`access-tracker.ts:54-80`). **Nothing in the search ranking reads `accessLog`.** It feeds only the retention score (§7.2).

## 5. Time handling

- Every observation/memory has an ISO `timestamp` / `createdAt` / `updatedAt`. The BM25 index keeps `timestamp` (`search-index.ts:12`) but does not use it in scoring.
- **No recency term in `mem::search` ranking.** `grep` for recency in search code finds only `src/functions/context.ts:87-264` (context-block ordering), `src/functions/lessons.ts:209` and `src/functions/reflect.ts:563`.
- Insight/lesson search rank = `confidence · relevance · 1/(1 + 0.01·days)` (`src/functions/reflect.ts:556-567`; same in `lessons.ts:209-210`), where days = since last reinforcement (or creation).
- Temporal graph is bi-temporal: `tcommit` (when recorded), `tvalid`/`tvalidEnd` (when true in the world; from LLM `valid_from`/`valid_to`), `supersededBy`. `mem::temporal-query` with `asOf` keeps edges with `tcommit ≤ asOf`, `tvalid ≤ asOf`, `tvalidEnd ≥ asOf`, then the latest per key (`src/functions/temporal-graph.ts:315-333`). Without `asOf` it returns current state.
- Memory TTL via `forgetAfter` (`remember.ts:179-181`).
- Retention decay (§7.2) is exponential in days since creation.
- Session/summary recency sort in context assembly (`context.ts:264`).

## 6. Dedup and identity

- Observation: exact duplicate tool events within 5 minutes (`dedup.ts`).
- Memory: Jaccard > 0.7 on tokens (>2 chars) within the same agent and compatible project supersedes; the old version is kept in KV with `isLatest=false` (`remember.ts:142-147`, `:183-196`).
- Insight/lesson ids: `fingerprintId(prefix, content)` = `prefix_` + first 16 hex of SHA-256 (`src/state/schema.ts:84-87`); an identical insight is reinforced rather than duplicated (`reflect.ts:98-107`; `acceptedIds` set in the same loop).
- Graph nodes: normalised name + type; temporal ingestion merges onto an existing node (`temporal-graph.ts:196-234`); edges keyed by (source, target, type) with versioning.
- Auto-forget contradictions: two memories sharing a concept with token-Jaccard > 0.9 → the older one is marked not-latest (`src/functions/auto-forget.ts:14`, `:99-146`).

## 7. Consolidation, reflection, decay and forgetting

### 7.1 Reflect (LLM, optional)
`src/functions/reflect.ts`. Builds concept clusters over semantic facts, lessons and tags (`:255-300`): a concept is kept if it occurs in ≥ 2 items; two concepts join a cluster if their document-set Jaccard is **> 0.3** (`:268-290`). For each cluster, LLM emits `<insight confidence title sources>` (`:33-44`); fingerprinted by content (`:…fingerprintId("ins_v1", …)`). Existing insight is **reinforced**: `confidence ← min(1, c + 0.1·(1 − c))`, `reinforcements++`, `lastReinforcedAt` (`:98-107`). New insights start with `decayRate = 0.05` per week (`:453`).

### 7.2 Retention score (LLM-free, computed on demand)
`mem::retention-score` (`src/functions/retention.ts:128-292`). Defaults `DEFAULT_DECAY` (`:21-27`): `lambda = 0.01`, `sigma = 0.3`, tier thresholds hot 0.7, warm 0.4, cold 0.15.
```
salience(mem)  = min(1, base + min(0.2, 0.02 · accessCount))     base by type: architecture .9, preference .85, pattern .8, bug .7, workflow .6, fact .5 (:104-114, :118-121)
             (semantic: base = max(0.5, confidence))
Δt             = days since createdAt
decay          = exp(−0.01 · Δt)                                  (inline at :150-153; same as computeRetention :88-90)
boost          = 0.3 · Σ_accesses 1 / days_since_access           (:66-80)   (only accesses with Δ > 0 days)
score          = min(1, salience · decay + boost)                 (:172-175, :217-220)
```
Tiers: hot ≥ 0.7, warm ≥ 0.4, cold ≥ 0.15 (`:251-264`). Note the boost is **not bounded per access**: an access 1 hour ago contributes `0.3 · 24 = 7.2`, which the `min(1, …)` clamps.
`computeRetention` at `:82-96` is defined but never called (dead code; the same formula is inlined).
`mem::retention-evict` (`:293-…`) deletes candidates with `score < threshold` (default cold = 0.15, `:299-302`), at most `maxEvict = 50` per call (`:303-307`). It is not on a timer; `grep` finds no scheduler for it (`src/index.ts` only schedules `auto-forget`, `consolidate-pipeline` and session finalise).

### 7.3 Semantic and procedural decay (LLM-free, timed)
`applyDecay` in `consolidation-pipeline.ts:22-50`:
```
anchor = max(lastAccessedAt || updatedAt, lastDecayedAt)
periods = floor(days_since_anchor / decayDays)                      (decayDays = CONSOLIDATION_DECAY_DAYS, default 30; src/config.ts:541-543)
strength = max(0.1, strength · 0.9^periods)                         (:43-46)
```
Applied to semantic and procedural records during `consolidate-pipeline`. This is the only place that reads `lastAccessedAt` for decay.

### 7.4 Insight and lesson decay (LLM-free, weekly)
```
weeks = (now − max(createdAt, lastDecayedAt, lastReinforcedAt)) / 7d
confidence = max(0.05, confidence − decayRate · weeks)              (reflect.ts:609; lessons.ts:346-347)
soft-delete when confidence ≤ 0.1 and reinforcements == 0          (reflect.ts:614)
```

### 7.5 Forgetting (automatic)
`mem::auto-forget` runs hourly by default (`AUTO_FORGET_INTERVAL_MS = 3600000`, `src/index.ts:653-661`; `AUTO_FORGET_ENABLED` to disable):
- TTL: delete memories past `forgetAfter` (`src/functions/auto-forget.ts:43-65`).
- Contradiction: described in §6, marks the older memory not-latest (`:99-146`).
- Low-value observation: age > 180 days and `importance ≤ 2` → delete observation and its search entries (`:163-200`).

`mem::evict` (`src/functions/evict.ts:18-40`, `DEFAULTS`): stale sessions after 30 days, low-importance observations (`importance < 3`) after 90 days, observation cap 10 000 per project; expired memories; non-latest memories after `lowImportanceMaxDays` (`:246-259`, `:335-…`).

### 7.6 Reinforcement
- Memory access: `accessLog.count` and last-20 timestamps (`access-tracker.ts`), used **only** by retention score (§7.2).
- Insight confirm: `c ← c + 0.1(1 − c)` (`reflect.ts:98-107`).
- Lesson confirm: `c ← c + 0.1(1 − c)` (`lessons.ts:58-60`).
- Procedural: `strength += 0.1` (cap 1) (`consolidation-pipeline.ts:232`).
- Semantic: `confidence = max(old, new)` and `accessCount++` (`:136-139`).

## 8. LLM steps and cost

| Step | LLM? | Notes |
|---|---|---|
| Observation compression (`observe`) | No by default; LLM if `AGENTMEMORY_AUTO_COMPRESS=true` | synthetic path `compress-synthetic.ts` |
| `mem::remember` | No | Jaccard supersession |
| Dedup window | No | SHA-256 |
| Heuristic graph | No | `graph.ts:372-460` |
| LLM graph extraction | Yes, opt-in | `GRAPH_EXTRACTION_ENABLED` |
| Temporal graph | Yes | `temporal-graph.ts` |
| Consolidate to memories | Yes (≤10 calls/run) | `consolidate.ts` |
| Semantic/procedural tiers | Yes (summarize) | `consolidation-pipeline.ts` |
| Reflect / insights | Yes | `reflect.ts` |
| Query expansion | Yes, optional | `query-expansion.ts` |
| BM25, vector, graph, RRF | No | |
| Cross-encoder rerank | No (ML model), opt-in | `RERANK_ENABLED` |
| Retention, decay, auto-forget | No | |

Cost: the default install with no LLM key still runs fully (BM25 + heuristic graph + synthetic observations). Vector search needs an embedding provider; without one it is BM25-only (`src/providers/embedding/index.ts:30`).

## 9. Reported results

From `README.md` and `benchmark/`:
- **LongMemEval-S** (ICLR 2025, 500 questions; `benchmark/LONGMEMEVAL.md`): retrieval-only, `recall_any@K` (any gold session in top K), no LLM in the loop.
  - BM25 + vector (0.4 / 0.6, no graph, no rerank): R@5 **95.2 %**, R@10 98.6 %, R@20 99.4 %, NDCG@10 87.9 %, MRR 88.2 %.
  - BM25 only: R@5 86.2 %, R@10 94.6 %, MRR 71.5 %.
  - MemPalace raw (vector-only, same embedder, from the report): R@5 96.6 %.
  - By type (hybrid R@5): knowledge-update 98.7 %, multi-session 97.7 %, single-session-assistant 96.4 %, temporal-reasoning 95.5 %, single-session-user 90.0 %, single-session-preference 83.3 %.
- **coding-agent-life-v1** (in-house, 15 sessions): hybrid P@5 0.240, R@5 1.000, 15/15 top-5 hit, p50 14 ms; grep baseline R@5 0.967 (`README.md:342-352`).
- Embedding for the reported numbers: `all-MiniLM-L6-v2`, 384 d, local.
- Competitor table in `benchmark/COMPARISON.md` (mem0, Letta, Zep, etc.) is partly from their published LoCoMo numbers (`README.md:545`).

How the LongMemEval bench is built (`benchmark/longmemeval-bench.ts`): **each haystack session is one observation** whose narrative is the full session text (`:156-163`, `title = first 80 chars`), timestamp = `new Date()` at run time (`:159`), embedding = first **512 characters** of the session (`:174`), and then `HybridSearch(bm25, vector, provider, kv, 0.4, 0.6, 0.0, false)` (`:188-196`), so the **graph arm is off and rerank is off**. Abstention questions are excluded (`:130-135`). No temporal or recency signal is used. The score is therefore session-level recall with no extraction step at all, and only the first 512 characters of each session are embedded.

## 10. What neocortex could steal

Ranked. Targets are `remem-v31/src/...`. All LLM-free unless marked.

1. **Retention prior that uses access history.** `retention.ts:66-122`: `score = min(1, salience·exp(−0.01·Δdays) + 0.3·Σ 1/Δdays_access)`, `salience = base_by_type + min(0.2, 0.02·count)`. Neocortex already writes `access_count` and `last_access` on every recall (`remember.rs:1199`, `recall.rs:874`) and never reads them in ranking. Add a bounded version (clamp each access term, e.g. `min(1, 1/Δdays)`) as a multiplicative prior in `recall.rs` after `rank::rrf` (`recall.rs:748`). LLM-free.
2. **Weighted RRF normalised by the best attainable score, plus per-session cap.** `hybrid-search.ts:127-226` (per-stream weights, `maxAttainable` normalisation, `combined = rrf·(1+0.05·(streams−1))`) and `:293-327` (at most 3 per session before backfill). Neocortex `rank.rs:30` already has weighted RRF with an agreement bonus (tests at `rank.rs:218`), so the new pieces are the normalisation for thresholding and the session/entity diversity cap, to add in `recall.rs` next to `rank::mmr`. LLM-free.
3. **Supersession by token Jaccard with a near-match hint.** `remember.ts:83-154`: candidates from BM25 top-50, Jaccard > 0.7 → supersede (version chain, old version leaves the index but stays in KV), 0.4–0.7 → return `nearMatch` hint, do not auto-merge. Map onto `remember.rs` (`live_duplicate` at `:1162`, `knn_memory` at `:1244`). The version chain (`parentId`, `supersedes`, `isLatest`) is a clean model for `invalidate` in `mcp.rs`. LLM-free.
4. **Bi-temporal edge versioning with an `asOf` filter.** `temporal-graph.ts:237-333`: edges have `tcommit`, `tvalid`, `tvalidEnd`, `supersededBy`; a new edge closes the previous latest one with `tvalidEnd = now`; `asOf` keeps `tcommit ≤ t`, `tvalid ≤ t`, `tvalidEnd ≥ t`. Neocortex has `as_of` snapshots (`recall.rs:37`), so compare the write-side closing rule in `remember.rs:1103-1234` (`end_memories_of_ended_facts`, `retract_relations`). LLM-free if the caller supplies validity.
5. **Insight reinforcement and weekly decay with soft delete.** `reflect.ts:98-107`, `:596-620`: `c ← c + 0.1(1 − c)` on re-derivation; `c ← max(0.05, c − decayRate·weeks)`; soft-delete at `c ≤ 0.1` with zero reinforcements. Apply to any consolidated belief record in neocortex. Arithmetic is LLM-free; the insight creation is not.
6. **Heuristic co-occurrence graph with an edge budget.** `graph.ts:372-460`: per observation, link concepts to files and neighbouring concepts with weight 0.4, dedupe unordered pairs, cap 12 edges per observation. Compare with `graph.rs` / `kg.rs`. LLM-free.
7. **Bounded graph path score = mean edge weight / path length, with cost = Σ 1/w.** `graph-retrieval.ts:243-244`, `:355`: a heap-ordered traversal, depth ≤ 8, state cap 512, edge cap 1024, path cost `Σ 1/max(w, 0.01)`. Use as a second scoring rule for the beam in `paths.rs`, alongside the existing semantic beam. LLM-free.
8. **Exact-duplicate suppression with a short TTL.** `dedup.ts:38-60`: SHA-256 of session, tool, input (first 500 chars), output digest; drop repeats for 5 min. Put into the capture path (`mcp.rs` or the ingest path in `remember.rs`). LLM-free.
9. **Concept clusters for reflection (Jaccard > 0.3 on document sets, ≥ 2 docs per concept).** `reflect.ts:255-300`. Use to pick the groups that a summariser (LLM or template) would consume. Cluster step LLM-free; summary step needs an LLM or a template (inference).

## 11. What is weak or wrong in it

1. **Recall has no time signal.** Search ranking ignores timestamps entirely (`search-index.ts:12` stores them, nothing reads them in scoring). Recency only appears in insight/lesson search (`reflect.ts:563`) and in context ordering. Older facts win ties; supersession only helps when the new text is nearly identical (Jaccard > 0.7).
2. **Access tracking does not affect ranking.** Every `recordAccessBatch` call writes `accessLog` (`access-tracker.ts`), and nothing in `mem::search` reads it. The retention score that does read it (`retention.ts`) is not connected to recall, and its `retention-evict` is not on a timer.
3. **Retention boost is unbounded per access.** `1/Δdays` explodes for accesses within a day (`retention.ts:74-77`). Only the outer `min(1, …)` saves it. `computeRetention` (`:82-96`) is dead code that duplicates the inlined formula.
4. **BM25 "idf" for prefixes is half-weight and ad hoc.** `search-index.ts:233-234` uses `0.5·idf` for prefix matches and takes the max over prefixes per document, which is not standard BM25 and is untested against a baseline in the repo (no ablation found).
5. **Vector search is brute force.** `vector-index.ts:99-128` scans every vector per query. Fine at the scales the repo targets, but there is no ANN index in this clone.
6. **Graph arm is off in the benchmark.** The LongMemEval bench sets graph weight to 0 (`longmemeval-bench.ts:194`). The reported 95.2 % therefore measures BM25 + vector only, so the graph layer has no measured contribution.
7. **Benchmark is retrieval-only, session-level, and easy to over-read.** Observations are whole sessions with the first 512 characters embedded (`longmemeval-bench.ts:156-174`). No extraction, no temporal reasoning, no QA accuracy. The README table mixes this with other systems' published LoCoMo QA numbers (`README.md:545`).
8. **Reflection and consolidation are LLM-gated and capped.** `MAX_LLM_CALLS = 10` per consolidate run (`consolidate.ts:119`); clusters need ≥ 3 observations with importance ≥ 5 (`consolidate.ts:91-94`, `:123`). Quiet sessions produce nothing, and the synthetic observations default to `importance = 5` (`compress-synthetic.ts:107`), so they pass that filter by default. (Inference: this means nearly all tool events feed consolidation.)
9. **Heuristic graph edges are noisy by construction.** Every consecutive concept pair and every concept–file pair gets `related_to` weight 0.4 (`graph.ts:372-460`), with no type information. The "graph" arm is therefore a co-occurrence graph, not a knowledge graph, unless the LLM extractor is enabled.
10. **Supersession uses token overlap, not meaning.** `Jaccard > 0.7` on tokens longer than 2 characters (`schema.ts:96-98`). Paraphrases ("partner" vs "married to") never supersede, and two different facts with the same long words can (inference from the metric).
