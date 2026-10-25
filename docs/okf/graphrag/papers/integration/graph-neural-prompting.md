---
type: "Paper"
title: "Graph Neural Prompting with Large Language Models"
description: "Plug-and-play graph neural prompting that feeds KG knowledge into a frozen pretrained LLM."
resource: "https://arxiv.org/abs/2309.15427"
tags: ["subgraph-level", "prompting", "knowledge-graph"]
year: 2023
venue: "AAAI 2024 (per README)"
llm_required: indexing
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2309.15427"
    title: "arXiv 2309.15427"
---
# Summary
- Argues that joint KG training is too costly for large LLMs and proposes a way to enhance pretrained LLMs with grounded knowledge from KGs.[^paper]

# Method
- Graph neural prompting: a GNN encodes a KG subgraph into prompts (abstract level; architecture not read in full).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- Needs an LLM at read time; the prompt is built from a GNN over a subgraph. The GNN part could run on CPU, but neocortex has no LLM in-engine. Watch.

[^paper]: arXiv 2309.15427
