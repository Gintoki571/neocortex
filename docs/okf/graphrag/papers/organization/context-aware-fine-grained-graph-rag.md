---
type: Paper
title: Context-Aware Fine-Grained Graph RAG for Query-Focused Summarization
description: FG-RAG adds context-aware entity expansion to graph retrieval and query-level fine-grained summarisation at generation time, for query-focused summarisation (CIKM 2025 short paper).
resource: https://dl.acm.org/doi/10.1145/3746252.3760935
tags:
- qfs
- fine-grained
- graph-rag
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: CIKM 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: answerthis
  resource: https://doi.org/10.1145/3746252.3760935
  title: AnswerThis exact-title search record (abstract, authors, DOI; no open full text)
- id: paper
  resource: https://dl.acm.org/doi/10.1145/3746252.3760935
  title: ACM DL page (blocked)
- id: listing
  resource: https://github.com/DEEP-PolyU/Awesome-GraphRAG
  title: Awesome-GraphRAG README entry
---

# Summary
- Abstract only: the full text was not reached. The ACM page is Cloudflare-gated and no open-access PDF was listed.[^answerthis][^paper]
- Targets query-focused summarisation (QFS) with GraphRAG, arguing that existing graph retrieval gives the LLM too little fine-grained context about the query.[^answerthis]
- Proposes FG-RAG with two parts: Context-Aware Entity Expansion during graph retrieval, and Query-Level Fine-Grained Summarization during response generation.[^answerthis]
- The abstract claims gains over other RAG systems on comprehensiveness, diversity and empowerment. No numbers were read.[^answerthis]
- Authors: Yubin Hong, Chaofan Li, Jingyi Zhang, Yingxia Shao. Four-page proceedings paper (pp. 4802-4806).[^answerthis]

# Method
- Context-Aware Entity Expansion: graph retrieval expands the retrieved entities to add neighbouring context before the LLM sees them (expansion rule not read).[^answerthis]
- Query-Level Fine-Grained Summarization: detail is added during generation so the summary follows the query (procedure not read).[^answerthis]
- Code is said to be at https://github.com/BuptWululu/FG-RAG (not read).[^answerthis]

# Results
- Abstract only: no tables or numbers were read. The claimed gains are qualitative in the abstract.[^answerthis]

# Relevance to neocortex
- Both steps run an LLM, so neocortex could only use the idea on the agent side. The expansion step is the part that could fit the engine: a graph-neighbour expansion of seed entities before ranking, with no LLM call. This is a reading of the abstract only; the exact expansion rule is unknown.
- Relevance: watch. Re-rate after reading the full text (ACM page blocked, GitHub code not read).

[^answerthis]: AnswerThis exact-title search record, https://doi.org/10.1145/3746252.3760935 (abstract only)
[^paper]: ACM DL page, https://dl.acm.org/doi/10.1145/3746252.3760935 (blocked)
[^listing]: Awesome-GraphRAG README, https://github.com/DEEP-PolyU/Awesome-GraphRAG
