---
type: "Paper"
title: "Dual Reasoning: A GNN-LLM Collaborative Framework for Knowledge Graph Question Answering"
description: "DualR: a GNN-based explorer on the KG that backtracks reasoning chains, which a frozen LLM reads as a multiple-choice prompt to pick the answer (README name: Explore then Determine)."
resource: "https://arxiv.org/abs/2406.01145"
tags: ["gnn", "knowledge-graph", "dual-process", "kgqa", "reasoning-chains", "metaqa"]
year: 2024
venue: "arXiv 2024 (README: Explore then Determine; v2 retitled Dual Reasoning)"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2406.01145"
    title: "arXiv abstract page (v2, submitted 3 Jun 2024, revised 6 Mar 2025)"
  - id: html
    resource: "https://arxiv.org/html/2406.01145"
    title: "arXiv HTML v2 full text (Sections 3-4, Tables 2, 3, 8; Appendix B)"
---
# Summary
- Two-tier reasoning: an LLM-empowered GNN explores the KG from the topic entity ("System 2"), and a frozen LLM determines the answer from a knowledge-enhanced multiple-choice prompt built from the explored chains.[^html]
- The README names the arXiv entry "Explore then Determine"; the fetched v2 is titled "Dual Reasoning". Same arXiv ID, same method.[^paper][^html]
- Evaluated on WebQSP, CWQ and MetaQA 1/2/3-hop, with Hits@1 as the metric.[^html]

# Method
- Semantic-aware exploration: candidate entities are expanded from the topic entity over L steps, with question-conditioned attention weights on edges used to prune irrelevant candidates. Hyperparameters per dataset (Table 8): L = 2 / 4 / 1 / 2 / 3 and K = 200 / 200 / 40 / 60 / 100 for WebQSP / CWQ / MetaQA-1/2/3.[^html]
- Reasoning chains: the top-N candidates by GNN probability are kept as reference answers, and a greedy backtrack traces each to the topic entity. The number of reference answers is three in the default setting (Table 13).[^html]
- Pre-training: GNN dimension 256, learning rate 1e-4, batch size 20, 3 layers.[^html]
- The LLM stays frozen in the answer-determination step; DualR-Llama2-13B, DualR-ChatGPT and DualR-GPT-4 are the three LLM backends.[^html]

# Results
- Table 2 (Hits@1 %), MetaQA-1 / 2 / 3: DualR-Llama2-13B 97.9 / 99.1 / 99.6; DualR-ChatGPT 98.1 / 99.7 / 99.7; DualR-GPT-4 98.3 / 99.9 / 99.9.[^html]
- Table 2, WebQSP / CWQ: DualR-Llama2-13B 78.3 / 58.0; DualR-ChatGPT 82.8 / 62.0; DualR-GPT-4 87.6 / 73.6.[^html]
- KG-only baselines in the same table: UniKGQA 97.5 / 99.0 / 99.1 on MetaQA-1/2/3; NSM 97.1 / 99.9 / 98.9.[^html]

# Relevance to neocortex
- The 3-hop chain search and reference-answer backtracking map onto neocortex's path beam. The KG-only baselines (NSM, UniKGQA) reach about 99 on MetaQA-3 without an LLM, which gives a concrete target for neocortex's offline MetaQA run (see [MetaQA-3](/benchmarks/metaqa-3hop.md)).[^html]
- The GNN explorer is trained, so it is not LLM-free in the usual sense, but the exploration step itself needs no LLM. A neocortex version would replace the GNN with its own relation scoring.
- Answer determination needs a frozen LLM, which is agent-side only. Watch.

[^paper]: arXiv abstract page (v2)
[^html]: arXiv HTML v2 full text (Sections 3-4, Tables 2, 3, 8, 13; Appendix B)
