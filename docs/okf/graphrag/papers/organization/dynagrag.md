---
type: Paper
title: 'DynaGRAG: Dynamic Graph Retrieval-Augmented Generation'
description: Graph RAG that de-duplicates and densifies the knowledge graph, then retrieves diverse subgraphs with a Dynamic Similarity-Aware BFS.
resource: https://arxiv.org/abs/2412.18644
tags:
- graphrag
- dsa-bfs
- subgraph-pruning
- gcn
- dedup
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2412.18644
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2412.18644
  title: arXiv HTML full text
---

# Summary
- Improves subgraph density and diversity: de-duplication, two-step mean pooling of embeddings, query-aware retrieval over unique nodes (abstract).[^paper]
- Traversal uses a Dynamic Similarity-Aware BFS (DSA-BFS) and a GCN-based pruning score (full text).[^html]
- Evaluation in the text read is a podcast-transcript pipeline scored with LLM-judged metrics, so the evidence is thin.

# Method
- Subgraph pruning scores come from relevance scores passed through a GCN that uses the graph structure.[^html]

# Relevance to neocortex
- DSA-BFS, a similarity-gated BFS, is a cheap traversal rule that neocortex could use over its entity graph without an LLM.
- The evaluation is weak for neocortex's purposes (LLM-judged response quality), so treat as a traversal idea only.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
