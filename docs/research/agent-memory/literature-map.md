# Literature map: agent memory, LLM-free structure, multi-hop without an LLM, cognitive mechanisms

Scope: 46 mechanism cards in `papers/`, covering about 50 sources read in HTML/PDF full text, plus primary sources I could only reach through secondary summaries (see "Not read" at the end). Everything below cites a card; each card cites the paper section or table. `(inference)` marks my own reasoning. The Awesome-GraphRAG OKF catalogue (`remem-v31/docs/okf/graphrag/`) was skimmed for adopt/adapt entries; this pass adds the literature beyond it and re-reads the entries that matter (HippoRAG 2, Graphiti/Zep, RAPTOR, GNN-RAG, SubgraphRAG) from full text.

neocortex weak spots used to rank everything: three-hop 0.667, relation paraphrase ("partner" vs "married to"), temporal reasoning over months, scale to 100k+ entities, LLM-free extraction quality.

## 1. Clusters and who builds on whom

```
Cluster 1  Agent memory systems and benchmarks
  MemGPT (paging, 2023) --> Generative Agents (recency/importance/relevance, reflection) --> MemoryBank (Ebbinghaus S+=1)
  Generative Agents --> A-Mem (LLM links + evolve) --> REALM (LLM reconsolidation of graph weights, 2026)
  RAPTOR (tree of summaries) --> HippoRAG (KG + PPR, hippocampal indexing) --> HippoRAG 2 (query-to-triple, passage nodes)
  HippoRAG/GraphRAG --> Zep/Graphiti (bi-temporal, 2025) --> Engram (bi-temporal, no LLM on write, 2026) / Agent Zero Memory (provenance)
  Mem0 (extract/ADD-UPDATE-DELETE) --> Mem0g (graph) ; Zep vs Mem0 disputes
  RippleMem (event graph, Jaccard cues, LLM-free edges, 2026) <-- A-Mem links, HippoRAG pattern completion
  EM-LLM (surprise + contiguity) <-- TCM (Howard & Kahana)
  Benchmarks: LoCoMo (2024) -> LongMemEval (2024) -> MemTrace, Memora/FAMA (2026) criticising both for shallow evidence depth

Cluster 2  Cheap structure from text
  OpenIE6 (BERT grid labelling) ; GLiNER (span-type matching, 50-300M) -> GLiREL (pair x label, 467M) ; ReLiK (retriever + reader over a relation KB)
  SPLADE v2 (sparse expansion) ; ColBERT -> ColBERTv2 -> PLAID (late interaction, centroid pruning)
  Query decomposition w/o LLM: Break It Down (QDMR) ; ONUS (retrieval-built pseudo-decompositions)

Cluster 3  Multi-hop without an LLM in the loop
  PPR / Katz / Bellman-Ford --> NBFNet (learned semiring) ; PPRGo (push-flow top-k PPR) ; PPR == SR (Millidge 2025)
  GNN-RAG (PPR subgraph + GNN answer scoring) ; SubgraphRAG (MLP + directional distance encoding) ; Beam Retrieval (chain-conditioned beam)
  KGE: ComplEx -> RotatE (relation patterns, composition) ; superseded on link prediction by NBFNet

Cluster 4  Cognitive mechanisms
  ACT-R (power-law base-level + fan) <-- Anderson & Schooler rational analysis ; testing effect / power-law forgetting
  TCM == successor representation == PPR (Gershman 2012, Stachenfeld 2017, Millidge 2025)
  CLS (fast episodic + slow structured, replay, schema-dependent consolidation) --> generative replay
  Reconsolidation + prediction error (NMPH) --> REALM
  Pattern separation/completion --> modern Hopfield (= attention over memory)
```

The unifying result of cluster 4 and 3 together: **TCM, the successor representation, personalized PageRank, and HippoRAG's graph channel are the same linear-algebra object** `(I - gamma T)^-1` (Gershman 2012 Sec 4; Stachenfeld 2017 Eq 6; Millidge 2025). neocortex already has the object (graph channel). What the literature adds are ways to make it cheaper (push-flow, cached top-k), better seeded (query-to-triple, node specificity/fan), learned (NBFNet edge weights, TD-SR), and time-aware (drifting context).

## 2. Strongest measured results (with the caveat that cross-paper numbers are not comparable)

