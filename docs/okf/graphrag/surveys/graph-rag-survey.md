---
type: Survey
title: "Graph Retrieval-Augmented Generation: A Survey"
description: Survey that formalises GraphRAG as graph-based indexing, graph-guided retrieval and graph-enhanced generation.
resource: "https://arxiv.org/abs/2408.08921"
tags:
- survey
- graph-rag
- taxonomy
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: ACM Transactions / JACM (per arXiv HTML header)
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/html/2408.08921
  title: arXiv HTML full text (read in part)
- id: abs
  resource: https://arxiv.org/abs/2408.08921
  title: arXiv abstract page
---

# Summary
- Surveys GraphRAG, which uses structure across entities to make retrieval more precise and complete, and which avoids retraining while curbing hallucination and stale knowledge.[^paper]
- Formalises the GraphRAG workflow as three stages: graph-based indexing, graph-guided retrieval and graph-enhanced generation.[^paper]
- Groups data sources into open knowledge graphs and self-constructed graph data; query enhancement into expansion and decomposition, and knowledge enhancement into merging and pruning.[^paper]
- Only the opening sections were read (about 60k characters); the tabular comparison of methods was not inspected.

# Method
- Not a method paper. The survey's taxonomy is the method description: indexing (graph selection or construction and index build), retrieval (query processing, retriever, organiser), generation.[^paper]

# Results
- Not applicable. No experiments are reported in the read portion.

# Relevance to neocortex
- Background reading for the GraphRAG design space. Not an implementation source. Use it to place neocortex's graph and vector paths in the taxonomy.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2408.08921 (read in part)
