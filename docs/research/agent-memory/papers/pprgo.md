# PPRGo: scaling GNNs with approximate PageRank (Bojchevski et al., KDD 2020)
Source: https://arxiv.org/abs/2007.01570 (HTML read; Sec 2-4, Fig 2-3).

**Problem.** Message-passing GNNs are I/O-bound on large graphs; precompute personalised PageRank neighbourhoods instead.

**Mechanism.** Propagation matrix `Pi_ppr = alpha (I - (1-alpha) D^-1 A)^-1`; approximate each row `pi(i)` with the **push-flow (forward push) algorithm** with residual threshold epsilon (cost O(1/(eps*alpha)) per node, independent of graph size), then **keep only the top-k entries** per node, giving a sparse matrix; predictions = sum over the top-k neighbours of pi(i,j) * MLP(x_j) (Sec 3). Sparse inference gives a further ~2x.

**Reported results.** A 12.4M-node, 173M-edge graph (MAG-Scholar) end-to-end (preprocess + train + infer) **in under 2 minutes on one machine**; accuracy plateaus around top-k = 32 and is monotone in smaller epsilon (Fig 2-3).

**What neocortex could steal.** neocortex's PPR (`recall.rs::overview_ppr`, graph channel) is a per-query power iteration or equivalent on the snapshot. Two upgrades from this paper, both LLM-free and CPU: (1) **forward push from the seed set** touches only nodes with residual > eps, so latency depends on the query's neighbourhood and not on entity count: this is what keeps p50 flat at 100k-1M entities; (2) **cache the top-k (k~32) PPR neighbours per entity** in a table updated incrementally (or lazily with a generation counter, like `generation()` in `recall.rs:29`), then multi-seed PPR = a weighted merge of cached rows by linearity (`pi(sum_i a_i e_i) = sum_i a_i pi(i)`), O(seeds * k). A cached `entity_ppr_top32` table makes the graph channel a SQL join. Verify first how the current implementation scales on world-1000 (p50 146 ms in `multihop.md` 3.1b).

**Weak / wrong.** Node classification, static graphs; top-k truncation drops long-range tail (fine for retrieval); incremental maintenance under edge deletion/invalidations needs care (inference).
