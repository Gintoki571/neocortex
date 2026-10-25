---
type: Paper
title: 'GRAG: Graph Retrieval-Augmented Generation'
description: Retrieves textual subgraphs with a divide-and-conquer ego-graph strategy in linear time, then softly prunes them before LLM generation.
resource: https://arxiv.org/abs/2405.16506
tags:
- subgraph-retrieval
- ego-graph
- soft-pruning
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
  resource: https://arxiv.org/abs/2405.16506
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2405.16506
  title: arXiv HTML full text
---

# Summary
- Targets networked documents (citation graphs, KGs): retrieves textual subgraphs rather than single passages (abstract).[^paper]
- Divide-and-conquer finds approximately optimal subgraphs in linear time by choosing top key nodes and merging their ego-graphs.[^html]

# Method
- Ego-graphs around top-ranked key nodes are merged (top-N) and softly pruned to drop redundant nodes and edges.[^html]
- Text view and graph view are both given to the LLM.[^html]

# Results
- Evaluated on WebQSP and ExplaGraphs (F1, Hit@1, Recall in the paper).

# Relevance to neocortex
- The ego-graph merge with soft pruning is a cheap subgraph step over neocortex's entity graph, and it needs no LLM. Subgraph context could feed the calling agent.
- Related: [ego-graph merge with pruning](/techniques/ego-graph-subgraph-pruning.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
