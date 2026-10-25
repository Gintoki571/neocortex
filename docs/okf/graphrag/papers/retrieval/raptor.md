---
type: Paper
title: 'RAPTOR: Recursive Abstractive Processing for Tree-Organized Retrieval'
description: Recursively embeds, clusters and summarises chunks into a tree, then retrieves across tree levels at inference.
resource: https://arxiv.org/abs/2401.18059
tags:
- tree-index
- recursive-summary
- clustering
- retrieval
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: ICLR 2024
llm_required: indexing
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2401.18059
  title: arXiv abstract
---

# Summary
- Builds a summary tree bottom-up by clustering and summarising chunks, then retrieves from all levels so both detail and abstraction can match (abstract).[^paper]
- Reports state-of-the-art on QuALITY with GPT-4, a 20% improvement (abstract).[^paper]

# Method
- Recursive embedding, clustering and LLM summarisation build the tree; the abstract does not give the clustering details; full text not read in this pass.

# Results
- QuALITY +20% with GPT-4 readout (abstract).

# Relevance to neocortex
- Summaries need an LLM at write time, so neocortex can only use the tree if the calling agent writes the summaries. Retrieval over a tree of embeddings is LLM-free and fits the vector index.
- Related to community-summary layers and to neocortex's consolidation ideas.

[^paper]: arXiv abstract
