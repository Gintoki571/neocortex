---
type: Paper
title: Don't Forget the Base Retriever! A Low-Resource Graph-based Retriever for Multi-hop Question Answering
description: 'GRiever: an LLM-free, multi-step graph retriever over a passage-triple index plus synonym expansion, running in hundreds of milliseconds.'
resource: https://aclanthology.org/2025.emnlp-industry.174/
tags:
- graph-retriever
- no-llm
- multi-hop
- triples-index
- synonym-expansion
- low-latency
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: EMNLP 2025 Industry
llm_required: indexing
neocortex_relevance: adopt
sources:
- id: paper
  resource: https://aclanthology.org/2025.emnlp-industry.174/
  title: ACL Anthology abstract page
- id: pdf
  resource: https://aclanthology.org/2025.emnlp-industry.174.pdf
  title: ACL Anthology PDF (text extracted with pdftotext)
---

# Summary
- Multi-step retrieval with no LLM call at query time; the abstract says it runs in a few hundred milliseconds.[^paper]
- Indexes passages alongside a triples graph and uses a hybrid retriever (BM25 plus dense) with aggressive filtering to cut latency.[^pdf]
- Reports higher passage recall than conventional retrievers and GraphRAG methods, and at least an order of magnitude faster than the GraphRAG baselines it compares to.[^pdf]

# Method
- Offline: a passage index, a triples index extracted from the passages, and a same-as index of synonyms with the passage ids they came from.[^pdf]
- Online: BM25 and dense retrieval on passages; triples found in the top passages are expanded along chains; synonyms of matched entities are added only when relevant to the query.[^pdf]
- Filtering on subject and chunk keeps the candidate set small; the introduction names a target of 100 to 500 ms for this kind of retrieval.[^pdf]

# Results
- Table 1 (MuSiQue, R@5): BM25 0.351, dense 0.319, RRF hybrid 0.394, composed 0.417, GRiever 0.456; R@10 GRiever 0.539 and R@15 0.573.[^pdf]
- Table 1 latency (ms@5) for GRiever on MuSiQue is 425 +/- 122 ms, against 23 ms for BM25 and 294 ms for dense; on 2Wiki R@5 is 0.676 vs 0.64 for BM25 and 0.624 for the composed method.[^pdf]
- HotpotQA R@5: GRiever 0.813 vs dense 0.728 and BM25 0.668.[^pdf]

# Relevance to neocortex
- The closest published fit to neocortex's constraints: no LLM at query time, CPU-only, a SQLite-friendly index (passages, triples, synonyms), and explicit multi-hop expansion over triple chains.
- Latency is the caveat: the paper reports about 425 ms at MuSiQue R@5, well above neocortex's 100 ms budget at 10k memories; the hardware is not stated in the text read, so re-measure before treating it as a target.
- Triple extraction is an offline step; the text read does not say whether that extractor is an LLM. neocortex would take triples from the calling agent at write time, which keeps the engine LLM-free.
- Related: [hybrid BM25 plus dense retrieval](/techniques/hybrid-lexical-dense-retrieval.md), [synonym expansion on entities](/techniques/entity-synonym-expansion.md).

[^paper]: ACL Anthology abstract page
[^pdf]: ACL Anthology PDF (text extracted with pdftotext)
