# neocortex: what would make it the best agent memory (synthesis and new ideas)

Inputs: `failure-analysis.md` (phase 1, this folder; 64 classified failures with measured experiments E1-E3), the five repo cards in `repos/`, the 46 paper cards and `literature-map.md` in `papers/` (partial when written; re-checked at the end), the OKF catalogue `remem-v31/docs/okf/graphrag/` (adopt/adapt entries) and the code at `367b9a9`. Citations: `repos/<card>.md §n`, `papers/<card>.md`, `src/<file>.rs:<line>` in `lanes/nc-research`. `(inference)` marks my own reasoning. Running lanes (nc-f3 temporal text, nc-f4 typo dedup, nc-f5 ops, nc-e45 links/retention, nc-hop trained hop scorer, nc-rerank cross-encoder) are built on, not re-proposed.

Failure classes used below (counts from `failure-analysis.md`, 64 failures over multihop-holdout2 = mh2, multihop-holdout = mh1, holdout):
A anchor not found / wrong anchor 22 · B fusion/ranking 13 · C temporal 10 · D hop scorer 7 · E aggregation 7 · F predicate paraphrase 2 · G extraction missed relation 2 · H implicit context 1. Cross-cutting: the answer memory is in the top 50 for 18/18 mh2 and 13/14 mh1 failures; 14/18 mh2 failures have a prose (relation-less) hop; the residual hop score is negative for the correct bridge hop in 3/3 measured chains.

Measured in phase 1 (scratch copy, not in any repo):

| experiment | mh2 acc@10 | mh1 acc@10 | holdout R / MRR | verdict |
|---|---|---|---|---|
| baseline 367b9a9 | 0.820 | 0.825 | 0.913 / 0.791 | |
| E1 unique first/last name token = exact anchor | 0.810 | **0.900** | 0.901 / 0.781 | right lever, wrong tier (fails holdout gate) |
| E2 keep semantic beam on history-intent questions | **0.840** | 0.825 | 0.913 / 0.785 | passes gates |
| E3a hop score = mean(raw, residual) cosine | 0.810 | 0.825 | 0.913 / 0.792 | no |
| E3b residual removes predicate vector, not fact vector | 0.790 | 0.800 | 0.913 / 0.788 | no |
| agent_sim recall -> follow -> recall-leaf | 0.920 | 0.838 | | the caller rescues mh2, not mh1 |

E3a/E3b are honest negatives: the negative residual is real (failure-analysis §D) but patching the residual alone does not fix the chain ranking, because the sum-of-cosines aggregate also rewards long hub walks and has no notion of where the answer sits. That motivates B3 below, for which I measured a preliminary signal (§B3).

---

## A. Bird's-eye map: what all these systems approximate

