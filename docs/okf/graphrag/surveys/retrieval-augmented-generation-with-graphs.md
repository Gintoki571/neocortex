---
type: Survey
title: "Retrieval-Augmented Generation with Graphs (GraphRAG)"
description: Survey that defines GraphRAG as a pipeline of query processing, graph-data retrieval, organisation and generation, and reviews its applications.
resource: "https://arxiv.org/abs/2501.00309"
tags:
- survey
- graph-rag
- taxonomy
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/html/2501.00309
  title: arXiv HTML full text (read in part)
- id: abs
  resource: https://arxiv.org/abs/2501.00309
  title: arXiv abstract page
---

# Summary
- Argues that graphs encode heterogeneous relational information, and that GraphRAG has to handle diverse graph formats and domain-specific relations, unlike conventional RAG where retriever, generator and data share one embedding space.[^paper]
- Frames GraphRAG as a pipeline: a query processor, a retriever over graph data, an organiser that refines the retrieved content, and a generator that produces the answer.[^paper]
- Lists graph query languages (Cypher, GraphQL, SPARQL) as the interface to property-graph stores.[^paper]
- Only the opening sections were read; the application tables were not inspected.

# Method
- Not a method paper. The organiser step (turning entities, relations, triplets, paths or subgraphs into context) is the part most relevant to neocortex's recall output.[^paper]

# Results
- Not applicable.

# Relevance to neocortex
- Reading list for the GraphRAG design space; not an implementation source. The organiser framing matches neocortex's split between retrieval and the calling agent's generation.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2501.00309 (read in part)
