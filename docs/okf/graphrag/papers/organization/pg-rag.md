---
type: Paper
title: Empowering Large Language Models to Set up a Knowledge Retrieval Indexer via Self-Learning
description: 'PG-RAG: an LLM writes concise notes from raw text, which form a pseudo-graph index; retrieval follows the most corroborated fact paths.'
resource: https://arxiv.org/abs/2405.16933
tags:
- pseudo-graph
- self-learning-index
- fact-paths
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: indexing
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2405.16933
  title: arXiv abstract
---

# Summary
- Has the LLM write concise notes into a pseudo-graph, then retrieves integrated fact paths corroborated by many notes (abstract).[^paper]

# Relevance to neocortex
- The corroboration-weighted path selection is LLM-free and could rank neocortex's fact paths. Index construction is LLM-driven.

[^paper]: arXiv abstract
