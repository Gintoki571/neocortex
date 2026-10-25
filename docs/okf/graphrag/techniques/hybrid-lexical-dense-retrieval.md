---
type: Technique
title: Hybrid lexical plus dense retrieval
description: Combine BM25 and dense vector scores (for example RRF) so exact names and paraphrases both match.
tags:
- bm25
- dense
- rrf
- hybrid
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Two ranked lists from BM25 and vector search are fused by reciprocal rank fusion or a weighted sum.

# Method
- Two ranked lists from BM25 and vector search are fused by reciprocal rank fusion or a weighted sum.
- Used as the seed step before graph expansion in several methods.

# Papers using it
- [GRiever](/papers/retrieval/dont-forget-the-base-retriever.md): BM25 plus dense in a hybrid retriever, then graph expansion.
- [CG-RAG](/papers/retrieval/cg-rag.md): sparse and dense signals inside a graph.

# Relevance to neocortex
- neocortex already fuses BM25, vectors and graph with weighted RRF; this file records that the seed-then-expand pattern is what the papers use.
