---
type: Paper
title: 'GraphCoder: Enhancing Repository-Level Code Completion via Code Context Graph-based Retrieval and Language Model'
description: Code completion with a code context graph (control, data and dependence edges) and a coarse-to-fine graph retrieval.
resource: https://arxiv.org/abs/2406.07003
tags:
- code-graph
- repository
- retrieval
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/abs/2406.07003
  title: arXiv abstract
---

# Summary
- Builds a code context graph of control-flow and data/control-dependence edges and retrieves coarse-to-fine (abstract).[^paper]

# Relevance to neocortex
- Code-domain. Skip; the coarse-to-fine retrieval over a typed graph is the only general idea.

[^paper]: arXiv abstract
