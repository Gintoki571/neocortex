---
type: "Paper"
title: "LLaGA: Large Language and Graph Assistant"
description: "LLaGA: a Vicuna-7B assistant that turns each node's fixed-size sampled neighbourhood into a template-ordered token sequence (ND and HO templates) and aligns it with a projector, for node classification and link prediction (ICML 2024)."
resource: "https://arxiv.org/abs/2402.08170"
tags: ["graph-to-llm", "node-classification", "link-prediction", "computational-tree", "vicuna"]
year: 2024
venue: "ICML 2024"
llm_required: indexing
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2402.08170"
    title: "arXiv abstract page"
  - id: html
    resource: "https://arxiv.org/html/2402.08170"
    title: "arXiv HTML full text (Sections 2-3, Tables 1-2, 5-6)"
---
# Summary
- Graph-to-LLM translation: each node becomes a fixed-shape sampled computational tree, linearised into node-embedding tokens, so a frozen-architecture LLM reads graph structure without changing its parameters.[^html]
- Two node-level templates: Neighborhood Detail (ND) describes the node and its neighbourhood, and Hop-Ordered (HO) lists neighbours hop by hop.[^html]
- Alignment tuning trains a projector, then the LLM is instruction-tuned for node classification, link prediction and node description.[^html]

# Method
- Sampled computational tree with a fixed neighbour sample size per hop (n1, n2, ...); the default text encoder is SimTeG and the LLM is Vicuna-7B-v1.5-16K.[^html]
- Datasets: ogbn-arxiv, ogbn-products, PubMed and Cora (citation and e-commerce graphs).[^html]
- Baselines: GNNs in three groups (single-focus, task-expert and classification-expert) including GCN, GraphSAGE, GAT, SGC, SAGN and NodeFormer.[^html]

# Results
- Table 1, single-focus setting, LLaGA-HO-7B node classification accuracy (Arxiv / Products / PubMed / Cora): 76.66 / 84.67 / 95.03 / 89.22; GraphSAGE 76.29 / 82.87 / 94.87 / 88.89.[^html]
- Table 1, single-focus link prediction accuracy (Arxiv / Products / PubMed / Cora): LLaGA-HO-7B 94.15 / 95.56 / 89.18 / 86.82; LLaGA-ND-7B 88.86 / 91.24 / 97.36 / 83.79 (ND, the best on PubMed); GraphSAGE 91.64 / 94.96 / 90.64 / 79.15.[^html]
- The paper's zero-shot and template ablations (Tables 5-6) are not summarised here.

# Relevance to neocortex
- The sampled-tree template is a fixed-budget neighbourhood serialisation, which is the same budget idea as neocortex's ego-graph pruning. The LLM part is not usable inside the engine.
- Skip the model. Keep the idea of a hop-ordered, budgeted neighbourhood text as a candidate format for what neocortex returns to the calling agent.
- Node classification and link prediction are not neocortex's tasks, so the numbers are a weak signal only.

[^html]: arXiv HTML full text (Sections 2-3, Tables 1-2, 5-6)
