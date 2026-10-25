---
type: Paper
title: 'CG-RAG: Research Question Answering by Citation Graph Retrieval-Augmented LLMs'
description: Citation-graph RAG whose LeSeGR retriever combines lexical (sparse) and semantic (dense) signals over query-relevant subgraphs.
resource: https://arxiv.org/abs/2501.15067
tags:
- citation-graph
- sparse-dense
- lexical-semantic
- graph-convolution
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: retrieval
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2501.15067
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2501.15067
  title: arXiv HTML full text
---

# Summary
- Retrieval that integrates sparse and dense signals inside a graph of chunks, with intra- and inter-document edges (abstract and full text).[^html]
- The LeSeGR method convolves over query-relevant subgraphs using both lexical matches (for exact cross-references) and semantic similarity.[^html]

# Method
- Edges connect chunks in the same document by structural dependency and across documents by citation links.[^html]

# Relevance to neocortex
- Sparse plus dense signals on a linked graph is what neocortex's fusion already does; LeSeGR's convolution over the subgraph is the extra step to test. No LLM is needed for the retrieval part as described.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
