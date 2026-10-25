---
type: "Open-source Project"
title: "DIGIMON (repository)"
description: "Code repository of the DIGIMON unified graph-RAG framework and its comparison harness; the study is in the benchmark entry."
resource: "https://github.com/JayLZhou/GraphRAG"
tags: ["open-source", "graphrag", "framework", "harness"]
language: "Python"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/JayLZhou/GraphRAG"
    title: "Repository README (read via raw.githubusercontent.com HEAD/main)"
  - id: api
    resource: "https://api.github.com/repos/JayLZhou/GraphRAG"
    title: "GitHub metadata (no licence reported by the API, 1,541 stars, last push 2025-07-01)"
---
# Summary
- Supported LLM backends: OpenAI models (gpt-4, gpt-3.5-turbo) and locally deployed models through Ollama and LlamaFactory; a config key api_type set to open_llm selects local models.[^code]
- The README lists LGraphRAG (local search in GraphRAG) among the methods; the unified framework is described in the benchmark entry.[^code]

# Method
- LLM calls run the indexing and answering stages; the reusable part is the harness.

# Results
- Not measured in this pass beyond the paper's numbers recorded in the benchmark entry.

# Relevance to neocortex
- Use as the comparison harness for neocortex against graph RAG variants. No licence is reported by GitHub, so read the repo's LICENSE file before copying code. Its LLM requirement means only the retrieval stage can run without one, after an adapter is written.[^code]

[^code]: Repository README (read via raw.githubusercontent.com HEAD/main)
