---
type: "Paper"
title: "Dual Reasoning: A GNN-LLM Collaborative Framework for Knowledge Graph Question Answering"
description: "Dual-process framework pairing a GNN-based external reasoner on the KG with an LLM. Listed in the README as Explore then Determine."
resource: "https://arxiv.org/abs/2406.01145"
tags: ["gnn", "knowledge-graph", "dual-process", "kgqa"]
year: 2024
venue: "arXiv 2024 (per README)"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2406.01145"
    title: "arXiv 2406.01145 (title changed from the README name)"
---
# Summary
- DualR pairs an external system that does explicit graph learning with the LLM's implicit reasoning.[^paper]
- The README lists this paper under its earlier title, Explore then Determine.[^paper]

# Method
- GNN-based explicit reasoning over the KG, feeding reasoning chains to the LLM (abstract level).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- A GNN over the KG could run on CPU, but the LLM step is agent-side. Watch.

[^paper]: arXiv 2406.01145 (title changed from the README name)
