# neocortex footprint baseline

Measured 2026-10-26 on the remem-v31 release binary. No code changed. Raw records: `lanes/tmp/footprint/results.jsonl` (scratch; the driver is `lanes/research/footprint-bench.py`).

## Setup and caveats

- **Binary:** `remem-v31/target/release/neocortex`, built 14:54 local, after the last source commit `6a97e00` (14:51 +05:45). HEAD `671e593` only changes AGENTS.md. A later `cargo build --release` timed out waiting on a build lock held by other lanes, so the existing binary was used.
- **Host:** i7-9750H (6 cores / 12 threads), 15.8 GB RAM, `/run/media/bindesh/E` is ext4 on `/dev/sda3`. Cold model reads measured at about 72 MB/s.
- **Shared machine:** other lanes ran evals and builds during every measurement. Load1 was 9 to 20 on 12 threads. Single-core wall times are inflated by contention on CPU 0. The CPU-time column is the uncontended estimate.
- **Cold vs warm:** "cold" means a new process and the model files (`model.safetensors`, `tokenizer.json`, `config.json`) evicted from page cache with `posix_fadvise(DONTNEED)` before each run (no root needed). "warm" means page cache left alone.
- **Peak RSS:** per-child `os.wait4` `ru_maxrss`, not `RUSAGE_CHILDREN` (which accumulates the max across children). Per-process values are sampled from `/proc/<pid>/status` every 20 ms (`RssAnon`, `RssFile`).
- **Sanity check:** fixtures eval gives recall 0.953, MRR 0.895 (84 queries). world-300 gives recall 0.965, MRR 0.881 (600 queries).

## Key numbers

| Metric | Value |
|---|---|
| Peak RSS, cold recall (wait4 max) | **1,187 MiB** (RssAnon 602 MiB + RssFile 584 MiB, transient during model load) |
| Steady-state RSS after warm-up (VmRSS, MCP server) | 614 MiB (VmHWM 1,188 MiB) |
| Warm MCP recall p50, 1 core | **773 ms** (run 1) / 874 ms (rerun); p95 949 / 1,088 ms |
| Warm MCP recall p50, 2 cores | **265 ms** (run 1) / 254 ms (rerun); p95 305 / 300 ms |
| Warm MCP recall p50, all cores | 201 ms; p95 231 ms |
| Smallest cgroup cap that works (`MemoryMax`, swap 0) | **610 MiB** (600 MiB is OOM-killed, rc 137) |
| Smallest `prlimit --as` that works | between 1,152 and 1,280 MiB (1,280 MiB works, 1,152 MiB aborts) |
| Model size on disk (loaded) | 604,153,320 B `model.safetensors` (576 MiB, F32) + 3.5 MB tokenizer |
| DB size, fixtures (95 memories) | 2,076,672 B at ingest (16,427 B/memory marginal) |
| DB size, world-300 (3,264 memories) | 21,303,296 B (6,369 B/memory marginal, 6,527 B gross) |
| Empty DB (schema only) | 516,096 B |
| Vector share of DB | fixtures 67%, world-300 78% (16.6 MB) |

## 1. Cold start: `neocortex recall "where does Ana work"` (fixtures DB)

Each cell is the median of 3 runs. Wall times are per process.

| Page cache | Cores | Median wall s | Range s | Median CPU s (user+sys) | Peak RSS MiB | RssAnon MiB | RssFile MiB |
|---|---|---|---|---|---|---|---|
| cold (model evicted) | all | 8.89 | 8.83–9.46 | 1.03 | 1,187 | 602 | 584 |
| cold (model evicted) | 1 (`taskset -c 0`) | 9.21 | 9.15–10.18 | 0.80 | 1,187 | 602 | 583 |
| cold (model evicted) | 2 (`taskset -c 0,1`) | 10.70 | 9.39–12.07 | 1.09 | 1,188 | 602 | 584 |
| warm (page cache) | all | 0.92 | 0.88–0.94 | 0.94 | 1,188 | 602 | 538 |
| warm (page cache) | 1 | 3.01 | 2.83–3.03 | 0.77 | 1,188 | 602 | 576 |
| warm (page cache) | 2 | 1.04 | 1.00–1.06 | 0.79 | 1,188 | 602 | 554 |

Cold wall time is almost all disk read (about 8 s for 576 MiB). Core count does not matter there. CPU time is about 1 s.

## 2. Cold `remember` of one short memory (copy of fixtures DB per run)

