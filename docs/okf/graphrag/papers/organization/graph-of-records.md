---
type: Paper
title: 'Graph of Records: Boosting Retrieval Augmented Generation for Long-context Summarization with Graphs'
description: Builds a graph linking retrieved chunks to the LLM responses generated from them, so earlier responses can be reused in later retrieval.
resource: https://arxiv.org/abs/2410.11001
tags:
- graph
- llm-responses
- long-context-summarization
- multi-query
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2410.11001
  title: arXiv abstract
---

# Summary
- Creates edges between retrieved text chunks and the LLM-generated responses that used them, then uses this graph to retrieve and summarise (abstract).[^paper]
- Full text not read in this pass; relevance from the abstract only.

# Relevance to neocortex
- Writing agent-produced answers back into the memory graph is what neocortex's remember call already allows. The retrieval side is LLM-driven; watch the idea of reusing past answers as nodes.

[^paper]: arXiv abstract
