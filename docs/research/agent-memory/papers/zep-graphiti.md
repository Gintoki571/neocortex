# Zep / Graphiti: bi-temporal knowledge-graph memory (Rasmussen et al., 2025)
Source: https://arxiv.org/abs/2501.13956 (HTML read; Sec 2 graph, 2.2.3 invalidation, Sec 3 retrieval, Tables 1-3).

**Problem.** Agent memory over evolving conversation + business data with correct "what is true now / was true then".

**Data model.** Three tiers: episode subgraph (raw messages, lossless), semantic entity subgraph (entities + fact edges), community subgraph (label-propagation clusters with LLM summaries) (Sec 2). Bi-temporal: timeline T (valid time) and T' (transaction time); each fact edge stores t_valid, t_invalid, t'_created, t'_expired (Sec 2.2.3). Entity and fact embeddings are 1024-d. Bidirectional index between episodes and derived facts.

**Write path.** Per message, an LLM extracts entities (with the last n=4 messages as context), then entity resolution = embedding top candidates + full-text search + LLM dedup decision; then fact extraction between entity pairs, edge dedup restricted to edges between the same two entities (Sec 2.2.1-2.2.2). Temporal extraction resolves "next Thursday" against the message reference time t_ref. A new edge is compared by an LLM with semantically similar existing edges; contradicted edges get t_invalid := new edge's t_valid, new information always wins (Sec 2.2.3). Communities updated by dynamic label propagation (assign new node to the plurality community of neighbours) (Sec 2.3).

**Read path.** search -> rerank -> constructor. Three searches: cosine, BM25 (Lucene) and breadth-first n-hop expansion from seeds; rerankers RRF, MMR, an episode-mentions frequency reranker, node-distance reranker, optional cross-encoder (Sec 3.1-3.2). Context = formatted facts with validity dates + entity summaries.

**Time handling.** The paper's main contribution (above). Edge invalidation, not deletion; as-of queries by filtering validity.

**Dedup / identity.** Embedding + BM25 candidates then LLM.

**Consolidation / forgetting.** Invalidation, communities. No decay.

**Cost.** Many LLM calls per message (extract entities, resolve, extract facts, dedup, temporal, invalidate). Retrieval p50 latency 2.58 s end-to-end including the reader on LongMemEval; context 1.6k tokens vs 115k full (Table 2).

**Reported results.** LongMemEval-S: gpt-4o 71.2% vs full-context 60.2%; gpt-4o-mini 63.8% vs 55.4%. By type (gpt-4o): temporal-reasoning 62.4 vs 45.1, multi-session 57.9 vs 44.3, single-session-preference 56.7 vs 20.0, but single-session-assistant 80.4 vs 94.6 (a loss: facts drop what the assistant said) and knowledge-update only 83.3 vs 78.2 (Table 3). DMR 94.8% (gpt-4-turbo) vs 94.4% full-context.

**What neocortex could steal.** neocortex already has the bi-temporal fact model (valid_from/valid_to, supersession). Concrete gaps this paper exposes: (a) an *episode<->fact bidirectional index* is present (MENTIONS + relation evidence) but its use in reranking, "episode-mentions" frequency rank (a fact backed by many memories outranks a one-off), is cheap: count distinct evidence memories per relation and add as an RRF term in `recall.rs`. No LLM. (b) the *single-session-assistant regression* (-14 pts) is the generic warning that structured facts must be paired with raw text (confirmed by Engram `engram-bitemporal.md`); neocortex keeps raw memory text, keep it that way. (c) dynamic label-propagation is a lighter incremental alternative to Leiden for nc-f5's communities.

**Weak / wrong.** LLM on every write; "new always wins" invalidation is wrong when a late-arriving old fact is ingested (should compare event time, not arrival). Gains are partly from reader-context shrinkage, not retrieval quality (full context is a weak baseline at 115k tokens for gpt-4o-mini).
