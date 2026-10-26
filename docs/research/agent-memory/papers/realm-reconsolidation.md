# REALM: retrieval-driven memory reconsolidation (Song et al., 2026)
Source: https://arxiv.org/abs/2609.16053 (HTML read; Sec 3.3-3.4 Eq 6-12, Tables 1-4, Fig 3, 6). Preprint.

**Problem.** Memory systems update on new input only; retrieval is a read-only endpoint. Neuroscience says recalled memories become labile and are re-stored (reconsolidation), so use retrieval feedback to reshape the graph.

**Data model.** Heterogeneous "cognitive graph": atomic memory nodes + typed weighted relations with descriptions (Sec 3.2). Edge weight `w_ij in [w_min, 1]`.

**Read path (exact).** Seed localisation then adaptive expansion with LLM-chosen "strategy atoms" `a_v = (mode, predicate, decay, inhibit)`; each reached node scores `s_aces(v) = beta*s_sim(q, desc_ij) + (1-beta)*w_ij - Delta_decay(t_v)` (Eq 8); final `Score(v) = alpha*s_aces + (1-alpha)*s_sim(v,q)` (Eq 9); top-K=10, seed plans 1 (LoCoMo) or 3 (LongMemEval), depth 3 or 5 (Appendix A.3).

**Reconsolidation write (exact, Sec 3.4).** After answering, the activated subgraph G_q is given to an LLM (with an inferred topic structure) which emits decisions `d = (v_i, v_j, a, r, c)` with `a in {add, strengthen, weaken}`, relation type r, confidence c in [0,1]. Edge update `w <- clip(w + eta*c, w_min, 1)` with `eta = 0.8` for add, `1 - w_b` for strengthen, `-(w_b - w_min)` for weaken, `w_b = max(w_min, min(1, w))` (Eq 11-12). Co-activation repeatedly under one reasoning context creates edges; edges that repeatedly surface irrelevant evidence are suppressed.

**Cost.** LLM for organisation, strategy selection and each reconsolidation step.

**Reported results (GPT-4o-mini).** Overall 75.97% LoCoMo and 65.11% LongMemEval, +7.17 / +1.31 over the strongest baseline. **Reconsolidation ablation (Table 3):** +2.01 overall on LoCoMo (multi-hop +4.97, open-domain +5.21) and +2.13 on LongMemEval (preference +6.66, knowledge-update +4.17); stable under shuffled question order (Fig 3). Adaptive strategy selection only marginally beats fixed path/subgraph-beam expansion (Table 4).

**What neocortex could steal.** The *weight-update rule* is LLM-free and bounded: `w <- clip(w + eta*c)`. Feedback signal without an LLM: the agent's own tool behaviour. When `recall` returns memories A and B together and the agent then cites/uses both (or calls `follow`/`remember` referencing both), strengthen the A-B co-retrieval edge; when a memory is returned repeatedly and never used, weaken it. Prefer this to deleting. Needs a small `used`/feedback hook in the tool schema (touches `mcp.rs`, `engine.rs`, `kg.rs` edge table); optional, off by default. The gain (+2 points) is small, so it is not a priority; it becomes interesting only with long usage history, which no static benchmark has. Note this overlaps nc-e45 (access reinforcement): reinforce *edges*, not just memories.

**Weak / wrong.** Gains are 2 points on LLM-judged benchmarks with small per-category counts and one backbone; no analysis of drift or error accumulation from self-modification; the benchmark question order is fixed, so "feedback" is partly leaked from the evaluation protocol.
