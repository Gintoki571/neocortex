---
type: Technique
title: Entity-chunk bidirectional index
description: Index entities to the chunks they occur in and chunks to their entities, so graph hops and text lookups both stay cheap.
tags:
- index
- entity-chunk
- graph-lookup
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Two one-to-many maps: entity to chunks and chunk to entities; many-to-many links are kept through both directions.

# Method
- Two one-to-many maps: entity to chunks and chunk to entities; many-to-many links are kept through both directions.
- Lookup from query entities to chunks is a map read; entity-coverage and occurrence counts rank chunks.

# Papers using it
- [E2GraphRAG](/papers/organization/e2graphrag.md): entity-to-chunk and chunk-to-entity indexes with the summary tree.

# Relevance to neocortex
- Maps onto neocortex's memory-entity links; the two directions are the tables neocortex likely already has. Adds the ranking-by-coverage rule for expansion.
