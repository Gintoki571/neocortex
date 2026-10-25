---
type: "Open-source Project"
title: "Graphiti"
description: "Framework for building temporal context graphs for agents, with bi-temporal facts that are invalidated rather than deleted, and hybrid retrieval."
resource: "https://github.com/getzep/graphiti"
tags: ["open-source", "temporal-kg", "agent-memory", "bi-temporal", "hybrid-retrieval"]
license: "Apache-2.0"
language: "Python"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/getzep/graphiti"
    title: "Repository README (read via raw.githubusercontent.com HEAD/main)"
  - id: api
    resource: "https://api.github.com/repos/getzep/graphiti"
    title: "GitHub metadata (Apache-2.0, 32,194 stars, last push 2026-10-25)"
---
# Summary
- Facts are edges between entities with validity windows; a contradicted fact is invalidated and kept with its history instead of being deleted. Each entity and fact traces back to the episodes (raw data) that produced it.[^code]
- Hybrid retrieval combines semantic embeddings, BM25 keyword search and graph traversal.[^code]
- Default LLM is OpenAI for inference and embedding; the README says it works best with LLMs that support structured output (OpenAI, Anthropic, Gemini), and Groq is also listed.[^code]
- Graph backends: Neo4j 5.26, FalkorDB 1.1.2, or Amazon Neptune with OpenSearch for full text; Kuzu 0.11.2 is marked deprecated.[^code]

# Method
- Ingestion is episode-based: each episode (text or structured JSON) is processed by an LLM to extract entities, facts and summaries, and contradiction judgments are LLM-driven in the baseline it is compared with.[^code]
- Ingestion concurrency is set with SEMAPHORE_LIMIT; the README advises raising it only if the LLM provider's throughput allows.[^code]

# Results
- The README compares with a sequential-summarization baseline qualitatively: temporal handling 'explicit bi-temporal tracking with automatic fact invalidation'; query latency 'typically sub-second' against 'seconds to tens of seconds'. These are the project's claims; no benchmark numbers were read.[^code]

# Relevance to neocortex
- Closest design to neocortex's bi-temporal facts (valid_from, valid_to, expired_at, supersession). Adapt the rule that a contradicted fact is invalidated and kept, and the time-scoped query. The LLM-driven extraction and contradiction judgments must stay out of neocortex.[^code]
- Graphiti's hybrid retrieval (BM25 + embedding + graph traversal) is the same fusion neocortex uses (weighted RRF); compare the fusion weights and whether graph traversal is weighted by temporal validity.

[^code]: Repository README (read via raw.githubusercontent.com HEAD/main)
