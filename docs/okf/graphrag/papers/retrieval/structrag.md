---
type: Paper
title: 'StructRAG: Boosting Knowledge Intensive Reasoning of LLMs via Inference-time Hybrid Information Structurization'
description: Chooses a structure type for the task, reconstructs documents into that structure at inference, and answers from it.
resource: https://arxiv.org/abs/2410.08815
tags:
- structurization
- hybrid
- inference-time
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2410.08815
  title: arXiv abstract
---

# Summary
- Picks the best structure type per task (e.g. table, graph), reconstructs documents into it, then infers the answer (abstract).[^paper]

# Relevance to neocortex
- Inference-time structuring is LLM-driven; the idea of choosing a structure per query could guide which neocortex view to return.

[^paper]: arXiv abstract
