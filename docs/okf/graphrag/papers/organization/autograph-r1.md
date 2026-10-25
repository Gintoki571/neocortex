---
type: Paper
title: 'AutoGraph-R1: End-to-End Reinforcement Learning for Knowledge Graph Construction'
description: Trains an LLM KG constructor with RL, using the downstream RAG utility as the reward.
resource: https://arxiv.org/abs/2510.15339
tags:
- kg-construction
- reinforcement-learning
- task-aware-reward
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: indexing
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/abs/2510.15339
  title: arXiv abstract
---

# Summary
- Frames graph generation as a policy-learning problem with task-aware rewards for graphs as knowledge carriers and as knowledge indices (abstract).[^paper]

# Relevance to neocortex
- Requires LLM training. Skip for neocortex's engine; the reward idea (score graphs by downstream recall) is a useful evaluation idea for neocortex's benchmark.

[^paper]: arXiv abstract
