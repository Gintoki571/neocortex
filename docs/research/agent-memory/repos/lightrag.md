# Mechanism card: LightRAG

Repo: `/run/media/bindesh/E/New Folder/rag/lightrag` (HEAD `25ed861e`, Python package `lightrag/`).
Citations are `lightrag/<path>:<line>` relative to the repo root. `(inference)` marks my own reasoning.
Read first: `AGENTS.md` (consistency-without-transactions rule, relation-weight contract, purge contract). Skipped: WebUI, deploy, parsers (`parser/`, MinerU, Docling), storage backends beyond NetworkX and the nano-vectordb contract, tests.

## 1. What problem it solves

Builds a knowledge graph from documents with LLM extraction, then answers queries by combining entity and relation vector search over that graph with the original text chunks. The product is a RAG framework with pluggable storage (`AGENTS.md`, "Storage Layer"). Retrieval modes: `local`, `global`, `hybrid`, `mix`, `naive`, `bypass` (`lightrag/base.py:93`, default `mix`).

## 2. Data model

Four stores, written independently with no cross-store transaction (`AGENTS.md`, "Consistency without transactions"):

| Store | Holds | Key |
|---|---|---|
| `full_docs` (KV) | raw document text | `doc-` + md5(content or path) (`lightrag/pipeline.py:999-1003`) |
| `text_chunks` (KV) | chunk text, token count, order, doc id, heading path | `chunk-` + md5(`f"{doc_id}:{chunk_content}"`) (`lightrag/utils_pipeline.py:174`) |
| `entities_vdb` | embedding of `f"{entity_name}\n{description}"` | `ent-` + md5(entity_name) (`lightrag/operate.py:1800`) |
| `relationships_vdb` | embedding of `f"{keywords}\t{src}\n{tgt}\n{description}"` | `rel-` + md5(sorted(src,tgt)) (`lightrag/operate.py:3390-3391`, `:2320`) |
| `chunks_vdb` | chunk embedding | chunk id |
| graph (NetworkX, file-backed) | entity nodes and relation edges | entity name, unordered pair |
| `doc_status` | pipeline state | doc id |
| `llm_response_cache` | every LLM response, keyed by args hash, including extraction results per chunk | `compute_args_hash` (`lightrag/utils.py:897-930`) |

- **Entity node**: `entity_name`, `entity_type`, `description` (fragments joined by `<SEP>`, `lightrag/constants.py:49`), `source_id` (chunk ids joined by `<SEP>`), `file_path`, `created_at`.
- **Relation edge**: `keywords` (comma-separated), `description`, `weight` (float), `source_id`, `file_path`, `created_at`. Stored under the sorted pair (`lightrag/utils.py:1076`, `lightrag/utils_graph.py:543`).
- **Per-document recovery anchors** `full_entities` and `full_relations` (`lightrag/lightrag.py:5055-5110` docstring): the write-ahead record of what each document contributed. Deletion uses them, not the chunk store.
- Hard caps: `DEFAULT_MAX_SOURCE_IDS_PER_ENTITY = 200` and `..._PER_RELATION = 200` (`constants.py:71-72`), limit method KEEP (oldest kept) or FIFO (`constants.py:74-78`, `utils.py:6697` `apply_source_ids_limit`). `DEFAULT_MAX_FILE_PATHS = 75` (`constants.py:84`).

## 3. Write path

### 3a. Ingest and identity

1. `apipeline_enqueue_documents` (`lightrag/pipeline.py`) assigns the doc id: `doc-md5(file_path)` if the source is known, else `doc-md5(content)` for raw input, else `doc-md5(f"{path}-{track_id}-{index}")` (`pipeline.py:997-1006`).
2. Cross-filename dedup on content: `compute_text_content_hash` = md5 of normalized text (`lightrag/utils_pipeline.py:947-955`), looked up in `get_existing_doc_by_content_hash` (`utils_pipeline.py:1069`). Duplicate records are noted at `pipeline.py:1352`.
3. Parse (native, MinerU, Docling; `lightrag/parser/`). Then chunk.

### 3b. Chunking

