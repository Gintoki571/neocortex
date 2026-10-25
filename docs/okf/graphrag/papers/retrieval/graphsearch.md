---
type: Paper
title: 'GraphSearch: An Agentic Deep Searching Workflow for Graph Retrieval-Augmented Generation'
description: 'Agentic multi-turn GraphRAG with dual-channel retrieval: semantic queries over chunks and relational queries over the graph.'
resource: https://arxiv.org/abs/2509.22009
tags:
- agentic
- dual-channel
- graphrag
- multi-turn
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2509.22009
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2509.22009
  title: arXiv HTML full text
---

# Summary
- Agent workflow with six modules that combines semantic chunk retrieval and relational graph queries across turns (full text and abstract).[^html]
- README lists this paper twice under different titles (Query-Centric Graph RAG in Knowledge Organization); the arXiv record is GraphSearch.

# Method
- Dual-channel retrieval: semantic queries over text and relational queries over the structured graph, issued by an agent over multiple turns.[^html]

# Relevance to neocortex
- The dual-channel split (semantic plus relational) is what neocortex already does in fusion; the agent loop is the calling agent's job. Use as a pattern for how the agent should call recall twice, not as an engine change.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
