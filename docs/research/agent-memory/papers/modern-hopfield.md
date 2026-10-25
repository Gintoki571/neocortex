# Modern Hopfield networks and dense associative memory (Krotov & Hopfield 2016; Ramsauer et al. 2020 "Hopfield Networks is All You Need")
Sources read: https://arxiv.org/abs/1606.01164 (DAM, Sec 2) and https://arxiv.org/abs/2008.02217 (HTML, appendix: energy, update rule, fixed-point and metastable-state analysis).

**Problem.** Classical Hopfield capacity is ~0.14N patterns; dense / continuous variants store exponentially many and retrieve in one step.

**Equations.** DAM energy `E = - sum_mu F(xi^mu . sigma)` with `F(x) = x^n`; n = 2 recovers classical Hopfield; capacity grows as ~N^(n-1) (Krotov & Hopfield Sec 2). Continuous modern Hopfield: energy with log-sum-exp; **update rule `xi_new = X softmax(beta X^T xi)`** (patterns X in columns), which is transformer attention with queries xi, keys/values X; converges in one step for well-separated patterns; storage capacity exponential in dimension for random patterns; three kinds of fixed points: (a) a single stored pattern (well separated), (b) a **metastable state = average of similar patterns**, (c) global average when beta is small (appendix Fig A.1). `beta` controls separation vs averaging.

**What it means for a memory index.** Retrieval by dense KNN + softmax-weighted averaging IS one Hopfield step over the stored memory matrix. The embedder's `X` is the pattern matrix, the query the partial cue. So: (1) the ranker `softmax(beta * cos)` with a tuned beta (inverse temperature) gives calibrated weights across candidates (use for the margin/abstention discussed in `pattern-separation-completion.md`); (2) *iterated* retrieval (re-query with `xi_new`) is the pattern-completion loop and is equivalent to a soft multi-hop in embedding space (cf. multi-hop dense retrieval in the catalogue); (3) capacity is not the limit at 100k memories: the quality limit is the embedder's separation of near-duplicates (metastable states = distinct memories that retrieve as their average).

**Implementation (LLM-free).** One extra matmul: `w = softmax(beta * X_topK @ xi)`, `xi' = X_topK^T w`, then KNN again with `xi'` (second hop) fused by RRF. Cost: K=100 candidates x 768 floats = microseconds. Test on the multi-hop sets as an iterative query refinement (the vectors are already fetched lazily in `paths.rs`).

**Weak / wrong.** Theory is for random or well-separated patterns; real embeddings are anisotropic and clustered (metastable averages dominate); no retrieval benchmark in these papers. The AI relevance is conceptual.

**What neocortex could steal, concretely.** Softmax-weighted second-hop re-query in embedding space (`recall.rs` vector channel, `paths.rs`) and top1-top2 margin as ambiguity/abstention signal. No LLM.
