---
type: Paper
title: 'RuleRAG: Rule-Guided Retrieval-Augmented Generation with Language Models for Question Answering'
description: Adds logical rules, many derived from KGs, as in-context guidance for both the retriever and the generator.
resource: https://arxiv.org/abs/2410.22353
tags:
- rules
- in-context
- kg-rules
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2410.22353
  title: arXiv abstract
---

# Summary
- Introduces rules for in-context learning to steer retrievers and generators, and builds rule-aware RAG benchmarks from KGs (abstract).[^paper]

# Relevance to neocortex
- Rules mined from the KG could drive neocortex retrieval filters without an LLM; the in-context use is agent-side.

[^paper]: arXiv abstract