| Result | Where | Number |
|---|---|---|
| Lean retrieved context beats full context | Engram, LongMemEval-S, official judge | 83.6% (9.6k tokens) vs 73.2% (79k); McNemar p<1e-6 |
| Graph memory vs full context | Zep, LongMemEval-S, gpt-4o | 71.2% vs 60.2% (but loses -14 pts on single-session-assistant) |
| PPR memory vs best dense retriever | HippoRAG 2, passage R@5 avg over 5 sets | 78.2 vs 73.4 (NV-Embed-v2 7B); MuSiQue 74.7 vs 69.7 |
| LLM triple filter worth little | HippoRAG 2 ablation | 87.1 -> 86.4 R@5 without it |
| Query-to-triple vs NER-to-node seeding | HippoRAG 2 ablation | MuSiQue 74.7 vs 53.8; HotpotQA 96.3 vs 78.8 |
| Weak OpenIE costs a lot | HippoRAG Table 5 (REBEL vs GPT-3.5) | MuSiQue 51.9 -> 39.6, 2Wiki 89.1 -> 76.5, HotpotQA 77.7 -> 59.2 |
| PPR propagation | HippoRAG Table 5 | 2Wiki R@5 61.4 (seeds only) -> 89.1 |
| Chain-conditioned beam | Beam Retrieval, MuSiQue-Ans retrieval EM | 53.50 -> 77.37 (beam 1) |
| Learned hop scorer with structure | SubgraphRAG, WebQSP shortest-path triple recall | 0.883 vs 0.714 cosine |
| Deterministic edges are enough | RippleMem, LoCoMo J | 87.14 vs 86.62 with LLM-chosen edges at 27.7x tokens; graph expansion worth +4.0, structural cues +3.3 |
| Doc-side sparse expansion | SPLADE-doc (no query encoder) | MS MARCO MRR@10 0.322 vs BM25 0.184 |
| Late interaction at CPU speed | PLAID, MS MARCO 8.8M passages | MRR@10 39.4, 185 ms on one CPU thread (vanilla 3,485 ms) |
| Retrieval-induced learning | Roediger & Karpicke | 2 days: 68% tested vs 54% restudied; 1 week 56% vs 42% |
| Reconsolidation gain | REALM | +2.0 (LoCoMo), +2.1 (LongMemEval); multi-hop +4.97 |
| Evidence use is the bottleneck | MemTrace | 220 of 300 probes reached the gold session but were unsolved vs 21 reach misses |

LoCoMo / LongMemEval "SOTA" claims of 93-96% (Agent Zero Memory 95.6 / 93.6) are self-reported with frontier backbones and LLM judges; Memora shows 94% (LoCoMo) and 85% (LongMemEval) of questions need at most two earlier sessions, so these two benchmarks are close to saturated and do not test months-scale consolidation.

## 3. Open contradictions between papers

1. **Does graph structure help agent memory?** Zep (+11 pts over full context), Mem0g (+ on temporal), HippoRAG 2 (+4.8 R@5 over dense) say yes. Selective Forgetting (LongMemEval, matched budget): graph F1 0.417 vs flat 0.468, delta -0.050 (95% CI [-0.085, -0.016]), and -30 points on assistant-turn recall (0.911 -> 0.607). Engram: facts-only read path loses recall, the hybrid (facts + raw chunks) is needed. Reconciliation (inference): graphs help multi-hop and update/temporal questions and hurt verbatim recall; the winning design keeps raw text next to structure, which neocortex does.
2. **Is an LLM needed to build the links?** A-Mem and Mem0 use LLM link/update decisions; RippleMem's controlled replacement shows an LLM edge-verifier gives +2.1 on multi-hop but lowers overall (86.62 vs 87.14); HippoRAG 2 shows the LLM triple filter is worth 0.7; but HippoRAG shows LLM-grade *extraction* is worth 12-18 R@5 over REBEL. So: LLM matters at *extraction*, not at link scoring or filtering.
3. **Memory vs full context.** Zep's own DMR table shows full-context gpt-4-turbo (94.4%) beats MemGPT (93.4%); Mem0's full-context J 72.9 beats every memory system (best 68.4); Engram and Zep beat full context on LongMemEval-S. The sign depends on the haystack (115k tokens is within modern windows), the model and the judge.
4. **Benchmark numbers are not comparable.** Zep 71.2% vs Agent Zero's 95.6% on the same benchmark; Mem0 66.88 (own paper, LoCoMo) vs Zep's 75.14 (Zep blog, seen only as a search result, unverified) vs 92.5 (Mem0 2026 blog, search result). Engram documents truncated baselines, home-grown judges and 18-item slices that read 83% vs 58% on the full set.
5. **Forgetting curve.** MemoryBank and Generative Agents use exponential decay; Wixted/Anderson-Schooler/ACT-R use power laws (`d = 0.5`). Selective Forgetting prunes 9.8% of nodes with F1 +0.001 but judged -1.6 (CI to -3.8) and has no random-prune control.
6. **Zero-shot NER/RE numbers vs reality.** GLiNER-L 60.9 average F1 and GLiREL 83.7 F1 (given gold spans) on benchmarks; neocortex's own independent holdout gives GLiNER2.5 entity F1 0.707 and relation F1 0.200. GLiREL's own document-level F1 drops 53 -> 26 with predicted coreference. Benchmarks overstate agent-text extraction.
7. **Reconsolidation direction.** NMPH says moderate reactivation *weakens* (opens updating) and strong reactivation strengthens; REALM strengthens on successful use and weakens on irrelevant surfacing. Not contradictory if "weaken" is read as "make editable", but no AI paper implements the nonmonotonic rule.
8. **One-hop vs multi-hop retrieval.** GNN-RAG: deep GNNs win multi-hop, LLM relation matching wins one-hop (theorem in appendix). SubgraphRAG: structure features help both but the MLP-only variant already carries one-hop. neocortex's lexical channel for one-hop and graph channels for chains matches this.

