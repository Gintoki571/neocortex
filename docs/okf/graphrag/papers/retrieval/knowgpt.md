---
type: Paper
title: 'KnowGPT: Knowledge Graph based PrompTing for Large Language Models'
description: KG prompting framework that extracts the most informative KG knowledge and turns it into hard prompts for closed-source LLMs (NeurIPS 2024).
resource: https://arxiv.org/abs/2312.06185
tags:
- prompting
- kg-selection
generated:
  by: claude-opus/okf-orchestrator
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: NeurIPS 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2312.06185
  title: arXiv abstract
- id: listing
  resource: https://openreview.net/forum?id=PacBluO5m7
  title: OpenReview listing from the Awesome-GraphRAG README (page not readable, bot check)
---

# Summary
- Targets three problems of KG-enhanced LLMs: huge search space, high API cost and laborious prompt engineering, for closed-source LLMs that accept hard prompts only.[^paper]
- Two modules: knowledge extraction (pick the most informative KG knowledge) and context-aware prompt construction (turn it into an effective prompt automatically).[^paper]
- Not the same work as KnowledGPT ([KnowledGPT](/papers/integration/knowgpt.md), EMNLP 2023).

# Results
- Reports beating all competitors on three benchmarks and 92.6% accuracy on the OpenbookQA leaderboard.[^paper]

# Relevance to neocortex
- Both modules run around an LLM; abstract only, method details not read. Agent-side idea at most: neocortex already returns compact facts, which a calling agent can place in its prompt.

[^paper]: arXiv abstract
