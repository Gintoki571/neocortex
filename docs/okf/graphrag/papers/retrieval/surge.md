---
type: Paper
title: Knowledge Graph-Augmented Language Models for Knowledge-Grounded Dialogue Generation
description: SURGE retrieves a relevant KG subgraph for dialogue and enforces fact consistency with contrastive learning.
resource: https://arxiv.org/abs/2305.18846
tags:
- dialogue
- subgraph-retrieval
- contrastive
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2023
llm_required: none
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2305.18846
  title: arXiv abstract
---

# Summary
- Retrieves the relevant subgraph from the KG first, then perturbs fact embeddings conditioned on that subgraph, with contrastive learning (abstract).[^paper]

# Relevance to neocortex
- Subgraph retrieval step is LLM-free but the model is a dialogue generator. Watch the subgraph-then-consistency idea.

[^paper]: arXiv abstract
