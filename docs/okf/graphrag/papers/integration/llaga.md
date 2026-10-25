---
type: "Paper"
title: "LLaGA: Large Language and Graph Assistant"
description: "Adapts graph neighborhoods into an LLM-compatible input format so a general LLM can work on graph data."
resource: "https://arxiv.org/abs/2402.08170"
tags: ["fine-tuning", "subgraph-level", "graph-to-text"]
year: 2024
venue: "ICML 2024"
llm_required: indexing
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2402.08170"
    title: "arXiv 2402.08170"
---
# Summary
- LLaGA keeps the general-purpose LLM and adapts graph-structured data into a format the LLM can take as input.[^paper]
- It targets the difficulty of translating graph structure into language.[^paper]

# Method
- Subgraph-level graph-to-sequence adaptation feeding an LLM (abstract level; template details not read).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- Needs LLM tuning and inference. The serialization idea is relevant to how the agent could send neighborhoods to neocortex. Link: [graph-to-text serialization](/techniques/graph-to-text-serialization.md).

[^paper]: arXiv 2402.08170
