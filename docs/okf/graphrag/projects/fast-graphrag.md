---
type: "Open-source Project"
title: "Fast GraphRAG"
description: "Promptable GraphRAG framework that domain-tunes entity types and explores the graph with PageRank for interpretable retrieval."
resource: "https://github.com/circlemind-ai/fast-graphrag"
tags: ["open-source", "graphrag", "pagerank", "entity-types", "promptable"]
license: "MIT"
language: "Python"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/circlemind-ai/fast-graphrag"
    title: "Repository README (read via raw.githubusercontent.com HEAD/main)"
  - id: api
    resource: "https://api.github.com/repos/circlemind-ai/fast-graphrag"
    title: "GitHub metadata (MIT, 3,965 stars, last push 2025-11-01)"
---
# Summary
- Dynamic data: the graph is generated and refined to fit a domain, guided by a DOMAIN prompt and an ENTITY_TYPES list (the README example uses Character, Animal, Place, Object, Activity, Event).[^code]
- Intelligent exploration: PageRank-based graph exploration for retrieval.[^code]
- Needs an OpenAI key by default; custom_llm.py shows OpenAI-compatible models and embedders; CONCURRENT_TASK_LIMIT caps parallel LLM calls, which helps with local models.[^code]

# Method
- Indexing: LLM-driven entity and relation extraction against the domain prompt and entity types; retrieval: PageRank from the query's matched nodes.[^code]

# Results
- No benchmark numbers in the README.[^code]

# Relevance to neocortex
- Personalized PageRank from matched seeds is already in neocortex (PPR seeding and communities). Compare its seeding and pruning against neocortex's. The typed entity-type list is a cheap idea to add to neocortex's entity schema.

[^code]: Repository README (read via raw.githubusercontent.com HEAD/main)