- Default strategy `chunking_by_token_size` (`lightrag/chunker/token_size.py:139`), `chunk_token_size = 1200` (`token_size.py:139`, `lightrag.py:1133` from env `CHUNK_SIZE`), `chunk_overlap_token_size = 100` (`token_size.py:283`; `lightrag.py:1138-1151`).
- Sliding window: stride = size - overlap = 1100 tokens (`token_size.py` `_window_step`, raises if overlap >= size). Each window's char span is recovered by delta decoding with a ±32-char `find` fallback (`token_size.py:44-75`), so `_source_span` maps tokens back to exact text.
- Other strategies: `recursive_character` (`chunker/recursive_character.py`, separator cascade `constants.py:103-130`), `semantic_vector` (`chunker/semantic_vector.py:467`), `paragraph_semantic` (`chunker/paragraph_semantic.py`, target 2000, `constants.py:145`). Chunk strategy chosen per file type through the pipeline.
- Chunks carry `chunk_order_index` and, for extraction prompts, a heading breadcrumb (`_truncate_section_context`, `operate.py:250`, `DEFAULT_MAX_SECTION_CONTEXT_TOKENS = 256`, `constants.py:43`).

### 3c. Entity and relation extraction (LLM)

`extract_entities` (`lightrag/operate.py:3943`), one task per chunk, bounded by `max_async` (default 4, `constants.py:96`).

- **Initial call**: `use_llm_func_with_cache(... cache_type="extract")` (`operate.py:4187-4196`). Prompt: `PROMPTS["entity_extraction_system_prompt"]` (`lightrag/prompt.py:56-141`) with `entity_types_guidance`, `{max_total_records}` (default 100, `constants.py:26`) and `{max_entity_records}` (default 40, `constants.py:27`). Output is delimiter-separated rows:
  - entity: `entity<|#|>name<|#|>type<|#|>description`, exactly 4 fields;
  - relation: `relation<|#|>source<|#|>target<|#|>keywords<|#|>description`, exactly 5 fields;
  - completion marker `<|COMPLETE|>` (`prompt.py:14-15`).
  Rules in the prompt: title-case names, third person, no pronouns, undirected relations, decompose n-ary statements into binary ones, relations only between entities emitted in the same response.
- **Parse**: `_process_extraction_result` (`operate.py:1509`) strips the completion marker, repairs records where the LLM used the tuple delimiter as the record separator (`operate.py:1552-1586`), fixes delimiter corruption (`utils.py:6893`), then `_handle_single_entity_extraction` (`operate.py:711`) and `_handle_single_relationship_extraction` (`operate.py:775`). Names go through `normalize_extracted_info` (`utils.py:5595`): HTML tags removed, full-width to half-width, Chinese punctuation to ASCII, outer quote pairs removed. Entity type validated by `_normalize_and_validate_entity_type` (`operate.py:663`). Relation weight is fixed at `1.0` for prompt rows (`operate.py:~830-856`, "Prompt text rows are 5 fields with no weight").
- **Gleaning** (second pass on the same chunk): `entity_extract_max_gleaning` default `1` (`constants.py:17`). The continue-prompt call replays the first response as history (`operate.py:4262-4272`). It is skipped when `gleaning_invariant_tokens + history + continue_prompt > MAX_EXTRACT_INPUT_TOKENS` (default `20480`, `constants.py:38`; check at `operate.py:4253-4260`, logs "Gleaning stopped").
- **Truncation**: if a response is truncated (`is_truncated_response`), a warning is logged and the tally is published (`_report_truncation`, `operate.py:4131-4145`, called at `:4199` and `:4277`).

### 3d. Graph merge (incremental, per entity and relation)

`merge_nodes_and_edges` (`operate.py:3515`) runs per-entity and per-relation keyed locks, then `_merge_nodes_then_upsert` (`operate.py:2430`) and `_merge_edges_then_upsert` (`operate.py:2783`).

