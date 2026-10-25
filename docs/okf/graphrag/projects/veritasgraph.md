---
type: "Open-source Project"
title: "VeritasGraph"
description: "Local GraphRAG pipeline combining tree-search document structure with KG reasoning and source attribution."
resource: "https://github.com/bibinprathap/VeritasGraph"
tags: ["open-source", "local", "ollama", "attribution"]
license: "MIT"
language: "Python"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/bibinprathap/VeritasGraph"
    title: "Repository README (read via raw.githubusercontent.com)"
---
# Summary
- Ingest and query need a local Ollama; the README says it runs without a GPU via a quick start.

# Method
- Pairs tree search over document structure with KG traversal for deterministic, reproducible retrieval.[^code]
- Exposes an MCP server (python -m veritasgraph_mcp).[^code]

# Results
- Not measured in this pass.

# Relevance to neocortex
- Source attribution is a feature neocortex lacks. Read the attribution design before reuse.

[^code]: Repository README and GitHub metadata
