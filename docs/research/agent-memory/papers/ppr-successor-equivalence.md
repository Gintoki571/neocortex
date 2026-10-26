# Equivalence of Personalized PageRank and Successor Representations (Millidge, 2025)
Source: https://arxiv.org/abs/2512.24722 (5-page note, PDF text read in full). Theory note, no experiments.

**Claim.** The hippocampal memory-retrieval account (HippoRAG-style PPR) and the hippocampal planning account (successor representation, SR) compute the same object: the stationary / discounted-occupancy distribution of a random walk on a graph. The paper proposes that the hippocampus computes this representation on arbitrary input graphs.

**Equations (cross-checked against Stachenfeld et al. 2017, Eq 3, 6).** SR: `M = sum_t gamma^t T^t = (I - gamma T)^-1`; PPR: `Pi = alpha (I - (1-alpha) P)^-1` with P the random-walk transition matrix. Identify `gamma = 1 - alpha`; then `Pi = alpha M`. So a PPR row from seed s is the SR row M(s, :) up to scale, and the damping factor in HippoRAG (0.5) corresponds to gamma = 0.5.

**Why it matters for neocortex.**
- It unifies four things neocortex treats separately: graph-channel PPR (HippoRAG), temporal-context drift (TCM = TD-learned SR; `tcm-cmr-successor.md`), the "predictive map" view of hippocampus (`predictive-map-sr.md`), and learned path semirings (`nbfnet.md`). An SR can be *learned incrementally by TD(lambda)* from the stream of observed transitions rather than recomputed by power iteration: `M(s,:) <- M(s,:) + eta (1_s + gamma M(s',:) - M(s,:))` per observed edge/transition. In a memory this gives an online, order-aware (not just structural) association strength: entities that tend to be mentioned *after* one another (temporal successors in the stream), not only linked by facts.
- SR smoothing by truncated eigendecomposition suppresses noisy edges (Stachenfeld Methods, spectral regularisation): relevant to noisy LLM-free extraction.

**Weak / wrong.** Short theoretical note; no empirical test; "the core function of the hippocampus" is hypothesis.

**What neocortex could steal, concretely.** Treat the graph channel as an SR row: online TD-SR update or cached top-k rows (`recall.rs::overview_ppr`, see `pprgo.md`), and spectral smoothing in the consolidation pass. No LLM.
