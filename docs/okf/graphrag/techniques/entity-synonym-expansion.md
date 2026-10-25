---
type: Technique
title: Entity synonym expansion
description: Add links between phrases whose embeddings are close, so one entity is found under its variants.
tags:
- synonyms
- entity-resolution
- embedding-threshold
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Phrase pairs above an embedding-similarity threshold get a synonym edge.

# Method
- Phrase pairs above an embedding-similarity threshold get a synonym edge.
- At query time, synonyms are added only when they match the query terms.

# Papers using it
- [HippoRAG 2](/papers/retrieval/hipporag-2.md): synonym edges by embedding similarity above a threshold.
- [GRiever](/papers/retrieval/dont-forget-the-base-retriever.md): a same-as index with synonym expansion limited to query-relevant synonyms.

# Relevance to neocortex
- Addresses weak spot (4), entity resolution of variants, without an LLM. The threshold needs tuning on neocortex's memories.
