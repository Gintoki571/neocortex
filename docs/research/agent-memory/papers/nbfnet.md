# NBFNet: Neural Bellman-Ford Networks for link prediction (Zhu et al., NeurIPS 2021)
Source: https://arxiv.org/abs/2106.06935 (HTML read; Sec 3, Table 1, Table 3).

**Problem.** Path-based link prediction that is inductive, interpretable and scalable, unifying PPR / Katz / Bellman-Ford.

**Mechanism.** Represent the pair (u,v) under query relation q as a generalised sum over paths of a product of edge representations: `h_q(u,v) = ⊕_{P in P_uv} ⊗_{e in P} w_q(e)` (Eq 1-2). Katz = (+,x, w=beta*w_e); **personalized PageRank = (+, x, w = alpha*w_uv / sum_v' w_uv')**; widest path / shortest path / distance are other semirings (Sec 3.1). Solve with the generalised Bellman-Ford recursion `h^(t)(u,v) = (⊕_{(x,r,v)} h^(t-1)(u,x) ⊗ w_q(x,r,v)) ⊕ h^(0)(u,v)`, boundary `h^(0)(u,v) = INDICATOR(u=v)`. NBFNet learns MESSAGE (TransE/DistMult/RotatE-style relation operators), AGGREGATE (sum/mean/max/PNA) and the final MLP; one forward from source u yields scores for **all** v at once (amortised cost O(|E|d/|V| + d^2) per triple) (Table 1).

**Reported results.** Knowledge-graph completion (Table 3): FB15k-237 MRR **0.415** (H@1 0.321, H@10 0.599), WN18RR MRR **0.551** (H@1 0.497), vs RotatE 0.338 / 0.476, Path Ranking 0.174 / 0.324, DRUM ~0.343 on FB15k-237. Fully inductive: generalises to unseen graphs; no node features. Depth is not a problem (no over-smoothing up to the tested layers).

**What neocortex could steal.** (1) The cleanest statement of what neocortex's PPR *is*: a path semiring with fixed weights, so "learn the edge weights" = NBFNet. A **per-relation learned scalar weight in PPR** (a diagonal `w_q(e)` conditioned on the query's predicate group) is a 1-parameter-per-predicate special case that can be fit with plain logistic regression on shortest-path supervision (same labels as SubgraphRAG) and costs one multiply per edge at recall. (2) Single-source-to-all-targets: given the question's anchor, scores for every candidate answer entity come out of one propagation, matching `follow`/`aggregate`. (3) Inductive: nothing to retrain when new entities arrive (unlike TransE-style KGE), which matters for a growing memory.

**Weak / wrong.** Trained on GPU (hardware, layer count and width not extracted; layer-count ablation is Table 6b); evaluated on 15k-entity graphs; link prediction on Freebase/WordNet, not sparse personal graphs; no time dimension.
