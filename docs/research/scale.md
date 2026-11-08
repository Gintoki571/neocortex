# neocortex at scale: accuracy and footprint at 3k to 108k memories

Measured 2026-10-27 on base `338884f` (main), release binary, no change to `src/`. Host: i7-9750H (6 cores / 12 threads), 15.8 GB RAM, shared with other lanes (load 5-8 during runs, so wall times and ingest rates are noisy; relative trends are reliable). Real model = UltraBERT-Embed-150M f32 unless stated. Raw logs and regeneration steps stay out of git (`lanes/tmp/nc-scale/`, see "Reproduce").

## 1. Worlds

`cargo run --release --example gen_world -- --people N --seed S --out ...` (new seeds; seed 7 is `eval/world-300.json`). Generation of all three took minutes in total; 10000 people (108,667 memories, 28 MB JSON) needed no workaround. All worlds have 600 queries in the same 7 categories.

| world | people | seed | memories | entities | relations | queries |
|---|---|---|---|---|---|---|
| world-300 (`eval/`, main) | 300 | 7 | 3,264 | 701 | 1,283 | 600 |
| world-1000 | 1,000 | 1001 | 10,833 | 1,510 | 4,265 | 600 |
| world-3000 | 3,000 | 3001 | 32,567 | 3,798 | 12,749 | 600 |
| world-10000 (hash embedder only) | 10,000 | 10001 | 108,667 | 11,681 | 42,562 | 600 |

Entities and relations come from `neocortex stats` on the ingested DBs (the world-10000 numbers from the hash-embedder DB; entity resolution there skips description conflicts, so its entity count is not comparable to the real-model rows).

## 2. Accuracy (real model, f32 DB, 600 queries per world)

`neocortex eval <world> --save-db` once per world, categories as printed by eval (acc@10 / recall / MRR). Ingest is one `remember_batch` pass inside eval.

| | world-300 | world-1000 | world-3000 |
|---|---|---|---|
| ingest wall | 284.0 s | 1,355.3 s | 4,361.4 s |
| ingest rate | 11.5 memories/s | 8.0 memories/s | 7.5 memories/s |
| overall acc@10 | 0.992 | 0.992 | 0.982 |
| overall recall | 0.965 | 0.956 | 0.959 |
| overall MRR | 0.881 | 0.852 | 0.855 |
| aggregation (60) | 1.000 / 1.000 / 1.000 | 1.000 / 1.000 / 0.983 | 1.000 / 1.000 / 0.972 |
| as-of (102) | 1.000 / 1.000 / 0.812 | 1.000 / 1.000 / 0.774 | 1.000 / 1.000 / 0.824 |
| current (102) | 1.000 / 1.000 / 0.889 | 1.000 / 0.951 / 0.858 | 1.000 / 0.961 / 0.838 |
| history (78) | 1.000 / 1.000 / 0.994 | 1.000 / 1.000 / 1.000 | 1.000 / 1.000 / 0.994 |
| temporal-window (60) | 1.000 / 1.000 / 1.000 | 1.000 / 1.000 / 1.000 | 1.000 / 1.000 / 1.000 |
| three-hop (78) | 1.000 / 0.761 / 0.803 | 1.000 / 0.761 / 0.734 | **0.897** / 0.765 / 0.671 |
| two-hop (120) | 0.958 / 0.979 / 0.793 | 0.958 / 0.975 / 0.753 | 0.975 / 0.979 / 0.795 |

Reading: acc@10 holds at 0.992 up to 10.8k memories and 0.982 at 32.6k (11 of 600 queries missed, 8 of them three-hop). Only three-hop acc@10 moves (1.000 -> 0.897). Three-hop recall is flat at 0.76 at every size (not size-dependent). MRR drops 0.026 from 300 to 3000 mostly in three-hop (0.803 -> 0.671) and `current` (0.889 -> 0.838). The 90% goal is not threatened at 32k, but the first crack is multi-hop, not ranking in general. Whether the three-hop drop is caused by size or by this seed's name collisions cannot be separated with one seed per size (the 3000-person world reuses surnames across 3-4 people, e.g. Mateo/Ranjit/Krish Shedhar).

q8 model on the same f32 DBs: world-1000 recall 0.957 / MRR 0.852 / acc@10 0.992; world-3000 0.959 / 0.856 / 0.982 (identical to f32 within 0.001). int8 vectors: world-1000 0.956 / 0.853 / 0.992; world-3000 0.959 / 0.855 / 0.982.

### Miss analysis, world-3000

The same DB re-run with every query at `k=50` (`world-3000-k50.json`, same memories) gives recall@50 0.991 vs recall@10 0.959. Expected memory keys of all queries that were not perfect at k=10 (318 keys): 290 in the top 10, 13 at rank 11-50 (ranking problem, candidate set has it), 15 outside the top 50 (retrieval miss: no channel brought it).

The 11 acc@10 misses (answer string absent from the rendered context), ranks of the expected memories at k=10 -> k=50:

| category | query | ranks k=10 | ranks k=50 | class |
|---|---|---|---|---|
| three-hop | what city is home to the manager of Joanna Sheprasad's sister | 2, -, - | 2, -, - | retrieval miss |
| three-hop | which city does the manager of Joanna Sheprasad's sister live in | 2, -, - | 2, -, - | retrieval miss |
| three-hop | which city does the manager of Daria Varani's brother live in | 1, -, - | 1, -, - | retrieval miss |
| three-hop | where does the manager of Krish Shedhar's brother live | 1, 6, - | 1, 6, - | retrieval miss |
| three-hop | what city is home to the manager of Krish Shedhar's brother | 1, 5, - | 1, 5, - | retrieval miss |
| three-hop | what city is home to the manager of David Chauchand's sister | 1, -, - | 1, -, - | retrieval miss |
| three-hop | what city is home to the manager of Jaya Sinraj's brother | 1, 7, - | 1, 7, - | retrieval miss |
| three-hop | which city does the manager of Noel Sinwani's brother live in | 1, -, - | 1, -, - | retrieval miss |
| two-hop | which city does Ida Khasri's manager live in | -, - | -, - | retrieval miss |
| two-hop | which language is used by Valentina Vohpati's project | 2, - | 2, 23 | ranking miss |
| two-hop | which language is used by Iris Vijpore's project | -, - | 10, 38 | ranking miss |

