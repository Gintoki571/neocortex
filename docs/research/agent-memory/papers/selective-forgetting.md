# Selective Forgetting: graph memory with importance-based pruning (Rusu et al., 2026)
Source: https://arxiv.org/abs/2608.28978 (HTML read; Sec 3, Sec 4, Appendix A.2). Preprint, negative-result flavour.

**Problem.** Is a KG better than flat RAG for agent memory, and can a forgetting module shrink it safely?

**Data model.** Typed nodes and attributed edges with `created_at, access_count, last_accessed_at, turns_at_creation, importance_score` (Sec 3.1).

**Write path.** One LLM (GPT-4o-mini) extraction per turn; title match or embedding cosine >= 0.92 de-duplication (Appendix).

**Read path.** LLM extracts question entities; embed; nodes with cosine > 0.75, top-5, BFS 2 hops; serialize subgraph (Sec 3.4). Each visited node gets `access_count++`.

**Forgetting (exact, Sec 3.5).** `Score = 0.35*recency + 0.25*frequency + 0.20*centrality + 0.20*turns_decay`; recency = exponential decay from last access, **90-day half-life**; frequency = log-normalised access count; centrality = log-scaled degree; turns_decay = exponential from creation turn, **1,000-turn half-life**. Every 400 turns prune nodes with score < 0.10 and their edges.

**Reported results.** LongMemEval, 500 questions: graph F1 0.417 vs flat vector (top-5) 0.468, paired bootstrap delta -0.050 (95% CI [-0.085, -0.016]); on questions about a specific earlier **assistant turn** judged correctness falls 0.911 -> 0.607 (decomposition into entities discards surface form); knowledge-update F1 0.456 vs 0.511 (conflict policy keeps stale values). Pruning once on a persistent 27,021-node graph removes 9.8% of nodes / 9.5% bytes with F1 change +0.001 (CI [-0.015, +0.016]) and judged correctness -1.6 points (CI [-3.8, +0.6]). Authors note the missing control: pruning the same fraction at random.

**What neocortex could steal.** (1) A concrete, cheap, LLM-free **forgetting score** to run offline: weights 0.35/0.25/0.20/0.20 over last-access recency, log access count, log degree, age. neocortex should apply it *only to archive/compact decisions* (e.g. drop the dense vector of a memory whose facts survive in relations, or exclude from the vec0 index), never to delete provenance. That addresses scale at 100k+ entities (vec0 table growth, S-08). (2) Warning data point: removing 10% of nodes by this score cost <= 3.8 points worst case, so aggressive pruning is unsafe without a random-prune control; run that control in neocortex's eval before adopting. (3) Keep raw text next to facts (assistant-turn collapse: 0.911 -> 0.607).

**Weak / wrong.** Single small extractor; pruning evaluated once; no random control; LongMemEval haystacks rebuilt per question so graph never really "persists" in experiment 1.
