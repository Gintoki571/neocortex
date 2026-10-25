---
type: "Technique"
title: "KG path beam search with LLM pruning"
description: "Beam search over KG relation paths, with an LLM choosing which candidates survive at each hop; depth D and width N bound the search."
tags: ["technique", "beam-search", "multi-hop", "knowledge-graph", "hits-at-1"]
llm_required: retrieval
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: tog
    resource: "https://arxiv.org/html/2307.07697"
    title: "Think-on-Graph, arXiv HTML full text"
  - id: rog
    resource: "https://arxiv.org/html/2310.01061"
    title: "Reasoning on Graphs, arXiv HTML full text"
---
# Summary
- Expand candidate relations and entities from the current frontier, keep the top N paths, and repeat up to depth D; ToG's defaults are N = D = 3.[^tog]
- Relation-path planning followed by beam-searched path retrieval is the RoG variant; it keeps K = 3 relation paths.[^rog]

# Method
- Each step: relation search from the current entities, LLM pruning of relations or triples to width N, entity search for the survivors, then an LLM check of whether the paths answer the question.[^tog]
- ToG-R prunes relations only and runs entity search after; the paper reports mixed results between the two variants.[^tog]
- Metric used by both papers: Hits@1 (exact match for ToG, top-1 correctness for RoG).[^tog][^rog]

# Results
- ToG with GPT-4 reaches 67.6 Hits@1 on CWQ and 82.6 on WebQSP; with ChatGPT it reaches 57.1 and 76.2 (Table 1).[^tog]
- RoG reaches 85.7 Hits@1 on WebQSP and 62.6 on CWQ.[^rog]
- ToG's ablation shows performance increases with depth and width from 1 to 4.[^tog]

# Relevance to neocortex
- The engine can run the search skeleton (frontier, width, depth, stop) with its own non-LLM scorer, since neocortex already has a relation beam. The LLM choices (which candidates survive, whether to stop) are agent-side: the engine returns candidates, the caller returns indices.
- Cost: one agent call per hop. The three-hop weak spot is the target, so measure on MetaQA-3 (see [MetaQA-3hop](/benchmarks/metaqa-3hop.md)) before tuning width.
- Papers: [Think-on-Graph](/papers/integration/think-on-graph.md), [Reasoning on Graphs](/papers/integration/reasoning-on-graphs.md), [KELP](/papers/integration/kg-enhanced-llm-path-selection.md) (replaces LLM path judgment with a learned path score).

[^tog]: Think-on-Graph, arXiv HTML full text
[^rog]: Reasoning on Graphs, arXiv HTML full text