| Page cache | Cores | Median wall s | Range s | Median CPU s | Peak RSS MiB |
|---|---|---|---|---|---|
| cold | all | 10.84 | 6.84–13.19 | 1.39 | 1,187 |
| cold | 1 | 14.70 | 13.14–15.02 | 1.07 | 1,188 |
| cold | 2 | 11.33 | 9.78–22.12 | 1.21 | 1,188 |
| warm | all | 0.96 | 0.83–1.32 | 1.14 | 1,187 |
| warm | 1 | 3.37 | 3.23–3.42 | 0.91 | 1,187 |
| warm | 2 | 0.93 | 0.85–0.95 | 0.72 | 1,188 |

## 3. Warm MCP latency (`neocortex mcp`, stdio, JSON-RPC lines)

50 `tools/call recall` requests from the 84 fixture queries. Three warm-up calls are excluded, and the first call (which loads the model) is reported separately.

| Cores | p50 ms | p95 ms | min / max ms | First call ms (incl. load) | VmRSS after warm-up MiB | VmHWM MiB |
|---|---|---|---|---|---|---|
| 1 | 773 | 949 | 471 / 972 | 1,891 | 614 | 1,188 |
| 2 | 265 | 305 | 205 / 463 | 562 | 614 | 1,188 |
| all | 201 | 231 | 164 / 263 | 753 | 614 | 1,188 |
| 1 (rerun) | 874 | 1,088 | 461 / 1,144 | 2,916 | 614 | 1,188 |
| 2 (rerun) | 254 | 300 | 174 / 436 | 797 | 614 | 1,188 |
| 2 under `MemoryMax=640M` | 286 | 326 | 247 / 342 | 894 | 614 | 1,188 |

No errors in any run. 7 threads at steady state.

## 4. Memory caps (fixtures DB, cold recall)

| Mechanism | Cap | Result |
|---|---|---|
| `prlimit --as` | 1 GiB | FAIL, abort (rc 134), "memory allocation of 7077888 bytes failed" (3 reps) |
| `prlimit --as` | 768 MiB | FAIL, same abort (3 reps) |
| `prlimit --as` | 512 MiB | FAIL, "Cannot allocate memory" while loading `model.safetensors` (3 reps) |
| `prlimit --as` | 1,024 / 1,152 MiB | FAIL (abort) |
| `prlimit --as` | 1,280 MiB, 1.5 GiB, 2 GiB | OK |
| `systemd-run --user --scope -p MemoryMax=N -p MemorySwapMax=0` | 1 GiB, 768 MiB, 700 MiB, 640 MiB, 620 MiB | OK |
| same | 610 MiB | OK (2 reps) |
| same | 600 MiB | SIGKILL, rc 137 (2 reps) |
| same | 512 MiB | SIGKILL during model load (rc -9) |

**Smallest working cap:** cgroup `MemoryMax=610M`. That is the RSS-based limit that counts. Address-space limits (`prlimit --as`) need more than 1.15 GiB. The likely cause is the 604 MB mapping plus allocator and thread reservations (not profiled).

The warm MCP p50 under 512 MiB was not measurable: the server is killed during model load. The closest measured run is 640 MiB on 2 cores (p50 286 ms, above).

## 5. Sizes

**Model files actually loaded (default path):**

| File | Bytes | Notes |
|---|---|---|
| `UltraBERT-Embed-150M/model.safetensors` | 604,153,320 | F32, loaded by `UltraBert::from_files` via mmap, then copied (see RSS) |
| `UltraBERT-Embed-150M/tokenizer.json` | 3,523,570 | loaded |
| `UltraBERT-Embed-150M/config.json` | 1,734 | loaded |

Not loaded by default: the MS-MARCO reranker (`rerank_top` defaults to 0 and needs `NEOCORTEX_RERANK`) and any extraction model. Only `embed.rs`, `ultrabert.rs`, and `rerank.rs` load weights.

**DB sizes and bytes per memory:**

| DB | Bytes | Memories | Bytes/memory (marginal, minus empty DB) | Bytes/memory (gross) |
|---|---|---|---|---|
| empty (schema) | 516,096 | 0 | — | — |
| fixtures | 2,076,672 (2,166,784 after recall writes) | 95 | 16,427 | 21,860 |
| world-300 | 21,303,296 | 3,264 | 6,369 | 6,527 |

The fixtures per-memory number is inflated by the small sample and 64-row vec0 chunk granularity. Use world-300 for the marginal cost.

**Vector tables (dbstat, `*_vector_chunks00` payload):**

| Table | fixtures bytes | world-300 bytes |
|---|---|---|
| `memories_vec` | 397,312 | 10,059,776 |
| `entities_vec` | 593,920 | 2,174,976 |
| `relations_vec` | 200,704 | 4,145,152 |
| `predicates_vec` | 200,704 | 200,704 |
| **Total vector payload** | **1,392,640 (67% of DB)** | **16,580,608 (78% of DB)** |