1. **Memory is an index over episodes, not a replacement for them.** HippoRAG calls the KG a hippocampal index over passages (`papers/hipporag.md`); Engram needs facts plus raw chunks (`papers/engram-bitemporal.md`); Selective Forgetting measures graph-only memory losing 30 points on verbatim recall (`literature-map.md` §3.1). *neocortex:* stores raw memories and the graph (good), but its read path treats raw text as second class: prose hops are discounted (`PROSE_HOP 0.8`, `paths.rs:418`), never rendered as facts (`paths.rs:721-727`), and a 0.25-weight channel carries them. 14/18 mh2 failures have a prose hop.
2. **Retrieval is inference over a latent chain.** PPR = successor representation = TCM context (`papers/ppr-successor-equivalence.md`, `papers/tcm-cmr-successor.md`); Beam Retrieval and SubgraphRAG learn to score chains conditioned on the question (`papers/beam-retrieval.md`, `papers/subgraphrag.md`). *neocortex:* has PPR plus two beams, but the semantic beam's score is a heuristic sum of cosines whose residual term is anti-correlated with true bridges (measured −0.217/−0.072/−0.026). The next step is a chain score that explains *parts* of the question (B3) and is learned (C2, nc-hop).
3. **Identity is a probabilistic decision, on both sides of the memory.** Write side: merge or not (Hindsight's scored resolver, `repos/hindsight.md` §3 W5; Graphify's veto layers, `repos/graphify.md` steal #2). Read side: which stored entity does "Noor's", "the boss", "the company" mean. *neocortex:* careful and calibrated at write (`entity_conflict 0.73`, homonym pools, `resolve.rs`), naive at read (exact alias n-gram, `recall.rs:515-529`). Read-side identity is the single largest loss: 22/64 failures.
4. **Time is a first-class dimension.** Zep/Graphiti and Engram are bi-temporal (`papers/zep-graphiti.md`, `papers/engram-bitemporal.md`); MemTrace shows trajectory questions are a separate skill (`papers/memtrace.md`). *neocortex:* bi-temporal storage is among the best (half-open validity, `expired_at`, as-of). The weakness is query time: temporal intent is a *switch* (`recall.rs:675` turns multi-hop off on "used to") rather than a weight; 10/64 failures are temporal.
5. **Memory should learn from being used.** Testing effect, reconsolidation, REALM's edge updates (`papers/testing-effect-forgetting.md`, `papers/realm-reconsolidation.md`); agentmemory's retention (`repos/agentmemory.md` steal #1). *neocortex:* logs `access_count` and (after nc-e45) blends retention at 0.05; nothing learns *which anchor, predicate or path* answered a question, or that a question went unanswered.
6. **Spend compute where it is cheap.** CLS replay (`papers/cls-consolidation.md`), Hindsight mental models and observations (`repos/hindsight.md` §7), RAPTOR trees (`papers/raptor.md`): precompute at write/idle time what reads will need. *neocortex:* zero idle compute; every multi-hop question is solved from scratch inside the 150 ms read budget.
7. **The caller is the reasoner.** StructGPT-style tools, Hindsight's reflect agent (`repos/hindsight.md` §7.3), Break It Down's operator vocabulary (`papers/break-it-down.md`). *neocortex:* has `follow`/`aggregate`, and the agent loop adds +0.10 on mh2, but `recall` never tells the agent what is missing or which entity to follow, so the loop adds nothing on mh1 (0.825 -> 0.838).

---

## B. Merged ideas (two or more sources combined; neither built it)

### B1. Tiered query anchors (alias + name token + role + description), fan-weighted
Sources: E1 (phase 1), Graphify per-term seed guarantee and generic-label cap (`repos/graphify.md` steal #1), Hindsight hub damping `1/sqrt(degree)` (`repos/hindsight.md` §3 W5), ACT-R fan `S - ln fan` (`papers/actr-base-level.md` l.20), neocortex's `is_role_noun` (`resolve.rs:913`) and `TERM_SYNONYMS` boss->manager (`paths.rs:121`).
- **Hypothesis.** Most anchor misses are first names ("Noor's"), role nouns ("the CFO", "the boss") and short descriptions ("the luthier", "the West African postdoc"). Resolving them in tiers, and letting weaker tiers start beams *without* the strong-anchor side effects, keeps E1's gain without E1's holdout losses.
- **Mechanism.** In `recall.rs` after `alias_entities` (`recall.rs:525`) build `anchors: Vec<(entity, tier, weight)>`:
  - T0 exact alias (today).
  - T1 a query token equal to the first or last token of exactly one multi-word entity name (E1), read from a new `entity_name_tokens(token, entity_id)` table written in `resolve.rs::insert_entity` (LIKE '% x' cannot use an index).
  - T2 role noun: a query token in `is_role_noun` or the synonym table, mapped to relations whose predicate canonical contains it (`cfo of`, `manage`, `head`, `lead`) -> the subject entity; if the query also names the object ("the CFO of Mirante Bio") require that object.
  - T3 description: the query noun phrase (B3's NP-span) KNN against `entities_vec` cards with a margin rule (top-1 minus top-2 >= 0.05, inference to calibrate).
  - Weight `w = tier_weight * max(0, 2 - ln degree)` (ACT-R fan). T0/T1 enable prose hops; only T0 is added to the "never pass through" set (`paths.rs:608-611`); T2/T3 only start beams and seed PPR.
- **Attacks.** A (22). Expected: mh1 +6 to +8 queries (E1 measured +8/−2; the two mh1 losses and both holdout losses involve the new anchor enabling prose hops or the pass-through block, inference to verify); holdout role/description cases (6, all at rank 2-5 or one miss) +0.01 to +0.03 MRR (inference).
- **Cost.** One indexed lookup per query token (<= 16) on `entity_name_tokens` (< 1 ms, inference), T3 one extra KNN on `entities_vec` (already done once, `recall.rs:532`, reuse it). Size: ~3 rows per entity.
- **Experiment.** Config `anchor_tiers` in {0, 01, 012, 0123} via `eval --sweep`; report mh1, mh2, holdout, fixtures, world-300 per category plus anchor counts per tier. Never tune on mh2 beyond the 4 configs.
- **Kill.** No config reaches mh1 >= 0.875 while keeping fixtures >= 0.95/0.89 and holdout >= 0.91/0.78.

### B2. Generic-alias demotion at write
Sources: ReMem "auto-link only on unique evidence" (`repos/remem.md` steal #3), Graphify "a generic label can seed only once" (`repos/graphify.md` steal #1), LightRAG shared sanitation (`repos/lightrag.md` steal #10), IDF.
- **Hypothesis.** Agent-supplied aliases such as "the company", "the bakery", "the CEO", "the marina", and a shared first name ("Simão" for two people) behave like common nouns; as exact anchors they hijack the beam and the pass-through block (3 mh2 failures).
- **Mechanism.** `resolve.rs::add_aliases` (`resolve.rs:806`) marks an alias `origin = 'weak'` when its norm is one token and (a) is a type-like or role noun (the entity's own type word, `is_role_noun`, a 300-word common-noun list), or (b) is the last token of >= 2 entity names, or (c) already names another entity. `alias_entities` (`recall.rs:191`) returns weak hits as T2/T3 anchors (B1), never T0. Existing DBs: a one-off `UPDATE` in the migration ladder.
- **Attacks.** A (3 mh2: Teresa/"company", bakery, Simão) and the hub noise in D. Expected +2 to +3 on mh2 (the Teresa and bakery chains exist in the graph and were blocked only by the anchor, failure-analysis §A.2).
- **Cost.** Write: one count query per new alias. Read: none.
- **Experiment.** mh2, fixtures, holdout, world-300; count weak aliases created per set.
- **Kill.** fixtures or holdout regress by more than 0.005, or mh2 gains fewer than 2 queries.

### B3. Two-ended chain scoring: rule-based question split (QDMR-lite) + chain-conditioned beam
Sources: Break It Down's operator view (`papers/break-it-down.md`), ONUS pseudo-decomposition (`papers/unsupervised-question-decomposition.md`), Beam Retrieval's "condition the next hop on the chain" (`papers/beam-retrieval.md`), SubgraphRAG's directional structure (`papers/subgraphrag.md`), phase-1 residual measurement.
- **Hypothesis.** A multi-hop question has two ends: a noun phrase that describes the first hop ("the clinical development head at Mirante Bio") and a wh-frame that names the last hop ("Where was ... born?"). Scoring the first hop against the NP-span and the last hop against the wh-span separates true chains from hub walks far better than the residual. Preliminary measurement on stored vectors (`resid` probe, failure-analysis §Method):

  | wh-span (query mode) | correct last hop | distractor last hop |
  |---|---|---|
  | "Where was the person born?" | 0.283 (m169 born in Lamego) | 0.072 (sits on board of Lisbon Science Park) |
  | "Which region is the hometown located in?" | 0.261 (m030 Évora capital of Alentejo) | 0.102 |
  | "Where did the person study?" | 0.274 (r22 studied at) | 0.120 |

  and NP-span "the clinical development head at Mirante Bio" vs first hop: correct r134 0.719, `founded by` 0.495. A 2-4x margin on three chains (n=3, inference that it generalises).
- **Mechanism.** `paths.rs`: new `split_question(q) -> (np_span, wh_span)` by rules: wh-word + auxiliary + final verb group form the wh-frame; the longest noun phrase containing an anchor or a role noun is the NP-span; the wh-span is the question with the NP-span replaced by "the person"/"the place" (by anchor type). Embed both in the *same* `embed` call as the query (`recall.rs:362`). Chain score for chain f_1..f_n: `S = cos(np, f_1) + cos(wh, f_n) + 0.5 * sum_{1<i<n} cos(r_i, f_i)`, divided by a length prior `(1 + 0.25 (n-2))` (constant to sweep), and 1-hop chains are emitted when they start at a fuzzy/bridge seed and `cos(wh, f_1)` is the max (fixes the 5 mh1 "answer is one hop from the bridge seed" cases, `paths.rs:635`). Fall back to today's residual when the split rule does not fire.
- **Attacks.** D (7), the hop part of A after anchors are fixed (5 residual mh1 cases), F partly. Expected +3 to +5 on mh2 (D cases whose correct chain is already found at rank 2-12: 5 of 7) and +2 to +3 on mh1 (inference).
- **Cost.** One or two extra short sequences in the existing embed batch; UltraBERT time grows roughly with total tokens, so a ~10-token span adds a fraction of the query embed (inference: < 10 ms; measure with `bench`). Beam work unchanged.
- **Experiment.** Config `hop_split` off/on; report how often the split fires per set; mh1, mh2 (one blind run), world-300 two/three-hop, fixtures, holdout, p50. Hand the two new features to nc-hop as inputs rather than competing with it.
- **Kill.** mh2 three-hop not up by >= 2 queries, or any gate regression, or the split rule fires on < 50% of mh1+mh2 questions.

### B4. Prose-terminal promotion, rendered as a fact line, with a margin gate
Sources: LightRAG weighted-polling quota (`repos/lightrag.md` steal #1), HippoRAG 2 passage nodes (`papers/hipporag2.md`), neocortex prose hops (`docs/research/multihop.md` §7, including the rejected reserved memory slot: +0.01 mh2, holdout −1).
- **Hypothesis.** When the best chain is right but ends in a prose memory, the answer never reaches the context (ethics-chair and protein-lab cases: chain rank 1 and 2, fused rank 20 and 17). Rendering that memory in the *facts* block, not in a memory slot, delivers it without displacing a memory (the displacement is what cost holdout last time, inference).
- **Mechanism.** `paths::facts` (`paths.rs:708`) emits a `FactHit` for a prose step (source = bridge entity, predicate = "said", fact = memory content trimmed to 200 chars, id = `m<id>`) when the chain's score beats the best chain with a different terminal by a margin δ (sweep 0, 0.05, 0.1). Also fix `recall.rs:933`: take the best lexical *and* the best semantic chain instead of `path_hits[..2]` of the lexical-first concatenation.
- **Attacks.** B (2 mh2 primary) and the prose contributor (14/18). Expected +2 on mh2 directly, +1 to +3 more combined with B3 (chains that B3 lifts to rank 1 and that end in prose).
- **Cost.** No latency (data already read); +1 fact line (~40 tokens).
- **Experiment.** mh2, mh1, holdout, fixtures. Add two eval columns to `eval.rs` so this cannot game acc@10: context tokens used, and "wrong-candidate presence" (answer strings of the query's `expect_absent` memories found in context).
- **Kill.** < +2 on mh2, or holdout/fixtures regress, or context tokens grow > 10%.

### B5. Name-containment edges and connector-aware proper-noun spans
Sources: Graphify's prefix-extension rule (a *veto* there, `repos/graphify.md` steal #2), HippoRAG synonym edges (`papers/hipporag2.md`), neocortex `similar` soft edges (`remember.rs:1084`), open item EX-01 (`lanes/briefs/open_items.md`).
- **Hypothesis.** "Hackerspace Aveiro" contains "Aveiro", "Cascais marina" contains "Cascais"; a soft containment edge supplies the missing last hop (G, 2 mh2). Separately, the heuristic extractor splits "Rua do Instituto Bacteriológico" into "Rua" + "Instituto Bacteriológico" and "Universidade da Beira Interior" into "Universidade" + ..., creating generic hub entities that the beam walks through (seen in 6/18 mh2 dumps).
- **Mechanism.** (1) `extract.rs` proper-noun spans keep lower-case connectors between capitalised words (`do, da, dos, das, de, del, della, di, du, van, von, der, la, le`) and drop a single generic head noun when it is part of a longer span. (2) On entity insert, look up `entity_name_tokens` (B1) for entities whose token set strictly contains, or is contained in, the new name with at least one place/org type, and write a `contains(a, b, 0.6)` row into `similar` with a kind column (or a new `name_part` table). (3) `search_semantic` treats `name_part` as a hop scored like a prose hop; PPR uses it as a weak edge.
- **Attacks.** G (2), hub noise in D. Expected +1 to +2 mh2 (both chains end in prose, so B4 is needed to render them; inference). Entity F1 on Romance names should rise.
- **Cost.** Write: one indexed token lookup per new entity. Read: a few more edges per entity.
- **Experiment.** `examples/extract_eval.rs` on `eval/extract/holdout.json` (entity F1 must not drop), mh2, fixtures, holdout, world-300.
- **Kill.** Holdout extraction entity F1 drops, or false containment links (e.g. "Paris" in "Paris Hilton") appear in a 50-pair manual check above 5%.

### B6. Chain-level cross-encoder verification
Sources: nc-rerank (MiniLM-L6 cross-encoder), Beam Retrieval's chain classifier (`papers/beam-retrieval.md`), Hindsight's cross-encoder pre-cap of 300 (`repos/hindsight.md` §4.6).
- **Hypothesis.** A cross-encoder judging `(question, "f1. f2. f3")` can tell a coherent chain from a hub walk, because the chain text reads as an answer passage. Memory-level reranking (nc-rerank) cannot see chains.
- **Mechanism.** After `search_semantic`, serialise the top 12 chains (relation facts or prose text) and score them with the nc-rerank model; reorder `sem_hits` before `paths::evidence` (`recall.rs:697`) and before rendering (B4).
- **Attacks.** D, F, residual A. Expected +3 to +5 mh2 if the cross-encoder transfers to chains (inference; MS MARCO training is passage QA).
- **Cost.** 12 pairs of ~60 tokens on a 22.7M model; expect 30-60 ms on CPU (inference; nc-rerank will measure the per-pair cost).
- **Experiment.** Build only after nc-rerank lands; compare with B3 alone and B3+B6.
- **Kill.** p50 + > 40 ms, or < +2 queries over B3.

### B7. Predicate soft-equivalence: argument-pair statistics + a shipped paraphrase lexicon + inverse awareness
Sources: `literature-map.md` §4 idea 1 (argument-pair overlap), HippoRAG synonym edges, ReMem corroborated merge gate (`repos/remem.md` steal #1), the predicate-group machinery of f2 (`resolve.rs:979`).
- **Hypothesis.** Paraphrase gaps ("partner"/"married to", "guides her career"/"mentors", "trained"/"studied at", "make a home"/"is based in", "manages"/"reports to") cannot be learned from one personal graph alone (each pair occurs once), so combine three weak signals: argument-pair Jaccard where data exists, inverse-pair detection, and a small *static* paraphrase lexicon distilled offline (by any LLM, once, shipped as data; no LLM in the loop).
- **Mechanism.** Table `predicate_soft(p, q, score, inverse)`, filled by a consolidation job (C7): `score = max(lexicon(p,q), jaccard(args(p), args(q)), jaccard(args(p), swap(args(q))))` with type-signature agreement. Read side: expand lexical-beam terms through it (`paths.rs:145 query_terms`) and, in `search_semantic`, score a hop by `max(cos(r, f), cos(r, f_alt))` where `f_alt` is the embedded canonical paraphrase of its predicate (one vector per predicate, cached).
- **Attacks.** F (2) and the paraphrase residue in mh1 (5 of the 14). Expected +2 to +4 over both multi-hop sets (inference).
- **Cost.** A few KB lexicon; one predicate-vector read per hop (cached per query).
- **Experiment.** Precision first: the 66-pair predicate protocol used for `pred_alias` (`engine.rs` comment) extended with 30 inverse pairs; then mh1/mh2.
- **Kill.** Precision < 0.95 on the pair set, or a directional error (manage vs reports-to treated as same direction) in any test.

### B8. Context-inherited membership for prose with no named owner
- **Measured (nc-context, 2026-10-26; branch `lane-nc-context`, knob `context_inherit`, not merged): killed.** Inherited org/place mentions: precision 0.023 on world-300 (2/87 correct), 1.00 on holdout (n=6); aggregation recall gain 0.000; knob on breaks holdout (recall 0.914 -> 0.903, MRR 0.794 -> 0.791). The session/scope window is not a reliable membership signal.
Sources: TCM drift context (`papers/tcm-cmr-successor.md`), Hindsight temporal links (`repos/hindsight.md` §3 W7), RippleMem structural edges (`papers/ripplemem.md`), LightRAG keyword-chunk bipartite (`techniques/keyword-chunk-bipartite-index.md` in OKF). nc-e45 measured no gain for time+shared-entity links; this targets the opposite case, memories that share *no* entity.
- **Hypothesis.** "Zoe Lindqvist joined as a PhD student" is about the Solberg Lab only by context. A weak, inherited mention from the session/temporal neighbourhood makes aggregation ("who works in the Solberg Lab", 6 holdout misses) and implicit-context queries ("Byåsen housewarming") answerable.
- **Mechanism.** In `remember.rs::write_prepared`, keep a per-(scope, session) running context: the entity distribution of the last N=8 memories (and optionally the drift vector `c_t = rho c_{t-1} + (1-rho) e_t`). A memory that mentions no org/place entity inherits a mention of the dominant org/place when its share is > 0.5, stored with `mentions.weight = 0.3` and `origin='context'`. `aggregate` and PPR read weighted mentions; rendering marks them as inferred.
- **Attacks.** E (7), H (1). Expected: holdout aggregation recall 0.47 -> 0.6+ (inference; depends on ingestion order, which eval preserves in batches of 256).
- **Cost.** Write: one small window query. Read: none.
- **Experiment.** holdout aggregation per query; precision audit of inherited mentions on holdout and world-300 (world has explicit ground truth for memberships).
- **Kill.** Inherited-mention precision < 0.8 or aggregation recall gain < 0.1.

---

## C. New ideas (not built anywhere, or proposed and never developed)

### C1. The write path learns from recall misses (implicit and explicit feedback)
- **Hypothesis.** The calling agent reveals what recall missed: after a recall it often opens an entity, follows a relation, or writes the answer it eventually found. Those events are free supervision for anchors (A), paraphrases (F) and fusion weights (B), specific to this user's vocabulary.
- **Mechanism.** `recall` returns a short `rid`; `mcp.rs` logs `(rid, query, returned ids)` in `recall_log`. Implicit signals within the same session and 10 minutes: `entity`/`follow` on id X, or `remember` whose entities include X -> `(rid, X, opened)`. Explicit, cheap: optional `feedback: {rid, used: [m12], missing: "..."}` on any tool call. A consolidation job (C7): (a) query n-grams that precede an opened entity in >= 2 sessions and name nothing else become weak aliases (T2 in B1: "boss" -> Ingrid); (b) `(query, used memory)` pairs refit the RRF channel weights per user by logistic regression on channel ranks (`rank.rs:30`), with a prior pulling to defaults; (c) when a memory written right after a failed recall answers it, the query text is added to that memory's `keywords` (doc2query from real queries, no model).
- **Attacks.** A, B, F over time. On the static evals: zero by construction.
- **Cost.** One log row per recall; jobs offline.
- **Experiment.** New eval `eval/replay-*.json` (to build): pairs of queries where the second is a paraphrase asked after the agent "used" the answer of the first; measure second-pass acc with learning on/off. Simulate the agent with `examples/agent_sim.rs`.
- **Kill.** Learned-alias precision < 0.9 on replay, or second-pass gain < +0.05.

### C2. Self-supervised hop scorer trained on the user's own graph
- **Hypothesis.** nc-hop trains on `gen_world` chains whose wording is the generator's (`docs/research/multihop.md` §2.3 notes the synonym table was written for it). Chains sampled from the user's own store, with template questions built from the stored predicate phrases and entity descriptions, adapt the scorer to the user's vocabulary, nightly and LLM-free.
- **Mechanism.** Sample anchored 2/3-hop chains from `relations` (+ prose hops); question template `"{wh(r_n)} {np(desc(B), r_1, A)} {verb(r_n)}?"` using `predicates.display`/`inverse_of` and entity `description`; paraphrase the template through B7's lexicon; hard negatives = sibling chains from the same anchor (the hub walks seen in phase 1). Features per hop: `cos(q,f)`, `cos(r,f)`, B3's `cos(np,f_1)` and `cos(wh,f_n)`, `f_{i-1}.f_i`, hop index, prose flag, `ln degree`, SubgraphRAG's directional distance from anchors (`papers/subgraphrag.md`). Logistic regression, < 100 weights in a `model_weights` table, retrained in seconds.
- **Attacks.** D and residual A/F. Expected: SubgraphRAG's learned scorer beat cosine 0.883 vs 0.714 on shortest-path triple recall (`papers/subgraphrag.md`); a conservative transfer is +3 to +6 on mh2 (inference).
- **Cost.** Inference: a dot product per hop. Training: seconds per night.
- **Experiment.** Train on mh1's DB graph only (its questions never used), test on mh2 once; compare with nc-hop's gen_world-trained weights and with B3.
- **Kill.** No gain over B3 on mh2 and world-300, or the template-question accuracy on held-out self-chains < 0.8 (it is learning templates, not paths).

### C3. Anticipatory composed facts: answer two-hop questions with one-hop retrieval
- **Measured (nc-composed phase 1, 2026-10-26; probe `examples/composed_eval.rs` on branch `lane-nc-composed`, not merged): killed.** Composed chain sentences inserted at rank 3: mh1 0.825 -> 0.900 (two-hop +5/-1), world-300 unchanged, but blind mh2 0.830 -> 0.770 (three-hop -5, updated-chain -1). Gains on the tuning set did not transfer; f32 storage would add ~59% DB size at world-300.
- **Hypothesis.** Most multi-hop questions ask an attribute of an entity described by its relation to an anchor ("where was the CFO of X born"). Precomputing, at idle time, the composed sentence for each (anchor-side relation, attribute relation) pair turns the hard two-hop search into a dense KNN that already works (single-hop acc is 1.000 on mh1).
- **Mechanism.** Job over chains `A -r1-> B -r2-> C` where `r2` is an attribute predicate (exclusive/functional groups: born in, grew up in, lives in, studied at, located in, headquartered in) or a prose memory naming B and one other entity, and `deg(B) <= 50` (Graphify's hub rule, `repos/graphify.md` steal #3). Text: `"{A}'s {role(r1)}, {B}, {display(r2)} {C}."` e.g. "Mirante Bio's head of clinical development, Carla Azevedo, was born in Lamego." Store in `composed(id, r1, r2_or_memory, text)` + `composed_vec` (int8); recall adds one KNN channel `comp` whose evidence is both memories, and renders the composed sentence as a fact. Budget by activation: compose first for entities with the highest ACT-R base level (`papers/actr-base-level.md`), cap 10 per bridge entity, refresh on writes touching r1/r2 (validity = intersection of both intervals).
- **Attacks.** D, A (descriptive anchors match the composed text directly: "the CEO of Mirante Bio" ~ "Mirante Bio's founder Luis Amaral"), F (the composed text uses the agent's phrase for r1). Expected: large on two-hop (inference: 4 of the 5 mh2 two-hop misses are a role relation followed by an attribute; the fifth adds a prose sibling hop), smaller on three-hop (needs composed + 1 hop).
- **Cost.** Size: mh2 ~400 composed rows (1.2 MB f32); world-1000 ~10 per person -> ~10k rows; at 100k entities with the cap and int8, ~1M x 768 B ≈ 0.75 GB worst case, so the activation budget (top 20% entities) is mandatory (~150 MB, inference). Read: one KNN (~2-5 ms at 100k rows, inference from current vec0 costs). Idle: one embed per composed row.
- **Experiment.** mh1, mh2, world-300/1000 per category, DB size, p50; ablate prose-composed rows.
- **Kill.** < +3 on mh2 two-hop, or DB size > 1.5x at world-1000, or any single-hop/holdout regression.

### C4. Recall that tells the agent the next hop ("dangling hop" hints)
- **Hypothesis.** The agent loop already lifts mh2 from 0.82 to 0.92 with a crude heuristic (longest exact entity name); it fails on mh1 because the heuristic needs an exact name. The semantic beam *knows* the bridge (it is often a fuzzy seed or the end of the best partial chain) and the wh-span (B3) names the missing relation. Returning that as a hint makes one cheap `follow` call (3 ms) close the gap.
- **Mechanism.** `RecallOutput.next: [{entity: "e151", name, want: "born in", why: "chain ends at a person; question asks where born"}]` when (a) the best chain's last hop does not match the wh-span (`cos(wh, f_n)` < τ) or (b) no chain covers both ends. Rendered as one line in `render.rs`. `skills/neocortex/SKILL.md` tells the agent to call `follow` on it.
- **Attacks.** Everything reachable in two calls: A (mh1), D, B. Expected agent accuracy: mh1 0.838 -> >= 0.90, mh2 0.92 -> >= 0.95 (inference from agent_sim: follow rescued 5/18 mh2 misses even with the weak heuristic).
- **Cost.** Zero in the core; ~20 tokens.
- **Experiment.** `examples/agent_sim.rs` variant that follows the hint instead of the longest-name heuristic; report calls/query.
- **Kill.** agent_sim with hints <= agent_sim without, or calls/query > 1.5.

### C5. The open-question ledger (negative memory that closes itself)
- **Hypothesis.** An unanswered question is information: it says what the user cares about and how they phrase it. Storing it, and matching new memories against it, closes vocabulary gaps with the asker's own words and gives abstention a learned prior instead of a raw cosine (the 0.18 threshold has no margin, `docs/research/multihop.md` §2.6).
- **Mechanism.** When `recall` abstains (`recall.rs:805`) or the agent sends `feedback.missing`, insert `(text, query_vec, asked_at, count)` into `open_questions` (deduped by cosine >= 0.9). In `remember`, one KNN of the new memory's vector (document mode) against open questions; if cosine >= τ (calibrate), the result says `answers open question q17 (asked 3x since 2026-09-02)`, the question text is appended to the memory's FTS `keywords`, and the question is closed. Abstention: if a new query is near a *closed* question, never abstain.
- **Attacks.** B/F paraphrase over time; abstention calibration.
- **Cost.** One KNN per write (open questions are few).
- **Experiment.** New longitudinal eval (to build): ask the question, then ingest the answer, then ask a paraphrase; measure match precision and final acc.
- **Kill.** False matches > 10% on the calibration set.

### C6. Learned schema expectations: surprise-gated encoding and cheap correction
- **Hypothesis.** Hand-seeded exclusivity (16 predicates) misses most real constraints; per-(entity type, predicate group) cardinality statistics learned from the store predict when a write is surprising ("Rui now has two live managers"), which is exactly when a supersede, a typo (REL-19) or a retraction (TM-10) is likely. Prediction error, not lexical cues, gates the update (`papers/reconsolidation-prediction-error.md`, `papers/em-llm.md`).
- **Mechanism.** Table `schema_stats(type, group, n_subjects, mean_live_values, p_single)`, updated incrementally on relation insert/end. At write, `surprise = -ln P(live values after write | type, group)` plus embedding novelty `1 - max cos(e, recent)`. Above τ: importance bump, and the `remember` result carries `conflict_candidate: {ends: r25, confirm_with: invalidate r25}` so the agent can correct memory with one call. Never ends a fact automatically unless the group is exclusive.
- **Attacks.** C (stale facts such as `Pedro Vieira -manage-> Rui` still live), F side effects. Expected small on current evals (+0 to +1), larger on month-scale drift (inference).
- **Cost.** One stats row update per relation write.
- **Experiment.** New drift world: `gen_world` with unmarked replacements (to build); precision/recall of suggested ends.
- **Kill.** Suggested-end precision < 0.9.

### C7. Sleep-time consolidation with a regression guard
- **Hypothesis.** Several of the ideas above are idle-time jobs (B2, B5, B7, C1, C2, C3, C6). Running them as one scheduler, with each job's effect checked against replayed real queries, gives CLS-style consolidation (`papers/cls-consolidation.md`) without risk of silent drift.
- **Mechanism.** `neocortex consolidate [--jobs ...]` and an MCP idle trigger (after N writes and T seconds idle). Jobs: alias demotion (B2), containment edges (B5), predicate soft-equivalence (B7), feedback learning (C1), scorer retraining (C2), composed facts (C3), schema stats (C6), and "prose lifting": run `extract_model` (rules tier) over prose memories and add a relation only when both endpoints resolve to existing typed entities and the predicate maps to an existing group (origin `lifted`, confidence 0.5). Guard: replay the last 200 logged recalls before and after; a job whose changes alter answers that had positive feedback is rolled back (one SAVEPOINT per job).
- **Attacks.** G, E and everything the jobs feed.
- **Cost.** Background CPU; one transaction per job.
- **Experiment.** Per-job ablation on all eval sets; replay stability rate.
- **Kill.** Replay instability > 5% of answered queries for any job.

### C8. Relational description aliases (entities indexed by how people describe them)
- **Hypothesis.** People refer to entities by their relations ("the allergist who treats Carlos", "the studio that designed the footbridge", "Sofia's oldest friend"). Indexing each entity under short descriptions generated from its own relations makes descriptive anchors a KNN lookup (A, T3 in B1) instead of a beam search.
- **Mechanism.** For each relation, write two short descriptions, one per side, from `display`/`inverse_of`: subject side "the {display-as-noun} {target}" ("the CFO of Mirante Bio"), object side "{source}'s {role}" or "the {type} that {display} {source}". Store in `entity_descriptions(entity, relation_id, text)` + vec (int8). Recall: KNN of B3's NP-span against it -> T3 anchors with weight = cosine margin. Ends with the relation's validity.
- **Attacks.** A (role/description: 6 holdout, several mh1/mh2). Expected: +0.01 to +0.03 holdout MRR, +2 to +4 multi-hop (inference).
- **Cost.** Two short embeds per relation at write (current write embeds content + cards + facts + names + predicates; this adds ~2 texts per relation, roughly +30% write embed time, inference). One KNN at read.
- **Experiment.** Anchor accuracy on all role/description questions (label the gold anchor), then end-to-end sets.
- **Kill.** Anchor accuracy < 0.8 on labelled descriptive questions, or write p50 > 200 ms.

Rejected after thought (not ranked): KGE training (RotatE/ComplEx) on sparse personal graphs (`literature-map.md` §6 agrees); LLM edge verification (RippleMem shows it lowers overall accuracy, `papers/ripplemem.md`); a GLiNER port for relations (independent relation F1 0.200, `docs/research/extraction.md`).

---

## D. Ranking: expected accuracy gain per unit of build effort

Gain is in answer-bearing queries on mh2 (100) and mh1 (80), plus holdout effect; numbers without a measurement are inference from the failure counts.

| rank | idea | effort | expected gain | evidence |
|---|---|---|---|---|
| **Quick wins (< 1 day)** | | | | |
| Q1 | E2: semantic beam on history-intent questions (`recall.rs:675`) | hours | mh2 +2, holdout MRR −0.006 (in gate) | measured |
| Q2 | B2 generic-alias demotion | < 1 day | mh2 +2 to +3 | chains exist, blocked only by the anchor |
| Q3 | B4 prose-terminal fact line + lexical/semantic interleave (`recall.rs:933`) | < 1 day | mh2 +2 (+1 to +3 with B3) | chain rank 1-2 measured |
| Q4 | B1 tier T1 (name-token anchors without the pass-through/prose side effects) | 1 day | mh1 +6 to +8 | E1 measured +8/−2 |
| Q5 | C4 dangling-hop hints + agent_sim variant | 1 day | agent mh1 0.84 -> 0.90+ | agent_sim measured the loop's headroom |
| **Medium bets (2-5 days)** | | | | |
| M1 | B3 two-ended split scoring (+ 1-hop from bridge seeds) | 3-4 days | mh2 +3 to +5, mh1 +2 to +3 | 2-4x margin on 3 chains |
| M2 | B1 tiers T2/T3 + C8 description aliases | 4-5 days | holdout MRR +0.01 to +0.03, multi-hop +2 to +4 | 6 holdout role cases |
| M3 | B5 containment edges + connector-aware spans | 2-3 days | mh2 +1 to +2, extraction F1 | 2 G cases, hub noise |
| M4 | B7 predicate soft-equivalence | 3-4 days | +2 to +4 across mh1/mh2 | F + paraphrase residue |
| M5 | B6 chain-level cross-encoder | 2 days after nc-rerank | +2 over B3 | inference |
| M6 | B8 context-inherited membership | 3 days | holdout aggregation +0.1 recall | 7 E cases |
| **Moonshots (1-3 weeks)** | | | | |
| X1 | C3 anticipatory composed facts | 1-2 weeks | two-hop near single-hop accuracy | 4 of the 5 mh2 two-hop misses are role -> attribute |
| X2 | C2 self-supervised hop scorer | 1-2 weeks (shares nc-hop code) | mh2 +3 to +6 | SubgraphRAG 0.883 vs 0.714 |
| X3 | C1 miss-driven learning + C5 open-question ledger | 2-3 weeks incl. new evals | long-run A/B/F; abstention | needs replay eval |
| X4 | C6 schema-surprise + C7 consolidation scheduler | 2-3 weeks | correctness over months | needs drift eval |

Rough ceiling if the quick wins and M1 stack without overlap: mh2 82 + 2 (Q1) + 2 (Q2) + 2 (Q3) + 4 (M1) = 92; mh1 66 + 7 (Q4) + 2 (M1) = 75/80 = 0.94 (inference: overlaps between classes will eat some of this; single-shot > 0.90 on both multi-hop sets looks reachable without a trained model).

### Top 5 to build next (lane-ready briefs)

**1. Lane `nc-anchor`: tiered query anchors + generic-alias demotion (B1 T0-T2, B2).**
- Goal: resolve first names, role nouns and generic aliases correctly at query time; attacks class A (22/64).
- Files: `src/recall.rs` (anchor tiers after `alias_entities`, path/sem starts, pass-through set), `src/paths.rs` (accept tiered anchors: `search`, `search_semantic` anchor arguments), `src/resolve.rs` (`add_aliases` weak origin, `insert_entity` writes `entity_name_tokens`), `src/db.rs`/`src/schema.sql` (new table, migration step marking existing weak aliases), `src/engine.rs` + `src/eval.rs` (`anchor_tiers` Config key in `set_config`/`CONFIG_KEYS`).
- Tests: `tests/recall.rs` "Noor's" resolves to Noor Haddad only when unique; two "Teresa"s -> no T1 anchor; "the company" alias stored weak and never blocks a chain; role noun "the CFO of Mirante Bio" -> Ricardo; existing prose-hop test unchanged.
- Gates: fmt, clippy, test; fixtures >= 0.95/0.89, holdout >= 0.91/0.78, mh2 >= 0.82; world-300 unchanged per category; bench p50 <= 150 ms.
- Measurement: sweep `anchor_tiers` {0, 01, 012} with `eval --sweep`; report mh1/mh2/holdout/fixtures per category, anchors per tier, and agent_sim on mh1.

**2. Lane `nc-histprose`: history-intent multi-hop + prose-terminal rendering (Q1 + Q3).**
- Goal: stop switching multi-hop off on "used to/before" questions; get the evidence of a correct prose-ending chain into the context.
- Files: `src/recall.rs` (the `negated || history_query` branch at `:675`; fact merge at `:933`), `src/paths.rs` (`facts` emits prose steps as `FactHit` with `m<id>`; margin δ), `src/render.rs` (fact line form for a prose step), `src/eval.rs` (two new report columns: context tokens, `expect_absent` answer leakage).
- Tests: history question "allergy of whoever used to lead X's team" returns the second hop; prose-terminal chain renders its memory as a fact; lexical-only hits do not crowd out the best semantic chain.
- Gates: as lane 1; additionally context tokens on fixtures/holdout +<= 10%.
- Measurement: mh2 (expect +3 to +4 combined), mh1, holdout, fixtures; sweep δ in {0, 0.05, 0.1}.

**3. Lane `nc-split`: two-ended chain scoring (B3), coordinated with nc-hop.**
- Goal: replace sum-of-residual-cosines ranking with NP-span/wh-span scoring, a length prior and 1-hop chains from bridge seeds.
- Files: `src/paths.rs` (`split_question`, chain score in `search_semantic`, 1-hop emission rule at `:635`), `src/recall.rs` (embed spans in the query batch at `:362`), `src/engine.rs`/`src/eval.rs` (`hop_split`, `chain_len_prior` keys).
- Tests: unit tests for `split_question` on 20 templated and 10 free-form questions (wh-span and NP-span strings); the Lamego chain outranks the founder/board hub walk on a small fixture; fallback when the split does not fire equals today's output.
- Gates: as lane 1; p50 increase <= 10 ms on `bench --n 10000`.
- Measurement: split fire rate per set; mh1, mh2 (one blind run per config, <= 4 configs), world-300 two/three-hop; export the two span cosines as features for nc-hop.

**4. Lane `nc-contain`: connector-aware spans and name-containment edges (B5).**
- Goal: stop generic fragment entities ("Rua", "Universidade") and link "Hackerspace Aveiro" to "Aveiro".
- Files: `src/extract.rs` (span rules), `src/resolve.rs` (containment lookup on insert via `entity_name_tokens` from lane 1, or its own token table if lane 1 is not merged), `src/remember.rs`/`src/schema.sql` (edge storage), `src/paths.rs` + `src/kg.rs` (walk the edge as a weak hop / PPR edge).
- Tests: "Rua do Instituto Bacteriológico" is one entity; "Universidade da Beira Interior" never yields "Universidade"; "Cascais marina" -> contains -> "Cascais"; "Paris Hilton" (person) does not link to "Paris" (type guard).
- Gates: as lane 1, plus `extract_eval` on `eval/extract/holdout.json` entity F1 >= baseline 0.698.
- Measurement: mh2 (the two G cases need lane 2's rendering), extraction F1, number of containment edges on world-300.

**5. Lane `nc-hint`: dangling-hop hints for the calling agent (C4).**
- Goal: when recall cannot finish a chain, say which entity and relation to `follow`.
- Files: `src/engine.rs` (`RecallOutput.next`), `src/recall.rs` (hint from the best partial chain / fuzzy bridge seed and the wh-span or the question terms), `src/render.rs`, `src/mcp.rs` (schema), `skills/neocortex/SKILL.md` (protocol), `examples/agent_sim.rs` (a `--hints` mode).
- Tests: a two-hop fixture where recall misses the answer returns a hint naming the bridge; JSON schema test; no hint when the answer is already in context (no wasted agent calls).
- Gates: as lane 1; output tokens +<= 30 per recall.
- Measurement: agent_sim mh1/mh2/world-300 with and without `--hints`: answered rate, calls per query.

Not in the top 5 but next in line: C3 composed facts (biggest structural bet; prototype as an offline job on the mh1/mh2 DBs to measure the two-hop lift before any schema work) and B7 predicate soft-equivalence (needs the precision protocol first).
