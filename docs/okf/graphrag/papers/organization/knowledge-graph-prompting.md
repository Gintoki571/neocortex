---
type: Paper
title: Knowledge Graph Prompting for Multi-Document Question Answering
description: Builds a passage-node knowledge graph over multiple documents and traverses it to form a prompt context for an LLM.
resource: https://arxiv.org/abs/2308.11730
tags:
- multi-document-qa
- kg-prompting
- graph-traversal
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2023
venue: AAAI 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2308.11730
  title: arXiv abstract
---

# Summary
- Constructs a KG over multiple documents whose nodes are passages and graph traversal selects the prompt context (abstract).[^paper]
- Full text not read in this pass.

# Relevance to neocortex
- Passage-node graphs with traversal are the same idea as HippoRAG's passage nodes; use HippoRAG 2 as the reference for the algorithm.

[^paper]: arXiv abstract
