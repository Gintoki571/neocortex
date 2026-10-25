---
type: Technique
title: Ego-graph merge with soft pruning
description: Merge ego-graphs of top key nodes and prune redundant nodes softly before returning context.
tags:
- ego-graph
- pruning
- subgraph
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Take the top-N key nodes, form their ego-graphs, merge, and soft-prune nodes and edges with low relevance.

# Method
- Take the top-N key nodes, form their ego-graphs, merge, and soft-prune nodes and edges with low relevance.

# Papers using it
- [GRAG](/papers/retrieval/grag.md): divide-and-conquer ego-graph merge with soft pruning, linear-time key node selection.
- [DynaGRAG](/papers/organization/dynagrag.md): similarity-aware BFS and GCN-based pruning scores.

# Relevance to neocortex
- Cheap subgraph selection over neocortex's entity graph with an LLM-free step, useful to keep recall output compact.
