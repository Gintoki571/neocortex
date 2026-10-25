---
type: "Open-source Project"
title: "Code-Graph-RAG"
description: "Graph RAG over multi-language codebases: Tree-sitter parse to a knowledge graph, queried in natural language and edited via MCP."
resource: "https://github.com/vitali87/code-graph-rag"
tags: ["open-source", "code-graph", "tree-sitter", "mcp"]
license: "MIT"
language: "Python"
llm_required: retrieval
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/vitali87/code-graph-rag"
    title: "Repository README (read via raw.githubusercontent.com)"
---
# Summary
- Graph RAG over multi-language codebases: Tree-sitter parse to a knowledge graph, queried in natural language and edited via MCP.

# Method
- Stores the graph in an embedded SQLite file by default; Memgraph and Neo4j are opt-in backends.[^code]
- Natural-language questions are turned into Cypher queries by an LLM.[^code]

# Results
- Not measured in this pass.

# Relevance to neocortex
- Its embedded SQLite graph store is the same storage style neocortex uses. The NL-to-Cypher step needs an LLM, so it is agent side.

[^code]: Repository README and GitHub metadata
