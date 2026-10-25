---
type: Technique
title: Personalized PageRank seeding
description: Seed a PageRank walk from query-matched nodes so scores spread to associated memories.
tags:
- ppr
- graph-retrieval
- multi-hop
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Seed nodes (matched entities or phrases, sometimes passage nodes too) get reset probability; a random walk with damping (0.85 in LinearRAG) distributes mass to neighbours; top-scored passages are returned.

# Method
- Seed nodes (matched entities or phrases, sometimes passage nodes too) get reset probability; a random walk with damping (0.85 in LinearRAG) distributes mass to neighbours; top-scored passages are returned.
- HippoRAG 2 adds passage nodes to the walk and weights them against phrase nodes (the weight factor is tuned on validation).

# Papers using it
- [HippoRAG 2](/papers/retrieval/hipporag-2.md): passage nodes plus PPR, LLM only for triple filtering.
- [LinearRAG](/papers/organization/linearrag.md): PPR on a passage-entity bipartite graph after entity activation; no LLM.

# Relevance to neocortex
- Cheap on CPU for sparse graphs of 10k memories. Pairs with neocortex's existing Personalized PageRank; the open item is the passage-node weight, which must be tuned on neocortex data.