9 of 11 are retrieval misses: the last hop (the manager's city, the relocation memory) is not in the 50 best candidates, so it is the path beams and the graph channel, not the cutoff, that fail. Example: Krish Shedhar's brother is Dylan Padchand; Dylan's manager is Pedro Tamzadeh (rank 6); the city memory "Pedro Tamzadeh relocated to Hyderabad" is outside the top 50 among 14 Pedro Tamzadeh memories, including an ended "settled in Bogota". Raising `k` would fix only the 2 ranking misses. `agent_sim`/`follow` is the existing remedy for hop 3; this lane did not test it at scale.

## 3. Footprint: real model, 1 and 2 cores

`taskset -c 0` and `taskset -c 0,1`; latency = `eval <world> --load-db <copy>` (600 queries, includes the query embedding); RSS = peak `ru_maxrss` of a cold CLI `recall` via python `RUSAGE_CHILDREN` (page cache warm, DB already in cache); DB copies for int8 (`migrate-vectors --to int8`) and q8 (`quantize-model`, 153 MiB weights, 1.9 s). Latency p50 / p95 in ms.

| config | world | 1 core p50 / p95 | 2 cores p50 / p95 | cold CLI recall wall / CPU, 1 core | peak RSS cold recall | DB bytes (per memory) |
|---|---|---|---|---|---|---|
| f32 DB, f32 model | 300 (3,264) | 165.6 / 222.9 | 105.7 / 165.2 | 0.50 s / 0.39 s | 612 MiB | 21,381,120 (6,551) |
| f32 DB, f32 model | 1000 (10,833) | 268.8 / 341.4 | 200.7 / 248.7 | 0.70-0.85 s / 0.59-0.60 s | 612.3-612.9 MiB | 65,765,376 (6,071) |
| f32 DB, f32 model | 3000 (32,567) | 345.3 / 450.6 | 274.8 / 360.8 | 0.78-0.88 s / 0.66-0.67 s | 618.1-618.5 MiB | 193,556,480 (5,943) |
| int8 DB, f32 model | 1000 | 211.1 / 338.2 | 180.2 / 234.3 | 0.80-0.90 s / 0.58-0.68 s | 614.7-615.2 MiB | 24,981,504 (2,306) |
| int8 DB, f32 model | 3000 | 305.6 / 403.9 | 241.8 / 325.2 | 0.73-0.76 s / 0.61-0.62 s | 618.0-618.5 MiB | 73,199,616 (2,248) |
| f32 DB, q8 model | 1000 | 334.0 / 405.0 | 242.3 / 292.9 | 0.63-0.71 s / 0.51-0.58 s | 236.0-236.4 MiB | 65,765,376 |
| f32 DB, q8 model | 3000 | 382.1 / 508.2 | 315.5 / 416.2 | 0.74-0.78 s / 0.62-0.64 s | 237.4-237.7 MiB | 193,556,480 |
| int8 DB, q8 model | 1000 | 324.6 / 391.7 | 226.8 / 283.8 | 0.69-0.73 s / 0.57-0.61 s | 236.7-237.1 MiB | 24,981,504 |
| int8 DB, q8 model | 3000 | 345.4 / 451.3 | 279.8 / 374.7 | 0.72 s / 0.60 s | 236.7-237.1 MiB | 73,199,616 |

Findings:

- Peak RSS is model-bound: 612 -> 618 MiB (f32) and 236 -> 237 MiB (q8) from 3.3k to 32.6k memories, i.e. about +0.2 MiB per 1,000 memories. The 2-3 GB budget is not threatened by RSS at this scale; q8 stays under 512 MiB. int8 vectors add +2-3 MiB at world-1000 (decoded vectors), nothing visible at 3000.
- Latency grows about linearly with memories: 1-core p50 166 -> 269 -> 345 ms at 3.3k -> 10.8k -> 32.6k (f32). Section 4 shows where it goes at 108k.
- int8 DB is 2.6x smaller (2.25 KB vs 5.9 KB per memory) and faster than f32 (305.6 vs 345.3 ms p50 at 3000, 1 core) with identical accuracy; the vec0 scan reads a quarter of the bytes. q8 model costs +11% (3000, f32 DB) to +13% (3000, int8 DB) p50 on 1 core for 381 MiB less RSS.
- 2 cores buy 20% at 3000 (345 -> 275 ms f32), less than at world-300 (166 -> 106 ms, 36%): the growing part (KNN scans, SQL) is single-threaded.
- The cold CLI recall CPU grows 0.39 -> 0.60 -> 0.66 s over the same range (snapshot build + KNN scans), on top of the model load.

## 4. Structural scale at 108,667 memories (hash embedder)

`NEOCORTEX_EMBEDDER=hash` (accuracy numbers below are meaningless and not reported as quality; they only prove the run is functional). The hash embedder removes the model forward pass, which isolates the storage/graph cost.

How the DB was built. The true write path (`remember_batch`) is O(N) per memory because of two brute-force KNN scans over `memories_vec` (below); at 108k memories it would take many hours. The true path was run for 54 minutes and reached 36,608 memories (19.6/s in the first 15 min, 10.6/s at 23k, then dropping) before it was stopped. The 108k DB for recall measurements was then built with a scratch binary that skips those two write-time scans (`NEOCORTEX_SKIPKNN`, not committed): 108,667 memories in 7,282 s (14.9/s average, about 10/s at the end of the run). It differs from a true-path DB only in that near-duplicate memories are not detected (irrelevant to the hash embedder) and `related` hints were not computed. DB: 649,105,408 B f32 (5,973 B/memory), 249,704,448 B after `migrate-vectors --to int8` (2,298 B/memory).

### Recall, 600 queries, `eval --load-db`

| config | p50 / mean / p95 | notes |
|---|---|---|
| f32, `taskset -c 0` | 694.3 / 1,068.0 / 2,451.6 ms | peak RSS of the eval process 169.8 MiB |
| f32, `taskset -c 0,1` | 629.6 / 964.9 / 2,186.9 ms | peak RSS 170.1 MiB |
| cold CLI recall, 1 core / 2 cores | 1.27-1.36 s / 1.23-1.27 s wall; CPU 1.14 s / 1.10-1.14 s | peak RSS 32.4-32.7 MiB (no model) |

First 120 queries only (core 10), same DB with and without the index suggested in "Bottleneck":

| config | p50 / mean / p95 |
|---|---|
| f32, as is | 680.9 / 1,079.9 / 2,354.9 ms |
| f32 + `CREATE INDEX idx_rel_subj ON relations(subj, predicate_id)` (copy of the DB, 0.09 s to build) | 434.7 / 435.3 / 484.2 ms |
| int8 + the index | 314.9 / 322.3 / 370.7 ms |

Results are identical with and without the index (recall 0.792, acc@10 0.825 on those 120 queries; int8 0.817 by hash-vector rounding).

### Where recall time goes at 108k (stage timers, 1 core, 600 queries, ms)

Timers are a scratch patch (`NEOCORTEX_TIMING`, applied to a copy of the source, never committed); stage means sum to the eval mean (1,064 vs 1,064 ms).

| stage | function | p50 | mean | p95 |
|---|---|---|---|---|
| BM25 | `recall::recall` FTS5 | 14.1 | 13.3 | 19.1 |
| memory KNN (k=max(5k,50)) | sqlite-vec `memories_vec` MATCH | 190.0 | 189.5 | 194.5 |
| as-of snapshot rebuild (cached otherwise) | `recall::snapshot` -> `kg::snapshot_with` | 0.0 | 37.7 | 83.4 |
| entity KNN | `entities_vec` | 18.6 | 18.5 | 19.3 |
| relation-fact seeds | `relations_vec` KNN | 74.4 | 75.4 | 87.0 |
| PPR push (ACL) | `graph::ppr_push` | 0.5 | 0.5 | 0.7 |
| path beams (lexical + semantic, relation KNN k=100) | `paths::search`, `paths::search_semantic` | 77.7 | 77.5 | 81.1 |
| fuse, rerank off, pack | - | 1.8 | 1.8 | 2.0 |
| access write | `Db::write` (WAL commit) | 0.3 | 21.8 | 0.4 (max 2,264) |
| **facts for the top-10 entities** | **`recall::facts_for_view`** | **249.7** | **617.3** | **1,987.6** |
| next hops | `recall::next_hops` | 0.0 | 1.2 | 9.5 |

### Bottleneck

Dominant stage: `recall::facts_for_view` (src/recall.rs), 58% of mean latency at 108k and the whole p95. It loads every relation of the 10 top entities (tools such as Kotlin/Go have 400-800 relations each: p50 121, mean 297, max 1,980 rows per call) and for each row runs the `current_values` query `SELECT count(*) FROM relations WHERE subj=?1 AND predicate_id IN (SELECT id FROM predicates WHERE group_id=?2) AND polarity=1 AND expired_at IS NULL AND valid_from<=?3 AND (valid_to IS NULL OR valid_to>?3)` before `.take(8)`. `relations` has indexes only on `(src, predicate_id)` and `(dst)`; the planner reports `SCAN relations`, 2.2 ms per row at 42.5k relations (measured on 120 queries: mean 646 ms of the 661 ms spent in the function is this count; the per-row `relations_vec` cosine fetch is 12 ms). Cost is rows x |relations|, i.e. roughly quadratic in graph size because hub fan-in grows with N.

The same unindexed `subj` predicate sits in the write path (`remember.rs` exclusivity and conflict queries, 6.6 ms per call at 42.5k relations by direct timing).

Next dominant stages once the scan is gone (index added, 120 queries, p50 435 ms): memory KNN 190 ms (44%), the two relation KNNs about 75 + 78 ms (35%), then entity KNN 19 ms, BM25 14 ms; int8 vectors cut the total to 315 ms. PPR and the graph walk are negligible (0.7 ms); they are not bottlenecks at 100k.

Cold CLI recall at 108k (1.3 s wall) splits into facts 686 ms (same count query), memory KNN 184 ms, snapshot build 79 ms, relation seeds 64 ms, path beams 71 ms, entity KNN 17 ms, BM25 21 ms.

### Ingest (write path) scaling

Per-memory write cost, ms (`write_prepared`, hash embedder, same machine):

| memories in DB | knn_memory (k=1 dedupe) | k=10 related block | resolve_entity | temporal links | total write |
|---|---|---|---|---|---|
| 1,000 | 0.78 | 0.61 | 2.5 | 0.5 | 6.9 |
| 4,000 | 5.8 | 4.0 | 8.7 | 1.7 | 26.0 |
| 8,000 | 12.9 | 8.3 | 10.6 | 2.1 | 43.9 |
| 14,000 | 22.8 | 17.2 | 7.4 | 2.4 | 63.3 |

Both memory scans (`remember::knn_memory`, and the `k=10` block that runs when a memory has entities but no relations) grow ~1.6 ms per 1,000 memories each: ~170 ms + ~140 ms per remember extrapolated to 108k, before the model forward pass. With both skipped (scratch build) the total still grows: 25 ms at 20k, 51 ms at 58k, 84 ms at 92k, with `resolve_entity` falling to 0.2 ms and temporal links rising 2.9 -> 8.3 ms; the relation loop is only 1.5-2.7 ms of it at 2k-22k memories (not timed at 100k). Most of the ~75 ms per memory at 100k is therefore in code not itemised here (memory row / FTS / vec0 insert, mentions, gazetteer, graph projection bookkeeping); it was not chased further.

Real-model ingest rate fell from 11.5/s (3.3k) to 8.0/s (10.8k) to 7.5/s (32.6k) even though the model forward dominates; a 100k real-model ingest would run at roughly 3-4/s by the same trend (8 h), not measured.

## 5. Recommendations (nc-scale proposed; items 1 and 2 implemented in nc-perf, see below)

1. Add `CREATE INDEX idx_rel_subj ON relations(subj, predicate_id)` as an additive schema step, and compute `current_values` after `.take(limit)` in `facts_for_view`. Measured on the 108k DB: p50 681 -> 435 ms, p95 2,355 -> 484 ms, identical results. Also speeds up `remember` on relation writes.
2. Make the write-time memory scans cheaper than O(N): `knn_memory` (k=1) and the `k=10` related block each scan all of `memories_vec`. Candidates: run one k=10 scan and derive the k=1 hit from it; or skip the related scan when the near-dup scan found nothing above the update threshold.
3. Recall at 100k then costs ~435 ms (f32) / ~315 ms (int8) on one core for the hash embedder, before the query embedding. The remaining linear terms are the three vec0 brute-force scans (memories, relations twice); int8 helps, a coarse pre-filter (entity-restricted KNN) would be the next lever.
4. Multi-hop at 32k: three-hop acc@10 fell to 0.897 from path-beam misses at hop 3. Test `agent_sim` / `follow` on world-3000 before changing retrieval.

## After nc-perf

Implemented from section 5: (1) `idx_rel_subj ON relations(subj, predicate_id)` in `schema.sql` and in `Db::ensure_additive_tables` (every v4 open; no version bump), and `facts_for_view` counts `current_values` only for the rows that survive `.take(limit)`; (2) `remember` runs one `memories_vec` scan: k=10 when the input has no relations (else k=1), the nearest row is the near-duplicate check and, for a stored memory, the first nine rows are the `related` candidates (the old second scan ran after the memory was inserted, so it was its own nearest row).

`EXPLAIN QUERY PLAN`, before -> after (world-300 DB, index added by hand for "after"):

| query | before | after |
|---|---|---|
| `current_values` count (recall) | `SCAN relations` | `SEARCH relations USING INDEX idx_rel_subj (subj=? AND predicate_id=?)` |
| write path, exclusive-ending (`remember.rs`) | `SCAN r` | `SEARCH r USING INDEX idx_rel_subj (subj=?)` |
| write path, conflict candidates (`remember.rs`) | `SCAN r` | `SEARCH r USING INDEX idx_rel_subj (subj=?)` |

Measured on this host, `taskset -c 0`, interleaved main/branch runs, both binaries on a copy of the same main-built DB (the branch adds the index on open):

| set | metric | main | nc-perf |
|---|---|---|---|
| world-300, real model, 5 paired runs | recall p50 (median) | 183.13 ms | 177.63 ms |
| world-3000 (hash embedder, 32,567 memories), 3 paired runs | recall p50 / mean / p95 | 204.41 / 211.50 / 271.05 ms (run 1) | 137.83 / 148.21 / 153.29 ms (run 1) |
| world-3000 (hash), ingest (both ingests ran concurrently on separate cores) | 32,567 memories | 2,517.7 s (12.9/s) | 2,015.8 s (16.2/s) |
| world-300, real model, ingest | 3,264 memories | 466.3 s | 461.9 s |

The world-300 p50 gain is small because that graph has only 1,283 relations; the effect grows with hub fan-in and graph size (world-3000: recall p50 -33%, p95 -44%; ingest -20%). Results are identical: acc@10 / recall / MRR and every per-category row match main on world-300 (0.992 / 0.965 / 0.881) and world-3000 (0.898 / 0.837 / 0.603), and the ingested DBs match table by table (`memories`, `entities`, `relations`, `mentions`, `memory_links`, `relation_evidence` row hashes equal on both worlds). Not re-measured at 108k (the DB from nc-scale was a scratch build).

## 6. Real model at 108,667 memories (nc-scale100k)

world-10000 (seed 10001) ingested with the real model into an int8 DB (`NEOCORTEX_VEC=int8`, `eval --save-db`) by the nc-perf build 5e3e082, before ER-08: 28,997 s (8.1 h, 3.7 memories/s average). Ingest rate by DB size: 13.5/s (0-11k), 8.1/s (30k), 6.3/s (50k), 4.5/s (70k), 2.4/s (90k), 1.1/s (100k-108k); the write path is still super-linear (open item WRITE-100K). `stats`: 108,667 memories, 11,677 entities, 42,561 relations, DB 261.5 MiB (2,522 B/memory).

Queried with main bf73846 (`eval --load-db`, a copy of the DB; the host was otherwise idle, load 1-4):

| | 1 core | 2 cores |
|---|---|---|
| recall p50 / p95 / mean | 478.3 / 555.4 / 484.8 ms | 395.1 / 442.1 / 398.3 ms |

| category | q | acc@10 | recall | MRR | agent_sim --hints |
|---|---|---|---|---|---|
| aggregation | 60 | 1.000 | 1.000 | 0.972 | 60/60 |
| as-of | 102 | 1.000 | 1.000 | 0.818 | 102/102 |
| current | 102 | 1.000 | 0.990 | 0.870 | 102/102 |
| history | 78 | 1.000 | 1.000 | 0.994 | 78/78 |
| temporal-window | 60 | 1.000 | 1.000 | 1.000 | 60/60 |
| three-hop | 78 | 0.987 | 0.812 | 0.659 | 77/78 |
| two-hop | 120 | 0.892 | 0.933 | 0.777 | 113/120 |
| overall | 600 | 0.977 | 0.961 | 0.854 | 592/600 = 0.987 (1.04 calls/query) |

Cold CLI recall, 1 core (`taskset -c 0`, python `RUSAGE_CHILDREN`): f32 model peak RSS 633 MiB (wall 1.2 s), q8 model 237 MiB (wall 3.8 s). RSS stays model-bound at 108k. Context tokens per query 427.5. Two-hop is the weakest category (0.892 one recall, 0.942 with follow); the DB predates ER-08, so some of those misses may be false person merges (world-10000 reuses surnames more than world-3000); re-ingest with ER-08 before chasing them (open item TWO-HOP-100K).

## After nc-hop3: the third hop is ended by a false entity merge

The world-3000 three-hop misses are not a fan-out or depth limit. Every one of the 8 replicated misses (6 unique
queries; the real-model world-3000 DB, `NEOCORTEX_DEBUG_HOPS` dump) has the same shape: hop 1 (the sibling) is a
live fact and is found at rank 0-1, and the next hop is **ended**, so `paths::incident` drops it (`vis.shows`),
the walk takes the *other* live fact on the same entity, and hop 3 is never reached.

| query (answer) | hop 1 | hop 2 | hop 3 | chain the beams take |
|---|---|---|---|---|
| Joanna Sheprasad's sister (Sofia) | r4265 live, hit #0 | r1452 "Anouk Kridhar's manager is Jason Darani" **ended** | r4302 unreachable | "Anouk Tridhar's manager is Max Dalgan" -> Aurangabad |
| Daria Varani's brother (Accra) | r5820 live, hit #0 | r1923 "Rajesh Kohvari's manager is Maja Deshsri" **ended** | r5396 unreachable | "Rajesh Mohvari's manager is Atul Sahrao" -> San Francisco |
| Krish Shedhar's brother (Hyderabad) | r7668 live, hit #0 | r3262 live | r7247 "Pedro Tamzadeh lives in Hyderabad" **ended** | "Pedro Samzadeh lives in Thessaloniki" |
| David Chauchand's sister (Kuwait City) | r4398 live, hit #0 | r1631 "Fiona Chazada's manager is Ravi Dharzada" **ended** | r5287 unreachable | "Fiona Chauzada's manager is Jason Raovari" -> Doha |
| Jaya Sinraj's brother (Dakar) | r3871 live, hit #0 | r2938 live | r5428 "Sara Banwani lives in Dakar" **ended** | "Sara Banani lives in Thiruvananthapuram" |
| Noel Sinwani's brother (Nagoya) | r6569 live, hit #0 | r4110 "Deepak Wagrao's manager is Adam Zainath" **ended** | r7376 unreachable | "Deepak Ragrao's manager is Lucia Charani" -> Delhi |

Each pair is two different people whose names the ER-07 near-name layer merges (same first name, surnames one
Damerau-Levenshtein edit apart, Jaro-Winkler >= `name_merge` 0.92, no blocker): Anouk Kridhar/Anouk Tridhar,
Rajesh Kohvari/Rajesh Mohvari, Pedro Tamzadeh/Pedro Samzadeh, Fiona Chazada/Fiona Chauzada, Sara Banwani/Sara
Banani, Deepak Wagrao/Deepak Ragrao. The merged entity has one `manager` and one `live in` slot, so the second
person's exclusive fact ends the first person's (`remember`'s exclusive ending). Re-ingesting only the 52
world-3000 memories that name Pedro Tamzadeh or Pedro Samzadeh with the real model reproduces it: one entity
"Pedro Samzadeh" (`mention_count` 28) and `Pedro Tamzadeh lives in Hyderabad` with `valid_to` set. The ending is
a write-path artifact of the merge, not a fixture fact: the memory "Pedro Tamzadeh relocated to Hyderabad." has
no `valid_to`.

Resolution: fixed at the write side by nc-er08 (main bf73846; a typo-sized near-name merge needs corroboration). Re-ingested world-3000 with the real model: acc@10 0.997, three-hop 1.000, two-hop 0.983, false merges 26 -> 0. nc-hop3's read-side knobs stay on branch `lane-nc-hop3` (cd957ea), not merged: `hop_ended` is no longer needed; `hop_last_open` (the hop-3 fact is outside the question's 100 nearest relations for 78 of 78 world-300 three-hop chains; opening low-degree entities lifts world-300 three-hop recall 0.761 -> 0.812, acc@10 unchanged) is an open item.

## After nc-write100k: the write path stops scanning the corpus

Section 4 found the two write-time memory scans growing ~1.6 ms per 1,000 memories each and ~75 ms per
remember at 100k in steps it did not itemise. nc-write100k added per-step timers
(`NEOCORTEX_WRITE_TIMING=<file>`, `src/wtiming.rs`: 11 `item` stages and a `chunk` line), measured a DB grown
to 102,656 memories (hash embedder, world-10000 slices, 1,024 fresh inputs per size) and replaced the stages
that grow with N. Base = main d4c4bd1 with the timers added and nothing else; both builds run interleaved on
copies of the same DBs. Wall rates on this host swing with the other lanes (the same build measured 8 and 22
memories/s an hour apart), so CPU per memory is the stable figure.

Per memory, ms:

| stage | 13k base | 13k branch | 50k base | 50k branch | 102k base | 102k branch |
|---|---|---|---|---|---|---|
| knn (near-dup + `related` candidates) | 21.90 | 5.73 | 83.65 | 7.58 | 285.29 | 13.74 |
| graph (graphqlite projection) | 16.00 | 1.98 | 51.85 | 2.35 | 158.94 | 2.41 |
| entities | 7.63 | 7.04 | 1.78 | 0.24 | 0.83 | 0.38 |
| relations | 2.59 | 1.25 | 9.78 | 0.70 | 7.70 | 0.13 |
| gazetteer | 0.83 | 0.51 | 2.92 | 0.95 | 5.94 | 0.79 |
| tlinks | 1.62 | 1.55 | 0.34 | 1.08 | 1.16 | 0.65 |
| mem/vec insert, mentions, retractions, finalize | 0.22 | 0.22 | 1.35 | 0.37 | 1.31 | 0.37 |
| item total | 48.74 | 18.22 | 140.27 | 12.52 | 449.76 | 18.31 |
| CPU per memory (process) | 46.6 | 16.5 | 127.5 | 11.0 | 289.0 | 17.8 |

The smallest column is 13k rather than the requested 10k: it is the first size both builds ran with the stage
timers. The base build's separate 10k run measured p50 39.9 ms against 43.6 ms at 13k, so nothing turns over
between the two.

Three changes:

- **knn.** `memories_vec` KNN (k=1 for the duplicate check, k=10 for `related`) compared every stored vector.
  `remember::neighbour_candidates` now collects a bounded set - the newest 128 FTS hits on the memory's
  longest content tokens, the newest 256 mentions of each mentioned entity, the newest 512 memories, cap 1024 -
  and `knn_among` scores it by cosine over point reads. 285 ms -> 14 ms at 102k.
- **graph.** Every edge was projected with `MATCH (a:...), (b:...) MERGE (a)-[r]->(b)`; graphqlite planned that
  as a cartesian product, so one projection walked every node (a memory with 3 mentions and 3 relations paid it
  six times). One `MERGE (a:...)-[r:...]->(b:...)` statement per edge: 159 ms -> 2.4 ms at 102k.
- **tlinks.** The `mentions` JOIN `memories` with `GROUP BY m.id` became an `idx_memories_valid` range scan with
  an EXISTS probe on the `mentions` primary key.

The stages that did not change shrank at 102k anyway because the old global KNN dragged 620 MB of vector blobs
through the page cache for every memory: relations 7.70 -> 0.13 ms, gazetteer 5.94 -> 0.79 ms, the inserts
1.3 -> 0.4 ms. That is cache damage, not a code change.

The residual knn cost is the point reads themselves: sqlite-vec answers `WHERE rowid=?` by reading chunk blobs
from the newest end, so an old row costs more than a recent one (31 us at the tail vs up to 1.7 ms at the head of
a 90k DB, measured row by row with a scratch probe). The FTS arm is bounded by `LIMIT` but not free on a cold
cache: `ORDER BY rowid DESC` over a large match set cost 245 ms cold / 7.7 ms warm on the 90k probe DB.

Ingest by corpus size, hash embedder, 1,024 fresh inputs at each size, batch, `taskset -c 0` (per-memory p50
from the item timers; rate = 1,000 / mean item time - other lanes shared the host, so wall rates run 2-2.5x
slower than this):

| memories | main p50 | main rate | branch p50 | branch rate |
|---|---|---|---|---|
| 13k | 43.6 ms | 20.6/s | 10.8 ms | 55.3/s |
| 50k | 128.0 ms | 7.1/s | 9.9 ms | 80.1/s |
| 102k | 360.7 ms | 2.2/s | 19.5 ms | 54.7/s |

The branch stays at 10-20 ms per memory at every size; the spread between its rows is host contention.

Ingest at 102,656 memories, hash embedder, 1,024 fresh inputs, `taskset -c 0` (the wall-time runs):

| metric | main | branch |
|---|---|---|
| batch `remember_batch`, wall rate | 2.11/s | 22.18/s |
| batch, p50 / mean / p95 per memory | 434.81 / 473.95 / 584.64 ms | 43.85 / 45.08 / 47.01 ms |
| batch, CPU per memory | 289.0 ms | 17.8 ms |
| single `remember`, p50 (3 interleaved pairs) | 432.39 ms (450.02 / 432.39 / 262.55) | 19.61 ms (31.20 / 19.61 / 19.57) |
| single `remember`, CPU per memory of the 3 runs | 445 / 400 / 259 ms | 31 / 20 / 20 ms |
| peak RSS | 20.0 MiB | 20.2 MiB |
| DB after the 1,024 writes | 624,635,904 B | 624,635,904 B |

Correctness at 102,656 memories (`wdcheck` / `wdnears` scratch probes, both builds on copies of the same DB):

| check | main | branch |
|---|---|---|
| exact repeat of old m66777 (115 chars) | `duplicate_of=m66777`, near none | same |
| one-letter variant of m66777 (cosine 0.979) | `near=(66777, 0.9789604)` | `near=(66777, 0.97896093)` |
| exact repeat of old m51328 (61 chars) | `duplicate_of=m51328`, near none | same |
| one-letter variant of m51328 (cosine 0.9366) | near none | near none |
| 40 sampled replays, exact duplicates | - | 0 differ |
| 40 sampled replays, near target | - | 1 differs (same text: m97524 vs the replay copy m102713) |
| 40 sampled replays, `related` | 20 non-empty | 2 differ (m15399 3 -> 0, m69293 3 -> 1) |

A one-letter edit of a 61-character sentence is 0.94 cosine on the hash embedder, below `near_dup` 0.97 on both
builds; the near band is only reachable for longer texts. The two `related` losses are the real cost of the
bounded set: a replay of an old memory whose related neighbours are older than the 256 newest mentions of the
entity (and outside the FTS top 128 and the newest 512) no longer lists them.

world-3000 (seed 3001, 32,567 memories, hash embedder), every input re-ingested through both builds (branch
2,895 s = 11.3/s; the main wall time is not quoted, the run was paused for ~2.5 h):

- `duplicate_of`: 0 of 32,567 differ.
- `near_duplicate`: 0 of 32,567 differ (id and cosine, tolerance 1e-5).
- `related`: 260 of the 6,294 non-empty lists differ. 163 hold the same ids in another order, 35 gain one
  neighbour (a candidate the old k=10 scan cut off - it spent one of its ten rows on the memory itself), 62
  swap one entry for another with the same cosine (a tie the two paths break differently), and 0 lose an entry
  main listed. On inputs with the same id order the cosines drift by <= 1e-6 (sqlite-vec's KNN and the
  point-read cosine sum the same f32 products in another order).
- Tables: both databases are 192,765,952 B with the same counts and aggregates - 32,567 memories, 3,826
  entities, 70,860 mentions, 12,750 relations (all live), equal id sums and content lengths.

So the bounded candidate set keeps every near-duplicate decision on this world, and changes only which of
several near-equal neighbours `related` lists.

Gates on world-300 with the real model are unchanged (final build, `eval/world-300.json --save-db`): acc@10
0.992, recall 0.965, MRR 0.881 - the same numbers as main d4c4bd1 - with every category at acc@10 1.000 except
two-hop 0.958; ingest 3,264 memories in 475 s (contended with another lane's model test). Recall latency on
the saved world-300 DB, 600 queries, 1 core, 3 interleaved pairs (`eval --load-db` on copies): main p50
253.5 / 230.0 / 172.5 ms, branch 224.7 / 223.4 / 312.5 ms (medians 230.0 vs 224.7; the third branch run hit
another lane's build). The read path does not touch this lane's code.

## After nc-scale2: 108,667 real-model memories on today's main

The section 6 ingest predates nc-write100k and was queried by bf73846. world-10000 (seed 10001) was re-ingested
with the real model into an int8 DB (`NEOCORTEX_VEC=int8`, `eval --save-db`) by main fdcad90 - nc-perf 5e3e082,
nc-write100k 06b695c, nc-er08 bf73846 and nc-inverse 717325e all in it - and measured with that build's binaries
on copies of the DB on 2026-10-29/30.

Ingest: 108,667 memories in 14,192 s (3.9 h, 7.66/s), from the log's progress lines:

| memories | rate |
|---|---|
| 0-10k | 3.07/s |
| 10k-20k | 6.85/s |
| 20k-30k | 10.11/s |
| 30k-40k | 9.19/s |
| 40k-50k | 9.19/s |
| 50k-60k | 10.61/s |
| 60k-70k | 9.83/s |
| 70k-80k | 9.56/s |
| 80k-90k | 9.16/s |
| 90k-100k | 8.48/s |
| 100k-108.7k | 8.87/s |

The write path no longer falls off with corpus size: after the slow first block (3.07/s; the log does not split
model load, page-cache warmth and host load) the rate stays between 6.9 and 10.6 memories/s to 108k, against
28,997 s (3.7/s average, 1.1/s in the last 9k) for the pre-nc-write100k build in section 6. `stats`: 108,667
memories, 11,909 entities, 42,571 relations, 1,388 similar, 27 predicates; DB file 274,010,112 B (2,522 B per
memory, int8).

Accuracy (`eval --load-db`, 600 queries):

| category | q | acc@10 | recall | MRR | agent_sim --hints |
|---|---|---|---|---|---|
| aggregation | 60 | 1.000 | 1.000 | 0.972 | 60/60 |
| as-of | 102 | 1.000 | 1.000 | 0.818 | 102/102 |
| current | 102 | 1.000 | 0.980 | 0.881 | 102/102 |
| history | 78 | 1.000 | 1.000 | 1.000 | 78/78 |
| temporal-window | 60 | 1.000 | 1.000 | 1.000 | 60/60 |
| three-hop | 78 | 1.000 | 0.812 | 0.666 | 78/78 |
| two-hop | 120 | 0.950 | 0.971 | 0.784 | 120/120 |
| overall | 600 | 0.990 | 0.966 | 0.859 | 600/600 = 1.000 (1.01 calls/query) |

Against section 6 (0.977 / 0.961 / 0.854, agent_sim 0.987): three-hop acc@10 is 1.000 (was 0.987) because ER-08
removed the false person merges that ended hop-2 facts ("After nc-hop3"); its recall is still 0.812, the hop-3
retrieval limit, unchanged. Two-hop acc@10 is 0.950 (was 0.892) and agent_sim answers all 120. The other
non-perfect rows are `current` recall 0.980 and `as-of` MRR 0.818.

Latency (`eval --load-db` on a copy; includes the query embedding; host shared with other lanes at load 3-10):

| | 1 core | 2 cores |
|---|---|---|
| recall mean / p50 / p95 | 508.4 / 473.3 / 919.7 ms | 452.2 / 416.8 / 614.2 ms |

Both latency rows are two separate `--load-db` passes on one copy, taken after another lane's real-model eval
finished; load was 2.8 rising to 6.4 during the 1-core pass, so its p95 (920 ms) contains a lane burst while its
p50 is stable (an earlier 1-core run under load 7-10 gave p50 475.0 / p95 589.6 ms; an earlier 2-core run p50
437.4 / p95 1,287.9 ms). At p50 the read path matches section 6 (473/417 vs 478/395 ms on the same size): this
lane does not touch it. Cold CLI recall (page cache warm; `RUSAGE_CHILDREN`): f32 model peak RSS 632.3 MiB, q8
model 237.2 MiB, both unchanged from section 6 (633 / 237) - RSS is model-bound at 108k.

### Two-hop misses: one query phrasing, ranking class

The eval reports 45 two-hop queries with a non-rank-1 expected memory; only 6 have the answer string absent from
the rendered context (acc@10 0.950 = 114/120). All 6 are "which language is used by <person>'s project" (5 of the
5 queries in that exact shape) or its "what is the main language of the project led by <person>" variant (1 of
2); both variants name the person, where the passing shapes name the project ("what language does the project X
leads use": 5 of 5 pass). Hop 1 (the `leads` fact) is ranked 2-6; hop 2 (the language memory) is inside the top
50 but far below 10, so every one is a ranking miss:

| query (answer) | hop 1 | hop 2, rank at k=50 | class |
|---|---|---|---|
| Divya Yadwani (Kotlin) | w076433, rank 3 | w011731, rank 27 | ranking |
| Gauri Mispore (Python) | w018038, rank 2 | w009238, rank 24 | ranking |
| Saanvi Deshrao (Java) | w067002, rank 2 | w001399, rank 39 | ranking |
| Luis Royrao (C#) | w068880, rank 2 | w005293, rank 25 | ranking (see below) |
| Esther Sahzada (Scala) | w076795, rank 2 | w011823, rank 39 | ranking |
| Daria Yadnath (Ruby) | w094289, rank 6 | w006636, rank 28 | ranking |

No miss is a retrieval miss (all six hop-2 memories are in the top 50) or caused by a person merge: each of the
six people is one `person` entity with a live `leads` relation to the expected project in the DB (a full
`merge_audit` roster run at 108k needs another multi-hour ingest and was skipped, since no person-level merge is
involved). The pattern is the same for all six: w009238 "Iris Platform is written in Python" carries the live
relation "Iris Platform uses Python" (r3026), the project's entity card lists it, and `recall` for the question
renders the hop-1 fact but not the hop-2 fact in the top 10. The question's "is used by" flips the stored `uses`
direction while asking from the person's side; the fact/path channels stop at hop 1 for this phrasing.

One miss also sits on a real entity-resolution defect, found while checking: "C#" and "C++" both normalise to
`c` (`text::norm` drops the punctuation), so they are one `tool` entity (id 260, name "C++", 803 mentions: r1717
"Quasar Signal uses C#" and every C++ fact share it). The relation text is right, but the entity name and its
card are not. Not fixed here; open item TOOL-NAMES.

## 7. 300k (hash embedder, main 159a461)

Does the read path still answer under 1 s on one core at 293k memories, and what grows between 108k and 293k. Measurement lane nc-scale300k, no code change; binaries `neocortex-clean-159a461` and `neocortex-timing-159a461` (the section 4/5 scratch timing patch, `NEOCORTEX_TIMING`), both built from 159a461.

World and DB. `gen_world --people 24000 --seed 28001` (260,793 memories) and `--people 3000 --seed 28002` (32,519) concatenated: `world-293k.json`, 293,312 memories, 600 queries, 81.9 MB (each half's queries reference only its own memories). Ingested by `NEOCORTEX_EMBEDDER=hash neocortex eval world-293k.json --save-db db293k-int8.db`; int8 vectors; 293,305 memories stored (7 inputs folded as near-duplicates). Kept for later lanes at `lanes/tmp/nc-scale300k/db293k-int8.db` (764 MB) and `lanes/tmp/nc-scale300k/world-293k.json`.

| | 108k (hash, section 4) | 293k |
|---|---|---|
| memories | 108,667 | 293,305 |
| entities / relations | - | 30,970 / 114,931 |
| mentions / memory_links | - | 651,587 / 995,786 |
| DB bytes (int8) | 249,704,448 | 763,772,928 |
| bytes per memory | 2,298 | 2,604 (+13%) |

Where the 2,604 B/memory goes (`dbstat`): `memories_vec` 227.6 MB (776 B per row: int8[768] + rowid), `relations_vec` 89.2 MB, `entities_vec` 24.0 MB - 341 MB, 45% of the file - then `node_props_text` 71.5 MB + its index 34.9 MB, `memories` 35.4 MB, `memory_links` 25.4 MB + its index 17.2 MB, `edges` 18.1 MB + indexes 34.6 MB. The vector share is why the read path is a bandwidth problem, not a graph problem.

Ingest: 293,305 memories in 44,360 s (12.3 h, 6.61/s average), rate per 20k block from the log:

| block | rate | block | rate |
|---|---|---|---|
| 0-20k | 20.5/s | 140k-160k | 11.9/s |
| 20k-40k | 19.9/s | 160k-180k | 11.0/s |
| 40k-60k | 19.0/s | 180k-200k | 9.5/s |
| 60k-80k | 18.1/s | 200k-220k | 9.8/s |
| 80k-100k | 16.8/s | 220k-240k | 9.9/s |
| 100k-120k | 15.1/s | 240k-259k | 9.9/s |
| 120k-140k | 12.9/s | 259k-279k | 1.1/s |
| | | 279k-293k | 1.9/s |

The last two rows are not neocortex: from ~267k (t=21,907 s) the host's disk was saturated by other lanes' builds (21-35 `rustc`, `/proc/pressure/io` some 70-97%), the process sat in state D on `folio_wait_bit_common` and read at 4.7 KB/s; 0-259k took 18,741 s (13.8/s), the last 34k took 25,619 s (1.3/s). On the write path itself the rate at 240k-259k is 9.9/s against 20.5/s at 20k, i.e. 2.1x slower per memory over 13x the corpus and flat from 200k: sub-linear, no cliff (nc-write100k's bounded candidate set holds).

Read path. 600 queries, `eval --load-db` on a copy, `taskset -c 0`, `systemd-run --user --scope -p MemoryMax=2G`, host shared with the other lanes (load 27-260):

| pass | mean / p50 / p95 | peak RSS | wall |
|---|---|---|---|
| A, clean build | 926.4 / 941.4 / 1,201.2 ms | 433.4 MiB | 574.4 s |
| B, repeat (noise band) | 936.1 / 948.8 / 1,288.0 ms | 426.9 MiB | 571.1 s |
| timing build (stage timers) | 964.7 / 940.9 / 1,554.5 ms | 436.7 MiB | 598.7 s |

`NEOCORTEX_VEC_PREFILTER` does not exist in 159a461 (`grep -rn` over `src/` finds nothing), so both A and B ran with it unset and B is the run-to-run band: p50 +0.8%, p95 +7.2%, wall +0.6%. Two choices were forced by the host: the DB copy lived on `/dev/shm` (a disk-backed pass at load 164 spent its time in state D at 7 MB/s), and the scope carried `-p CPUWeight=10000` so the measured process got a full core (checked: state R, 291 s CPU in 300 s wall); a dedicated 2-core box gives the same. Accuracy is the hash embedder's and is a sanity check only - identical in all three passes:

| category | q | acc@10 | recall | MRR |
|---|---|---|---|---|
| aggregation | 60 | 0.817 | 0.767 | 0.561 |
| as-of | 102 | 0.559 | 0.510 | 0.305 |
| current | 102 | 0.990 | 0.853 | 0.538 |
| history | 78 | 0.641 | 0.532 | 0.445 |
| temporal-window | 60 | 0.850 | 0.750 | 0.617 |
| three-hop | 78 | 1.000 | 0.863 | 0.759 |
| two-hop | 120 | 0.975 | 0.967 | 0.774 |
| overall | 600 | 0.838 | 0.758 | 0.572 |

Context 423.9 tokens/query, hints 17/600, `expect_absent` leaked 2/151. Same numbers as the ingest run's own eval, so `--load-db` scores what `--save-db` built.

Stage timers (1 core, 600 queries, mean ms; the total row is the eval's own mean, the stage rows are the timer means - they sum to it within 0.3 ms at 293k and 13 ms at 108k). The 3.3k column is the same binary on `eval/world-300.json` (3,264 memories, 200 queries) and shows which stages are per-query overhead rather than N:

| stage | function | 3.3k | 108k (section 4, f32, pre-nc-perf) | 293k | x108k->293k |
|---|---|---|---|---|---|
| memory KNN (k=5000) | `recall::recall` `memories_vec` MATCH | 6.4 | 189.5 | 370.5 | 1.96 |
| path beams | `paths::search` + `paths::search_semantic` | 5.3 | 77.5 | 153.1 | 1.98 |
| relation-fact seeds | `recall::relation_seeds_with_config` | 3.4 | 75.4 | 147.0 | 1.95 |
| as-of snapshot (cached; p50 0.1) | `recall::snapshot` -> `kg::snapshot_with` | 1.1 | 37.7 | 121.2 | 3.22 |
| entity KNN | `entities_vec` MATCH | 1.4 | 18.5 | 54.8 | 2.96 |
| BM25 | FTS5 `memories_fts` | 1.2 | 13.3 | 37.1 | 2.79 |
| temporal window scan | `recall::recall` (window queries) | 0.4 | - | 34.4 | - |
| PPR push (ACL) | `graph::ppr_push` | 0.3 | 0.5 | 16.3 (p50 1.5; mean is a few queries) | 32.6 |
| causal channel | `recall::recall` | 0.3 | - | 8.1 | - |
| facts for top entities | `recall::facts_for_view` | 2.5 | 617.3 | 7.6 | 0.01 |
| access write (WAL commit) | `Db::write` | 0.3 | 21.8 | 5.8 | 0.27 |
| entities, next hops, pack, embed, fuse | - | 2.5 | 3.0 | 8.8 | - |
| **total** | | **25.4** | **1,068.0** | **964.7** | **0.90** |

Growth is linear in rows: `mem_knn` costs 1.75 ms per 1,000 memories at 108k (f32 DB) and 1.26 at 293k (int8), and each of the three vec0 stages lands at 1.95-2.0x its 108k value for 2.70x the memories - the scans grow with the table, not with the graph. int8 did not make the scan cheaper per row: the scan is per-row overhead, not bytes. The 108k comparison mixes builds (the section 4 timers predate nc-perf) and vector modes; only `facts` is affected by that mix, and it fell from 617.3 ms (58% of the 108k mean, the unindexed `current_values` count) to 7.6 ms after nc-perf. Without that fix 293k would run at ~1.5 s p50, not 0.94 s.

Top three cost centres at 293k (mean, share of 964.7 ms):

1. **38.4% - memory KNN**, `recall::recall` (src/recall.rs:1111): `SELECT rowid,1-distance FROM memories_vec WHERE embedding MATCH ?1 AND k=5000 AND live=1`, a brute-force scan of 293,305 x int8[768] rows (228 MB) per query.
2. **15.9% - path beams**, `paths::search` (src/paths.rs:789) and `paths::search_semantic` (src/paths.rs:980), whose `relations_vec` MATCH k=100 at src/paths.rs:1004 scans 114,931 x int8[768] rows (89 MB).
3. **15.2% - relation-fact seeds**, `recall::relation_seeds_with_config` (src/recall.rs:561), the second `relations_vec` scan (`k=10`, src/recall.rs:613) plus the `relations_fts` hits; together with the beams the relation table is streamed twice per query.

`facts_for_view` (src/recall.rs:2237) is no longer a cost centre at 293k (7.6 ms; fan-in is bounded at 10 relations per entity in this world, and the count runs only on kept rows). The as-of snapshot (12.6%) is charged to the as-of queries only (p50 0.1 ms, p95 276.2).

Read volume: 60 queries on the same DB read 30.3 GB (`rchar`), 505 MB per query, 0.66x the 764 MB file - each query streams the whole vector set. On a 2-3 GB box the file fits in page cache but leaves little for the agent; on anything slower than a warm cache the read path is IO-bound, and this section's ingest tail is what that looks like.

Answer: **yes, p50 stays under 1 s on one core** - 941.4 / 948.8 ms (clean passes), mean 926.4 / 936.1 ms, p95 1.20 / 1.29 s, peak RSS 433 MiB under a 2 GiB cap. The headroom is 6% at p50: 293k costs ~940 ms of which 370 ms is one vector scan, and the scan adds ~1.3 ms per 1,000 more memories, so ~340k is where p50 crosses 1 s on this CPU. The lever is the pre-filter from section 5 item 3 (restrict `memories_vec` to candidate entities before the scan); nothing else is close.

Peak RSS is no longer model-bound: 170 MiB at 108k (hash) -> 433 MiB at 293k, +1.4 MiB per 1,000 memories of read-path working set (row maps, k=5000 result set, snapshot), so with the q8 model (237 MiB) the same query would sit near 670 MiB - still inside 2 GiB, above the 512 MiB working budget.

Reproduce (293k):

```bash
export TMPDIR=... NEOCORTEX_EMBEDDER=hash NEOCORTEX_DB=.../never.db
cargo run --release --example gen_world -- --people 24000 --seed 28001 --out world-a.json
cargo run --release --example gen_world -- --people 3000  --seed 28002 --out world-b.json
# concatenate the two `memories`/`queries` arrays into world-293k.json
neocortex eval world-293k.json --save-db db293k-int8.db        # 44,360 s here, IO-contended
cp db293k-int8.db /dev/shm/base.db                             # keep the scans off the disk
taskset -c 0 neocortex eval world-293k.json --load-db /dev/shm/base.db
NEOCORTEX_TIMING=rt.tsv taskset -c 0 neocortex-timing-159a461 eval world-293k.json --load-db /dev/shm/base.db
```

## After nc-relpre: the relation prefilter A/B (2026-11-05)

Lane nc-relpre ported the I6 relation-seed prefilter of the killed branch
`lane-nc-fanout` (commit d97817f) as the opt-in `rel_prefilter` knob (default off;
`eval --set rel_prefilter=on` replaces the global `relations_vec` KNN for the
relation-fact seeds with a bounded read of the relations incident to the first 20 query
anchors, anchor-less questions keep the global KNN; only this was ported - the hub cap and
early truncation of nc-fanout were not).

A/B on the existing 108,667-memory real-model world-10000 DB (`eval --load-db`, a fresh copy
under `lanes/tmp/nc-relpre/db/` per run because `eval` records access, real UltraBERT f32,
`taskset -c 0`, 600 queries):

| round | cfg | acc@10 | recall | MRR | three-hop recall | p50 / p95 ms | expect_absent leaked |
|---|---|---|---|---|---|---|---|
| 1 | off | 1.000 | 0.972 | 0.861 | 0.812 | 599 / 1759 | 2/153 |
| 1 | on | 1.000 | 0.974 | 0.866 | 0.829 | 836 / 6714 | 1/153 |
| 2 | off | 1.000 | 0.972 | 0.861 | 0.812 | 3333 / 5540 | 2/153 |
| 2 | on | 1.000 | 0.974 | 0.866 | 0.829 | 1890 / 4361 | 1/153 |
| 3 | off | 1.000 | 0.972 | 0.861 | 0.812 | 704 / 1523 | 2/153 |
| 3 | on | 1.000 | 0.974 | 0.866 | 0.829 | 471 / 905 | 1/153 |

With the knob on, recall 0.972 -> 0.974, MRR 0.861 -> 0.866 and three-hop recall 0.812 -> 0.829
on world-10000; acc@10 held at 1.000. The p50 did not move cleanly: the round-1 `on` pass ran at
~15% CPU against 43% for its `off` partner (wall p50 load-dominated, p95 6714 ms) and the round-2
`off` pass was the slowest of the set (p50 3333 ms); round 3 favours `on` (471 vs 704 ms).

Release gates (briefs/common.md commands, `run_gates.sh`, default embedder, `--set
rel_prefilter=<cfg>`, all ten runs exit 0):

| set | off | on | gate |
|---|---|---|---|
| fixtures | 0.953 / 0.895 | 0.953 / 0.895 | pass (gate >= 0.953 / 0.895) |
| holdout | 0.914 / 0.795 | 0.914 / 0.795 | pass (gate >= 0.909 / 0.790) |
| multihop-holdout | acc@10 0.825 | 0.825 | pass (gate >= 0.825) |
| multihop-holdout2 | acc@10 0.820 | 0.820 | pass (gate >= 0.82) |
| world-300 | acc@10 1.000, p50 208 ms | 1.000, p50 235 ms | pass (gate >= 0.99) |

Default decision (lane nc-relpre, 2026-11-05): **keep `rel_prefilter` off**. No accuracy loss
with the knob on (acc@10 equal-or-better everywhere tested, tiny recall/MRR/three-hop gains on
world-10000); the p50 gain did not hold across all rounds cleanly (round-1 `on` was load
contaminated and slower, round-2 `off` was the outlier) and the knob was slightly slower on the
small gate sets (fixtures p50 179 vs 71 ms, holdout 126 vs 73 ms, world-300 235 vs 208 ms), so the
flip criterion (equal-or-better acc@10 on every set AND a p50 gain holding across all rounds) is
not met. The knob stays opt-in via `eval --set rel_prefilter=on`; the default stays off.

## 8. Memory KNN Prefilter Scale Evaluation (nc-bitscale2, 2026-11-05)

Does the sign-bit prefilter (`Config.vec_prefilter = "bit"`, keeping derived `memories_bits`
with Hamming top-N candidates and exact vector rerank) justify becoming the default at scale?
Measurement lane nc-bitscale2 evaluated 3 paired interleaved rounds (off, bit, off, bit, ...)
at 108,667 memories (real UltraBERT-150M model, int8 vectors, world-10000.json) and 293,305
memories (hash embedder, int8 vectors, world-293k.json) on 1 core (`taskset -c 0`, 600 queries):

### 108k (Real model, UltraBERT-150M)

- One-off table build: DB 261.32 -> 274.80 MB (+13.49 MB, +5.16%, ~130 B/memory).
- Accuracy: **100% identical** across all 600 queries and all categories (acc@10 1.000,
  recall 0.972, MRR 0.861).
- 1-core p50 latency across 3 paired rounds:
  - Round 1: 601.08 -> 444.63 ms (-26.0%)
  - Round 2: 477.43 -> 419.33 ms (-12.2%)
  - Round 3: 523.49 -> 428.09 ms (-18.2%)
  - Mean p50: 534.00 -> 430.68 ms (-19.3%). Peak RSS ~763 MiB flat.

### 293k (Hash embedder, NEOCORTEX_EMBEDDER=hash)

- One-off table build: DB 728.48 -> 764.68 MB (+36.20 MB, +4.97%, ~129 B/memory).
- Accuracy: **Regresses** under sign-bit quantization:
  - Overall: acc@10 0.838 -> 0.810 (-0.028), recall 0.764 -> 0.759, MRR 0.577 -> 0.568.
  - Regressions: temporal-window 0.850 -> 0.650 (-0.200), history 0.641 -> 0.449 (-0.192),
    aggregation 0.817 -> 0.750 (-0.067).
  - Cause: hash embedder vectors are non-negative and sparse; binary sign quantization maps
    positive coordinates to 1, losing cosine angle fidelity and dropping candidate recall.
- 1-core p50 latency across 3 paired rounds:
  - Round 1: 1050.70 -> 876.38 ms (-16.6%)
  - Round 2: 1170.97 -> 791.83 ms (-32.4%)
  - Round 3: 1045.54 -> 816.91 ms (-21.9%)
  - Mean p50: 1089.07 -> 828.37 ms (-23.9%). Peak RSS ~438 MiB flat.

### Recommendation

**Keep `vec_prefilter` default as `off`**. While `bit` consistently cuts p50 latency by 12-26%
at 108k and 16-32% at 293k, it breaks accuracy on hash-embedder databases (acc@10 0.838 -> 0.810).
The knob should remain opt-in (`--set vec_prefilter=bit` or `NEOCORTEX_VEC_PREFILTER=bit`)
specifically for real-model deployments where accuracy is invariant.
```

---

Orchestrator note: real-model deployments are the case that matters (identical accuracy, -19% p50 at 108k); the hash-embedder loss comes from sign bits on non-negative sparse vectors. A real-model-only default needs a real-model 293k DB first (open item BIT-DEFAULT).

## After nc-hay1: crowding channels (2026-11-07)

Keeping a hand-written set findable inside the 108,667-memory world-10000 store
(`eval --haystack lanes/tmp/nc-scale2/w10k-int8.db`, real UltraBERT, `NEOCORTEX_VEC=int8`).
Diagnosis (`examples/hay_diag.rs`, loss table in `loop/reports/nc-hay1.md`): 27
answered-alone/missed-in-haystack queries across fixtures/holdout/multihop-holdout.
Three loss classes: **fusion crowding** (dominant: gold at graph#1-39 / bm25#2-9 / vec
ahead of the world, but a fused score of 0.04-0.25 loses to distractors that sit top-5
in two channels, because the graph channel mirrors the vec channel through its 0.6
memory-seed mass); **semantic-beam hop loss** (multi-hop answers split on the rendered
context: a hop-2/hop-3 relation ranks >2000 in the 42k `relations_vec` and is outside
the beam's KNN-100, so the chain never walks); **pool loss** (gold outside every
channel's fixed-size pool for a few anchor-less paraphrases; unchanged by any fix
below).

Three product changes (two scale-activated at 50k stored memories, one shelf-wide):

1. `anchor` channel (`Config.w_anchor` 2.5, gated by `ANCHOR_MIN_MEMORIES` = 50_000):
   memories mentioning a strong anchor inside the top PPR window (bounded by
   `fetch_depth`, no new scans, one mentions probe), cosine-ranked. At the small
   sets the same top memories already follow from the PPR seeds and the sibling boost
   re-orders them for the worse (fixtures 0.953 -> 0.929 at full weight), so it is
   silent below the gate. Inside the 108k store: holdout recall 0.744 -> 0.834,
   fixtures 0.875 -> 0.869, multihop-holdout unchanged (no anchors in its questions),
   blind multihop-holdout2 0.660 -> 0.650 (−1 query, gate-insensitive at every weight
   >= 1.0, and saw no benefit below 1.0).
2. Semantic-beam bridge admission (`SEM_WIDEN_CAP` 32, gated by
   `SEM_WIDEN_MIN_STORE` = 50_000): a frontier entity with <= 32 incident
   relations always lets its own facts through the relevance gate, whatever their
   cosine. Beam depth stays 3 (depth 4 measured worse: 0.850 -> 0.812). Inside the
   haystack: mh1 acc 0.825 -> 0.850 (with the bridge change below, per-item below).
3. `bridge_pull` bridge candidate pool 3 -> 6 entities (ungated): mh1 alone
   0.863 -> 0.912, mh1 in the haystack 0.825 -> 0.900. All alone gates hold or
   improve (fixtures 0.953, holdout 0.914, mh1 0.912, blind 0.820,
   world-300 acc 0.998/recall 0.969).

Reproduce: `./target/release/neocortex eval <set>.json --haystack <w10k-int8.db>` (or
`--load-db <saved haystack copy>` for re-scoring without re-ingesting).

## After nc-scale3: 108,667 real-model memories on main 92adf50 (2026-11-08)

Measurement lane, no code. The other real-model 108k store (nc-scale2) was ingested on fdcad90, before ~25 merges
(ER-08 follow-ups, nc-ended, nc-h313, nc-hay1 anchor channel + beam admission, nc-nexthop, nc-rename, nc-h203,
nc-opt, nc-evidence). This run re-ingests world-10000 (seed 10001, read-only at `lanes/tmp/nc-scale100k/world-10000.json`)
on main 92adf50 (code d00a0ba) with `NEOCORTEX_VEC=int8 neocortex eval <world> --save-db`. Binaries built from the
worktree (`cargo build --release` + `--example agent_sim`, 4 m 01 s, log `lanes/tmp/nc-scale3/build.log`; copies at
`lanes/tmp/nc-scale3/bin/`, everything run from the copies). DB kept at `lanes/tmp/nc-scale3/w10k-int8.db` (+ `.keys.json`).

Ingest: 108,667 memories in 11,470,894 ms (3 h 11 min, 9.47/s average). Log `lanes/tmp/nc-scale3/ingest.log`.
Rate per 10k block (memories/s):

| 0-10k | 10-20k | 20-30k | 30-40k | 40-50k | 50-60k | 60-70k | 70-80k | 80-90k | 90-100k | 100k-108.7k |
|---|---|---|---|---|---|---|---|---|---|---|
| 12.22 | 10.78 | 9.71 | 7.72 | 11.13 | 9.30 | 9.63 | 9.19 | 8.88 | 8.61 | 8.50 |

No super-linear tail and no cliff; the slowest block (7.72/s, 30-40k) is in line with nc-scale2's 6.9-10.6/s steady
state. `validate` on a copy: `ok: no problems`. `stats`: 108,667 (live 108,667) memories, 11,910 entities, 42,571
relations (live), 1,394 similar, 27 predicates, cold 0; db 261.7 MiB; embedder ultrabert-150m. DB file
274,272,256 B = 2,525 B per memory (int8; nc-scale2 store: 2,522 B, similar edges 1,388).

Accuracy (`eval --load-db` on a copy, 600 queries; from the ingest run itself):

| category | q | acc@10 | recall | MRR |
|---|---|---|---|---|
| aggregation | 60 | 1.000 | 1.000 | 0.964 |
| as-of | 102 | 1.000 | 1.000 | 0.883 |
| current | 102 | 1.000 | 0.990 | 0.892 |
| history | 78 | 1.000 | 1.000 | 1.000 |
| temporal-window | 60 | 1.000 | 1.000 | 1.000 |
| three-hop | 78 | 0.949 | 0.654 | 0.639 |
| two-hop | 120 | 1.000 | 0.867 | 0.762 |
| overall | 600 | 0.993 | 0.927 | 0.863 |

hints 18/600 queries (~1.0 tokens/query); expect_absent leaked 3/153; context 463.9 tokens/query;
latency mean/p50/p95 440.80/423.86/538.76 ms.

Versus the nc-scale2 store on the same world (fdcad90): overall acc@10 0.990 -> 0.993, MRR 0.859 -> 0.863,
two-hop acc@10 0.950 -> 1.000 (the six "which language is used by <person>'s project" ranking misses are gone,
consistent with nc-hay1's passive-phrasing and anchor work), as-of MRR 0.818 -> 0.883, current recall 0.980 -> 0.990,
but three-hop acc@10 1.000 -> 0.949 (recall@10 0.812 -> 0.654; recall@50 = 1.000) and three-hop MRR 0.666 -> 0.639.

agent_sim --hints (fresh copy, taskset -c 0): 596/600 answered (0.993); step 1 recall 0.993 | +follow 0.993 |
+recall-leaf 0.993; calls/query 1.01; recall p50 633 ms, follow p50 108 ms; the 4 misses are the three-hop acc misses
and carry no next-hop hint (0/0 hint-bearing misses followed). nc-scale2: 600/600.

Latency (`eval --load-db` on a copy, includes the query embedding; host shared with other lanes, load 21-103 during
this lane, versus 3-10 for nc-scale2, so absolute p50s are inflated and the 1-core/2-core comparison is meaningless
here; accuracy is load-independent and identical across both passes):

| | 1 core (taskset -c 0) | 2 cores (taskset -c 0,1) |
|---|---|---|
| recall mean / p50 / p95 | 807.96 / 585.57 / 959.90 ms | 672.75 / 637.33 / 845.91 ms |

nc-scale3 ingest-run p50 (all cores) was 423.86 ms, inside the nc-scale2 band (473.3 / 416.8 ms). Cold CLI recall
(page cache warm, `taskset -c 0`, child RUSAGE): f32 model peak RSS 635.8 MiB, q8 model 237.6 MiB (nc-scale2:
632.3 / 237.2) — RSS is model-bound at 108k, unchanged.

Haystack, same binary, copies of both stores (`eval <set> --haystack <copy>`, NEOCORTEX_VEC=int8; NEW = this store,
OLD = nc-scale2 store, the store's age is the only difference; logs `lanes/tmp/nc-scale3/step4-haystack.log`):

| set | NEW acc@10 / recall / MRR | OLD acc@10 / recall / MRR | NEW p50 | OLD p50 |
|---|---|---|---|---|
| fixtures | - / 0.869 / 0.842 | - / 0.869 / 0.842 | 302.0 ms | 331.0 ms |
| holdout | - / 0.834 / 0.703 | - / 0.834 / 0.703 | 316.5 ms | 333.5 ms |
| multihop-holdout | 0.900 / 0.621 / 0.809 | 0.900 / 0.621 / 0.809 | 318.6 ms | 322.3 ms |
| multihop-holdout2 (BLIND: summary lines only) | 0.650 / 0.618 / 0.804 | 0.650 / 0.618 / 0.804 | 393.2 ms | 318.7 ms |

Every accuracy cell is identical between the two stores, and matches the merged nc-hay1 haystack measurements
(mh1 0.900, mh2 0.650, holdout 0.834). The NEW store carries one more entity (11,910 vs 11,909; relations equal
at 42,571). The mh2 p50 delta (393 vs 319 ms) is host load, not the store (its p95 was 917 ms; every other p50
matches within 30 ms).

### The four misses: all ranking class, hop 3

Fresh copy, `--failures` at k=10, then a k=50 pass (`lanes/tmp/nc-scale3/step5-failures.log`; a k=50 copy of the
world fixture was emitted by `lanes/tmp/nc-scale3/make_k50.py`):

- recall@50 = 1.000 overall (three-hop recall 1.0): every expected memory is in the top 50, so there are no
  retrieval misses. MRR/acc of the k=50 pass are unchanged (three-hop acc still 0.949, same 4 misses); one expected
  memory, w034105, moved from outside the top 10 in the k=10 pass to rank 9 in the k=50 pass on the retention bump
  the first pass recorded.
- All four acc misses are three-hop "the manager of <person>'s sibling" chains whose answer is the city-move memory;
  at k=50 the expected memories sit at:

| query (missing answer) | hop memories: sibling / reports-to / city-move | class |
|---|---|---|
| where does the manager of Rakesh Davishen's sister live (Kanpur) | w047717@1 / w058808@12 / w093973@11 | ranking |
| which city does the manager of Ryan Shipore's brother live in (Vienna) | w050427@2 / w046588@16 / w075770@14 | ranking |
| which city does the manager of Umar Thamani's brother live in (Aurangabad) | w081083@1 / w046918@17 / w095665@14 | ranking |
| what city is home to the manager of Yusuf Triraj's sister (Brussels) | w062718@3 / w034105@- (k=10) -> @9 (k=50) / w071607@12 | ranking |

- Write path is clean: `entity` cards for all four managers show the live `lives in` facts (Graham Deshvari ->
  Kanpur since 2026-07-16; Eamon Sodzada -> Vienna since 2026-05-08; Oliver Upamani -> Aurangabad since 2026-07-22;
  Ibrahim Yadrao -> Brussels since 2026-04-23), the sibling -> manager `has manager` rows are live, and the
  intervening values are correctly ended (e.g. Graham Deshvari ends "used to report to Maja Versingh"). No wrong
  merge, no ended fact retired early. This is the nc-scale2 two-hop miss pattern one hop later: ranking, not
  retrieval, not resolution. The hop-3 city-move memories rank 11-17 at k=50, i.e. just past the acc@10 cut.

Caveats: the two `--load-db` latency passes re-record access on the copy, so pass 2 saw pass 1's retention history
(accuracy identical); the step-3 copy itself was measured before any recall and is the pristine one for the acc
figures above. The f32/q8 RSS numbers were taken warm-cache, as in nc-scale2.

## Reproduce

```bash
export TMPDIR=... NEOCORTEX_DB=.../never.db NEOCORTEX_MODEL_DIR=.../UltraBERT-Embed-150M
cargo run --release --example gen_world -- --people 1000  --seed 1001  --out world-1000.json
cargo run --release --example gen_world -- --people 3000  --seed 3001  --out world-3000.json
cargo run --release --example gen_world -- --people 10000 --seed 10001 --out world-10000.json
neocortex eval world-3000.json --save-db world-3000-f32.db          # ~73 min on this host
taskset -c 0 neocortex eval world-3000.json --load-db <copy of the db>
neocortex --db <copy> migrate-vectors --to int8
neocortex quantize-model $NEOCORTEX_MODEL_DIR ultrabert-q8 --dtype q8
NEOCORTEX_EMBEDDER=hash neocortex eval world-10000.json --save-db world-10000-hash.db   # many hours; see section 4
```

`world-3000-k50.json` is world-3000 with `"k": 50` on every query (candidate-recall analysis).
