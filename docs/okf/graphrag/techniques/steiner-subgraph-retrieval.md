---
type: Technique
title: Steiner subgraph retrieval
description: Select a small connected subgraph that holds the most query-relevant nodes, by prize-collecting Steiner tree.
tags:
- pcst
- subgraph
- context-budget
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Nodes and edges get prizes from similarity to the query; the Prize-Collecting Steiner Tree keeps a connected subgraph with high prize and low edge cost.

# Method
- Nodes and edges get prizes from similarity to the query; the Prize-Collecting Steiner Tree keeps a connected subgraph with high prize and low edge cost.
- The result is a bounded, connected context.

# Papers using it
- [G-Retriever](/papers/retrieval/g-retriever.md): PCST over textual graphs.
- [AGRAG](/papers/organization/agrag.md): minimum-cost maximum-influence subgraph solved greedily (NP-hard problem).

# Relevance to neocortex
- Useful for returning a connected memory subgraph to the agent under a size budget. PCST is a standard solver; a greedy variant keeps latency predictable.
