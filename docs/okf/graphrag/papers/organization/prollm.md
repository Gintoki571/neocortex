---
type: Paper
title: 'ProLLM: Protein Chain-of-Thoughts Enhanced LLM for Protein-Protein Interaction Prediction'
description: Protein-interaction prediction with KG-based chain-of-thought (CoLM 2024).
resource: https://openreview.net/forum?id=2nTzomzjjb
tags:
- protein
- biology
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: CoLM 2024
llm_required: both
neocortex_relevance: skip
sources:
- id: paper
  resource: https://openreview.net/forum?id=2nTzomzjjb
  title: 'OpenReview page (blocked: verification challenge, not completed)'
- id: arxiv
  resource: https://arxiv.org/abs/2405.06649
  title: arXiv preprint 2405.06649 with the same title (abstract read via arXiv API)
---

# Summary
- ProLLM is an LLM for protein-protein interaction prediction. It turns protein relations from structured data into natural-language prompts and models the signalling pathway as a chain of thought ("Protein Chain of Thought", ProCoT).[^arxiv]
- The OpenReview page showed a verification challenge and was not read; the arXiv preprint abstract is the source.[^paper]

# Method
- ProCoT: a signalling pathway is written as a reasoning chain from upstream to intermediate to downstream proteins, used to predict interactions across indirect links.[^arxiv]
- Training uses the ProCoT prompt format; the abstract also mentions an embedding-replacement experiment.[^arxiv]

# Results
- Not recorded: the abstract gives no numbers and the body was not read.

# Relevance to neocortex
- Biology domain; skip.

[^paper]: OpenReview, https://openreview.net/forum?id=2nTzomzjjb (blocked)
[^arxiv]: arXiv abstract, https://arxiv.org/abs/2405.06649
