# neocortex failure analysis: where the accuracy goes

Build: `lanes/nc-research` at `367b9a9` (the brief's baseline), `cargo build --release`, UltraBERT-Embed-150M, CPU. Main has since moved (nc-f4, nc-f5 and nc-e45 merged, `9c2d900`+); none of those merges touches the code paths blamed below (`git diff 367b9a9 HEAD -- src/recall.rs` adds a fuzzy lookup to `lookup_entity_fuzzy` for the `entity` tool and the E4a temporal links, not the query-anchor or beam code), but the counts should be re-run on main before a lane relies on them (inference).

Runs (TMPDIR and DBs under `lanes/tmp/research/`):

```
neocortex eval eval/<set>.json --failures --save-db lanes/tmp/research/<set>.db
neocortex --json eval eval/<set>.json --failures --load-db lanes/tmp/research/<set>.db
```

| set | reproduced | committed |
|---|---|---|
| multihop-holdout2 | acc@10 0.820 (two-hop 0.875, three-hop 0.667, updated-chain 0.867, distractor 0.933) | 0.820 |
| multihop-holdout | acc@10 0.825 (two-hop 0.825, three-hop 0.720, single-hop 1.000) | 0.825 |
| holdout | recall 0.913 / MRR 0.791 | 0.913 / 0.791 |

Latency in these runs was p50 264 ms (holdout2), 177 ms (holdout): the machine was running four other lanes' cargo builds at the same time, so these latencies are not comparable to the committed numbers.

## Method

The eval's `--failures` output only lists memory keys and channel ranks. To see *why*, I built two read-only probes outside the repos:

- `lanes/tmp/research/probe/` (a separate crate with a path dependency on a scratch copy of the source, `lanes/tmp/research/nccopy/`): replays every fixture query against the saved eval DB with the fixture clock (`Engine::with_clock`), at k=10 and k=50, and dumps memories, rendered facts and entity cards. Its acc/recall/MRR reproduce `neocortex eval` exactly on all three sets (0.820 / 0.825 / 0.913-0.791).
- The scratch copy prints, behind `NC_DEBUG_PATHS`, the lexical and semantic path hits that `recall` builds (`recall.rs:697`, just before `paths::search` / `paths::search_semantic`), with anchors and semantic start entities.
- Probe modes `cos`, `mcos`, `resid`: rank and cosine of given relations in the relation KNN (`relations_vec`, k=100), of memories in the live vector channel, and the residual hop-2 cosine of a chain exactly as `paths::search_semantic` computes it (`paths.rs:666 residual_after`).
- `examples/agent_sim.rs` (recall -> follow -> recall-leaf) on both multi-hop DBs.

Every failure got one **primary** class (the first thing that, fixed, would have produced the answer in the context, judged from the path dumps), plus secondary contributors. Classes follow the brief; "write-time" means the loss is already in the stored data, "read-time" means the data was there and recall did not surface it.

## Taxonomy counts

64 failures: 18 answer misses on multihop-holdout2 (of 100), 14 on multihop-holdout (of 80), and the 32 holdout queries with recall < 1 or MRR < 1 (holdout has no `answers`; 9 are true misses, 1 is an `expect_absent` violation, 22 are rank 2-5 MRR losses).

| primary class | write / read | mh-holdout2 | mh-holdout | holdout | total |
|---|---|---|---|---|---|
| A. anchor not found or wrong anchor | read (write when a generic alias is stored) | 3 | 13 | 6 | **22** |
| B. fusion / ranking (data reached, not surfaced) | read | 2 | 0 | 11 | **13** |
| C. temporal | read | 3 | 0 | 7 | **10** |
| D. hop scorer dropped the right path | read | 7 | 0 | 0 | **7** |
| E. other: aggregation / set completion | read + write | 0 | 0 | 7 | **7** |
| F. predicate paraphrase did not match | read (write: no predicate synonymy) | 1 | 1 | 0 | **2** |
| G. extraction missed the relation | write | 2 | 0 | 0 | **2** |
| H. other: context only implicit in a neighbouring memory | write | 0 | 0 | 1 | **1** |
| total | | 18 | 14 | 32 | 64 |

Write-time vs read-time, primary only: **3 write-time (G + H)** and **61 read-time**; 3 of the anchor failures (generic aliases "company", "bakery") are also write-time hygiene. The single most important number in this document: **for 18 of 18 multihop-holdout2 failures and 13 of 14 multihop-holdout failures, the memory that contains the answer is in recall's top 50** (`answer-in-k50` in the probe). The memory is stored, indexed and reachable; the loss is in choosing it.

Secondary contributors that cut across classes:

| contributor | count | evidence |
|---|---|---|
| the gold chain contains a text-only (prose, no relation) hop | 14 / 18 mh2 failures | failure rate 14/51 = 27% when the answer sits only in a prose memory vs 4/49 = 8% when it is a relation (mh2 has 55/220 prose memories) |
| residual hop scoring gives the correct bridge hop a **negative** score | 3 / 3 chains measured | see D below |
| a correct full-coverage chain was found but ranked below the top 2 that get rendered as facts | 7 / 18 mh2 | `best-cover` rank 2, 4, 5, 7, 12 in the dumps |
| answer is one hop from a fuzzy start entity (the bridge), but the semantic beam only emits >= 2-hop chains | 5 / 14 mh-holdout | `paths.rs:635` "One-hop facts are already the vector channel's job" |
| heuristic extractor split a Portuguese proper noun and created a hub entity ("Rua", "Universidade", "Largo") that chains unrelated prose | visible in 6 / 18 mh2 dumps | `m070`, `m078`, `m084`, `m137` mention entity "Rua"/"Universidade" |

Agent in the loop (`agent_sim`): multihop-holdout2 0.820 -> 0.870 (+follow) -> **0.920** (+recall-leaf), 1.31 calls/query; multihop-holdout 0.825 -> 0.838 -> **0.838**. The agent loop cannot rescue multihop-holdout because its step 2 needs the same exact entity name that class A lacks.

## A. Anchor not found or wrong anchor (22, read-time)

`recall.rs:515-529`: exact anchors are query n-grams (<= 4 words, `extract::ngrams`) that equal an `entity_aliases.alias_norm`. Only exact anchors start the lexical beam (`paths.rs:300`, `recall.rs:652`), enable prose hops (`paths.rs:458`, "Prose hops need an exact anchor"), and are never passed through (`paths.rs:608-611`).

1. **First names and role descriptions are not aliases (13 mh-holdout, 6 holdout).** multihop-holdout says "Noor's", the stored entity is "Noor Haddad" with aliases `noor haddad` only (checked in the DB). So `anchors = []` for 13 of 14 failures; the semantic beam starts from fuzzy entity-KNN seeds, which are often the *answer type* ("Bremen", "Naples", "Lisbon" for "where was ... born"), and walks *through* Noor Haddad as a hub.
   - "Where was the coach of Noor's rowing club born?" expected Hamburg (`m060`); top semantic chain `coach->Keel & Oar | member of<-Noor Haddad | treat<-Nadia Sokolova` scores 0.938; the bridge Marco Ferri is itself a fuzzy seed, and `Marco Ferri -born in-> Hamburg` is one hop from it, which the beam never emits.
   - holdout: "What does the boss's son play at weekends?", "Where does the luthier's spouse run a research group?" (miss), "the West African postdoc": role/description anchors with no alias.
   - **Measured (E1, scratch copy only, `NC_TOKEN_ANCHORS`)**: a query word that is the first or last token of exactly one multi-word entity name becomes an exact anchor. multihop-holdout **0.825 -> 0.900** (+8 / -2 queries), multihop-holdout2 0.820 -> 0.810 (-1), holdout recall/MRR 0.913/0.791 -> 0.901/0.781 (-1 miss, -1 rank). The anchor gap is real and large, but naive token anchors fail the holdout gate because a new anchor also switches on prose hops and the pass-through block (inference from the two holdout regressions: "Which funding body supports the Lon purification student?" and "which talk got the nod...").
2. **Generic agent aliases become anchors (3 mh2, write-time hygiene).** The fixture writer supplied aliases "the company" (Mirante Bio), "the bakery" (Padaria do Largo), "the CEO" (Luis Amaral), "the marina", "the hackerspace" (DB rows `company|Mirante Bio`, `bakery|Padaria do Largo`, ...). Any question with the word "company" anchors Mirante Bio, and the anchor pass-through block then forbids the real chain.
   - "Which parish is the headquarters of the company Teresa joined in 2024 in?" (Campo de Ourique): chain Teresa -> Mirante Bio -> prose `m070` -> prose `m084` must pass *through* Mirante Bio, which "company" made an anchor.
   - "In which part of town is the bakery of Sofia's oldest friend...": same with "bakery".
3. **Shared short alias anchors a lookalike (1 mh2).** "Simão" is an alias of both Simão Pais and Simão Pinho; the distractor Simão Pais is anchored and its chains outrank Simão Pinho's (`best-cover` rank 8).

## B. Fusion / ranking (13, read-time)

- **mh2 (2): the right chain was found and ranked first or second, but its evidence never reached the context.** "Where did the person who now chairs the ethics committee ... study?": semantic chain #1 is `chair<-Afonso Reis | P[m077] -> University of Coimbra | ...`, i.e. correct. The answer is only in prose memory `m077`; `paths::facts` renders relations only (prose steps have `relation_id 0` and are skipped, `paths.rs:722-727`), and the `sem` channel's weight 0.25 leaves `m077` at fused rank 20. Same for the protein-lab Braga query (chain rank 2, answer in prose `m138`, fused rank 17).
- Also structural: `recall.rs:933` renders facts from `path_hits[..2]` where `path_hits = lexical ++ semantic`; whenever the lexical beam returns >= 2 hits, no semantic chain is rendered at all (code reading; it did not decide any of the 18 here because the lexical beam fired on only 4 of them).
- **holdout (11): rank 2-5 on paraphrase/synonym/preference/procedure queries**, e.g. "Which yearly exercise tests the lab's emergency systems?" (`m063` at rank 2 behind the freezer memory), "how large was the European funding package?" (`m073` rank 2 behind the Novo Nordisk grant), and the known synonym trap "the boat she takes out on the river" (expect_absent kayak outranks the scull, `docs/research/multihop.md` §7). These are dense-similarity ties between near-topic memories; the lane nc-rerank cross-encoder is aimed at exactly this.

## C. Temporal (10, read-time)

- **History-intent gate switches multi-hop off (2 mh2).** `recall.rs:675`: `if negated || history_query { (Vec::new(), Vec::new()) }`. "What is the allergy of whoever *used to* lead Mirante's assay team?" and "the sister of the person who *used to* run people operations" are multi-hop questions about a past role; `temporal::history_intent` fires on "used to", both beams are skipped (`sem hits: 0` in the dumps), and the second hop is never found. **Measured (E2, `NC_SEM_ON_HISTORY`: keep the semantic beam, lexical off, on history questions)**: mh2 **0.820 -> 0.840** (+2), mh-holdout unchanged, holdout 0.913 / 0.791 -> 0.913 / 0.785 (inside the 0.91 / 0.78 gate).
- **Ended relation invisible to a question about a former role (1 mh2).** "Iberia Labs' bioinformatics head": relation 132 (`Gustavo -lead bioinformatics at-> Iberia Labs`) is ended by `m195` (moved to Mirante), so `Visibility::shows` hides it without history intent; no chain from Iberia Labs reaches Gustavo. (The fixture is arguably ambiguous: the current Iberia head is nobody.)
- **holdout (7)**: three `history` queries where the older state ranks 2nd behind the newer one ("what did Ingrid drink before switching to cortado"); "where is the Lon paper now" and "which outlet *finally* took the manuscript" (the acceptance must beat the earlier rejections of the same manuscript; one miss); "the apartment they had before the move"; "must be finished by Christmas" (a holiday name, not parsed as a December window; miss).

## D. Hop scorer dropped the right path (7, read-time)

The semantic beam (`paths.rs:428-664`) scores hop *i* by `cos(r_i, f_i)` where `r_1 = q` and `r_{i+1} = normalise(r_i - (r_i . f_i) f_i)` (`residual_after`, `paths.rs:666`), and ranks chains by the sum.

**Mechanistic finding: the residual penalises the correct bridge hop.** Two consecutive hops of a real chain share the bridge entity ("Carla Azevedo heads clinical development at Mirante Bio" / "Carla Azevedo was born in Lamego"), so their vectors are correlated, while the question rarely names the bridge. Then `resid . f2 = (q.f2 - (q.f1)(f1.f2)) / sqrt(1 - (q.f1)^2)` goes negative exactly when hop 1 is a strong match. Measured with the `resid` probe on stored vectors:

| query | hop 1 | hop 2 (correct) | q.f1 | q.f2 | f1.f2 | resid.f2 |
|---|---|---|---|---|---|---|
| Where was the clinical development head at Mirante Bio born? | r134 heads clinical development | m169 born in Lamego (prose) | 0.655 | 0.173 | 0.515 | **-0.217** |
| Which region is the hometown of the man who started Mirante Bio located in? | r1 founded by | r9 grew up in Évora | 0.597 | 0.203 | 0.437 | **-0.072** |
| Where did the CEO of Mirante Bio study? | r1 founded by | r22 studied at | 0.557 | 0.226 | 0.445 | **-0.026** |

Meanwhile chains that wander through *unrelated* facts (low `f1.f2`, so nothing is subtracted) keep positive hop scores and win on the sum: `found by -> Luis | sit on board of -> Lisbon Science Park | P[m196] -> Lisbon` (0.645) beats the true 2-hop chain (0.655 - 0.8*0.217 = 0.48). Two further biases follow from the same formula: a sum of mostly positive cosines prefers 3 hops to 2 (the Campolide and Alentejo chains are found but outranked by their own 3-hop extensions or by hub walks), and nothing makes a chain *end* at the answer type.

The 7: CEO-of-Mirante study (`m021` fused rank 21; also the lexical beam needs coverage 2 of {ceo, study}, `paths.rs:319`, and "CEO" lives in an alias, not a relation), Alentejo (full chain at semantic rank 12), Gonçalo's manager's university (rank 7), Campolide (rank 5), Tidewater designer's district (rank 4), Covilhã (not in top 12; chain walks backward from the fuzzy seed UBI), clinical head Lamego (the negative residual above).

## E. Aggregation / set completion (7, holdout)

All 7 holdout `aggregation` queries ("who works in the Solberg Lab", "major equipment in the lab", "lab mishaps and lessons") fail; 6 are misses at k=8. The members are mostly prose memories with no relation to a shared entity ("Zoe Lindqvist joined as a PhD student" names no lab), so neither `aggregate` (relation-only, `paths.rs:1000`) nor a ranked list of 8 can return the set. Write-time part: no category/membership edge is stored; read-time part: a ranked top-k is the wrong shape for "list all".

## F. Predicate paraphrase (2)

- "Where was the partner of Ravi Menon born?" (the known one): `married to` vs "partner"; `reside in -> Antwerp | born in <- Seun` wins.
- "Who currently manages Rui, and which city did that person grow up in?": stored as `Joana Pires -manage-> Rui` (inverse of "reports to", `resolve.rs`), the fact text says "reports to"; KNN rank 19 of 173 at cosine 0.177 (`cos` probe), lower than `Rui works at Mirante Bio` (0.337). Write-time side effect: `Pedro Vieira -manage-> Rui` is still live because the text "took over from Pedro Vieira" retracts nothing (TM-10, lane nc-f3).

## G. Extraction missed the relation (2, write-time)

"Which district is the hackerspace where Foli's plugin lead learned to code in?" and "Which area is the marina where the CFO keeps his boat located in?": the answers are in prose `m134` ("Aveiro is the capital of the Aveiro district") and `m135` ("Cascais belongs to the Greater Lisbon area"), which mention the entities **Aveiro** and **Cascais**, while the chain ends at **Hackerspace Aveiro** and **Cascais marina**. Nothing links a name to the place inside it, so the last hop does not exist in the graph.

## H. Implicit context (1, write-time)

"Byåsen housewarming": `m082` ("Ingrid and Daniel hosted a housewarming...") never says Byåsen; the place is only in the adjacent memory about the move (`m080`). BM25 still ranks it first; fusion puts it third.

## What this says about where the next points come from

1. **Anchors (22 / 64).** First-name, role and description references must resolve to an exact anchor; generic aliases must not. Measured upside on mh-holdout alone: +0.075 acc@10, but the naive version costs holdout, so the anchor must be *tiered* (strong anchors start beams; weak token/role anchors start beams but do not enable the pass-through block).
2. **Hop scoring (7 primary, plus the negative-residual effect inside most of the anchor and fusion cases).** The residual must stop subtracting the shared bridge entity; chains need a length-neutral score and an answer-type end condition; one-hop answers from a bridge seed must be allowed.
3. **Prose evidence (secondary in 14 / 18 mh2).** A chain whose last step is a prose memory must put that memory in the context, and prose memories need cheap links (name containment, Portuguese/Romance connectors in proper nouns).
4. **History gate (2 measured, +0.02).** A one-line change, measured inside the gate.
5. Ranking ties on holdout (11) belong to nc-rerank; aggregation (7) needs a set-shaped answer path, not better ranking.

Paths: probe crate `lanes/tmp/research/probe/`, scratch source `lanes/tmp/research/nccopy/` (experiment flags `NC_DEBUG_PATHS`, `NC_TOKEN_ANCHORS`, `NC_SEM_ON_HISTORY`, `NC_HOP_MIX`, `NC_PRED_RESIDUAL`), dumps `lanes/tmp/research/probe-*.json` / `probe-*.paths`, experiment outputs `lanes/tmp/research/{e1,e2,NC_*}-*.json`.
