---
type: Paper
title: 'PathRAG: Pruning Graph-based Retrieval Augmented Generation with Relational Paths'
description: Retrieves key relational paths between retrieved nodes with flow-based, distance-aware pruning to cut redundancy before prompting.
resource: https://arxiv.org/abs/2502.14902
tags:
- path-retrieval
- flow-pruning
- indexing-graph
- token-reduction
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2502.14902
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2502.14902
  title: arXiv HTML full text
---

# Summary
- Argues graph RAG errors come from redundant retrieved information, not missing information (abstract).[^paper]
- Selects key paths between retrieved nodes with a resource-allocation (flow) pruning rule, then prompts with paths rather than a flat list (full text).[^html]

# Method
- Node retrieval from the indexing graph using query keywords; path retrieval over node pairs; flow-based pruning with distance awareness (full text).[^html]
- Paths are used to structure the prompt in place of a flat list of retrieved items (abstract).[^paper]

# Relevance to neocortex
- Flow-based path pruning is a deterministic, CPU-cheap way to bound the subgraph between two recalled entities, which is the multi-hop bridge neocortex needs. The paper's keyword extraction step was not checked in this pass.
- Related: [flow-based path pruning](/techniques/flow-path-pruning.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
