---
type: Paper
title: 'KET-RAG: A Cost-Efficient Multi-Granular Indexing Framework for Graph-RAG'
description: Graph-RAG index with a small LLM-built knowledge-graph skeleton from PageRank-selected chunks plus a cheap text-keyword bipartite graph, to cut indexing cost.
resource: https://arxiv.org/abs/2502.09304
tags:
- graphrag
- indexing-cost
- skeleton-graph
- keyword-graph
- pagerank
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: indexing
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2502.09304
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2502.09304
  title: arXiv HTML full text
---

# Summary
- Targets indexing cost: a full LLM triple extraction over large corpora is expensive, so only a few core chunks get LLM extraction.[^html]
- Two parts: Skeleton-RAG (LLM-extracted KG skeleton over core chunks) and Keyword-RAG (text-keyword bipartite graph, no LLM).[^html]
- Reports Skeleton-RAG cuts indexing cost by 20% while keeping retrieval quality; the combined method is claimed comparable to or better than Microsoft GraphRAG.[^html]

# Method
- Core chunks are selected by PageRank over an intermediate KNN graph of chunk embeddings.[^html]
- Keyword-RAG builds a text-keyword bipartite graph, mimicking KG retrieval at lower cost; retrieval runs from seed nodes by embedding similarity, then local search.[^html]

# Results
- Evaluates 13 solutions on three datasets across indexing cost, retrieval quality and generation quality (numbers not extracted here).[^html]

# Relevance to neocortex
- Keyword-RAG is the closest idea to neocortex's constraints: keyword-to-chunk bipartite lookup with no LLM at indexing. It would be a cheap baseline for the entity layer when the agent provides no triples.
- The skeleton part needs an LLM at write time, which neocortex only has through the calling agent. Use the keyword part first.
- Related: [keyword-chunk bipartite index](/techniques/keyword-chunk-bipartite-index.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
