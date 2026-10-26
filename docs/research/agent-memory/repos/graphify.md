# Mechanism card: graphify

Repo: `/run/media/bindesh/E/New Folder/rag/graphify` (HEAD `23f2ffa`, release 0.9.58, Python).
Citations are `graphify/<path>:<line>` relative to that repo root. `(inference)` marks my own reasoning, not a repo claim.
Skipped on purpose: UI/HTML export, deploy, tests, skill installers, the 40-language extractor bodies (only their dispatch and contract were read).

## 1. What problem it solves

Turns a folder (code, markdown, PDFs, images, audio) into one knowledge graph (`graph.json`, NetworkX node-link) that a coding agent queries instead of grepping raw files. Code is parsed deterministically with tree-sitter. Docs, papers and images go through an LLM "semantic" pass. The query side is LLM-free: lexical seeding plus BFS over the stored graph, rendered under a token budget (`graphify/serve.py`, MCP stdio/HTTP). Pipeline is stated in `ARCHITECTURE.md:6-9`: `detect → extract → build → cluster → analyze → report → export`.

## 2. Data model

- **Node** (`graphify/ids.py:50-93`): `id` is `normalize_id("_".join(parts))`, lowercase `[a-z0-9_]`. Semantic ids follow the prompt rule `{stem}_{entity}` where stem is the path with `/` and `.` turned into `_` (`graphify/llm.py:478-508`). Attributes: `label`, `file_type` in `{code, document, paper, image, rationale, concept}` (`graphify/semantic_cleanup.py:35`), `source_file`, `source_location` (`"L42"`), `node_kind` (`page` or `heading`, set by markdown extractor), optional `rationale`, `community` after clustering.
- **Edge**: `source`, `target`, `relation` (`calls`, `imports`, `references`, `cites`, `conceptually_related_to`, `shares_data_with`, `semantically_similar_to`, `implements`, `indirect_call`, `contains`, ...), `confidence` in `{EXTRACTED, INFERRED, AMBIGUOUS}`, `confidence_score`, `weight`, `source_file`, `source_location`, optional `context`. Schema rubric (`ARCHITECTURE.md:72-80`).
- **Confidence scores actually emitted** (grep of `extract.py`):
  - EXTRACTED 1.0: an import or an explicit type-qualified reference, e.g. `Type.method()`. `graphify/extract.py:7517` (`confidence_score = 1.0` when import evidence exists).
  - INFERRED 0.85: name-resolved cross-file call with no import evidence, `extract.py:7522`; and callbacks passed by name (`indirect_call`), `extract.py:7491`.
  - INFERRED 0.8: instance call `obj.method()` whose receiver type comes from local inference, e.g. `extract.py:3431`, `3720`, `3863`, `4085`, `4456` (one per language member-call resolver).
  - The comment at `extract.py:7491-7496` says 0.8 was moved to 0.85 so the value sits in the documented rubric `{0.55, 0.65, 0.75, 0.85, 0.95}`.
- **Hyperedge**: `{id, label, nodes[], relation, confidence, confidence_score, source_file}`. The prompt allows at most 3 per chunk (`llm.py:478` text). `semantic_cleanup.py:30-34` caps: 25 MB fragment, 10,000 nodes, 100,000 edges, 10,000 hyperedges, 256 members per hyperedge, id length 256; id regex `^[\w.:-]+$` (`semantic_cleanup.py:41`).
- **Storage on disk**: `graphify-out/`: `graph.json`, `GRAPH_REPORT.md`, `wiki/`, `graph.html`, `cache/ast/`, `cache/semantic-{mode}/`, `.graphify_labels.json`, `.graphify_build.json`, `.pending_changes`. Path guard `security.py` (`validate_graph_path` must stay inside `graphify-out/`, per `ARCHITECTURE.md:88-95`).

## 3. Write path

### 3a. Deterministic extraction (no LLM)