## 4. Ideas the literature points at that nobody has built in an LLM-free memory

Ranked by (expected effect on a named weak spot) / (cost). Each needs no LLM at run time. Items marked `*` are not in the lanes already running (nc-f3, f4, f5, e45, hop, rerank).

1. `*` **Distributional predicate synonymy from argument pairs.** Merge/alias predicates (`partner`, `married to`, `spouse of`) when the (subject, object) pair sets of their facts overlap, or one is the inverse of the other. Sources: HippoRAG synonym edges at cosine 0.8 (they work at node level); ReLiK's closed relation KB; RotatE/ComplEx learn relation similarity from exactly this co-occurrence; neocortex today aliases predicates by verb-phrase embedding cosine (`resolve.rs:979`), which cannot see "partner" ~ "married to". Cheap: a SQL self-join on `relations(subject,object)` + Jaccard, run in the consolidation pass. Targets the open paraphrase gap and the three-hop middle-hop misses. (inference on effect; the mechanism is classical OpenIE/relation clustering which I did not re-read.)
2. `*` **Document-side sparse expansion (SPLADE-doc) of relation facts and memories** into FTS5. `splade-v2.md`. Query side stays plain tokens, so zero read-path model cost. Needs a ~66M MLM (new model file).
3. `*` **LLM-free HippoRAG 2 seeding in the graph channel:** query-to-fact KNN seeds with score-proportional reset, memories as PPR nodes (reset mass x 0.05), seeds weighted by fan `max(0, S - ln degree)`. HippoRAG 2 ablations: filter -0.7, passage nodes +6.1, query-to-triple vs NER seeding +21 (MuSiQue). `hipporag2.md`, `actr-base-level.md`.
4. `*` **TCM context-drift channel:** store the drifting context vector `c_t = rho c_{t-1} + (1-rho) e_t` with every memory; retrieve by context similarity (several rho: hours, days, months); chain from a retrieved memory by *its* context vector with forward bias. Targets temporal reasoning over months and "what else happened then". `tcm-cmr-successor.md`. Nobody has this in an agent memory (EM-LLM has only fixed neighbours inside a KV cache).
5. `*` **Prediction-error-gated, labile-window writes.** A write may end a fact only if it mismatches the slot of a live fact (high PE); recalled facts are labile for W hours and writes that overlap them are reconciled rather than duplicated. `reconsolidation-prediction-error.md`, `engram-bitemporal.md`. Complements nc-f3 retraction work (TM-10) and REL-19 (typo ends correct fact).
6. `*` **Schema-consistency gate + prioritised, interleaved replay consolidation** (CLS). Schema-consistent writes go straight to the slow store; inconsistent ones wait for replay with old facts interleaved. Replay priority from surprise and feedback. Re-runs improved extractors over old memories. `cls-consolidation.md`.
7. `*` **Learned edge weights in PPR (NBFNet-lite):** one scalar per predicate group (or per predicate x direction) fit by logistic regression on shortest-path labels from generated worlds. NBFNet shows PPR is a special case of a learnable path semiring. Overlaps the training data effort of nc-hop but needs no hop MLP. `nbfnet.md`.
8. `*` **Composition-rule materialisation:** mine rules `r1(x,y) & r2(y,z) => r3(x,z)` from the graph (AMIE-style, not read here; RotatE shows composition as `r3 = r1 o r2`), write derived edges with provenance `inferred` at consolidation so a two-hop question is one hop. Reduces the beam depth that costs the three-hop accuracy.
9. `*` **Embedding-novelty surprise as importance and event boundary** (replaces the 1-10 LLM poignancy in Generative Agents and the token NLL in EM-LLM): `surprise = 1 - max cos(e_t, recent)`, boundary when `> mu + gamma sigma`. Feeds replay priority and archive decisions. `em-llm.md`, `generative-agents.md`.
10. `*` **Retrieval-feedback edge reinforcement** without an LLM: co-used memories strengthen their link (`w <- clip(w + eta c)` with REALM's eta rules), ignored surfacing weakens; separate "surfaced" from "used" from "re-ingested" (testing effect). `realm-reconsolidation.md`, `testing-effect-forgetting.md`. Overlaps nc-e45 on memory-level reinforcement; the new part is edge-level and the surfaced/used distinction.
11. **Spectral / SR smoothing of the entity graph** to compensate for noisy LLM-free extraction (truncated eigendecomposition, TD-SR rows). `predictive-map-sr.md`, `ppr-successor-equivalence.md`.
12. **Structural (who/where/when) Jaccard + temporal-overlap edges between memories**, built at insert time from KNN + inverted indexes, with one bounded expansion round after first-hop recall. RippleMem Eq 2-4 without the LLM controller (the agent is the controller). `ripplemem.md`.
13. **Age- and FAMA-stratified retrieval-only evaluation** (Memora, MemTrace): same fact probed after 10/100/1000 intervening memories, penalty for returning ended facts, trajectory questions. Memora and MemTrace have no retrieval-only, no-LLM-judge variant; LongMemEval has Recall@k on labelled evidence turns but does not penalise returning ended facts.
14. **Forward-asymmetric temporal neighbours** (contiguity effect) in `temporal_link_ids`: one-parameter A/B.
15. **Push-flow PPR with a cached top-32 neighbour table** for scale (PPRGo): keeps the graph channel flat as entities grow. `pprgo.md`.

## 5. Cognitive mechanisms as implementable specs (equations, SQLite + embedding implementation, evidence)

| Mechanism | Equation(s) | SQLite + embedding implementation | Cost | Evidence status |
|---|---|---|---|---|
| ACT-R base-level + fan | `B_i = ln sum t_k^-d` (d=0.5); `A_i = B_i + sum_j W_j S_ji + eps`; `S_ji = S - ln fan_j` (S~2); `P = 1/(1+e^-((A-tau)/s))`; approx `B ~ ln(n/(1-d)) - d ln L` | `access_count`, `created_at` (or `accesses` log); fan = entity degree in `mentions`; rank term in RRF; threshold for abstention | O(1) per candidate | Strong for humans (rational analysis); untested vs recency baselines on agent benchmarks |
| Testing effect / retrieval-induced strengthening | `R = m(1+ht)^-f` (power law, secondary source); RK2006: 2 d 68% vs 54%, 1 wk 56% vs 42% | log `used` events (weight 1.0) vs `surfaced` (0.3); spacing bonus `1 + c ln(1 + gap)`; never delete, only rank/archive | one insert per use | Behavioural, strong; AI value is inference |
| Context-dependent memory (TCM/CMR) | `t_i = rho_i t_{i-1} + beta t_IN`, `||t||=1`; `t_i . t_j = rho^|i-j|`; `M += alpha f t^T` | extra vec0 column `context_vec` per memory; running vector per stream; multi-rho; chained recall with forward bias | one extra KNN + one 768-d row per memory | Strong for free recall; none for agent QA |
| Systems consolidation / replay / CLS | no closed form; interleaved replay with priority (reward, novelty) | consolidation worker: sample `p ~ surprise * (1+fb) / (1+replays)` + uniform old share; rerun extraction/resolution/communities | background CPU | Strong for neural nets and rodent data; AI-memory use is inference |
| Schema-dependent consolidation | consistent -> fast no-interference; inconsistent -> slow, needs interleaving (Kumaran 2016) | per-type predicate/cardinality stats; direct write vs `conflict_candidate` + replay | stats table | Simulation + rodent evidence; implementation inference |
| Reconsolidation on recall + prediction error | NMPH: weak react. no change, moderate weakens (plastic), strong strengthens; REALM `w <- clip(w + eta c)` | `labile_until` on recalled rows; reconcile overlapping writes; PE = slot mismatch; edge weight update | one column | Human/animal evidence mixed; LLM-based +2 pts (REALM) |
| Pattern separation / completion | `xi' = X softmax(beta X^T xi)` (modern Hopfield); capacity DAM `K_max = alpha_n N^(n-1)` | two similarity bands (same / lure / independent); store discriminating tokens; softmax over top-K with margin as ambiguity/abstention signal; optional second-hop re-query | one matmul | Anatomy + fMRI; algorithmic mapping hypothesised |
| Spreading activation (ACT-R / PPR) | `A_i = B_i + sum W_j S_ji`; PPR `Pi = alpha (I-(1-alpha)P)^-1` == `alpha * SR(gamma=1-alpha)` | existing graph channel; fan-weighted seeds; cached top-k; TD-SR | cheap | Strong in IR (HippoRAG 1/2) |
| Predictive coding / surprise-gated encoding | `surprise > mu_{window} + gamma sigma_{window}` (EM-LLM Eq 1); PE as mismatch with live slot | embedding-novelty vs recent memories; slot mismatch for supersession; both feed importance/replay | already have the KNN | Human event-boundary match (EM-LLM Fig 4); embedding version is inference |

Not an implementation recommendation list for every row: priority order is in section 6.

## 6. What to do first (ranked, with the experiment that would prove it)

1. Seeds: query-to-fact weighted PPR seeds + memory nodes + fan weighting (idea 3). Experiment: multihop-holdout2 three-hop 0.667, world-1000 p50. Files: `recall.rs` graph channel, `paths.rs`.
2. Predicate synonymy by argument-pair overlap (idea 1). Experiment: "partner of Ravi Menon" query in multihop-holdout (the known miss), plus a precision check that it does not merge `manager of`/`reports to` incorrectly (direction!). Files: `resolve.rs::resolve_predicate`, consolidation pass.
3. Age-stratified, FAMA-style retrieval-only eval (idea 13) so that any temporal change (TCM channel, activation term, ended-fact penalties) has a metric. No code in the engine.
4. Context-drift channel (idea 4) + forward-asymmetric neighbours (idea 14). Experiment: the new months-horizon world from item 3.
5. Learned per-predicate PPR weights (idea 7) or the hop MLP from nc-hop, whichever wins on three-hop.
6. Doc-side sparse expansion (idea 2) only if item 2 leaves a paraphrase gap, because it adds a model file.

Things the literature says not to spend time on for neocortex: KGE training (RotatE/ComplEx) on sparse personal graphs; a ColBERT index (PLAID shows scale is available later, not needed at 100k); MemoRAG/RAPTOR-style LLM summary trees; MemGPT-style server-side paging; per-memory LLM importance; LLM edge verification.

## 7. Not read in full, or reached only through secondary text

- Primary text not obtained: Anderson & Schooler 1991, Howard & Kahana 2002, Polyn et al. 2009 (CMR), McClelland et al. 1995 (scanned PDF, not read), Wixted & Carpenter 2007, Carpenter et al. 2008, Nader et al. 2000, Klinzing et al. 2019 (sleep consolidation; paywalled), Gilboa & Marlatte 2017 and van Kesteren et al. (schema; site returned a stub), Itti & Baldi (Bayesian surprise), REBEL, PRA (Lao & Cohen), TransE, AMIE. Equations attributed to them were taken from the papers that quote them (cited per card) or from my own knowledge and are marked.
- Seen as abstracts only (search results, 2026): LongMemEval-V2 (2605.12493), SodaMem (2608.08055; 92.8% LongMemEval-S), LeanMem (2608.03463), Memanto (2604.22085; 89.8/87.1), StructMem (2604.21748), TriMem (2605.19952), LazyMem (2607.22690), StructMemEval (2602.11243). Treat their numbers as unverified.
- PDFs downloaded but unused: Wake-Sleep Consolidated Learning (2401.08623), SynapticRAG (2410.13553; judged by title only, not read).
