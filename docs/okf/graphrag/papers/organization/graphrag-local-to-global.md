---
type: Paper
title: 'From Local to Global: A Graph RAG Approach to Query-Focused Summarization'
description: 'Microsoft GraphRAG: an LLM builds an entity knowledge graph and hierarchical community summaries, answering global questions by map-reduce over the summaries.'
resource: https://arxiv.org/abs/2404.16130
tags:
- graphrag
- community-summaries
- leiden
- global-queries
- query-focused-summarization
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2404.16130
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2404.16130
  title: arXiv HTML full text
---

# Summary
- Answers corpus-wide questions that RAG cannot, by pre-generating summaries for communities of an LLM-built entity graph.[^paper]
- Indexing uses an LLM in two stages: entity knowledge graph extraction, then community summary generation.[^paper]

# Method
- Entity and relation extraction by an LLM from source chunks; community detection (Leiden) over the entity graph; LLM summaries per community.[^html]
- Global queries map over community summaries and reduce to an answer; local queries use entity neighbourhoods.[^html]

# Results
- The paper reports win rates against naive RAG on global sensemaking questions (figures in the text).

# Relevance to neocortex
- Indexing is LLM-heavy, so it cannot run inside neocortex. The useful part for neocortex is the community layer: Louvain communities already exist in neocortex, and summaries could be written by the agent as memories.
- Global query answering is an agent-side pattern rather than an engine feature.
- Related: [community summaries](/techniques/community-summaries.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
