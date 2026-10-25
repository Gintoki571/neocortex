---
type: "Paper"
title: "GraphWiz: An Instruction-Following Language Model for Graph Problems"
description: "An open instruction-tuned LLM for graph problems, trained on explicit reasoning-path data (GraphInstruct)."
resource: "https://arxiv.org/abs/2402.16029"
tags: ["fine-tuning", "subgraph-level", "instruction-data", "graph-problems"]
year: 2024
venue: "KDD 2024"
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2402.16029"
    title: "arXiv 2402.16029"
---
# Summary
- Introduces GraphInstruct, an instruction-tuning dataset for graph problems with explicit reasoning paths.[^paper]
- Builds GraphWiz, an open model that solves many graph problem types and writes out its reasoning.[^paper]

# Method
- Instruction tuning on GraphInstruct; the abstract is cut before the full training recipe, so no further detail is recorded.[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- Solves algorithmic graph puzzles, not memory retrieval. Skip for neocortex; the dataset idea of explicit reasoning paths could inform evaluation.

[^paper]: arXiv 2402.16029
