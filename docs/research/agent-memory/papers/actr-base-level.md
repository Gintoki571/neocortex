# ACT-R declarative memory: base-level activation, spreading activation, fan effect (Anderson & Schooler 1991; Anderson et al. 2004; Anderson & Lebiere 1998)
Sources read: Brasoveanu, "Intro to the ACT-R subsymbolic level" (https://abrsvn.github.io/files/ACT-R_subsymbolic_3.pdf, eq 2, 12-15); Taatgen, Lebiere, Anderson, "Modeling paradigms in ACT-R" (https://www.ai.rug.nl/~niels/publications/taatgenLebiereAnderson.pdf, noise s ~ 0.25); Anderson & Bothell 2004 "An integrated theory of the mind" (https://zoo.cs.yale.edu/classes/cs671/12f/12f-papers/anderson-act-r.pdf). Anderson & Schooler 1991 PDF could not be fetched; its claim is quoted via Brasoveanu Sec 1.2 (p. 407 of the original).

**Problem it solves.** Predict which memory a person can retrieve and how fast, from the history of use: rational analysis says memory is tuned to the probability that an item will be *needed*, and the need probability in real environments (NYT headlines, child-directed speech, email) is a power function of frequency and of recency (Anderson & Schooler 1991).

**Equations (exact).**
- Base-level activation: `B_i = ln( sum_{k=1..n} t_k^(-d) )`, t_k = time since the k-th access (practice), **d = 0.5** in nearly all models. Base-level tracks the log odds that the item is needed.
- Total activation: `A_i = B_i + sum_{j in context} W_j * S_ji + epsilon`, `W_j = W/n_context`, noise epsilon ~ logistic with scale s (0.25-0.4).
- Associative strength: `S_ji ~ ln( P(i|j)/P(i) )`, in practice `S_ji = S - ln(fan_j)` with **S ~ 2** and fan_j = number of facts attached to cue j (the **fan effect**: the more things a cue is associated with, the less it activates each).
- Retrieval probability: `P_i = 1 / (1 + exp(-(A_i - tau)/s))` (tau = retrieval threshold, e.g. -2; s = 0.4).
- Latency: `T_i = F * exp(-A_i)`, with `F ~ 0.35 * exp(tau)`.
- Cheap approximation used when only a count n and an age L are stored (optimised learning; recalled from Anderson & Lebiere/Petrov, quoted by a search snippet of Gonzalez & Lebiere, not read): `B_i ~ ln( n / (1 - d) ) - d * ln(L)`.

**Time handling.** Every access timestamp matters; recent and many accesses dominate; a single old access decays as a power law (slower than exponential).

**Dedup / forgetting.** No deletion: activation below tau means not retrievable.

**What an SQLite + embedding system implements (LLM-free).**
1. Storage: either an `accesses(memory_id, t)` log (exact `ln(sum t_k^-0.5)` with a window function; cost O(accesses)) or the two-number approximation: `n` (the existing `access_count`) and `created_at` (age L) for `B ~ ln(n/(1-d)) - d ln(L)`, one multiply and one log per candidate. neocortex already keeps `access_count,last_access` (`recall.rs:1021`); add the approximation as a **rank term** in RRF fusion, not a replacement of the existing `0.9 + 0.1*2^(-age/half_life)` (`recall.rs:907`) until A/B-tested on the age-stratified eval (`memtrace.md`). Use a floor so unused memories are still recallable by relevance (activation only re-orders).
2. **Fan effect = IDF for graph edges**: spreading from an entity j to its memories scales by `S - ln(fan_j)`; in PPR seeds this is HippoRAG's node specificity `1/|P_i|` (`hipporag.md`). Implement as seed weight `max(0, S - ln(degree_j))`. This is exactly the hub fix needed in `multihop.md` 3.3.
3. Context term `sum_j W_j S_ji`: the "current context" = entities in the query (and in the last few recalled memories of the session, giving priming) spreading to memories that mention them. This is the `mentions` join; weight each by `S - ln(fan)`.
4. Retrieval threshold -> **abstention**: replace the raw cosine gate (`ABSTAIN_COSINE 0.18`, overlapping distributions in `multihop.md` 2.6) by thresholding a combined activation `A_i` that includes frequency/recency and fan, or by `P_i` from the logistic: ACT-R's noise parameter gives a calibrated-looking soft gate.
5. Noise: add logistic noise to ties to avoid always returning the same top-k (explore-exploit) — optional; off by default for determinism.

**Evidence it works for memory systems.** Rational-analysis fits to human data (Anderson & Schooler 1991); used in ACT-R models since 1993. For LLM agents: Generative Agents' recency `0.995^h` is a crude form; MemoryBank's `e^(-t/S)` another. No LLM-agent paper in this review tests ACT-R B_i against those on a benchmark; (inference) the gain is likely modest on LongMemEval-style single-shot questions and larger on long deployments.

**Weak / wrong.** d = 0.5 and S = 2 are tuned to lab data; the power-law-of-need holds for language corpora and may not match an agent's own access pattern; with n=1 and a long age the term collapses and must be floored.

**What neocortex could steal, concretely.** Items 1-4 above: `B ~ ln(n/(1-d)) - d ln(L)` as an RRF rank term from existing `access_count`/`created_at` (`recall.rs:907`, `:1021`); `S - ln(degree)` fan weighting of PPR seeds (`recall.rs::overview_ppr`, `paths.rs`); activation-based abstention in place of the raw cosine gate. No LLM.
