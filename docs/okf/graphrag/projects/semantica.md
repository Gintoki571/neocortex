---
type: "Open-source Project"
title: "Semantica"
description: "Open-source semantic layer and GraphRAG framework between raw corpora and LLMs."
resource: "https://github.com/Hawksight-AI/semantica"
tags: ["open-source", "semantic-layer", "graphrag", "ontology"]
license: "MIT"
language: "Python"
llm_required: none
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/Hawksight-AI/semantica"
    title: "Repository README (read via raw.githubusercontent.com)"
---
# Summary
- Deterministic, no-LLM graph construction and reasoning, with an ontology layer and explainable decisions.

# Method
- Graph storage backends: RDF stores (Oxigraph, Blazegraph, Jena, RDF4J) and labeled property graphs (Neo4j, FalkorDB, AGE, Neptune), plus vector stores.[^code]
- Ships an MCP server, a CLI, a REST API and integrations with Agno, CrewAI and LangChain.[^code]

# Results
- Not measured in this pass.

# Relevance to neocortex
- Closest in spirit to neocortex's no-LLM constraint, but a much broader stack. Mine its ontology and explainability ideas; do not adopt the stack.

[^code]: Repository README and GitHub metadata
