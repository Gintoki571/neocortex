---
type: Technique
title: Keyword-chunk bipartite index
description: A bipartite graph of keywords and chunks as a cheap alternative to a full KG.
tags:
- index
- keywords
- bipartite
- low-cost
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Keywords extracted without an LLM (for example by term statistics) link to the chunks where they occur.

# Method
- Keywords extracted without an LLM (for example by term statistics) link to the chunks where they occur.
- Retrieval walks keyword to chunk to keyword like KG retrieval, at lower indexing cost.

# Papers using it
- [KET-RAG](/papers/organization/ket-rag.md): Keyword-RAG as a text-keyword bipartite graph; Skeleton-RAG as the LLM-built part.

# Relevance to neocortex
- Cheapest variant of entity linking for neocortex; works with FTS5 terms as keywords. Check KET-RAG's keyword extraction method before copying (not read in this pass).
