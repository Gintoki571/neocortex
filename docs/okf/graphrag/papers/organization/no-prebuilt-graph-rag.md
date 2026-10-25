---
type: Paper
title: 'You Don''t Need Pre-built Graphs for RAG: Retrieval Augmented Generation with Adaptive Reasoning Structures'
description: Builds the reasoning structure per query at retrieval time instead of pre-constructing a corpus graph, to avoid token cost and update latency.
resource: https://arxiv.org/abs/2508.06105
tags:
- adaptive-structure
- query-time-graph
- no-prebuilt-graph
- token-cost
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2026
venue: AAAI 2026
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2508.06105
  title: arXiv abstract
---

# Summary
- Argues pre-built GraphRAG graphs cost many tokens and add update latency, and that query types need different logic structures (abstract).[^paper]

# Relevance to neocortex
- The idea of building a small structure per query from retrieved candidates is LLM-free if the structure is built from entity overlap. Needs the LLM in its own pipeline, so watch the idea only.
- Venue AAAI 2026 per the README.

[^paper]: arXiv abstract
