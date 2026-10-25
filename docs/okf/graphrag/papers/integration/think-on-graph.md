---
type: "Paper"
title: "Think-on-Graph: Deep and Responsible Reasoning of Large Language Model on Knowledge Graph"
description: "LLM-guided beam search over a KG: the LLM explores relations and prunes candidate triples at each hop, with depth D and width N."
resource: "https://arxiv.org/abs/2307.07697"
tags: ["knowledge-graph", "beam-search", "multi-hop", "graph-enhanced-cot", "hits-at-1"]
year: 2023
venue: "ICLR 2024"
llm_required: retrieval
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2307.07697"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2307.07697"
    title: "arXiv HTML full text (Sections 2-4, Tables 1-3)"
---
# Summary
- ToG identifies initial entities, then iterates exploration (relation search, relation or triple pruning, entity search) and reasoning (the LLM judges whether the top-N paths answer the question) until it succeeds or reaches depth D_max.[^html]
- Relation-based variant ToG-R: relation search, relation prune and entity search per iteration, with the top-N relation chains kept; ToG-R does not prune triples.[^html]
- The paper's ablation varies depth and width from 1 to 4; the default setting is N = D_max = 3 for beam search.[^html]

# Method
- Beam search over KG paths. Each step: retrieve candidate relations for the current entities, let the LLM keep the top N, search their neighbours, and let the LLM keep the top N triples (ToG) or relation chains (ToG-R).[^html]
- Stopping: the LLM evaluates whether the current paths contain enough information; if not, another iteration runs, up to D_max.[^html]
- Metric: Hits@1 (exact match) for all datasets.[^html]
- Knowledge sources: Freebase for CWQ, WebQSP, GrailQA, SimpleQuestions and WebQuestions; Wikidata for QALD10-en, T-REx, Zero-Shot RE and Creak.[^html]

# Results
- Table 1, ToG with GPT-4 (Hits@1 %): CWQ 67.6, WebQSP 82.6, GrailQA 81.4, QALD10-en 53.8, SimpleQuestions 66.7, WebQuestions 57.9, T-REx 77.1, Zero-Shot RE 88.3, Creak 95.6. ToG-R with GPT-4: CWQ 69.5, WebQSP 81.9.[^html]
- Same table, chain-of-thought with ChatGPT: CWQ 38.8 and WebQSP 62.2; ToG with ChatGPT: CWQ 57.1 and WebQSP 76.2. The prior fine-tuned SOTA on CWQ is 70.4, above ToG with GPT-4 (67.6).[^html]
- Table 3 (1,000-question subsets of CWQ and WebQSP): ToG with Freebase 58.8 / 76.2 vs with Wikidata 54.9 / 68.6; the paper concludes that Freebase suits these datasets better.[^html]
- Ablation: performance increases with depth and width (Figure 3); the paper reports ToG outperforming ToG-R on most datasets, with exceptions such as CWQ under GPT-4 (ToG-R 69.5 vs ToG 67.6).[^html]

# Relevance to neocortex
- neocortex's multi-hop path beam has the same shape: entity start, expand relation facts, keep width N, stop at depth D. The depth and width knobs can run in the engine with its own non-LLM scorer; the LLM pruning steps are the part that must stay on the agent side.[^html]
- A concrete 2-call protocol would work: the engine returns the candidate predicates and neighbours for a set of entities; the calling agent returns the indices to keep. Each hop costs one agent call, so the three-hop case costs three.[^html]
- The depth-3, width-3 default is the setting to match when comparing with ToG; measure on MetaQA-3 ([MetaQA-3hop](/benchmarks/metaqa-3hop.md)) and report Hits@1 for a like-for-like check.[^html]
- Link: [KG path beam search](/techniques/kg-path-beam-search.md); related: [Reasoning on Graphs](/papers/integration/reasoning-on-graphs.md), [Plan-on-Graph](/papers/integration/plan-on-graph.md), [KELP](/papers/integration/kg-enhanced-llm-path-selection.md) (path scoring by latent semantic matching).

[^html]: arXiv HTML full text (Sections 2-4, Tables 1-3)
