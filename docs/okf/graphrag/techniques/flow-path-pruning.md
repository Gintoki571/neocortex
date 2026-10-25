---
type: Technique
title: Flow-based relational path pruning
description: Keep the relation paths that carry the most resource between retrieved nodes, pruning redundant paths.
tags:
- paths
- flow
- pruning
- redundancy
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Resource-allocation (flow) scores over the graph, with distance awareness, decide which paths between retrieved nodes to keep.

# Method
- Resource-allocation (flow) scores over the graph, with distance awareness, decide which paths between retrieved nodes to keep.

# Papers using it
- [PathRAG](/papers/retrieval/pathrag.md): flow-based pruning with distance awareness over indexing-graph paths.

# Relevance to neocortex
- A deterministic way to pick the bridge path between two recalled entities for a multi-hop answer, which targets neocortex's three-hop weakness. Its value depends on the graph density; test on neocortex's graph.
