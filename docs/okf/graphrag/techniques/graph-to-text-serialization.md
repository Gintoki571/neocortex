---
type: "Technique"
title: "Graph-to-text serialization"
description: "Serialize a graph neighbourhood or problem into text or token sequences an LLM can read, with templates that fix the order of nodes and hops."
tags: ["technique", "graph-to-text", "serialization", "prompting", "node-sequence"]
llm_required: retrieval
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: llaga
    resource: "https://arxiv.org/html/2402.08170"
    title: "LLaGA, arXiv HTML full text"
  - id: lang
    resource: "https://arxiv.org/abs/2308.07134"
    title: "Language is All a Graph Needs, arXiv abstract"
---
# Summary
- LLaGA reorganizes graph data into node sequences instead of natural-language descriptions, to avoid ambiguous text.[^llaga]
- Sequences are built with node-level templates for each central node, then projected into LLM token embeddings.[^llaga]
- Language is All a Graph Needs argues that graph problems can be cast into the generative language-model framework; the abstract is the only part read.[^lang]

# Method
- Sample a fixed number of neighbours per hop (n1, n2, ...), traverse the computational tree level by level, and emit a fixed-length node sequence.[^llaga]
- The serialization is deterministic; only the model reading it is learned in LLaGA's case.[^llaga]

# Results
- LLaGA tunes its projector on three tasks: node classification, link prediction and node description, with the same templates reused for link prediction.[^llaga]. Result numbers were not transcribed in this pass.
- No numbers recorded for the serialization paper.[^lang]

# Relevance to neocortex
- Deterministic and cheap, so it fits neocortex: the recall response could include a compact rendering of entity neighbourhoods and relation paths (hop by hop, with valid_from and valid_to), so a caller LLM reads structure without an LLM call inside the engine.
- The per-hop sample size is the same knob as neocortex's beam width; reuse the same sampling in the rendering.
- Papers: [LLaGA](/papers/integration/llaga.md), [Language is All a Graph Needs](/papers/integration/language-is-all-a-graph-needs.md).

[^llaga]: LLaGA, arXiv HTML full text
[^lang]: Language is All a Graph Needs, arXiv abstract
