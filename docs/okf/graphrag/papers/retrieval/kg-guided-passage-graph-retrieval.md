---
type: Paper
title: Knowledge Guided Text Retrieval and Reading for Open Domain Question Answering
description: Open-domain QA that expands keyword-retrieved seed passages by traversing a passage graph whose edges come from a knowledge base or co-occurrence.
resource: https://arxiv.org/abs/1911.03868
tags:
- passage-graph
- kb-traversal
- seed-expansion
- open-domain-qa
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2019
llm_required: none
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/1911.03868
  title: arXiv abstract
---

# Summary
- Expands a seed set of keyword-retrieved passages by graph traversal over KB-derived or co-occurrence edges, to raise coverage (abstract).[^paper]
- Reports 2-11% absolute gains over non-graph baselines on WebQuestions, Natural Questions and TriviaQA (abstract).[^paper]
- The reader is BERT-based and propagates information across related passages; this part needs a neural reader.

# Relevance to neocortex
- Seed-then-expand over an entity-linked passage graph is LLM-free and fits neocortex: seeds from BM25 and vectors, expansion over shared entities. The reader part is not needed.

[^paper]: arXiv abstract
