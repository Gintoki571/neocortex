---
type: Paper
title: 'DBCopilot: Natural Language Querying over Massive Databases via Schema Routing'
description: A compact copilot model routes a question to the right database schema before SQL generation, decoupling routing from NL2SQL.
resource: https://arxiv.org/abs/2312.03463
tags:
- schema-routing
- nl2sql
- compact-model
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: EDBT 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2312.03463
  title: arXiv abstract
---

# Summary
- Decouples schema-agnostic NL2SQL into schema routing and SQL generation, with a compact routing model (abstract).[^paper]

# Relevance to neocortex
- Routing over many schemas with a compact model is a pattern for choosing which neocortex namespace or memory group to search, if neocortex ever hosts many. Watch.

[^paper]: arXiv abstract