1. Dispatch by suffix: `_get_extractor` (`graphify/extract.py:6049`), per-file `_extract_single_file` (`:6105`), parallel driver `_extract_parallel` (`:6152`), entry `extract(paths, root=...)` (`:6324`). Files are collected by `collect_files` (`:7766`).
2. Per-language tree-sitter extractors (`graphify/extract.py:1684` `extract_python`, `:1692` `extract_js`, and ~40 more) emit definitions, `contains`, `imports`, and raw call sites. Raw calls are resolved later in a cross-file pass.
3. **Cross-file call resolution guard** (`graphify/extract.py:7405-7530`): a raw call binds only if
   - the callee name has exactly one definition in the corpus (`extract.py:7431` `if len(candidates) == 1:`), or
   - the caller's file has an import edge to exactly one of several candidates (named-import disambiguation, `extract.py:7434-7440` comment).
   Otherwise the call is dropped. This is the "single-candidate guard: never fabricate" rule (`extract.py:4527`).
   JS/TS direct calls with no import evidence are dropped (`extract.py:7463-7470`, issue #1659).
4. **Markdown** (`graphify/extractors/markdown.py:238`): pure line parser, no tree-sitter. Nodes: one `page` node per file (frontmatter parsed onto it), one `heading` node per heading. Edges: `file contains heading`, `parent heading contains child heading` by level, `heading references X` for backticked names matching a known pattern, and `file references doc` for inline links, reference-style links and `[[wikilinks]]`. Fenced code is skipped. External URLs and anchors are dropped.
5. **Rationale**: comments starting `# NOTE:`, `# WHY:`, `# HACK:`, `# IMPORTANT:`, `# TODO:`, `# FIXME:`, `# RATIONALE:` become `rationale` nodes or attributes (`extract.py:1274`, `_extract_python_rationale` at `:1315`).

### 3b. Semantic (LLM) pass for docs, papers, images

- **Per-file read cap**: 20,000 chars (`graphify/llm.py:30`, `_FILE_CHAR_CAP`). Per-file wrapper overhead 160 chars (`llm.py:34`). Token estimate: `tiktoken` `cl100k_base` if installed, else `chars/4` (`llm.py:37`, `_estimate_file_tokens` at `:2086-2140`). PDFs are estimated from converted text, not compressed bytes (`llm.py:2091-2115`, #2903).
- **Slicing oversized text** (`graphify/file_slice.py`): splittable suffixes (`:38-45`: md, mdx, txt, rst, html, yaml, ...; PDF via converter, `:47`). `slice_boundaries(text, max_chars)` (`:157-176`) cuts each window of `max_chars` at the latest boundary, searched in this order: `"\n#"` (cut keeps the heading with the next slice), `"\n\n"`, `"\n"` (`_BOUNDARY_SEPARATORS` `:97`, `_best_cut` `:138-155`). Falls back to a hard cut. Slices are gap-free and concatenate back to the original text. `expand_oversized_files(files, _FILE_CHAR_CAP)` is called at `llm.py:2621`.
- **Packing** (`_pack_chunks_by_tokens`, `llm.py:2143-2186`): units are grouped by parent directory (sorted), then added greedily until `current_tokens + cost > token_budget`. At most `_MAX_IMAGES_PER_CHUNK = 20` images per chunk (`llm.py:787`); each image costs `_IMAGE_TOKEN_ESTIMATE = 1600` tokens (`llm.py:781`). Default `token_budget = 60_000` (`extract_corpus_parallel`, `llm.py:2558`).
- **Concurrency**: thread pool, `max_concurrency=4` default (`llm.py:2560`, comment: conservative for rate limits).
- **Adaptive retry**: when the response is truncated (`finish_reason="length"`), the chunk is bisected and each half re-extracted (`_extract_with_adaptive_retry`, `llm.py:2302`; `max_retry_depth` default 3 at `llm.py:439-460`).
- **Output parsing**: `_parse_llm_json` (`llm.py:1122`) strips `<think>` blocks and fences, scans balanced-object candidates (`:1037-1120`), caps input at 10 MB (`llm.py:982`), then `_sanitize_fragment` (`llm.py:985`) and `validate_semantic_fragment` (`semantic_cleanup.py:44`).
- **Prompt-injection wrapping**: each source is wrapped in `<untrusted_source path sha256>` (`llm.py:557-598`); the system prompt says content inside is data, not instructions (`llm.py:478-496`).
- **Prompt** (`graphify/llm.py:478-508`, `_EXTRACTION_SYSTEM`): EXTRACTED/INFERRED/AMBIGUOUS rules; rationale stored as an attribute, not a node; edge direction rule (source is the actor, `calls` source contains the call site); hyperedge rule (3+ nodes sharing a concept, max 3 per chunk). Deep mode appends `_DEEP_EXTRACTION_SUFFIX` (`llm.py:510-516`), which allows more INFERRED edges for concrete architectural signals.

### 3c. Entity dedup (`graphify/dedup.py:550` `deduplicate_entities`)

Called from `build()` (`graphify/build.py:1398-1459`, call at `:1451`). Passes:

- **Guard 0, repo mixing** (`dedup.py:~566-572`): nodes with more than one `repo` tag raise `ValueError`. Cross-project dedup is disabled.
- **Pass 0, ID collisions**: one survivor per id. The survivor is the minimum of `_collision_rank` (`dedup.py:412-436`). That ranks each node's `source_file` relative to the scan root, with lifecycle penalties for archived path segments (`_ARCHIVED_PATH_SEGMENTS` `:364`, `_lifecycle_penalty` `:378`). Same-source losers fill the survivor's missing attributes (`_merge_missing_attributes` `:452`).
- **Pass 1, exact normalized label**: `_norm` (`dedup.py:20-25`) is NFKC, casefold, then `[\W_]+ → space`. Code symbols are skipped (`_is_code` `dedup.py:252`; #1205). Same-file exact matches merge. Cross-file exact matches merge only for `concept` nodes, or for `document`/`rationale` nodes that read as entities (`_reads_as_file_entity` `:317`), and only if `_entropy(label) >= 2.5` bits/char (`_ENTROPY_THRESHOLD = 2.5`, `dedup.py:244`; `_entropy` `:28-37`).
- **Pass 2, fuzzy**:
  - Candidates: non-code, norm-unique labels with entropy >= 2.5 (`dedup.py:~715-730`).
  - Signature: MinHash with `num_perm=128` over char-3-grams of the space-stripped normalized label (`dedup.py:40-56`, `_NUM_PERM = 128` `:248`; numpy re-implementation `graphify/_minhash.py:20-107`, no datasketch).
  - Candidate generation: LSH with `threshold=0.7` (`_LSH_THRESHOLD`, `dedup.py:245`).
  - Score: `JaroWinkler × 100` for same-file pairs or when the label is under 12 chars (`dedup.py:778`). Plain `Jaro × 100` for cross-file labels of 12+ chars (`dedup.py:776`, #1243: the prefix bonus fabricated merges such as `testing-library jest-native` vs `react-native`).
  - Community boost `+5.0` if both nodes share a community and both are 12+ chars (`_COMMUNITY_BOOST` `dedup.py:247`; applied at `:809`). **In the build path this never fires**: `build()` passes `communities={}` (`graphify/build.py:1452`), so `c1 is None`.
  - **Merge if score >= 92.0** (`_MERGE_THRESHOLD`, `dedup.py:246`; test at `:811`).
- **Veto guards** (checked before the score test, any one blocks the pair):
  - `_is_variant_pair` (`dedup.py:62-74`): under 12 chars, same stem, different digit or letter suffix (`M1` vs `M1 Pro`, `cranel` vs `cranelr`).
  - `_short_label_blocked` (`dedup.py:77-91`): under 12 chars, unless same length, Damerau-Levenshtein <= 1 and JW >= 97 (true typos).
  - Prefix extension (`dedup.py:~790-794`): `getActiveSession` vs `getActiveSessions` (#1201).
  - `_numeric_tokens_differ` (`dedup.py:97-117`): digit runs compared as multisets with leading zeros stripped (`ADR 0011` vs `ADR 0013`; #1284).
  - `_content_token_swap` (`dedup.py:149-195`): template siblings that differ in one content word, ignoring stopwords (`_STOPWORDS` `:120`, #2576).
  - `_crossfile_fileanchored_blocked` (`dedup.py:200-241`): rationale/document nodes across files (boilerplate headings).
- **Pass 3, LLM tiebreak** (opt-in, `dedup_llm_backend`): pairs with `75 <= score < 92` are batched 30 at a time (`dedup.py:985-992`, `_llm_tiebreak`, ambiguous collection at `:1041`). Without the flag, these pairs are not merged.
- **Winner and rewire**: union-find components (`dedup.py:839`). Winner by `_pick_winner` (`:963`, richness score `_content_richness` `:942`). Losers' missing fields are merged in. Edge endpoints and hyperedge members are rewired (`_remap_hyperedge_members` `:510`, #2805). Self-loops are dropped.

### 3d. Graph construction

- `build_from_json` (`graphify/build.py:799`): returns `nx.Graph` (undirected by default) or `DiGraph` when `directed=True`. Parallel edges collapse on `(source, target, relation)` (`dedupe_edges`, `build.py:575-596`) in the raw and incremental paths. Alias folding and id coercion (`build.py:188-300`). `validate.py` checks the schema.
- Edge weights: `weight` is carried through; `confidence_score` is kept alongside `confidence`.

### 3e. Community detection (`graphify/cluster.py`)

- **Algorithm**: Leiden via `graspologic_native.leiden`, `seed=42`, `resolution` default 1.0 (`cluster.py:22-95`). Fallback `graspologic.partition.leiden`, then `networkx.louvain_communities(seed=42, threshold=1e-4)` (`cluster.py:132-166`).
- **Determinism**: edges are canonicalized and sorted before partitioning (`cluster.py:110-131`, with a measured 70-vs-69 community count drift under two `PYTHONHASHSEED`s).
- **Isolates** become singleton communities. Optional `exclude_hubs_percentile` removes high-degree nodes before partitioning, then reattaches each to its majority-neighbour community (`cluster.py:223-300`).
- **Oversize split**: `max_size = max(10, floor(0.25 * N))` (`cluster.py:169-172`, `:299-304`). Any community above it is re-partitioned on its induced subgraph (`_split_community` `:328-344`).
- **Cohesion re-split**: `cohesion = actual_edges / (n(n-1)/2)` (`cluster.py:346-355`). A community with `size >= 50` and cohesion `< 0.05` is re-split (`cluster.py:171-172`, `:311-317`). This targets doc-hub bridges (CLAUDE.md connected to everything).
- **IDs**: sorted by `(-size, sorted members)` for a total order (`cluster.py:321-325`). `remap_communities_to_previous` (`cluster.py:361-409`) does greedy max-overlap matching to the previous assignment so IDs survive rebuilds. `community_member_sigs` (`cluster.py:202-220`) hashes member sets so a stale LLM label is not reused after re-scoping.

### 3f. Community labels, god nodes, surprises, report

- **Default label** (LLM-free): `label_communities_by_hub` (`cluster.py:175-199`). The community is named after its highest-degree member, ties by id. `Community {cid}` if no member is present.
- **Optional LLM label**: `label_communities` (`graphify/llm.py:3383`), batches of 100 communities (`_LABEL_BATCH_SIZE`, `llm.py:3233`), 12 sampled labels per community (`llm.py:3231`), labels truncated to 60 chars (`llm.py:3232`). Overrides the hub label when configured.
- **God nodes**: top-degree nodes after dropping file, concept and JSON-key nodes (`graphify/analyze.py:109-146`).
- **Surprising connections** (`graphify/analyze.py:218-290`, `_surprise_score`): sum of
  - confidence bonus: AMBIGUOUS 3, INFERRED 2, EXTRACTED 1 (`analyze.py:235`);
  - +2 when the two files are in different categories (code vs paper vs doc);
  - +2 when the two top-level directories differ;
  - +1 when the Leiden communities differ (`analyze.py:271`);
  - x1.5 (integer truncation) for `semantically_similar_to` (`analyze.py:277`);
  - +1 for a peripheral node (degree <= 2) reaching a hub (degree >= 5) (`analyze.py:283-287`).
  Structural bonuses are suppressed for INFERRED `calls`/`uses` that cross languages or join code to docs (`_suppress_structural`, `analyze.py:~250`), which the source calls resolver pollution.
- **Outputs**: `report.generate` writes `GRAPH_REPORT.md` (`report.py`, 346 lines); `wiki.to_wiki` writes one markdown article per community plus `index.md` (`wiki.py`, 405 lines); `suggest_questions` (`analyze.py:443`) and `graph_diff` (`analyze.py:571`).

## 4. Read path (LLM-free, `graphify/serve.py`)

Entry: `_query_graph_text` (`serve.py:1276-1335`), via MCP tools in `_build_server` (`serve.py:1622`).

1. **Tokenize**: `_search_tokens` = `[^\W_]+` after diacritic strip, lowercased (`serve.py:182-193`); `_` splits, like `-`. `_query_terms` (`serve.py:272-291`): `\w+` tokens, drop `_QUERY_STOPWORDS` (`serve.py:240`), fallback to all tokens if everything is a stopword. Chinese is segmented.
2. **IDF**: `idf(t) = ln(1 + N / (1 + df(t)))`, where `df(t)` counts nodes whose normalized label contains `t` (`serve.py:308-330`), cached on the graph object.
3. **Per-node score** (`_score_query`, `serve.py:501-676`). Constants (`serve.py:294-305`):
   - `EXACT_MATCH_BONUS = 1000`, `PREFIX_MATCH_BONUS = 100`, `SUBSTRING_MATCH_BONUS = 1`, `SOURCE_MATCH_BONUS = 0.5`, `RATIONALE_MATCH_BONUS = 0.75`.
   - Whole-query tier for multi-word queries: exact on label, token form or id gives `1000 * 10 * joined_w`; prefix gives `100 * 10 * joined_w`. `joined_w = max idf over terms` (`serve.py:~560-580`).
   - Per term `t` with weight `w = idf(t)`, only the strongest tier counts: exact label `1000*w`, prefix `100*w`, else substring `1*w`. Source path hit adds `0.5*w`. Rationale hit adds `0.75*w`.
   - Coverage: the exact and prefix tiers are multiplied by `(matched / n_terms)^2` (`serve.py:~632-634`, #1602). Source and rationale hits do not count toward coverage.
   - Ties: shorter label, then node id (`serve.py:~646`).
   - Prefilter: trigram index with `guard_frac = 0.10` (`serve.py:422-489`). Falls back to a full scan if the needle is too common.
4. **Seeds** (`_pick_seeds`, `serve.py:702-863`): up to `max_k = 3`. Keep the next node only if `score >= 0.2 * top_score` (`gap_ratio`, `serve.py:704-705`, `:765`). Dedup seeds by normalized label (`#1766`). Then add the best node for every query term that has any match (`serve.py:~803-820`, #1445), subject to the same label cap. Relational-intent verbs (`calls`, `uses`) are removed from this guarantee (`serve.py:~850-860`, #2507).
5. **Traversal** (`_bfs`, `serve.py:970-998`, default `depth = 3`): frontier expansion. Hub threshold is `max(50, p99 of degree)`. Hubs are not expanded as transit, except seeds. After the BFS, `_complete_induced_edges` (`serve.py:920`) adds every edge among visited nodes. A DFS mode exists (`serve.py:1001`).
6. **Context filters**: `_infer_context_filters` (`serve.py:881`) can restrict traversal to a subtree.
7. **Render under budget** (`_subgraph_to_text`, `serve.py:1028-1185`): `char_budget = token_budget * 3` (`serve.py:1034`), default `token_budget = 2000`, so about 6,000 chars. Seed nodes render first. Over-budget output is cut at a line boundary with a `[!] TRUNCATED` notice (`_cut_lines_to_budget`, `serve.py:1186-1211`).
8. **Other tools**: `shortest_path` (`serve.py:1479`), `explain` via `_find_node_tiers` (`serve.py:1338-1453`).

Retrieval is lexical and structural. No embedding is used anywhere in `serve.py`, and the file does not import `llm`. Paraphrase queries depend on the labels that the LLM pass produced.

## 5. Incremental updates and deletion

- **File identity**: `file_hash` = SHA-256 of the file body with YAML frontmatter stripped, keyed by path relative to root (`graphify/cache.py:428-530`, `_body_content` `:177`). Stat fast path on `(size, mtime_ns)`. A stat signature is trusted only if the mtime is older than a 2 s granularity window (`cache.py:219-260`, `_stat_sig_fresh`), so a same-second rewrite cannot reuse a stale hash.
- **AST cache**: `cache/ast/` keyed by file hash (`cache.py:38` `_AST_CACHE_SCHEMA = 2`).
- **Semantic cache**: `check_semantic_cache` (`cache.py:1239-1323`) returns cached nodes, edges and hyperedges plus the list of uncached files. The key includes the prompt fingerprint, a 12-char digest (`cache.py:81`, `prompt_fingerprint` `:100-121`), and a mode namespace (`cache/semantic-{mode}/`, #1894). `save_semantic_cache` (`cache.py:1385`). `prune_semantic_cache` drops entries whose hash is no longer live (`cache.py:1185`).
- **Watch loop** (`graphify/watch.py:2182`, `watch(watch_path, debounce=3.0)`): debounce 3 s. A code-only batch triggers `_rebuild_code` (`watch.py:1314`), which is AST-only and costs no API. A batch touching docs or papers is flagged as needing the LLM (`_batch_needs_llm_flag`, `watch.py:2172`). Changes queue in `.pending_changes` (`watch.py:22-78`) under a rebuild lock (`watch.py:162`).
- **Per-tier replace on merge** (`build_merge`, `graphify/build.py:1783-1800`): a re-extracted `source_file` replaces its prior nodes and edges for each tier it actually contains (AST vs semantic, `_is_ast_tier` `build.py:45`). Files absent from the new chunks are kept. Deleted files are removed with `prune_sources`, tier-blind. Safe to repeat.
- **Deletion detection**: a source path that disappears under the watched root is added to `deleted_paths` (`watch.py:1527-1585`). `_reconcile_existing_graph` (`watch.py:726-1013`) evicts those files' nodes and edges, and reconciles markdown links (`_reconcile_markdown_links` `:521`).
- **Shrink guard** (`_check_shrink`, `watch.py:1120-1192`): a write that loses nodes is refused unless `force`, unless every lost node belongs to a file that was rebuilt this run or has no `source_file`. Nodes from failed sources never excuse a loss. This catches silent half-written semantic passes.

## 6. Consolidation and forgetting

- Graph-level: none. Nodes are never aged out. Deletion is file-driven (above).
- `reflect.py` (separate feature, `graphify reflect`): reads Q&A memory docs written by `save-result`, scores each cited source with a signed, time-decayed weight, and writes `graphify-out/reflections/LESSONS.md`.
  - Decay: `weight = 0.5 ** (age_days / 30)` (`reflect.py:51`, `_decay` `:275-285`).
  - A node is "preferred" only after `>= 2` distinct useful results (`reflect.py:52`).
  - Sections: preferred, tentative, contested (recency decides), known dead ends, corrections. Output is byte-stable for a given `now`.

## 7. Time handling

- No temporal model in the graph. `captured_at` appears on semantic nodes (`llm.py` prompt schema, `ingest.py`) but no ranking code reads it.
- Time is used only for: file mtime in the cache (`cache.py:219`), git HEAD in watch (`watch.py:249`), and the decay in `reflect.py`.

## 8. Dedup and identity (summary)

- Node identity is the normalized id (`ids.py`). Entity identity across chunks is then resolved by section 3c: exact normalized label within a file, `concept` across files, and fuzzy JW/Jaro with the vetoes above.
- Edge identity is `(source, target, relation)` (`build.py:575`). Relation direction is not normalized for undirected builds.

## 9. Cost and LLM calls

Query path: 0 LLM calls (`serve.py` does not import `llm`). Code build: 0 LLM calls (tree-sitter plus the deterministic resolvers). Clustering and reports: 0 LLM calls by default.

| # | LLM call | Where | When it fires | LLM-free equivalent (inference) |
|---|---|---|---|---|
| 1 | Semantic extraction of each packed chunk (nodes, edges, hyperedges) | `llm.py:2550` `extract_corpus_parallel`, `:2870` `_call_llm`, `:478` prompt | Any doc, paper or image, unless cache hits | Section-level extraction: markdown headings and links are already deterministic (`markdown.py:238`). Add a small NER or GLiNER-class model for entities, and co-occurrence inside a section for INFERRED `conceptually_related_to` edges, with hyperedges from section membership. Lower precision than the prompt, but zero API cost. |
| 2 | Deep-mode extraction (same call, more INFERRED edges) | `llm.py:510` suffix | `--mode deep` | Same as 1, plus a relation-verb rule set. |
| 3 | Vision on images inside chunk 1 | `llm.py:766-1000` | Image files | OCR plus caption model (inference). |
| 4 | Community naming | `llm.py:3383` `label_communities`, `:3492` `generate_community_labels` | Only if an LLM backend is configured | Already the default: hub-node label, `cluster.py:175`. |
| 5 | Dedup tiebreak for pairs in [75, 92) | `dedup.py:985` `_llm_tiebreak` | Only with `dedup_llm_backend` | A pair classifier or cross-encoder over the two labels plus context (inference). |
| 6 | Audio transcription | `transcribe.py:23` (local Whisper, not an LLM) | Audio or video | Kept as is. |

Latency: no benchmark in the repo. Parallel AST extraction at `extract.py:6152`. Query is in-memory BFS, so latency is bounded by graph size (inference).

## 10. Reported results (self-reported, not reproducible from this checkout)

Source: `graphify/BENCHMARKS.md`, `graphify/README.md:127-135`.

- LOCOMO (n=300): QA accuracy 45.3% (graph-expand config), recall@10 0.497. Reference points quoted: supermemory 49.7% QA at about 11x ingest cost, BM25 31.3%, mem0 27.3%, hybrid RRF 43.3%, dense RAG 41.3%. Recall@10 for mem0 0.048 and BM25 0.362.
- LongMemEval-S (n=50): QA 76% (tied with dense RAG at 76%), recall@10 0.844 vs dense 0.848.
- Ingest cost: about $1.40 for LOCOMO vs supermemory $15.67, mem0 $3.48. Graph build: $0 LLM credits.
- ERPNext code suite (n=6 graded questions): key-fact coverage 70.8% (grep and read) to 82.0% with one graphify tool, about 140K tokens per query.
- Temporal: 689 weekly AST checkpoints on ERPNext, 2011-2026. Nodes 3,069 to 22,620; edges 2,900 to 48,710 (`BENCHMARKS.md`, "Results: temporal").
- Judge: Kimi K2.6, key-fact coverage `(covered + 0.5 * partial) / total`, blind-validated against a second judge at 90.6% agreement, kappa 0.81 (`BENCHMARKS.md` "Judge and grading").

Caveats I verified:
- The harness the numbers come from is not in this checkout. `BENCHMARKS.md` cites `memory/runner.py` and `crosstool/run.py`, and neither directory exists. The "graph-expand" adapter is not in the tree either. Treat the numbers as unverified.
- The in-repo `graphify/benchmark.py` only measures token reduction with `chars/4` (`benchmark.py:13` `_CHARS_PER_TOKEN = 4`). It does not measure retrieval quality.
- The embedder in the harness is BGE-m3, which is not part of graphify. The harness also runs its adapters on a shared index, so the baselines are not a clean ablation.

## 11. What is weak or wrong

- **The community boost in dedup is dead on the build path.** `build()` passes `communities={}` (`build.py:1452`), so `c1 is None` at `dedup.py:805` and `+5.0` never applies. The docstring of `dedup.py:247` implies it does.
- **Paraphrase recall is structurally weak.** The query side is token, substring and trigram only (`serve.py:501`). There is no embedding fallback. Synonyms reach the graph only through whatever the LLM pass wrote as a label.
- **Call resolution is conservative to the point of losing recall.** One-candidate-only binding (`extract.py:7431`) drops every ambiguous name, such as a common method name defined in several classes. This is a deliberate god-node guard (#543), but it means cross-file calls to overloaded names are simply absent.
- **Node labels are a hard dependency.** Dedup, seeds and report all key on `label`. A bad LLM label corrupts several stages at once.
- **Reported benchmark numbers are not reproducible from this repo** (section 10), and the LOCOMO "recall@10" is computed over a graph built from conversation docs, not over raw turns. The comparison is therefore not like-for-like with mem0 and supermemory.
- **Ad-hoc heuristics without evaluation.** Surprise score weights (`analyze.py:218-290`), the 0.2 seed gap ratio, the 0.25 community size fraction and the p99 hub cutoff are all unvalidated constants.
- **Time is ignored in the graph.** A deprecated decision and a current one are the same node unless the LLM rewrites the label.

## 12. Steal list (ranked; neocortex files it would touch)

1. **Per-term seed guarantee plus gap-ratio seeding, with a label cap** (`serve.py:702-820`; #1445, #1766). Each query term that matches anything seeds at least one candidate, and a generic label can seed only once. Touches `src/recall.rs` (channel seeding) and `src/paths.rs` (beam seeds, `BEAM = 24` at `paths.rs:22`). LLM-free.
2. **Entity-resolution vetoes before a fuzzy score** (`dedup.py:62-241`): numeric-token multiset, prefix extension, short-variant suffix, content-token swap, cross-file plain Jaro for long labels. Touches `src/resolve.rs` alongside the existing `entity_seed 0.55`, `pred_alias 0.80`, `entity_conflict 0.73` thresholds in `src/engine.rs` (Config default block). LLM-free; use MinHash over char-3-grams plus LSH at 0.7 for candidate generation (`dedup.py:40-56, 245`).
3. **Never transit through hubs** (`serve.py:970-998`: `max(50, p99 degree)`, seeds exempt) and **exclude hubs before community detection** (`cluster.py:223-255`). Touches `src/paths.rs` (beam expansion, `EDGES_PER_ENTITY = 12`) and `src/kg.rs:303-304` (the louvain call). LLM-free.
4. **Cohesion re-split plus stable community IDs** (`cluster.py:169-172, 346-409`): re-split communities of size >= 50 with cohesion < 0.05, then remap IDs to the previous assignment by greedy max-overlap. Touches `src/kg.rs:303` and the cached map in `src/recall.rs:1402` (`cached_communities`).
5. **Per-file replace on re-import, tier-aware** (`build.py:1783-1800`, `prune_sources`). Re-importing a source should replace only the memories and edges that came from that source, not append. Touches `src/import.rs:16` (`import_remem`) and the write helpers in `src/remember.rs`.
6. **Shrink guard with an attribution exception** (`watch.py:1120-1192`). A bulk write that would drop records from files not in the write set is refused unless forced. Touches `src/import.rs` and `src/engine.rs` (write entry points). LLM-free.
7. **Stat-then-hash cache with a granularity window** (`cache.py:219-260, 428-530`) plus **cache key that includes the producer fingerprint** (`cache.py:81-121`). Skip re-embedding unchanged text, keyed on (embedder name, dims, content hash). Touches `src/embed.rs`, `src/db.rs` (content_hash already exists), and `src/remember.rs:245` (`check_embedder`).
8. **Surprise score for bridging memories** (`analyze.py:218-290`): bonus for cross-community and cross-type links, and for a peripheral node reaching a hub, with structural bonuses suppressed for low-trust edges. Touches `src/render.rs` or the `overview` path in `src/recall.rs:1467` to surface bridge facts. LLM-free.
9. **Corroboration gate before promotion** (`reflect.py:52`, `min_corroboration = 2`): a memory becomes "preferred" only after two distinct confirmations, and single-source items stay "tentative". Touches `src/engine.rs` (`w_retention`, `access_keep`, already present) and `src/recall.rs`. LLM-free.
10. **Hub-label community naming as the default** (`cluster.py:175-199`): deterministic name from the highest-degree member, ties by id. Touches `src/kg.rs` or `src/render.rs` wherever communities are named. LLM-free, and already what neocortex needs.

## Summary of routes

- Write: tree-sitter/markdown -> cross-file resolver (single-candidate guard) -> optional LLM semantic pass -> `deduplicate_entities` (exact, then MinHash-LSH + JW/Jaro with 92 merge threshold and vetoes) -> NetworkX -> Leiden (seed 42) with size and cohesion re-split -> hub-label names -> `graph.json` and report.
- Read: tokenize -> IDF-weighted tiered score x coverage^2 -> gap-ratio seeds plus per-term guarantee -> BFS depth 3 with hub transit cap -> render at 3 chars/token budget.
- Update: content-hash per file, per-tier replace, debounce 3 s, shrink guard. Delete: prune by source file.
