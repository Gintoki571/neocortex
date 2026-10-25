---
type: Paper
title: 'KG2RAG: Knowledge Graph-Guided Retrieval Augmented Generation'
description: Semantic seed chunks are expanded through m-hop neighbourhoods of a KG built over the chunks, then organised into a KG-based context.
resource: https://arxiv.org/abs/2502.06864
tags:
- kg-guided-expansion
- chunk-graph
- seed-retrieval
- hybrid
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: NAACL 2025
llm_required: indexing
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2502.06864
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2502.06864
  title: arXiv HTML full text
---

# Summary
- Starts from semantic seed chunks, then expands through the KG: chunks linked to entities in the m-hop neighbourhood of the seeds join the context.[^html]
- Adds a KG-based context organisation step that filters and orders chunks using the subgraph as a skeleton.[^html]

# Method
- Seed chunks by embedding similarity; graph-guided expansion traverses m hops of the entity subgraph linked to seeds and reads out associated chunks.[^html]
- Expanded chunks are scored by similarity to the query and an undirected weighted subgraph is built to organise them.[^html]

# Results
- Results tables exist in the paper; numbers were not extracted in this pass.

# Relevance to neocortex
- The expansion step is LLM-free and maps onto neocortex's entity graph: seed memories from BM25 or the vector index, then hop expansion over entity links. This is a cheap way to add associativity without an LLM.
- Risk: the KG is built from chunks, which is the LLM-dependent part; neocortex's entity links come from the agent at write time.
- Related: [KG-guided chunk expansion](/techniques/kg-guided-chunk-expansion.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
