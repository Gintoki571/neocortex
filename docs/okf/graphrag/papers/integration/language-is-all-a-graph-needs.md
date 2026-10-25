---
type: "Paper"
title: "Language is All a Graph Needs"
description: "Casts graph problems as text so a generative language model can solve them within one framework."
resource: "https://arxiv.org/abs/2308.07134"
tags: ["graph-to-text", "subgraph-level", "serialization"]
year: 2023
venue: "EACL 2024 Findings"
llm_required: indexing
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2308.07134"
    title: "arXiv 2308.07134"
---
# Summary
- Argues that graph problems can be brought into the generative language-model framework, since language describes complex structure well.[^paper]
- Notes that existing work on this integration is still limited.[^paper]

# Method
- Serialize graph inputs as text for a generative LM (abstract level).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- Text serialization of subgraphs is a reusable idea for the agent side. Link: [graph-to-text serialization](/techniques/graph-to-text-serialization.md).

[^paper]: arXiv 2308.07134
