# Lane results ledger (2026-10-25 to 2026-10-27)

One row per lane: what it tried, the deciding numbers (real UltraBERT-Embed-150M unless noted), and the
outcome. Process: `lanes/AGENTS.md` in the workspace (outside this repo). "Blind" = multihop-holdout2
(workers may run it, never read it); "private" = multihop-holdout3 (orchestrator only, never shown to
workers). Idea letters refer to `agent-memory/ideas.md`, which carries a "Measured" line per idea.

## Merged

| lane | change | deciding numbers | merge |
|---|---|---|---|
| nc-e45 | temporal/causal memory links, access-reinforced retention | no gain from links (kept off), retention blend 0.05 | 9c2d900 |
| nc-f5 | readable Cypher memory nodes, vec0 chunk_size 64, deterministic Leiden communities | first-insert reservation 12.6 MiB -> small | 2a0f5a3 |
| nc-f4 | layered near-name dedup (typo-sized edits, subset blocker, cosine veto only) | real-model false merges 4 -> 0 (Bergen Airport South, Novak Group North, Jan Nováček, Anna Tomczuka); accepted Wisniewski/Wisniowski | cc83d43 |
| nc-hop | learned logistic hop scorer | hurts both multihop sets: kept off | 09303a3 |
| nc-f3 | text-only retraction (whole-object rule, hedges report only), plans, recurrence, ordering/durations | real-model false endings 4 -> 0 | 77f9dd8 |
| nc-rerank | opt-in MiniLM cross-encoder on memories | holdout MRR +0.013, blind -0.01, latency 2-4x: opt-in only | 2411fbc |
| nc-hint | `next:` hints for unfinished chains | agent_sim mh1 0.838 -> 0.863 | 67f31f6 |
| nc-ex01 | unit-label entities, no unit entities after numbers, known-word compounds, separator prefix | EX-01..04 closed | cee08b7 |
| nc-anchor | tiered query anchors (`anchor_tiers`) | mh1 +0.06, blind 0.820, private flat: kept off | 6a97e00 |
| nc-ops01 | tests remove every temp dir and DB | leftover temp dirs 0 | b1e1e1f |
| nc-tm10c | past-dated text retractions retire ingest-time facts; "left his/her/their job/position at" | real-model retraction table 21/21 (incl. 5 negatives) | 8af053c |
| nc-vec8 | opt-in int8 vectors (`NEOCORTEX_VEC=int8`, `migrate-vectors`) | DB -58%; private 0.875 equal; holdout MRR 0.792 (0.001 under floor): not default | 7cb8ea0 |
| nc-lite | streaming f32 loader; opt-in q8 GGUF embedder; `quantize-model` | cold-recall peak RSS 1,187 -> 614 MiB (f32), 235 MiB (q8); q8 private 0.867 | 5351d49 |
| nc-histprose | semantic beam on history questions; three rendered chains; prose fact lines opt-in | blind 0.830 -> 0.850; private 0.875 equal; prose lines no private gain, +1.5-4.5% tokens: off | 1b0e1e9 |
| nc-skill | SKILL.md synced with current behaviour | claims checked against src/ | 25eabf8 |
| nc-contain | connector spans (B5), EX-05 with common-noun guard, possessive ends spans, name-containment links | heuristic entity F1 0.675 -> 0.762; accepted exception blind 0.830 -> 0.810 (0.820 after histprose), private 0.875 -> 0.867, each lost query traced to rank noise from removed junk entities | f95327e |
| nc-scale | measurement at 11k / 33k / 109k memories | see `scale.md` | dfe7e79 |
| nc-perf | `idx_rel_subj`, deferred `current_values`, one `memories_vec` scan per remember | 33k: recall p50 -33%, p95 -44%, ingest -20%, results byte-identical | 5e3e082 |

## Killed (branch kept, not merged)

| lane | idea | deciding numbers |
|---|---|---|
| nc-composed | C3 anticipatory composed facts | mh1 0.825 -> 0.900 but blind 0.830 -> 0.770 |
| nc-context | B8 context-inherited membership | inherited-mention precision 0.023 on world-300; aggregation gain 0; holdout breaks |
| nc-split | B3 two-ended chain scoring | mh1 and blind three-hop -1 each; 1-core p50 +109%; UltraBERT hop cosines flat (0.206 vs 0.211 right/wrong) |
| nc-chainx | cross-encoder verification of top chains | mh1 +2 (tuning set), blind and world-300 unchanged; p50 +51%, RSS +228 MB |

## Where main stands (5e3e082)

| measure | value |
|---|---|
| fixtures recall / MRR | 0.953 / 0.895 |
| holdout recall / MRR | 0.914 / 0.795 |
| multihop-holdout / blind / private acc@10 (one recall) | 0.825 / 0.820 / 0.867 |
| agent_sim --hints (recall + follow, ~1.3 calls/query) | 0.863 / 0.910 / 0.925 |
| world-300 / world-1000 / world-3000 acc@10 | 0.992 / 0.992 / 0.982 (three-hop 1.000 / 1.000 / 0.897) |
| cold-recall peak RSS | 612-618 MiB f32 model, ~237 MiB q8, flat in DB size |
| DB bytes per memory | ~5,950 f32 vectors, ~2,250 int8 |

## Lessons

- Multi-hop gains tuned on multihop-holdout or multihop-holdout2 repeatedly vanished on the private set
  (nc-anchor, nc-hop, nc-histprose prose lines, nc-composed). Only the private set decides.
- Five cosine-based chain-ranking ideas and one off-the-shelf cross-encoder failed: bi-encoder hop cosines
  are flat. The open next step is a scorer trained on (question, chain) pairs from generated worlds.
- Hash-embedder tests hide real-model false merges and false retractions: every entity/temporal lane gets
  a real-model check table.
- Lanes based on an old main merge cleanly but break semantically: `cargo check --all-targets` after every
  merge.
- At 33k memories, 9 of 11 misses are retrieval misses (answer outside the top 50), not ranking.
