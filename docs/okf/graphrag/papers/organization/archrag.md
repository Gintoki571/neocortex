---
type: Paper
title: 'ArchRAG: Attributed Community-based Hierarchical Retrieval-Augmented Generation'
description: Community-based hierarchical GraphRAG that augments queries with attributed communities and uses LLM-based hierarchical clustering.
resource: https://arxiv.org/abs/2502.09891
tags:
- communities
- hierarchical-clustering
- attributed-graph
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2502.09891
  title: arXiv abstract
---

# Summary
- Augments the question with attributed communities and proposes LLM-based hierarchical clustering to cut online token use (abstract).[^paper]

# Relevance to neocortex
- Attributed communities are close to neocortex's Louvain communities with entity attributes. The clustering is LLM-based; the attributed-community lookup is the transferable part.

[^paper]: arXiv abstract
