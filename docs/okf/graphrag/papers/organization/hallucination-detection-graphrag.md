---
type: Paper
title: Detecting Hallucinations in Graph Retrieval-Augmented Generation via Attention Patterns and Semantic Alignment
description: Two lightweight interpretability metrics (path reliance and semantic alignment) to flag hallucinations in GraphRAG generation.
resource: https://arxiv.org/abs/2512.09148
tags:
- hallucination
- interpretability
- attention
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2512.09148
  title: arXiv abstract
---

# Summary
- Proposes Path Reliance Degree and Semantic Alignment Score as metrics on the model's internal state (abstract).[^paper]

# Relevance to neocortex
- Needs model internals; not for the engine. The path-reliance idea (over-reliance on shortest-path triples) is a diagnostic for neocortex's path beam.

[^paper]: arXiv abstract
