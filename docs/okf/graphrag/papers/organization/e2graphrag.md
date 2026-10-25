---
type: Paper
title: 'E^2GraphRAG: Streamlining Graph-based RAG for High Efficiency and Effectiveness'
description: GraphRAG variant that builds the entity graph with SpaCy (no LLM) plus a summary tree, bidirectional entity-chunk indexes, and adaptive local/global retrieval with no LLM calls at query time.
resource: https://arxiv.org/abs/2505.24226
tags:
- graphrag
- spacy
- entity-chunk-index
- adaptive-retrieval
- summary-tree
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: indexing
neocortex_relevance: adopt
sources:
- id: paper
  resource: https://arxiv.org/abs/2505.24226
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2505.24226
  title: arXiv HTML full text
---

# Summary
- Indexes with an LLM only for the summary tree; the entity graph uses SpaCy on CPU, and both run in parallel.[^html]
- Retrieval uses no LLM: query entities come from SpaCy, and an adaptive rule chooses local (entity graph) or global (collapsed-tree dense) retrieval.[^html]
- Reports that a GraphRAG-local indexing run on a 200k-token book takes about four hours, the cost this design avoids.[^html]

# Method
- Chunks of 1200 tokens with 100-token overlap; entity graph from SpaCy; two one-to-many indexes, entity-to-chunk and chunk-to-entity, link graph and summary tree.[^html]
- Query: SpaCy extracts query entities; entities not in the graph are dropped; if none remain, the query goes to collapsed-tree dense retrieval (RAPTOR-style, BGE-M3 in the paper).[^html]
- Local path: chunks reached within h hops of the query entities are kept; h is tightened step by step if the set is too large, with entity-aware ranking (entity coverage, then occurrence) to pick top-k.[^html]
- Chunks are merged to reduce redundancy and token cost.[^html]

# Results
- Compared against GraphRAG-Local, GraphRAG-Global, LightRAG-Hybrid and RAPTOR on 200k-token novel datasets; the paper's Figure 3 gives indexing time against document length.[^html]
- The paper's own complexity note: the retrieval phase uses no LLM calls; the main cost is SpaCy entity extraction.[^html]

# Relevance to neocortex
- The no-LLM retrieval path maps to neocortex: SpaCy-style entity lookup from the query, entity-to-chunk indexes, and hop-bounded expansion are all CPU operations on SQLite tables.
- Adaptive fallback to dense search when no query entity is found is directly useful for neocortex, which has a vector index; it also covers the entity-resolution weak spot only partly (no typo handling).
- The summary tree needs an LLM at write time, so skip it unless the agent supplies summaries.
- Related: [entity-chunk bidirectional index](/techniques/entity-chunk-bidirectional-index.md), [LLM-free entity extraction](/techniques/llm-free-entity-extraction.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
