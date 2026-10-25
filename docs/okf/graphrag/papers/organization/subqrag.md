---
type: Paper
title: 'SUBQRAG: Sub-Question Driven Dynamic Graph RAG'
description: Decomposes a question into ordered sub-questions and expands the KG on the fly from source documents when triples are missing.
resource: https://arxiv.org/abs/2510.07718
tags:
- sub-questions
- dynamic-expansion
- multi-hop
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2510.07718
  title: arXiv abstract
---

# Summary
- Retrieves triples per sub-question and extracts new triples from source documents when the graph is insufficient (abstract).[^paper]

# Relevance to neocortex
- Sub-question chaining is a planning step for the calling agent. The on-demand extraction needs an LLM, so it stays agent-side.

[^paper]: arXiv abstract
