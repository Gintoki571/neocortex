---
type: Paper
title: 'HippoRAG 2: From RAG to Memory'
description: Passage-augmented Personalized PageRank over an LLM-built open KG, with an LLM filter on retrieved triples, for non-parametric continual memory.
resource: https://arxiv.org/abs/2502.14802
tags:
- retrieval
- personalized-pagerank
- passage-nodes
- open-kg
- continual-learning
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: ICML 2025
llm_required: both
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2502.14802
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2502.14802
  title: arXiv HTML full text
---

# Summary
- Builds on HippoRAG: an open KG of triples extracted from passages, then Personalized PageRank (PPR) from query-matched seed nodes.[^html]
- Adds passage nodes to the PPR graph so queries shape the search over both phrases and passages, not only phrase nodes.[^html]
- Reports passage recall@5 gains over the strongest 7B dense retriever, NV-Embed-v2: +5.0% on MuSiQue and +13.9% on 2Wiki.[^html]
- Uses an LLM for offline OpenIE and online triple filtering, so the method as published is not LLM-free.

# Method
- Offline: an LLM extracts triples (OpenIE) from passages; synonym edges between phrase nodes are added when embedding similarity exceeds a threshold.[^html]
- Online: an embedding model ranks triples (query-to-triple, top-5 kept after LLM filtering); up to k phrase nodes become PPR seeds, weighted by their ranking scores.[^html]
- Passage nodes receive reset probability proportional to embedding similarity, multiplied by a weight factor tuned on validation (Table 5 of the paper); top-ranked passages by PPR score go to the reader.[^html]
- Defaults follow HippoRAG; the paper reports NV-Embed-v2 as the retriever and Llama-3.3-70B-Instruct for extraction and filtering.[^html]

# Results
- Passage recall@5 on MuSiQue and 2Wiki are the paper's highest across most datasets; the 2Wiki gain over NV-Embed-v2 is +13.9 points.[^html]
- QA F1: +9.5% over NV-Embed-v2 on 2Wiki and +3.1% on LV-Eval.[^html]
- Continual-learning experiment (4 corpus segments) shows its gain over NV-Embed-v2 stays consistent as segments are added.[^html]

# Relevance to neocortex
- Most direct fit for the multi-hop weak spot: PPR gives associative multi-hop retrieval without a learned hop model. The published pipeline needs an LLM for OpenIE and triple filtering, so neocortex can only copy the graph step: the agent (the calling LLM) would supply the triples at write time, which matches neocortex's design.
- The triple-filter step is the LLM call to drop. Lexical or embedding filtering would replace it, but the paper gives no LLM-free ablation in the text read.
- Cost on CPU: PPR over the entity graph is cheap, but the paper reports token and time cost per method in Appendix F, not per-recall latency. Check against the ~100 ms budget at 10k memories before adopting.
- Related: [personalized PageRank seeding](/techniques/ppr-seeding.md), [LLM-free entity extraction](/techniques/llm-free-entity-extraction.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
