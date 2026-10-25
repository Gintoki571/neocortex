---
type: Technique
title: Community summaries for global questions
description: Pre-summarise graph communities so corpus-wide questions can be answered from summaries.
tags:
- communities
- summaries
- global-queries
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: indexing
neocortex_relevance: adapt
---

# Summary
- Detect communities (Leiden in GraphRAG) over the entity graph; an LLM summarises each community; global queries map over summaries and reduce.

# Method
- Detect communities (Leiden in GraphRAG) over the entity graph; an LLM summarises each community; global queries map over summaries and reduce.
- Summaries are regenerated when the graph changes.

# Papers using it
- [GraphRAG](/papers/organization/graphrag-local-to-global.md): Leiden communities and LLM summaries, map-reduce for global questions.
- [ArchRAG](/papers/organization/archrag.md): attributed communities with LLM hierarchical clustering.

# Relevance to neocortex
- neocortex has Louvain communities; the summary text would have to come from the agent at write time. The map-reduce reader is agent-side.
