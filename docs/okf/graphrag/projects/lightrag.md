---
type: "Open-source Project"
title: "LightRAG"
description: "Graph-based RAG with entity and relation indexing and dual-level (low-level and high-level) retrieval, from the HKUDS group."
resource: "https://github.com/HKUDS/LightRAG"
tags: ["open-source", "graphrag", "dual-level", "reranker", "graph-indexing"]
year: 2024
venue: "arXiv 2024"
license: "MIT"
language: "Python"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2410.05779"
    title: "LightRAG paper, arXiv HTML full text (Sections 3-4, Table 1)"
  - id: code
    resource: "https://github.com/HKUDS/LightRAG"
    title: "Repository README (read via raw.githubusercontent.com HEAD/main)"
  - id: api
    resource: "https://api.github.com/repos/HKUDS/LightRAG"
    title: "GitHub metadata (MIT, 40,318 stars, last push 2026-10-22)"
---
# Summary
- Indexing: the LLM profiles each entity node and relation edge into a text key-value pair (K, V), so the graph is searchable by key and by vector.[^paper]
- Dual-level retrieval: low-level retrieval finds specific entities with their attributes and relations; high-level retrieval aggregates across related entities for broader themes; hybrid mode combines both.[^paper]
- Repository: role-specific LLM settings (EXTRACT, QUERY, KEYWORDS, VLM), a reranker enabled by default for mixed queries, and four chunking strategies (Fix, Recursive, Vector, Paragraph).[^code]
- Default storages are in-memory with local file persistence (JsonKVStorage, NanoVectorDBStorage, NetworkXStorage, JsonDocStatusStorage); the README says this is for small-scale testing, and PostgreSQL is recommended for production.[^code]

# Method
- Indexing: the document is split into chunks; the LLM extracts entities and relations from each chunk and writes key-value profiles; the graph and the vector index are built from them.[^paper]
- Query: keywords are extracted (high-level and low-level keywords), matched against the vector index of entities or relations, and the neighbourhood is assembled into the context.[^paper]
- The embedding model must be fixed before indexing and reused at query time; changing it means re-indexing.[^code]

# Results
- Win rates against NaiveRAG on the paper's four datasets (overall dimension): Agriculture LightRAG 67.6% vs 32.4%; CS 61.2% vs 38.8%; Legal 84.8% vs 15.2%.[^paper]
- Against RQ-RAG and HyDE, LightRAG wins most dimensions (for example Comprehensiveness 68.4% vs RQ-RAG on Agriculture; 74.0% vs HyDE on Agriculture).[^paper]

# Relevance to neocortex
- Dual-level retrieval (entity-level keys and relation-level keys) is close to neocortex's entity and relation split, so LightRAG is the most relevant baseline to compare with. Its win rates are judged by an LLM and are not directly comparable with neocortex's retrieval metrics.[^paper]
- Its warning on in-memory defaults supports keeping neocortex's single SQLite file. The LLM extraction stays on the write side, which neocortex does not run.

[^paper]: LightRAG paper, arXiv HTML full text (Sections 3-4, Table 1)
[^code]: Repository README (read via raw.githubusercontent.com HEAD/main)
