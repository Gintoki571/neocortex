---
type: "Paper"
title: "GraphGPT: Graph Instruction Tuning for Large Language Models"
description: "Graph-oriented LLM that aims at zero-shot generalization across graph datasets and tasks through graph instruction tuning."
resource: "https://arxiv.org/abs/2310.13023"
tags: ["fine-tuning", "instruction-tuning", "node-level"]
year: 2023
venue: "SIGIR 2024 (per README)"
llm_required: indexing
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2310.13023"
    title: "arXiv 2310.13023"
---
# Summary
- Targets graph tasks where GNNs need task-specific retraining; proposes an LLM that generalizes zero-shot across datasets and tasks.[^paper]
- The abstract motivates self-supervised, label-free generalization for graph models; it does not establish a retrieval method.[^paper]

# Method
- Graph instruction tuning of an LLM (abstract only; the training recipe was not read).[^paper]

# Results
- Not read beyond the abstract, so no numbers are recorded here.[^paper]

# Relevance to neocortex
- Requires fine-tuning an LLM, so it cannot run inside neocortex. Only the idea of pairing a graph encoder with an LLM is transferable, and only on the agent side. Watch.

[^paper]: arXiv 2310.13023