- Each vector is 768 F32 = 3,072 B. `memories_vec` is about 3,082 B per memory.
- Relation and entity vectors add about 1.9 KB per memory on world-300.
- Other large world-300 tables: `node_props_text` (708,608), `memories` (471,040), `edges` (188,416), FTS (446,464 total).

## 6. Portability scan (old x86 only; ARM/NEON skipped per scope change)

- **Build flags:** no `.cargo/config`, no `build.rs` in neocortex, no `target-cpu`/`RUSTFLAGS` in the repo. The build targets x86_64 baseline (SSE2).
- **Features:** `cargo tree -e features | grep -i "mkl|accelerate|avx|cuda|metal"` is empty. candle-core and candle-nn use default features only.
- **Compile-gated candle paths:** `cpu/mod.rs:66` and `quantized/mod.rs:7` (AVX2), `quantized/k_quants.rs`, `cpu/avx.rs` (F16C). These are compiled out in the default build, so they do not run.
- **Runtime-dispatched SIMD (safe on old CPUs):**
  - gemm 0.19 picks `fma` or falls back to scalar at runtime (`gemm-common/src/gemm.rs:1015–1021`). `x86-v4` is off, so no AVX-512 path.
  - memchr uses AVX2 behind runtime detection.
  - aho-corasick teddy checks `ssse3` and `avx2` at runtime (`builder.rs:679,705`).
  - rand_chacha (via ppv-lite86 `dispatch!`) selects `ssse3`/`sse41` paths at runtime.
- **Binary contents (objdump of `neocortex`):** 914 `vfmadd`, 6,982 lines with `ymm`, zero `zmm`. All of it sits in gemm microkernels and gemv, plus memchr AVX2 functions (81 functions with VEX ops). `pshufb` appears only in aho-corasick teddy and rand_chacha. The `tzcnt` count is `rep bsf`, which runs as `bsf` on old CPUs.
- **C dependencies:**
  - `rusqlite` with `bundled` (SQLite amalgamation, `libsqlite3-sys` 0.38.2, plain `cc`, no `-march`). Portable.
  - `sqlite-vec` 0.1.9 (`sqlite-vec.c`, plain `cc`). Its AVX path is opt-in (`SQLITE_VEC_ENABLE_AVX`), which `build.rs` does not set, so it builds scalar.
  - `graphqlite` 0.12.0 embeds a prebuilt `libs/graphqlite-linux-x86_64.so` (1.08 MB, `build = false`). It is extracted to `$TMPDIR/graphqlite/` at runtime. Objdump shows zero AVX, SSSE3 or SSE4 instructions (SSE2 only), so it is old-CPU safe. It is an opaque binary, and it needs an exec-capable temp dir (it falls back to `~/.cache/graphqlite` on noexec).
  - `tokenizers` with `default-features = false` (no onig C dependency).
- **Blockers that would crash on old x86:** none found.
- **Blockers that would be slow on old x86 (not measured):** no CPU without FMA is available here. Without FMA, gemm uses the scalar kernel on the F32 BERT-150M. That path would make per-query embedding much slower than the 0.8 s single-core CPU time measured here. candle `quantized` is not used by neocortex, so there is no q8/q4 path today.

## Top 5 footprint costs (ranked)

1. **F32 UltraBERT weights.** 604 MB on disk. Steady-state RSS is about 602 MiB anonymous; the peak shows the file mapping and an anonymous copy at the same time (inferred from RssAnon and RssFile, not traced). The transient peak is 1.19 GB, which drives the cgroup and address-space caps. Halving to F16 or quantizing to q8 would cut this most.
2. **Cold model read.** About 8 s of disk I/O for 576 MiB on this disk. This dominates cold-start wall time (CPU is about 1 s). It depends on storage speed.
3. **Vector payload in SQLite.** 768 F32 per vector. Vectors are 78% of the world-300 DB and about 5 KB per memory. Relation and entity vectors add about 1.9 KB per memory.
4. **Per-query BERT forward pass.** Warm recall p50 is 0.77–0.87 s on 1 core and about 0.26 s on 2 cores. Query embedding is the dominant warm cost.
5. **Address-space reservations.** `RLIMIT_AS` needs more than 1.2 GiB even though RSS is about 614 MiB steady. Sandboxes that cap virtual memory need that much headroom.

## Commands used

Build and sanity (no code changed):

