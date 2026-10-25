---
type: Paper
title: 'GFM-RAG: Graph Foundation Model for Retrieval Augmented Generation'
description: A graph neural network trained as a graph foundation model over the knowledge graph, used as a retriever that generalises to unseen datasets without fine-tuning.
resource: https://arxiv.org/abs/2502.01113
tags:
- graph-foundation-model
- gnn
- kg-retriever
- zero-shot
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: NeurIPS 2025
llm_required: retrieval
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2502.01113
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2502.01113
  title: arXiv HTML full text
---

# Summary
- A GNN over the KG that reasons over the graph to rank retrieved knowledge; claims generalisation to unseen datasets without fine-tuning (full text).[^html]

# Method
- The GNN is pretrained on large graphs and used as the retriever at inference; the text read did not give per-query latency.[^html]

# Relevance to neocortex
- A pretrained GNN is CPU-feasible at inference for small graphs, but it needs a training run neocortex does not have. Watch: adoption would require an offline model, which the 0.1.4 engine does not ship.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
