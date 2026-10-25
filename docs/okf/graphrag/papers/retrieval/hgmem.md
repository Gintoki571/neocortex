---
type: Paper
title: 'HGMEM: Hypergraph-based Working Memory to Improve Multi-step RAG for Long-Context Complex Relational Modeling'
description: Working memory as a hypergraph whose hyperedges group related facts and evolve by update, insertion and merge operations during multi-step RAG.
resource: https://arxiv.org/abs/2512.23959
tags:
- hypergraph-memory
- working-memory
- multi-step-rag
- high-order-relations
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2512.23959
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2512.23959
  title: arXiv HTML full text
---

# Summary
- Replaces passive fact storage in multi-step RAG working memory with a hypergraph that captures high-order correlations among facts (abstract).[^paper]
- The memory evolves through update, insertion and merging operations (full text).[^html]

# Method
- Hyperedges group facts; memory operations are applied as the reasoning proceeds.[^html]

# Relevance to neocortex
- Hyperedges over facts are a natural extension of neocortex's memory-to-entity links, and the merge rule resembles neocortex's supersession. The method is LLM-driven at each step, so only the data structure is a candidate.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