- **Entity identity**: exact string equality of the normalized name. No fuzzy matching anywhere in the write path. (inference: identity risk comes from this, see section 11.)
- **Type**: majority vote, `Counter` over new and stored types (`operate.py:~2578-2585`). Ties go to the first inserted type (Python stable sort, `reverse=True`).
- **Descriptions**: `_combine_descriptions_dedup` (`operate.py:2385-2429`) keeps stored fragments first, then new ones, deduplicated after sanitization, across stored and new fragments (#3367). Then `_handle_entity_relation_summary` (`operate.py:373-531`), a map-reduce summary:
  - if `len(fragments) < 8` (`DEFAULT_FORCE_LLM_SUMMARY_ON_MERGE`, `constants.py:30`) and tokens `< 1200` (`DEFAULT_SUMMARY_MAX_TOKENS`, `constants.py:32`): join with `<SEP>`, no LLM (`operate.py:~438-446`);
  - if tokens `<= 12000` (`DEFAULT_SUMMARY_CONTEXT_SIZE`, `constants.py:36`) or `<= 2` fragments: one LLM summary (`_summarize_descriptions`, `operate.py:533`);
  - otherwise split into chunks that fit, summarize each, and repeat.
  Fragments are ordered by timestamp, then by longer first (`operate.py:~2590-2595`).
- **Relation weight contract** (`AGENTS.md`, "Relation weight contract"): `weight >= len(distinct real source ids)`. A new source adds its weight once. Merging uses `max(all input weights, distinct merged real source ids)` (`operate.py:~2982-3001`). Legacy rows are repaired opportunistically, not by sweep.
- **Vector upsert** with ids `ent-md5(name)` and `rel-md5(src+tgt)` plus the reverse hash (`operate.py:3390-3391`).

### 3e. Community detection and summaries

None. LightRAG has no clustering, no community reports and no hierarchical summaries. The graph is flat. (Verified by grep for community or leiden in `lightrag/*.py`: no hits in the write path.)

## 4. Read path

Entry: `kg_query` (`lightrag/operate.py:4657`), `naive_query` (`operate.py:6661`), dispatched from `lightrag.py`.

### 4a. Keyword extraction (one LLM call, cached)

- `get_keywords_from_query` (`operate.py:4914`) returns `(hl_keywords, ll_keywords)` unless the caller supplied them (`operate.py:4936-4943`). Otherwise `extract_keywords_only` (`operate.py:5051`) calls the `keywords_extraction` prompt (`prompt.py:484`), which asks for strict JSON `{"high_level_keywords": [...], "low_level_keywords": [...]}`. Parsing: `_parse_keywords_payload` (`operate.py:4990`), normalized by `_normalize_keyword_list` (`operate.py:4946`).
- High-level keywords describe the theme and intent. Low-level keywords name specific entities and terms. Empty `ll` in local, hybrid, mix falls back to the query, and empty `hl` in global, hybrid, mix does the same (`operate.py:4719-4726`).
- Keyword results are cached in `hashing_kv` (`operate.py:5126-5130`).

### 4b. Vector retrieval (one batch embed)

`_perform_kg_search` (`operate.py:5219-5439`) embeds `[query, ll_keywords, hl_keywords]` in a single batch call (`operate.py:5265-5288`), only for the texts the mode needs.

Per mode:

- **local** (`_get_node_data`, `operate.py:6090`): `entities_vdb.query(ll, top_k)` with cosine threshold `0.2` (`constants.py:62`, `COSINE_THRESHOLD` env). Entities keep vector order. Relations come from `_find_most_related_edges_from_entities` (`operate.py:6150`), sorted by `(rank = graph degree of the edge, weight)` descending (`operate.py:6199-6201`).
- **global** (`_get_edge_data`, `operate.py:6365`): `relationships_vdb.query(hl, top_k)`. Entities are the endpoints in first-seen order (`_find_most_related_entities_from_relationships`, `operate.py:6424`).
- **hybrid**: local plus global. **mix**: hybrid plus `chunks_vdb.query(query, chunk_top_k)` (`_get_vector_context`, `operate.py:5162`, `chunk_top_k = 20`, `constants.py:58`).
- **naive**: `naive_query` (`operate.py:6661`), chunks only, top_k = chunk_top_k.

Fusion between entity-side and relation-side lists is **round-robin interleave with dedupe**, not score fusion (`operate.py:5374-5400`: `for i in range(max_len)`, local then global, skip seen names or sorted pairs).

### 4c. Chunk selection from the graph

For each selected entity (or relation) the chunks are read from its `source_id` list (`_find_related_text_unit_from_entities`, `operate.py:6206`; relations `operate.py:6457`).

1. Count how many selected entities cite each chunk. Sort each entity's chunks by that count, descending (`operate.py:6256-6270`).
2. Method `kg_chunk_pick_method` (default `"VECTOR"`, `constants.py:64`):
   - VECTOR: `num_of_chunks = int(related_chunk_number * n_entities / 2)` with `related_chunk_number = 5` (`operate.py:6290`, `constants.py:63`). Candidate chunks are ranked by cosine between the query embedding and each chunk embedding, read from `chunks_vdb.get_vectors_by_ids` (`utils.py:6177-6300`). A mismatch between text store and vector store aborts to WEIGHT. Empty result falls back to WEIGHT.
   - WEIGHT: `pick_by_weighted_polling(entities, max=5, min=1)` (`utils.py:6095-6176`). Quota for position `i` of `n` is `round(max - i/(n-1) * (max - min))`, so 5, then decreasing to 1. Each entity takes up to its quota of unused chunks. Leftover quota is handed out one at a time round-robin (`utils.py:6150-6175`).
3. Fetch chunk text from `text_chunks` by id, deduplicated in order.

Chunk lists from entities, from relations, and from vector search are merged round-robin with dedupe by chunk id (`_merge_all_chunks`, `operate.py:5655-5762`). In mix mode the vector list is first.

### 4d. Token budgeting

Constants (`base.py:114-132` read env with defaults from `constants.py:57-61`):
- `top_k = 40` (entities and relations from the vector stores);
- `chunk_top_k = 20`;
- `max_entity_tokens = 6000`;
- `max_relation_tokens = 8000`;
- `max_total_tokens = 30000`;
- `enable_rerank` default `true` (`base.py:166`, env `RERANK_BY_DEFAULT`).

Steps:
1. **Entities and relations** (`_apply_token_truncation`, `operate.py:5440-5608`): each item is rendered as JSON (`json.dumps(x, ensure_ascii=False)`), joined by `\n`, and cut to the budget by `truncate_list_by_token_size` (`utils.py:3973-4010`). That function keeps whole leading items only, never half an item, and re-verifies the count because BPE is not monotone in length.
2. **Chunks** (`_build_query_context`, `operate.py:5965`): the chunk budget is
   `available_chunk_tokens = max_total_tokens - (sys_prompt_tokens + kg_context_tokens + query_tokens + buffer_tokens)`, with `buffer_tokens = 200` (`operate.py:5864-5870`, same formula again at `:6783`).
3. Optional rerank (`apply_rerank_if_enabled`, `lightrag/utils.py:~6378-6470`): calls `rerank_model_func(query, documents, top_n)`, maps results back by index, then truncates. Provider adapters in `lightrag/rerank.py` (`cohere_rerank` `:448`, `jina_rerank` `:515`, `ali_rerank` `:555`, generic `:240`). Default binding `"null"` (`constants.py:68`), so rerank is off unless configured. `MIN_RERANK_SCORE` default 0.0 (`constants.py:67`).

### 4e. Answer generation

One LLM call with the assembled context: `rag_response` prompt (`prompt.py:334`) for graph modes, `naive_rag_response` (`prompt.py:388`) for naive. `include_references` (`base.py`) adds a reference list. Answer cache: `_answer_cache_kv` (`operate.py:4626-4656`) keys on query, mode and settings (`operate.py:~4800-4830`). It is bypassed when `conversation_history` is set, so a multi-turn answer never poisons the shared cache.

## 5. Incremental updates

- New documents write only their own chunks and merge their extraction into existing graph nodes and edges (section 3d). Existing entities and relations are not re-extracted.
- Re-ingesting the same content is a no-op through the content-hash and doc-id checks (section 3a).
- Rebuild of affected graph elements after a deletion is `rebuild_knowledge_from_chunks` (`operate.py:1103`). It reads the cached extraction results for each referenced chunk (`_get_cached_extraction_results`, `operate.py:1416-1508`) and regenerates each entity and relation from the chunks that still reference it. If the cache misses, it logs "rebuild may use structural fallback" and does not call the LLM again (`operate.py:1173-1190`). So rebuild is LLM-free when the cache is warm.

## 6. Deletion

Entry: `adelete_by_doc_id` (`lightrag/lightrag.py:5763`), with 11 numbered steps in the source (1: read doc status; 2: chunk ids; 4: purge KG contributions, then chunks; 9: delete LLM cache while the doc status still exists; 11: delete the original document and status).

Core: `_purge_kg_contributions` (`lightrag/lightrag.py:5055-5300`). Candidates come from `full_entities` and `full_relations` anchors (or explicit args). For each candidate:
- intersect the doc's chunk ids with the union of chunk-tracking entries (`entity_chunks`, `relation_chunks`) and graph `source_id` lists;
- **delete outright** if no remaining chunk owns it, otherwise **rebuild** from the remaining chunks (`rebuild_knowledge_from_chunks`, section 5);
- remove deleted nodes and edges from graph and vector stores, persist, then delete chunks, then delete the anchor rows last so every intermediate failure is still retryable.

Governing invariant (`AGENTS.md`, "Purge recovery contract"): a purge must never delete a chunk or anchor that names objects and leave those objects behind. It fails closed (`RecoveryAnchorMissingError`, HTTP 409) unless one of four proofs holds: `anchors`, `pre_graph`, `journal`, `empty_scope`. Full contract: `docs/design/PurgeRecoveryContract.md` (not read).

Accepted residues (per `AGENTS.md`, "Consistency without transactions"): ordering is chosen so that a crash leaves an object that is harmless to keep, not an object that was deleted by mistake.

## 7. Time handling

- Every node and edge has `created_at` (set at write time). Query output converts it with `time.strftime` (`operate.py:5485`, `:5505`) and shows it to the LLM. Nothing filters or ranks by time.
- Description fragments are ordered by timestamp, then by length, during merge (section 3d).
- No event time, no validity interval, no supersession. A later fact about the same entity is appended as another fragment and summarized (section 3d). Contradictions are resolved by the LLM summary, not by a rule.

## 8. Dedup and identity (summary)

- Entities: exact normalized name. No fuzzy, phonetic or embedding merge.
- Relations: unordered pair of entity names (`utils.py:1076`).
- Chunks: md5 of `doc_id:content`. Documents: md5 of path or content.
- Descriptions: exact string equality after sanitization (`operate.py:2385-2429`).
- Entity type: majority vote across mentions.

## 9. Consolidation and forgetting

- Summaries are the only consolidation: over-long description lists are map-reduced into one LLM summary (section 3d).
- No decay, no access reinforcement, no eviction. Old facts remain until their document is deleted.
- Source-id lists are capped at 200 per node and per edge (KEEP or FIFO, section 2).

## 10. Cost and LLM calls

Per document (ingest):

| # | LLM call | Where | Trigger | Count |
|---|---|---|---|---|
| 1 | Entity and relation extraction | `operate.py:4187` (`cache_type="extract"`), prompt `prompt.py:56` | Every chunk | 1 per chunk, roughly 1200-token inputs |
| 2 | Gleaning | `operate.py:4262` | Every chunk, if `entity_extract_max_gleaning >= 1` and tokens fit | 1 per chunk by default |
| 3 | Description map-reduce summary | `operate.py:533` `_summarize_descriptions`, called from `operate.py:373` | Entity or relation with >= 8 fragments or > 1200 tokens | Rare-to-frequent for hub entities |
| 4 | Optional VLM for images and tables | `prompt_multimodal.py`, multimodal pipeline | Multimodal content only | Per item |

Per query:

| # | LLM call | Where | Trigger |
|---|---|---|---|
| 5 | Keyword extraction (JSON) | `operate.py:5051` `extract_keywords_only`, prompt `prompt.py:484` | Unless hl and ll are supplied; cached |
| 6 | Answer generation | `operate.py:4657` `kg_query`, prompts `prompt.py:334` (graph) or `:388` (naive) | Every non-cached query |
| 7 | Rerank (external API, not an LLM) | `utils.py:~6378`, `rerank.py` | Only when a rerank function is configured |

Embedding calls: one batch per query (`operate.py:5265-5288`), one per entity/relation/chunk at write time.

LLM-free equivalents (inference):
- Call 1 (extraction) is the cost driver. An LLM-free equivalent would be a NER model plus dependency or pattern-based relation extraction, feeding the same merge code. The merge code and schema would not change. Extraction quality would drop for implicit relations.
- Call 2 (gleaning) is a recall booster. LLM-free: rerun the extractor on the same chunk with a different span order, or drop it.
- Call 3 (summary) is replaceable by the existing join path (`operate.py:438-446`) with a fragment cap, or by a fixed tie-breaker (most recent, or most supported).
- Call 5 (keywords) is replaceable by the query tokens and a noun-phrase chunker. Note that LightRAG already has an LLM-free fallback: `ll_keywords = [query]` when both are empty (`operate.py:4723-4726`).
- Call 6 (answer) is out of scope for neocortex, which returns context and leaves answering to the calling agent.

Latency: not reported in the repo. Query latency is dominated by call 5 and call 6 plus vector lookups. Write latency is dominated by calls 1 and 2 (inference, based on the code paths).

## 11. Reported results

Source: `lightrag/README.md:466-488`, `docs/Reproduce.md`, `lightrag/evaluation/`.

- Pairwise LLM-judged win rates on UltraDomain (Hugging Face `TommyChien/UltraDomain`), on three criteria (Comprehensiveness, Diversity, Empowerment) and an overall pick per domain, NaiveRAG vs LightRAG (`README.md:466-488`). Overall pick: Agriculture 45.2% vs 54.8%, CS 48.0% vs 52.0%, Legal 47.2% vs 52.8%, Mix 50.4% vs 49.6%. On Mix, NaiveRAG wins.
- Query generation (`docs/Reproduce.md:9-24`): 5 users x 5 tasks x 5 high-level questions per dataset, generated by a prompt. Evaluation prompt (`Reproduce.md:39-75`) asks the judge to pick Answer 1 or 2 per criterion and then an overall winner. The judge model is not named in the table.
- RAGAS evaluator (`lightrag/evaluation/eval_rag_quality.py`, `README_EVALUASTION_RAGAS.md`): faithfulness, answer relevance, context recall and context precision, each with a "> 0.80 good" band. The "~89-100% per question" claim (`README_EVALUASTION_RAGAS.md:37`) is measured on the three-document sample set in `lightrag/evaluation/sample_documents/`, not on a benchmark.

Caveats:
- The win-rate evaluation is LLM-judged and pairwise, with no judge validation in the repo.
- Only the README reports the UltraDomain numbers. The repo does not contain the run logs.
- The RAGAS number is on a toy corpus.

## 12. What is weak or wrong

- **Entity identity is exact-string.** Two mentions of the same entity with different surface forms (`Google` vs `Google LLC`, `Dr. Lee` vs `Lee`) become two nodes. Nothing merges them after the fact. This is the dominant quality risk in the graph (inference from `operate.py:2430` and `utils.py:5595`).
- **Fusion is round-robin, not scored.** The entity list, the relation list and the chunk list are interleaved by rank position (`operate.py:5374-5400`, `operate.py:5965+`). A strong vector hit and a weak graph hit get equal priority by position. There is no score calibration between channels.
- **Cosine threshold is a single global 0.2** (`constants.py:62`) and is not calibrated per embedder. The code allows any value via `vector_db_storage_cls_kwargs`.
- **No temporal model.** Contradicting facts accumulate as description fragments and are summarized by an LLM (section 7). A superseded fact persists until the LLM summary happens to drop it.
- **Relation weight is effectively a count** (`operate.py:~2982-3001`). It rewards repetition, not confidence, and the prompt rows always write `1.0`.
- **The write path is not transactional** across four stores (`AGENTS.md`, "Consistency without transactions"). The repo documents the residues and recovery paths. A crash can leave graph and vector stores out of step until the next merge or rebuild.
- **Unverified benchmark.** The UltraDomain numbers come from the README alone, and the judge is not validated in the repo.
- **Vector retrieval in graph modes uses a single query embedding** for entity and relation search, and a separate keyword embedding. The keywords are LLM output, so a keyword miss is an unrecoverable recall loss in local, global, hybrid and mix.

## 13. Steal list (ranked; neocortex files it would touch)

1. **Chunk selection by weighted polling with a linear quota** (`utils.py:6095-6176`, `pick_by_weighted_polling`). When turning entity or relation hits into supporting memories, give the top entity a quota of 5 and decay linearly to 1, then fill leftover quota round-robin. Touches `src/recall.rs` (evidence attached to `entity()` at line 1216 and to `relation_seeds_with_config` at line 242). LLM-free.
2. **Whole-item token truncation** (`utils.py:3973-4010`, `truncate_list_by_token_size`): keep only leading whole items under a budget, count the separators, and re-verify because BPE is not monotone. Touches `src/recall.rs` (`budget` at line 410, `rank::pack` at line 973) and `src/render.rs`. LLM-free.
3. **Two-level query channels with separate budgets** (`operate.py:5219-5270`, `_perform_kg_search`; `_apply_token_truncation` at `operate.py:5440`: 6000 tokens for entities, 8000 for relations). Neocortex already has entity KNN and relation hits (`src/recall.rs:517-547`); give each its own token budget before fusion. Touches `src/recall.rs` and `src/engine.rs`. LLM-free.
4. **Description merge with a fragment cap and a join fast-path** (`operate.py:373-460`, `_handle_entity_relation_summary`; join when `< 8` fragments and `< 1200` tokens, `constants.py:30-32`). Neocortex is LLM-free, so the join path is the only path needed: dedupe fragments after sanitization (`operate.py:2385-2429`) and join with `<SEP>`, capping the number kept. Touches `src/remember.rs` and `src/resolve.rs` (write of entity text). LLM-free.
5. **Per-document recovery anchors for purge** (`lightrag.py:5055-5300`, `full_entities` and `full_relations` rows deleted last; "delete outright vs rebuild from remaining sources"). Neocortex has `forget` (`src/engine.rs`, `src/remember.rs`) but no record of what each write created. Store the entity and edge ids written per memory, delete outright only when no other memory supports them, and rebuild the rest. Touches `src/db.rs` (new anchor table), `src/remember.rs` (write), and `src/engine.rs` (forget). LLM-free.
6. **Source-id cap with an explicit KEEP or FIFO policy** (`constants.py:71-78`, `utils.py:6697`). Neocortex's entity-to-memory links are unbounded; a hub entity can accumulate thousands. Cap the list and record the policy. Touches `src/db.rs` (entity-memory link table) and `src/paths.rs` (fan-out). LLM-free.
7. **Relation weight as a lower bound on distinct evidence** (`AGENTS.md`, "Relation weight contract"; `operate.py:2982-3001` `max(weight, evidence_count)`). Neocortex edges can carry a count that never falls below the number of distinct supporting memories, validated before the first write. Touches `src/db.rs` (edge weight) and `src/remember.rs`. LLM-free.
8. **Opportunistic repair, not sweep** (`AGENTS.md`, "Relation weight contract", last bullet). Document the residue and repair legacy rows only on the next touch. Touches `src/db.rs` schema-migration notes (SCHEMA_VERSION `db.rs:15`). LLM-free.
9. **Rebuild from cached extraction, fallback to structural** (`operate.py:1173-1190`). If neocortex ever rebuilds derived edges, reuse stored per-memory extraction results and never re-call a model on the write path. Touches `src/remember.rs` and `src/engine.rs`. LLM-free.
10. **Shared sanitation for every write path** (`utils.py:5595-5640`, `normalize_extracted_info`: full-width folding, quote stripping, HTML tag removal). Touches `src/text.rs` (`norm_name`, `fold`) and `src/resolve.rs`. LLM-free.

## Summary of routes

- Write: doc id by md5 -> token-window chunks (1200 tokens, overlap 100) -> LLM extraction plus one gleaning pass -> delimiter parse -> exact-name entity merge with majority type and capped source ids -> map-reduce description summary above 8 fragments or 1200 tokens -> four stores written without a transaction.
- Read: LLM keywords (hl, ll) -> batch embed -> vector top_k 40 with cosine 0.2 -> graph expansion of entities and relations -> chunks by VECTOR (cosine over candidate chunks) or WEIGHT (linear quota 5 to 1) -> round-robin merge -> token budget 6000 / 8000 / 30000 with whole-item truncation -> one answer call.
- Delete: anchors -> outright delete or cache-based rebuild -> chunks -> anchors last -> LLM cache.
