---
type: Paper
title: 'Medical Graph RAG: Towards Safe Medical Large Language Model via Graph Retrieval-Augmented Generation'
description: Medical GraphRAG with triple graph construction linking documents to medical sources and controlled vocabularies, and U-Retrieval.
resource: https://arxiv.org/abs/2408.04187
tags:
- medical
- graphrag
- triple-graph
- evidence
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: CVPR 2025
llm_required: both
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/abs/2408.04187
  title: arXiv abstract
---

# Summary
- Builds a triple-linked graph connecting user documents to credible medical sources and controlled vocabularies, with U-Retrieval over it (abstract).[^paper]

# Relevance to neocortex
- Domain-specific, LLM-built graph. The evidence-linking pattern (provenance links to vocabularies) is a useful idea for neocortex's sources field, not the algorithm.

[^paper]: arXiv abstract
