---
type: Paper
title: 'LightRAG: Simple and Fast Retrieval-Augmented Generation'
description: Graph-structured text index with dual-level (entity-level and relation-level) keyword retrieval over a vector store of entities and relations.
resource: https://arxiv.org/abs/2410.05779
tags:
- graphrag
- dual-level-retrieval
- entity-relation-index
- vector-db
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/abs/2410.05779
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2410.05779
  title: arXiv HTML full text
---

# Summary
- Adds an entity-relation graph to the text index; retrieval matches query keywords to entity and relation vectors (local and global keys).[^html]
- Index and query both call an LLM: index-time extraction of entities and relations per chunk, query-time generation of local and global keywords.[^html]
- Claims lower cost and faster incremental updates than GraphRAG, and beats GraphRAG in its UltraDomain experiments on larger corpora.[^html]

# Method
- Index: an LLM extracts entities and relations from each chunk; entity and relation descriptions are embedded in vector stores.[^html]
- Query: an LLM produces local keywords (matched to entities) and global keywords (matched to relations); matched items plus 1-hop graph neighbours form the context.[^html]
- Modes: naive, local, global and hybrid retrieval; the hybrid mode combines the two dual-level results.[^html]

# Results
- Evaluated on four UltraDomain datasets (Agriculture, CS, Legal, Mix); outperforms NaiveRAG and GraphRAG in the paper's win-rate comparisons.[^html]
- Paper's own cost analysis: LLM calls at index time per chunk and at query time for keyword generation.[^html]

# Relevance to neocortex
- The dual-level idea (separate lookup keys for entities versus relations) is copyable with neocortex's entity and relation tables and a vector index, without an LLM. The LLM keyword step at query time would be replaced by the calling agent, which already supplies entities when it writes.
- Index-time LLM extraction is the part neocortex cannot use inside the engine; relation extraction without an LLM is weak (see weak spot 2), so this method needs the agent to supply relations.
- Related: [LLM-free entity extraction](/techniques/llm-free-entity-extraction.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
