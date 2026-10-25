---
type: "Technique"
title: "Graph instruction tuning"
description: "Fine-tune an LLM on graph task instructions, with a graph encoder and a trained projector, so it answers graph questions directly."
tags: ["technique", "fine-tuning", "instruction-tuning", "graph"]
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: graphgpt
    resource: "https://arxiv.org/html/2310.13023"
    title: "GraphGPT, arXiv HTML full text (read in part)"
  - id: graphgpt-abs
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
- GraphGPT tunes an LLM on graph instructions with a text-graph grounding step and a two-stage process, so it generalises zero-shot across graph datasets and tasks.[^graphgpt]
- The graph encoder and the LLM stay frozen; only a lightweight graph-text alignment projector is trained.[^graphgpt]
- MuseGraph and GraphWiz use instruction tuning on graph mining and graph problem data.[^musegraph][^graphwiz]

# Method
- Stage 1: self-supervised graph matching on unlabeled graph structure, used as the instruction, trains the projector.[^graphgpt]
- Stage 2: task instructions tune the model for downstream graph tasks; the projector is the only trained part in the text read so far.[^graphgpt]
- Training data is built from graph tasks with explicit instructions (abstract level).[^musegraph][^graphwiz]

# Results
- Not located in the parsed GraphGPT text: the extraction gave only fragments of result tables (for example PubMed and arXiv dataset rows), so no headline accuracy is recorded here.[^graphgpt]
- MuseGraph and GraphWiz: abstract only, no numbers.[^musegraph][^graphwiz]

# Relevance to neocortex
- Needs an LLM at training time, which neocortex cannot host. The projector idea (train a small adapter to map graph encodings into a text model's space) is out of scope for the CPU-only engine.
- Only the instruction-data design could inform an evaluation set. Papers: [GraphGPT](/papers/integration/graphgpt.md), [MuseGraph](/papers/integration/musegraph.md), [GraphWiz](/papers/integration/graphwiz.md).

[^graphgpt]: GraphGPT, arXiv HTML full text, https://arxiv.org/html/2310.13023 (read in part)
[^musegraph]: MuseGraph, arXiv abstract
[^graphwiz]: GraphWiz, arXiv abstract
