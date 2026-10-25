---
type: Paper
title: Graph Neural Network Enhanced Retrieval for Question Answering of LLMs
description: Builds a passage graph (structure-related or keyword-shared passages) and runs a GNN over retrieval scores; RGNN-Ret extends it across hops.
resource: https://arxiv.org/abs/2406.06572
tags:
- gnn
- passage-graph
- retrieval
- multi-hop
- rgnn
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: retrieval
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2406.06572
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2406.06572
  title: arXiv HTML full text
---

# Summary
- Treats passages as isolated in dense retrieval; instead connects passages that are adjacent or share keywords and lets a GNN pass relevance between them.[^html]
- Extends to multi-hop with a recurrent GNN (RGNN-Ret) that merges passage graphs from earlier steps.[^html]
- Claims higher QA accuracy with a single LLM query than multi-query baselines (abstract).[^paper]

# Method
- Graph of passages: edges for contiguous passages and for shared keywords; initial scores from BM25 or dense similarity.[^html]
- A GNN (trained) propagates scores over the graph, so a passage with a strong neighbour (e.g. the background passage sharing the 'D'Angelo' keyword) is ranked up.[^html]

# Results
- On 2WikiMQA, GNN-Ret with one LLM query beats baselines that use multiple queries (Figure 2); RGNN-Ret reports state-of-the-art on the four benchmarks (numbers not extracted).[^paper]

# Relevance to neocortex
- A learned score-propagation step over a keyword-shared passage graph is CPU-feasible and LLM-free at query time. It is a candidate second-stage reranker over neocortex's BM25 and vector hits, targeting the paraphrased-hop gap only partly.
- Needs training data and a GNN; neocortex would need an offline training step that the engine does not have today.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
