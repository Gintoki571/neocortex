---
type: Paper
title: Empowering GraphRAG with Knowledge Filtering and Integration
description: GraphRAG-FI filters retrieved graph knowledge in two stages and balances it against the model's own reasoning via logits-based integration.
resource: https://arxiv.org/abs/2503.13804
tags:
- filtering
- graphrag
- logits-integration
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: retrieval
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2503.13804
  title: arXiv abstract
---

# Summary
- Two-stage filtering removes noisy retrieved graph information; integration uses logits so the model's intrinsic reasoning is not suppressed (abstract).[^paper]

# Relevance to neocortex
- Filtering of retrieved graph facts is cheap and LLM-free if done by scores; the logits integration is LLM-side.

[^paper]: arXiv abstract
