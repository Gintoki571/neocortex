---
type: "Open-source Project"
title: "GraphRAG-SDK (FalkorDB)"
description: "Toolkit for GraphRAG on FalkorDB, with ontology-constrained extraction, multi-path retrieval and a retrieval trail for grounding checks."
resource: "https://github.com/FalkorDB/GraphRAG-SDK"
tags: ["open-source", "graphrag", "ontology", "falkordb", "multi-tenant"]
license: "Apache-2.0"
language: "Python"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/FalkorDB/GraphRAG-SDK"
    title: "Repository README (read via raw.githubusercontent.com HEAD/main)"
  - id: api
    resource: "https://api.github.com/repos/FalkorDB/GraphRAG-SDK"
    title: "GitHub metadata (Apache-2.0, 1,027 stars, last push 2026-10-25)"
---
# Summary
- Pipeline: load and chunk into a lexical graph, extract and refine entities and relations, persist to FalkorDB, then multi-path retrieval (vector similarity plus relationship traversal).[^code]
- The ontology constrains what can be extracted, so the graph stores typed, checkable facts.[^code]
- Entity mentions are linked to source chunks with MENTIONED_IN edges; return_context=True returns the retrieval trail so an application can validate claims or refuse to generate when evidence is insufficient.[^code]
- Install: Python 3.10+, pip install graphrag-sdk[litellm] (LiteLLM for model access); a pdf extra adds PDF ingestion. Multi-tenant mode is listed among its features.[^code]

# Method
- Extraction and refinement use an LLM through LiteLLM; retrieval combines vector and graph traversal; persistence is FalkorDB.[^code]

# Results
- The README reports benchmark-leading accuracy and reduced hallucinations; no numbers were read, so these are unverified claims.[^code]

# Relevance to neocortex
- Two ideas to borrow: the ontology-constrained extraction (typed relations with a fixed vocabulary, which would help neocortex's relation and entity resolution) and the retrieval trail (return the evidence chain with each answer so the caller can check it). Both are deterministic in principle; the extraction step itself needs an LLM and so is write-side on the agent.[^code]

[^code]: Repository README (read via raw.githubusercontent.com HEAD/main)
