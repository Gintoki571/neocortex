---
type: Paper
title: 'FastRAG: Retrieval Augmented Generation for Semi-structured Data'
description: Schema and script learning extract structure from semi-structured network data, then text search and KG querying answer questions.
resource: https://arxiv.org/abs/2411.13773
tags:
- semi-structured
- schema-learning
- kg-query
- cost
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2411.13773
  title: arXiv abstract
---

# Summary
- Uses schema and script learning so entire sources are not sent to an LLM; combines text search with KG querying (abstract).[^paper]
- Reports up to 90% time and 85% cost savings over GraphRAG (abstract).[^paper]

# Relevance to neocortex
- Learning a script once and running it without the LLM is the cheapest path to structured ingestion for neocortex. Watch; the domain is network operations.

[^paper]: arXiv abstract
