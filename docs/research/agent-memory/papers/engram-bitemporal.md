# Engram: bi-temporal dual-process memory engine (Wang, 2026)
Source: https://arxiv.org/abs/2606.09900 (HTML read in full; Sec 3, Table 1, Table 2, Fig 4). Independent preprint, single author, AGPL code.

**Problem.** Memory systems are cheaper than full context but not more accurate; build a lean-context engine that beats full-context on LongMemEval-S.

**Data model.** Episode (raw lossless turn, event time + ingested-at), Fact (atomic S-P-O with embedding, salience, confidence, provenance ids, `valid_at/invalid_at`, `created_at/expired_at`, `supersedes` pointer), Entity/Relation (Sec 3.1). Invariant: never delete a contradicted fact; set `invalid_at`.

**Write path.** System-1 (hot, <50 ms budget, **no LLM**): append episode, resolve identity, light embedding, enqueue. System-2 (async): rule-based fact extraction by default (LLM optional), build bi-temporal graph, detect conflicts, score salience, decay/reinforce, session summaries (Sec 3.2-3.3).

**Conflict resolution (cheap-then-escalate, Sec 3.4).** Same (subject, predicate) slot with a different object, or embedding similarity of the same attribute under a different free-form predicate, or content subsumption (elaboration vs contradiction); if clearly contradictory and temporally ordered: `old.invalid_at <- new.valid_at`, `new.supersedes <- old.id`, no LLM; only ambiguous cases escalate.

**Read path (exact, Eq 1, Table 1).** `score = w_sem cos + w_lex bm25 + w_graph prox + w_rec e^(-dt/tau) + w_sal sal` with weights 1.0 / 0.6 / 0.8 / 0.3 / 0.25 (fused by weighted RRF, k_RRF = 60); optional cross-encoder rerank off by default; **as-of filter**; abstention gate; context = up to 8 resolved facts + top-15 fused items + 2 raw session chunks + 28 session summaries, ~9.6k tokens.

**Time / dedup / forgetting.** Bi-temporal; salience decay/reinforcement is the forgetting mechanism (described, not ablated).

**Cost.** Retrieval sub-second; end-to-end ~60 s p50 dominated by the answering LLM; fact extractor an LLM (doubao-seed-1.6-flash) in the headline run.

**Reported results.** LongMemEval-S, official judge, same answerer+judge: lean 83.6% [80.1, 86.6] vs full-context 73.2% [69.2, 76.9] at 9.6k vs 79k tokens, McNemar p < 1e-6 (81 questions gained, 29 lost). Per category: knowledge-update 87.5, temporal 81.1, abstention 86.7, multi-session 79.3, preference 73.3. **Facts-only read path loses recall; hybrid facts + raw chunks is "load-bearing"** (stated as a development observation, not an ablation). Documents measurement pitfalls (truncated baselines, home-grown judges, small samples: an 18-item slice read 83% vs 58% on the full set).

**What neocortex could steal.** This is the closest published design to neocortex (single store, no LLM on write, bi-temporal, RRF hybrid, abstention). Checklist to diff against neocortex: (1) *embedding-similarity slot match for different free-form predicate under same subject* as a conflict trigger: neocortex's exclusive-predicate logic uses canonical predicate groups; add "same subject + near predicate (cos >= 0.85) + different object => candidate supersession, report as conflict candidate" for non-exclusive cases. Touches `resolve.rs::resolve_predicate`, `remember.rs::write_prepared`. (2) *Content subsumption* test (new text contains the old claim) to separate elaboration from contradiction. (3) Return raw text beside facts, always. (4) Adopt the paper's discipline: only full-set numbers, paired McNemar for any claimed gain on the eval sets (neocortex evals are 80-100 queries: report CIs).

**Weak / wrong.** One benchmark, one answerer; no LoCoMo; "salience decay" and consolidation untested; the 60 s latency is dominated by the reader so says nothing about memory latency; self-graded by an LLM judge.
