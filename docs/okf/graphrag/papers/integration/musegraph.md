---
type: "Paper"
title: "Graph-oriented Instruction Tuning of Large Language Models for Generic Graph Mining (MuseGraph)"
description: "MuseGraph: one LLM (LLaMA3-8B default) tuned on compact text graph descriptions with GPT-4 chain-of-thought instruction packages, for node classification, link prediction and graph-to-text across datasets (arXiv 2024)."
resource: "https://arxiv.org/abs/2403.04780"
tags: ["graph-instruction-tuning", "graph-description", "chain-of-thought", "node-classification", "link-prediction"]
year: 2024
venue: "arXiv 2024 (per README)"
llm_required: indexing
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2403.04780"
    title: "arXiv abstract page (v3)"
  - id: html
    resource: "https://arxiv.org/html/2403.04780"
    title: "arXiv HTML v3 full text (Sections III-IV, Tables I-VIII)"
---
# Summary
- Compact graph description: a graph is textualised through neighbours and walks so that key structure fits the LLM's token budget.[^html]
- Diverse instruction generation: chain-of-thought instruction packages distilled from GPT-4 for each task and dataset.[^html]
- Graph-aware instruction tuning: one model is tuned on a mixture of task packages (node classification, link prediction, graph-to-text) with LLaMA-Factory.[^html]

# Method
- Default foundation model LLaMA3-8B; other backbones tested: Qwen2-7B-Instruct, LLaMA1-7B and LLaMA2-7B.[^html]
- Compact description combines a node's attributes with its one-hop neighbours and walk-based context, with ablations on neighbour inclusion (Figure 7).[^html]
- Due to GPU limits, some GNN+LLM comparison models were run from public checkpoints (HiGPT, GraphGPT).[^html]

# Results
- Table V (micro-F1 / macro-F1), MuseGraph LLaMA3-8B: IMDB 76.57 / 73.78; Freebase 78.01 / 59.62; Cora 86.83 / 84.74; Arxiv 67.80 / 47.71. Baselines on the same columns: GCN IMDB 64.23 / 58.43, Cora 82.87 / 81.29, Arxiv 71.98 / 51.26; TAPE Cora 88.19 / 87.16, Arxiv 74.36 / 55.74.[^html]
- So on the two homogeneous datasets MuseGraph is below TAPE and, on Arxiv, below GCN; the claim of outperforming GNNs holds for the heterogeneous IMDB and Freebase columns.[^html]
- Table VI (link prediction AUC), MuseGraph LLaMA3-8B: Yelp 80.07, MIMIC-III 69.92; GraphSAGE 68.85 and 53.40 on the same columns.[^html]

# Relevance to neocortex
- Skip the model: a fine-tuned 8B LLM is needed at read time, which neocortex's engine does not allow.
- The compact description (neighbours plus walks inside a token budget) is the same budget problem neocortex solves with ego-graph pruning. The results do not show that the description wins on its own.
- The paper's Arxiv and Cora rows show a learned text model losing to plain GCN on Arxiv, which is a caution for any LLM-based graph claim.[^html]

[^html]: arXiv HTML v3 full text (Sections III-IV, Tables V-VIII)
