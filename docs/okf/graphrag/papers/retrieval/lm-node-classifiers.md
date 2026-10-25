---
type: Paper
title: How to Make LLMs Strong Node Classifiers?
description: Makes an off-the-shelf LM (Flan-T5) match GNN node classifiers by enriching its input with retrieved neighbours and pruning classes with a small GNN.
resource: https://arxiv.org/abs/2410.02296
tags:
- node-classification
- lm-graph
- retrieval-augmented-input
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/abs/2410.02296
  title: arXiv abstract
---

# Summary
- Enriches the LM input with topological and semantic retrieval and prunes candidate classes with a lightweight GNN (abstract).[^paper]

# Relevance to neocortex
- Node classification, not memory recall; skip. The neighbour-enriched input is a general idea only.

[^paper]: arXiv abstract
