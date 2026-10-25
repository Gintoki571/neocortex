---
type: "Open-source Project"
title: "Cognee"
description: "Open-source memory engine that builds knowledge graphs and embeddings from data for agents, with a no-LLM local mode based on GLiNER."
resource: "https://github.com/topoteretes/cognee"
tags: ["open-source", "agent-memory", "knowledge-graph", "local", "gliner", "remember-recall"]
license: "Apache-2.0"
language: "Python"
llm_required: none
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/topoteretes/cognee"
    title: "Repository README (read via raw.githubusercontent.com HEAD/main)"
  - id: api
    resource: "https://api.github.com/repos/topoteretes/cognee"
    title: "GitHub metadata (Apache-2.0, 32,731 stars, last push 2026-10-23)"
---
# Summary
- Runs with no LLM key: the README's quickstart uses cognee.remember to extract a graph with the local GLiNER model and embed text with a local embedding model; both download on first use.[^code]
- Retrieval returns the matching source text; no LLM generates an answer in that mode. Generated answers and vision or transcription need additional LLM configuration.[^code]
- Surface: Python API (remember, recall), CLI (cognee-cli remember / recall), a server with default ports API 8000, UI 3000, MCP 8001, and plugin or MCP connections for agents.[^code]
- Default mode is multi-tenant with authenticated /api/v1 calls; the README says to set DEFAULT_USER_PASSWORD to keep authentication on for local use.[^code]

# Method
- Ingestion: text is chunked, GLiNER extracts entities and relations with a fixed label set, and a local embedding model embeds the chunks; the graph and vectors are stored together.[^code]
- LLM-dependent improvement stages skip automatically when no key is configured; text ingestion, retrieval and session storage work without one.[^code]
- Graph memory on Postgres is offered and described as a demo feature; the README warns that production-ready versions are a licensed offering.[^code]
- The GLiNER extractor is described as a demo of Cognee's small-model pipeline; a higher-accuracy version is offered separately.[^code]

# Results
- No benchmark numbers were read in the README, so no accuracy claim is recorded.[^code]

# Relevance to neocortex
- The closest peer to neocortex's constraint (CPU, no LLM in the engine). Compare directly: its GLiNER extraction against neocortex's measured GLiNER F1 (0.20 on the team's own set), its local embedding model against UltraBERT, and its remember and recall calls against neocortex's remember and recall.[^code]
- Two things to check before any borrowing: its default storage backend (not verified in the README) and whether its no-LLM mode meets neocortex's ~100 ms recall budget at 10k memories (not measured).[^code]
- Apache-2.0; code reuse is allowed with notice.

[^code]: Repository README (read via raw.githubusercontent.com HEAD/main)
