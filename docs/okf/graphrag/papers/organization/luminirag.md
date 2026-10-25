---
type: Paper
title: 'LuminiRAG: Vision-Enhanced Graph RAG for Complex Multi-Modal Document Understanding'
description: 'Single-author TechRxiv preprint, a graph RAG for multi-modal documents that adds vision-language extraction and cross-modal graph links (abstract only).'
resource: https://www.techrxiv.org/users/867713/articles/1248304-luminirag-vision-enhanced-graph-rag-for-complex-multi-modal-document-understanding
tags:
- multimodal
- graphrag
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: TechRxiv 2024 (preprint)
llm_required: both
neocortex_relevance: skip
sources:
- id: answerthis
  resource: https://doi.org/10.36227/techrxiv.173386549.94077641/v1
  title: AnswerThis exact-title search record (abstract, author Lionel Martis, DOI)
- id: pdf
  resource: https://www.techrxiv.org/doi/pdf/10.36227/techrxiv.173386549.94077641
  title: TechRxiv PDF (blocked, HTTP 403 to plain download; not read)
- id: paper
  resource: https://www.techrxiv.org/users/867713/articles/1248304-luminirag-vision-enhanced-graph-rag-for-complex-multi-modal-document-understanding
  title: TechRxiv article page (blocked by verification screen)
- id: listing
  resource: https://github.com/DEEP-PolyU/Awesome-GraphRAG
  title: Awesome-GraphRAG README entry
---

# Summary
- Abstract only. The TechRxiv PDF returned HTTP 403 and the article page stayed on a verification screen, so no full text was read.[^pdf][^paper]
- A single-author preprint by Lionel Martis (2024) that extends GraphRAG and LightRAG to multi-modal documents.[^answerthis]
- Claims three things: selective extraction of relevant content, a vision-language pipeline that keeps links between text and visual elements, and a knowledge graph with cross-modal relations.[^answerthis]
- Evaluated on financial documents (financial reports, transaction documents, contracts) against traditional RAG. The abstract gives no numbers.[^answerthis]

# Method
- Content relevance assessment filters extracted information from multi-modal documents; the exact rule was not read.[^answerthis]
- Vision-language processing keeps the semantic relation between text and visual elements; cross-modal graph construction extends GraphRAG; a query engine uses adaptive semantic thresholding and response verification.[^answerthis]

# Results
- Abstract only: no numbers. The abstract says gains are largest on tables, cross-references and multi-modal relations.[^answerthis]

# Relevance to neocortex
- Requires vision-language models and LLM calls for extraction and answering, and targets document understanding rather than agent memory. Not usable inside the engine.
- Relevance: skip. Re-check only if the full text becomes available.

[^answerthis]: AnswerThis search record, https://doi.org/10.36227/techrxiv.173386549.94077641/v1 (abstract only)
[^pdf]: TechRxiv PDF, https://www.techrxiv.org/doi/pdf/10.36227/techrxiv.173386549.94077641 (blocked, HTTP 403)
[^paper]: TechRxiv article page, https://www.techrxiv.org/users/867713/articles/1248304-luminirag-vision-enhanced-graph-rag-for-complex-multi-modal-document-understanding (blocked)
[^listing]: Awesome-GraphRAG README, https://github.com/DEEP-PolyU/Awesome-GraphRAG
