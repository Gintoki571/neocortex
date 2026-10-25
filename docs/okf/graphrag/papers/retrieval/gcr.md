---
type: Paper
title: 'Graph-constrained Reasoning: Faithful Reasoning on Knowledge Graphs with Large Language Models'
description: GCR constrains LLM decoding with a KG-Trie index of reasoning paths, so generated paths exist in the KG.
resource: https://arxiv.org/abs/2410.13080
tags:
- kg-trie
- constrained-decoding
- faithful
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/abs/2410.13080
  title: arXiv abstract
---

# Summary
- Integrates KG structure into decoding through a trie over KG reasoning paths (abstract).[^paper]

# Relevance to neocortex
- Constrained decoding needs the LLM. The trie of valid paths is an index idea but the engine has no decoder.

[^paper]: arXiv abstract
