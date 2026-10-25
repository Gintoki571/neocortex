---
type: "Open-source Project"
title: "nano-graphrag"
description: "Small, hackable GraphRAG implementation with networkx graph storage, nano-vectordb and local, global and naive query modes."
resource: "https://github.com/gusye1234/nano-graphrag"
tags: ["open-source", "graphrag", "minimal", "networkx", "community-report"]
license: "MIT"
language: "Python"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/gusye1234/nano-graphrag"
    title: "Repository README (read from the main branch as readme.md; HEAD/master returned 404)"
  - id: api
    resource: "https://api.github.com/repos/gusye1234/nano-graphrag"
    title: "GitHub metadata (MIT, 4,004 stars, last push 2026-01-27)"
---
# Summary
- Needs two LLM roles: a stronger model for planning and answers and a cheaper one for summaries; any OpenAI-compatible or custom async LLM function can be plugged in, and Ollama and transformers examples are given.[^code]
- Storage: nano-vectordb for vectors, networkx for the graph by default; Neo4j is an option.[^code]
- Query modes: global (community reports), local (entity neighbourhood, recommended by the README as more scalable), and naive RAG.[^code]
- Incremental insert adds only new content, but the communities are recomputed and the community reports regenerated on each insert.[^code]

# Method
- Pipeline: chunk, extract entities and relations with the LLM, build the graph, detect communities, write a report per community, then answer by mode.[^code]

# Results
- No benchmark numbers in the README.[^code]

# Relevance to neocortex
- A readable reference for how a minimal GraphRAG is wired, and a good place to read the local query mode. The rebuild-the-communities-on-every-insert cost is what neocortex should avoid: its Louvain communities should be incremental or lazy.[^code]

[^code]: Repository README (read from the main branch as readme.md; HEAD/master returned 404)
