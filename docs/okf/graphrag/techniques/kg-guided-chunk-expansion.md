---
type: Technique
title: KG-guided chunk expansion
description: Expand a set of seed chunks through the entity graph to pull in related chunks that the vector search missed.
tags:
- expansion
- graph-hops
- seed-chunks
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Seeds come from semantic search; m-hop neighbours of seed entities are read out to their chunks.

# Method
- Seeds come from semantic search; m-hop neighbours of seed entities are read out to their chunks.
- Expansion is capped by hop count and re-ranked by similarity to avoid context blow-up.

# Papers using it
- [KG2RAG](/papers/retrieval/kg2rag.md): seed chunks, m-hop KG expansion, KG-based organisation.
- [E2GraphRAG](/papers/organization/e2graphrag.md): local retrieval with a hop threshold h tightened when the set is too large.

# Relevance to neocortex
- Matches neocortex's expansion from recalled memories over entity links. The hop cap and ranking rule are the parts to copy.
