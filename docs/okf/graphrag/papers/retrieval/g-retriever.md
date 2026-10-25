---
type: Paper
title: 'G-Retriever: Retrieval-Augmented Generation for Textual Graph Understanding and Question Answering'
description: Retrieves a query-relevant subgraph from a textual graph by solving a prize-collecting Steiner tree problem, then answers with an LLM over the subgraph.
resource: https://arxiv.org/abs/2402.07630
tags:
- graphqa
- pcst
- subgraph-retrieval
- gnn
- textual-graph
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: retrieval
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2402.07630
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2402.07630
  title: arXiv HTML full text
---

# Summary
- Adds retrieval over a textual graph so an LLM can answer questions about it without its context being overwhelmed; introduces the GraphQA benchmark.[^paper]
- Subgraph retrieval is cast as a Prize-Collecting Steiner Tree problem: nodes and edges get prizes from cosine similarity to the query.[^html]

# Method
- Node and edge text is embedded; the top-k most similar items get higher prizes; PCST keeps a connected subgraph that maximises prize minus edge cost, which keeps the graph small and relevant.[^html]
- The retrieved subgraph is textualised and passed to the LLM with the question.[^html]

# Results
- Results and GraphQA numbers are in the paper; not extracted in this pass.

# Relevance to neocortex
- PCST is a clean, deterministic subgraph selector that needs no LLM; it fits neocortex's need for a bounded context from the entity graph, and it is CPU-cheap for small candidate sets.
- The paper's per-query subgraph is for the calling agent to read, which is the right split for neocortex.
- Related: [Steiner subgraph retrieval](/techniques/steiner-subgraph-retrieval.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
