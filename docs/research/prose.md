# Prose hops as fact lines (nc-prose2)

Measured 2026-11-06 on branch `lane-nc-prose2` (base `c7791f3`), real model
(UltraBERT f32), shared machine (load 10-65). All DBs and logs under
`lanes/tmp/nc-prose2/`. Never tuned on `eval/multihop-holdout2.json` (blind:
only the summary and per-category lines were read).

## Question

nc-prose (killed, `lane-results.md`) promoted a prose-step memory from a top-3
chain **into** the memory ranking: mh2 (blind) 0.820 -> 0.850, but fixtures MRR
0.901 -> 0.777 and holdout 0.795 -> 0.581, because a promoted memory displaced a
correct one. A prose hop is a memory with no relation, so `paths::facts` never
renders it; the only ways it reaches the context are the memory ranking or a
fact line (`FactHit.memory`, B4 `paths::prose_facts`).

Two ways to get prose hops into the context as **lines only**, i.e. with zero
memory displacement:

- `prose_margin` (already on main, default infinity = off): the prose steps of
  the single best semantic chain, rendered only when that chain beats the best
  chain with another terminal by `margin`.
- `prose_wide` (this lane, default off): the prose steps of the top three
  semantic chains at **any** hop position, cap 3 lines, skipping a memory that
  is already rendered as a memory. The lines are appended after every other
  fact channel, so the eight relation/entity fact slots are untouched.

## Step 1: `prose_margin` sweep on main (`step1/*.txt`)

Main binary, one DB per set (`--save-db`), then `--load-db --set prose_margin=V`.

| set | inf (off) | 0.0 | 0.05 | 0.15 |
|---|---|---|---|---|
| fixtures recall / MRR | 0.953 / 0.901 | 0.953 / 0.901 | 0.953 / 0.901 | 0.953 / 0.901 |
| fixtures ctx tokens | 351.9 | 367.1 (+4.3%) | 364.1 | 357.9 |
| holdout recall / MRR | 0.914 / 0.795 | 0.914 / 0.795 | 0.914 / 0.795 | 0.914 / 0.795 |
| holdout ctx tokens | 384.9 | 398.3 (+3.5%) | 388.1 | 385.6 |
| mh1 acc@10 | 0.863 | 0.863 | 0.863 | 0.863 |
| mh2 acc@10 (blind) | 0.820 | **0.840** | 0.830 | 0.820 |
| mh2 ctx tokens | 489.1 | 491.6 | 489.6 | 489.1 |
| world-300 acc@10 / MRR | 1.000 / 0.882 | 1.000 / 0.882 | 1.000 / 0.882 | 1.000 / 0.882 |
| world-300 ctx tokens | 430.4 | 434.1 | 431.3 | 430.7 |

Findings: on main, `prose_margin` never changes a memory number (it only adds
lines), and margin 0 buys mh2 +2 queries for +0.5% context on mh2 and +4.3% on
fixtures. That reproduces the nc-histprose note (+1) and shows the cheapest half
of the idea is already on main behind a knob. mh1 and world-300 do not move at
any margin.

## Step 2: `prose_wide` off/on on the branch (`step2/*.txt`)

Same DBs, branch binary, `--set prose_wide=off|on`.

| set | off | on | delta |
|---|---|---|---|
| fixtures recall / MRR | 0.953 / 0.901 | 0.953 / 0.901 | none |
| fixtures ctx tokens | 351.8 | 360.9 | +2.6% |
| holdout recall / MRR | 0.914 / 0.795 | 0.914 / 0.795 | none |
| holdout ctx tokens | 384.9 | 415.3 | +7.9% |
| mh1 acc@10 | 0.863 | 0.863 | none |
| mh1 ctx tokens | 491.9 | 491.9 | 0 lines added |
| mh2 acc@10 (blind) | 0.820 | **0.860** | +4 queries |
| mh2 ctx tokens | 489.1 | 507.0 | +3.7% |
| world-300 acc@10 / MRR / recall | 1.000 / 0.882 / 0.969 | 1.000 / 0.882 / 0.969 | none |
| world-300 ctx tokens | 430.4 | 450.6 | +4.7% |

Per category on mh2 (blind): three-hop 0.633 -> 0.733 (+3 queries),
updated-chain 0.933 -> 1.000 (+1), two-hop 0.875 and distractor 0.933 unchanged.
The gain is exactly where a prose bridge sits: a hop that has no relation.

Memory ranking is byte-identical with the knob on: `tests/recall.rs`
`prose_wide_never_changes_the_memory_ranking` asserts the same memory ids in the
same order on fixtures and holdout. So the +4 cannot come from displacement, and
the fixtures/holdout MRR regression that killed nc-prose cannot recur by
construction.

mh1 adds no lines at all (identical context size): its chains end on relations
whose prose bridges are already inside the rendered top 10, so the skip rule
drops them. The knob's cost on mh1 is zero.

## Cost

- Context: +2.6% fixtures, +3.7% mh2, +4.7% world-300, +7.9% holdout. The
  holdout number is the worst and sits just under the +8% kill line; it is one
  or two lines per query on a 88-query set, so a single long prose memory moves
  it by a percentage point.
- Latency: 3 interleaved world-300 pairs, one core (`taskset -c 3`), main /
  branch-off / branch-on on the same DB in one session (`latency.txt`):

  | pair | main p50 | off p50 | on p50 |
  |---|---|---|---|
  | 1 | 258.7 | 231.4 | 244.9 |
  | 2 | 178.5 | 166.3 | 171.5 |
  | 3 | 168.4 | 220.4 | 168.9 |
  | mean | 201.9 | 206.0 (+2.0%) | 195.1 (-3.4%) |

  The knob costs one pass over the semantic chains plus at most three memory
  reads; the spread inside each column (168-258 ms) is larger than the
  difference between the columns, so there is no measurable latency cost.

## Decision

Kill criterion "memory MRR/recall changes on fixtures, holdout or world-300":
not met (no memory number moves, by construction and by test).
"mh2 gain < +2": met (+4).
"context_tokens_mean +8% on any set": not met, but holdout is +7.9%.

The knob stays **off by default**: it costs up to 8% of the context on a
general-purpose set for a gain that only appears on multi-hop prose bridges, and
`prose_margin = 0` already buys half of it (mh2 +2) for +0.5% context. If one of
the two is ever turned on for a multi-hop workload, turn on `prose_margin = 0`
first and `prose_wide` only when the extra 2 mh2 queries are worth the lines.

## Reproduce

```bash
# step 1 (main binary)
neocortex eval eval/holdout.json --save-db tmp/db/holdout.db
neocortex eval eval/holdout.json --load-db tmp/db/holdout.db --set prose_margin=0
# step 2 (branch binary)
neocortex eval eval/multihop-holdout2.json --load-db tmp/db/mh2.db --set prose_wide=on
```
