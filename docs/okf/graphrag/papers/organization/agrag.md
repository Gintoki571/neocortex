---
type: Paper
title: 'AGRAG: Advanced Graph-based Retrieval-Augmented Generation for LLMs'
description: Graph RAG whose graph is built from statistics rather than LLM entity extraction, and whose retrieval selects a minimum-cost maximum-influence subgraph.
resource: https://arxiv.org/abs/2511.05549
tags:
- graphrag
- statistics-graph-construction
- mcmi-subgraph
- greedy
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: indexing
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2511.05549
  title: arXiv abstract
---

# Summary
- Replaces LLM entity extraction with a statistics-based method, which the abstract says avoids hallucination and error propagation.[^paper]
- Retrieval formulates reasoning as a Minimum Cost Maximum Influence (MCMI) subgraph problem, proved NP-hard, solved with a greedy algorithm, and the subgraph is shown to the LLM as explicit reasoning paths.[^paper]

# Relevance to neocortex
- Statistics-based graph construction and a greedy subgraph selector are both LLM-free and CPU-cheap, the two parts neocortex needs. The NP-hard formulation is solved greedily, so cost is bounded.
- Strong candidate to test against neocortex's entity graph; the paper's own results were not read in this pass.
- Related: [LLM-free entity extraction](/techniques/llm-free-entity-extraction.md), [Steiner subgraph retrieval](/techniques/steiner-subgraph-retrieval.md).

[^paper]: arXiv abstract
