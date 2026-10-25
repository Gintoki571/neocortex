---
type: "Open-source Project"
title: "ApeRAG"
description: "Production RAG platform combining Graph RAG, vector search and full-text search with AI agents and MCP support."
resource: "https://github.com/apecloud/ApeRAG"
tags: ["open-source", "platform", "graphrag", "mcp", "kubernetes"]
license: "Apache-2.0"
language: "Python"
llm_required: both
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/apecloud/ApeRAG"
    title: "Repository README (read via raw.githubusercontent.com)"
---
# Summary
- Production RAG platform combining Graph RAG, vector search and full-text search with AI agents and MCP support.

# Method
- Docker Compose deployment; the README describes a platform rather than a library.[^code]
- MCP support lets assistants query the knowledge base.[^code]

# Results
- Not measured in this pass.

# Relevance to neocortex
- A full service stack; too heavy for neocortex's single-file design. Skip, except as a reference for MCP exposure.

[^code]: Repository README and GitHub metadata