```bash
export TMPDIR="/run/media/bindesh/E/New Folder/rag/lanes/tmp/footprint"; mkdir -p "$TMPDIR"
export NEOCORTEX_MODEL_DIR='/run/media/bindesh/E/New Folder/rag/UltraBERT-Embed-150M'
cd "/run/media/bindesh/E/New Folder/rag/remem-v31"
cargo build --release          # no-op in effect: binary built 14:54, after last src commit
export NEOCORTEX_DB="$TMPDIR/fixtures.db"
./target/release/neocortex eval eval/fixtures.json --save-db "$TMPDIR/fixtures.db"        # 68 s
./target/release/neocortex eval eval/world-300.json --save-db "$TMPDIR/world300.db"        # 625 s ingest, ~17 min incl. 600 queries
```

Benchmark driver (`lanes/research/footprint-bench.py`, sets `TMPDIR`, `NEOCORTEX_DB` and `NEOCORTEX_MODEL_DIR` per run):

```bash
python3 -I footprint-bench.py cold "$TMPDIR/fixtures.db" cold   # cold-cache recall and remember, all/1/2 cores, 3 reps
python3 -I footprint-bench.py cold "$TMPDIR/fixtures.db" warm   # warm-cache variant
python3 -I footprint-bench.py mcp  "$TMPDIR/fixtures.db"        # 50 recall calls, p50/p95 on 1, 2 and all cores
python3 -I footprint-bench.py cap  "$TMPDIR/fixtures.db"        # prlimit --as and systemd-run MemoryMax at 1G/768M/512M
python3 -I footprint-bench.py mcp-cap "$TMPDIR/fixtures.db"     # 2 cores under MemoryMax=512M (server killed at load)
```

Follow-up cap probes (run one at a time):

```bash
prlimit --as=$((n*1024*1024)) "$BIN" recall "where does Ana work"      # n = 1024, 1152, 1280, 1536, 2048
systemd-run --user --scope -q -p MemoryMax=${m} -p MemorySwapMax=0 "$BIN" recall "where does Ana work"   # m = 600M..768M
systemd-run --user --scope -q -p MemoryMax=640M -p MemorySwapMax=0 taskset -c 0,1 "$BIN" mcp   # via the driver's mcp() helper
```

Sizes and dbstat (`empty.db` made with `neocortex stats` on a new path):

```bash
stat -c %s "$TMPDIR/fixtures.db" "$TMPDIR/world300.db" "$TMPDIR/empty.db"
sqlite3 "$DB" "SELECT name, SUM(pgsize) FROM dbstat GROUP BY name ORDER BY 2 DESC LIMIT 15;"
sqlite3 "$DB" "SELECT name, SUM(pgsize) FROM dbstat WHERE name LIKE '%_vec%' GROUP BY name ORDER BY 2 DESC;"
```

Portability scan:

```bash
grep -rnw "target_arch\|target_feature\|target_os\|is_x86_feature_detected\|avx\|avx2\|sse4_1\|neon" src/ Cargo.toml
cargo tree -e features | grep -i "mkl\|accelerate\|avx\|cuda\|metal"        # empty
objdump -d --no-show-raw-insn target/release/neocortex > neocortex.dis
awk -F'\t' 'NF>=2{split($2,a," "); print a[1]}' neocortex.dis | sort | uniq -c   # mnemonic counts (vfmadd, ymm, pshufb, ...)
objdump -d --no-show-raw-insn ~/.cargo/registry/src/*/graphqlite-0.12.0/libs/graphqlite-linux-x86_64.so > graphqlite.dis   # same counts: zero AVX/SSSE3/SSE4
```

## Not measured

- Old x86 CPUs, real or emulated. A CPU without FMA or AVX would show the gemm scalar-path latency, which is unknown here.
- Warm MCP p50 under 512 MiB. The server is killed during model load.
- Cold-page-cache numbers on other storage. Here they come from ext4 on this drive at about 72 MB/s.
- Android and ARM (out of scope per the coordinator's scope change).

## After nc-lite (2026-10-26)

Streaming f32 loader (one copy of the weights) and an opt-in Q8_0 model (`neocortex quantize-model`, `NEOCORTEX_EMBED_DTYPE=q8`): cold-recall peak RSS 1,187 MiB (mmap loader) -> 612 MiB (f32) -> 237 MiB (q8); model file 604 MB -> 161 MB; q8 vs f32 embedding cosine min 0.998 / mean 0.9996 over 927 eval texts. Table, gates and the reasons candle's `QMatMul` is not used (compile-time SIMD selection, scalar in the default build, 14 s per recall) are in `AGENTS.md` (Verification, Local Contracts, Layout).
