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

## 5. Recommendations (nothing implemented in this lane)

1. Add `CREATE INDEX idx_rel_subj ON relations(subj, predicate_id)` as an additive schema step, and compute `current_values` after `.take(limit)` in `facts_for_view`. Measured on the 108k DB: p50 681 -> 435 ms, p95 2,355 -> 484 ms, identical results. Also speeds up `remember` on relation writes.
2. Make the write-time memory scans cheaper than O(N): `knn_memory` (k=1) and the `k=10` related block each scan all of `memories_vec`. Candidates: run one k=10 scan and derive the k=1 hit from it; or skip the related scan when the near-dup scan found nothing above the update threshold.
3. Recall at 100k then costs ~435 ms (f32) / ~315 ms (int8) on one core for the hash embedder, before the query embedding. The remaining linear terms are the three vec0 brute-force scans (memories, relations twice); int8 helps, a coarse pre-filter (entity-restricted KNN) would be the next lever.
4. Multi-hop at 32k: three-hop acc@10 fell to 0.897 from path-beam misses at hop 3. Test `agent_sim` / `follow` on world-3000 before changing retrieval.

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
