---
type: "Paper"
title: "MuseGraph: Graph-oriented Instruction Tuning of Large Language Models for Generic Graph Mining"
description: "One foundation model that combines GNN and LLM strengths to handle diverse graph mining tasks and datasets."
resource: "https://arxiv.org/abs/2403.04780"
tags: ["fine-tuning", "path-level", "graph-mining"]
year: 2024
venue: "arXiv 2024 (per README)"
llm_required: indexing
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2403.04780"
    title: "arXiv 2403.04780"
---
# Summary
- Proposes MuseGraph, a single model that integrates GNN and LLM strengths for generic graph mining.[^paper]
- Aims to train one model for many graph tasks and datasets instead of re-training GNNs per task.[^paper]

# Method
- Graph-oriented instruction tuning of an LLM, with GNN components (abstract level; details not read).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- Needs LLM training and inference, so it is agent-side only. Watch for graph-to-prompt compression ideas, linked to [graph instruction tuning](/techniques/graph-instruction-tuning.md).

[^paper]: arXiv 2403.04780
