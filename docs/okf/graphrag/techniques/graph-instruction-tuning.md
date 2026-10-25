---
type: "Technique"
title: "Graph instruction tuning"
description: "Fine-tune an LLM on graph task instructions so it answers graph questions directly."
tags: ["technique", "fine-tuning", "instruction-tuning", "graph"]
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: graphgpt
    resource: "https://arxiv.org/abs/2310.13023"
    title: "GraphGPT, arXiv abstract"
  - id: musegraph
    resource: "https://arxiv.org/abs/2403.04780"
    title: "MuseGraph, arXiv abstract"
  - id: graphwiz
    resource: "https://arxiv.org/abs/2402.16029"
    title: "GraphWiz, arXiv abstract"
---
# Summary
- GraphGPT tunes an LLM on graph instructions to generalize zero-shot across graph datasets and tasks.[^graphgpt]
- MuseGraph and GraphWiz use instruction tuning on graph mining and graph problem data.[^musegraph][^graphwiz]

# Method
- Training data is built from graph tasks with explicit instructions (abstract level).[^graphgpt][^musegraph][^graphwiz]

# Results
- Not read beyond the abstracts.[^graphgpt]

# Relevance to neocortex
- Needs an LLM at training time, which neocortex cannot host. Only the instruction-data design could inform an evaluation set. Papers: [GraphGPT](/papers/integration/graphgpt.md), [MuseGraph](/papers/integration/musegraph.md), [GraphWiz](/papers/integration/graphwiz.md).

[^graphgpt]: GraphGPT, arXiv abstract
[^musegraph]: MuseGraph, arXiv abstract
[^graphwiz]: GraphWiz, arXiv abstract
