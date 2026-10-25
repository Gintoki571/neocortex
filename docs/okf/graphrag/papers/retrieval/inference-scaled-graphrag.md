---
type: Paper
title: 'Inference Scaled GraphRAG: Improving Multi Hop Question Answering on Knowledge Graphs'
description: 'Adds inference-time compute to LLM graph traversal: sequential deep chain-of-thought traversal plus parallel majority voting over sampled trajectories.'
resource: https://arxiv.org/abs/2506.19967
tags:
- graphrag
- inference-scaling
- kg-traversal
- majority-vote
- grbench
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/abs/2506.19967
  title: arXiv abstract
---

# Summary
- Combines sequential scaling (deep chain-of-thought graph traversal) with parallel scaling (majority voting over sampled trajectories) in an interleaved reasoning-execution loop (abstract).[^paper]
- Evaluated on GRBench (abstract).[^paper]

# Relevance to neocortex
- Every step is an LLM call, and the method spends more inference compute by design. Not usable inside neocortex. Skip; the majority-vote over traversal trajectories is an agent-side idea.

[^paper]: arXiv abstract
