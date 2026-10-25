---
type: Paper
title: 'OG-RAG: Ontology-Grounded Retrieval-Augmented Generation For Large Language Models'
description: Builds a hypergraph of domain documents whose hyperedges are grounded in an ontology, then retrieves the minimal hyperedge set covering a query.
resource: https://arxiv.org/abs/2412.15235
tags:
- ontology
- hypergraph
- minimal-cover
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: retrieval
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2412.15235
  title: arXiv abstract
---

# Summary
- Constructs a hypergraph where each hyperedge is a cluster of facts grounded by ontology, and an optimisation picks the minimal hyperedges (abstract).[^paper]

# Relevance to neocortex
- The minimal-cover retrieval step is LLM-free once hyperedges exist; ontologies would come from the agent or from user schemas. A candidate for neocortex's context-set selection.

[^paper]: arXiv abstract
