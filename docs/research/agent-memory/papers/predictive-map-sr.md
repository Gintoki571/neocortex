# The hippocampus as a predictive map: successor representation (Stachenfeld, Botvinick, Gershman, Nat. Neurosci. 2017)
Source: https://www.biorxiv.org/content/10.1101/097170v4.full.pdf (PDF text read; Eq 1-3, 5-6, spectral regularisation in Results/Methods).

**Problem.** What does the hippocampus represent? Answer: the expected discounted future occupancy of states from the current state, not the current location.

**Equations (exact).** Value `V(s) = sum_s' M(s,s') R(s')`; `M(s,s') = E[ sum_{t>=0} gamma^t 1(s_t = s') | s_0 = s ]`; closed form `M = sum_t gamma^t T^t = (I - gamma T)^-1`; online TD update after a transition `s_t -> s_{t+1}`: `M(s_t,s') <- M(s_t,s') + eta [ 1(s_t=s') + gamma M(s_{t+1},s') - M(s_t,s') ]`. Entorhinal grid cells are proposed to be low-frequency eigenvectors of M; **spectral regularisation** (reconstruct M from a truncated eigendecomposition) smooths noise and stabilises the map when the transition statistics are estimated from few samples.

**What an SQLite + embedding system implements.** Treat entities (or memories) as states and observed co-mentions/relations as transitions. Because `PPR(alpha) = alpha * SR(gamma = 1-alpha)` (`ppr-successor-equivalence.md`), neocortex's graph channel already computes one SR row on demand. New options: (1) online TD update of a sparse top-k SR row per entity as memories arrive, avoiding re-running PPR (see PPRGo cache, `pprgo.md`); (2) predictive generalisation: `M(s,:) * R` where R is the query-relevance vector spreads credit over entities that *lead to* relevant ones (the "reward" is the question's seed set); (3) low-rank smoothing: compute the top-r eigenvectors of the normalised entity adjacency (r ~ 64, Lanczos, offline, same job as Leiden communities) and add the cosine in that spectral embedding as an additional edge weight, to bridge entities not directly linked (denoise extraction errors). All CPU; entity-count-scalable with sparse eigensolvers.

**Evidence.** Neural: place-field skewing along frequently travelled routes, grid-cell structure (Stachenfeld Fig 3-6). For LLM memory: HippoRAG's PPR gain over no-propagation +28 R@5 on 2Wiki (`hipporag.md`). No paper here tests TD-learned or spectral SR in an agent memory (open idea).

**Weak / wrong.** Spatial navigation domain; states are Markov in the models while memory streams are not.

**What neocortex could steal, concretely.** Spectral (truncated-eigendecomposition) edge smoothing and TD-updated SR rows for the entity graph, computed in the same offline job as communities (`recall.rs::cached_communities`). No LLM.
