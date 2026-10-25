---
type: "Paper"
title: "GraphWiz: An Instruction-Following Language Model for Graph Computational Problems"
description: "GraphInstruct (nine graph problem types with explicit reasoning paths) and GraphWiz, a 7B to 13B LLM tuned on it with DPO; it beats GPT-4 on the average over the test set (KDD 2024)."
resource: "https://arxiv.org/abs/2402.16029"
tags: ["graph-reasoning", "instruction-tuning", "dpo", "graphinstruct", "algorithms"]
year: 2024
venue: "KDD 2024"
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2402.16029"
    title: "arXiv abstract page (fetched)"
  - id: html
    resource: "https://arxiv.org/html/2402.16029"
    title: "arXiv HTML full text (Sections 3-4, Tables 1, 4)"
---
# Summary
- GraphInstruct: instruction data for nine graph computation tasks (cycle, connectivity, bipartite, topology, shortest path, triangle, max flow, Hamiltonian path, subgraph matching), each with an explicit reasoning path.[^html]
- GraphWiz: an open model (LLaMA 2-7B, LLaMA 2-13B, Mistral-7B) tuned on GraphInstruct, with a second DPO phase on reasoning preferences.[^html]
- Reported average accuracy on the GraphInstruct test set: GPT-4 2-shot 43.81, GraphWiz-DPO on LLaMA 2-7B 65.00.[^html]

# Method
- Phase 1: mixed-task instruction tuning. Phase 2: DPO alignment, with preference pairs of correct and incorrect reasoning paths.[^html]
- Training: learning rate 2e-5, 3 epochs, max length 2048, batch size 16 or 32, alpaca instruction format, DeepSpeed; GraphInstruct answers generated with 2-shot CoT prompting.[^html]
- Evaluation: test accuracy per task, transfer to NLGraph (cycle detection, max flow, shortest path), and a comparison with GCN, GIN and GAT on the simpler binary tasks.[^html]

# Results
- Table 4 average accuracy: GPT-4 zero-shot 27.67; GPT-4 2-shot 43.81; Mistral-7B naive SFT 46.56, GraphWiz 53.75, GraphWiz-DPO 58.22; LLaMA 2-7B GraphWiz-DPO 65.00; LLaMA 2-13B GraphWiz 57.39 and GraphWiz-DPO 63.89.[^html]
- Graph neural networks (GCN 68.00, GIN 66.50, GAT 66.25 on average) are competitive on the binary tasks but the paper notes they fall behind on multi-step numerical tasks.[^html]

# Relevance to neocortex
- Skip for the engine: it is a 7B to 13B model used to write reasoning over a graph in text, which needs a model at read time. neocortex's path search is an algorithm, so it already does what these tasks ask for, without an LLM.
- Useful as a test set: the nine task types (cycle, connectivity, shortest path, max flow) are exact graph computations that neocortex's graph layer could be checked against without any LLM, by comparing against a reference algorithm.[^html]
- The GNN comparison supports the same point: for path and connectivity questions, an explicit graph algorithm beats a learned model.

[^paper]: arXiv abstract page
[^html]: arXiv HTML full text (Sections 3-4, Tables 1, 4)
