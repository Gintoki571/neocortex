# RAPTOR: recursive abstractive tree retrieval (Sarthi et al., ICLR 2024)
Source: https://arxiv.org/abs/2401.18059 (HTML read; Sec 3, Fig 3, Tables 6, 7).

**Problem.** Chunk retrieval cannot answer questions that need document-level themes.

**Data model.** Tree over ~100-token chunks: leaves = chunks; each upper node = LLM summary of a cluster of children; clusters from UMAP-reduced embeddings + Gaussian mixture, number of clusters by BIC, soft assignment so a chunk can sit in several clusters (Sec 3).

**Write path.** Embed chunks, cluster, summarise with an LLM, embed summaries, recurse until clustering fails. LLM calls ~ number of internal nodes.

**Read path.** *Collapsed tree*: put all nodes from all layers in one pool, cosine to the query, take top nodes until a token budget; found better than layer-by-layer tree traversal (Fig 3, Sec 4.1).

**Time / dedup / forgetting.** None; static corpus, rebuild to update.

**Cost.** Offline LLM-heavy; query-time is one KNN.

**Reported results.** QuALITY accuracy 82.6% with GPT-4 vs previous best 62.3% (Table 7; +21.5 on QuALITY-HARD); NarrativeQA ROUGE-L +7.3 over BM25 and +2.7 over DPR (Table 6). In HippoRAG 2's re-run RAPTOR averages 65.6 passage recall@5 vs 73.4 for plain NV-Embed-v2 (2502.14802 Table 3), i.e. it often hurts retrieval of exact facts.

**What neocortex could steal.** The multi-granularity pool and "collapsed tree" are what neocortex's `overview`/communities already provide; Leiden communities (nc-f5) are the LLM-free cluster layer. Use *extractive* community nodes (top-weight member facts, no LLM) as the summary layer and score them in the same RRF as memories. Touches `recall.rs::overview`, `cached_communities`.

**Weak / wrong.** Static; summary drift; independent evaluation shows no gain for fact lookup; adds LLM cost for update-heavy memories.
