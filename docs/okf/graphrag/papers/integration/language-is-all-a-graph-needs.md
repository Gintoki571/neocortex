---
type: "Paper"
title: "Language is All a Graph Needs"
description: "InstructGLM: a generative LLM (Flan-T5 or LoRA-tuned LLaMA) instruction-tuned on text serialisations of citation graphs for node classification, with a link-prediction auxiliary task (EACL 2024 Findings)."
resource: "https://arxiv.org/abs/2308.07134"
tags: ["graph-language-model", "instruction-tuning", "node-classification", "ogbn-arxiv", "cora", "pubmed"]
year: 2023
venue: "EACL 2024 Findings"
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2308.07134"
    title: "arXiv abstract page"
  - id: html
    resource: "https://arxiv.org/html/2308.07134"
    title: "arXiv HTML full text (Sections 3-4, Tables 1-4)"
---
# Summary
- InstructGLM casts node classification as text generation: each node is serialised with its title, abstract and neighbour text into an instruction, and the LM generates the label.[^html]
- Multi-task, multi-prompt instruction tuning, with self-supervised link prediction as an auxiliary task.[^html]
- Datasets: ogbn-arxiv (54/18/28 split), Cora and PubMed (raw text of papers as node features).[^html]

# Method
- Instruction prompts describe the graph structure in text (neighbours and their labels or titles) so the model reads structure without a GNN.[^html]
- Backbones: Flan-T5-base, Flan-T5-large and LLaMA-7B (LoRA). Training uses the generative instruction-tuning loss on ground-truth labels.[^html]
- Link prediction is trained as an auxiliary self-supervised task on the same prompts.[^html]

# Results
- Table 1 (ogbn-arxiv accuracy, OGB and GIANT columns): InstructGLM with LLaMA-7B 75.70 / 76.42, Flan-T5-large 74.67 / 74.80, Flan-T5-base 73.51 / 74.45; the strongest GNN rows are DRGAT at 74.16 / 76.11 and AGDN at 73.75 / 76.02.[^html]
- Table 2 (Cora and PubMed accuracy): the table lists GNN and graph-transformer baselines up to Snowball-V3 at 89.59 (Cora) and 91.44 (PubMed); the InstructGLM rows are reported in the same table.[^html]
- Table 4 (PubMed, 60 training nodes, low label ratio): Flan-T5-large 88.93 / 94.62, Flan-T5-base 90.77 / 94.45, LLaMA-7B 87.08 / 93.84 (two columns per row, as printed).[^html]

# Relevance to neocortex
- Skip for the engine: an instruction-tuned 0.2B to 7B generative LM at training and inference. neocortex's constraints (no LLM in the engine, CPU) exclude it.
- The text-serialisation of neighbourhoods is the one idea neocortex can reuse, on the agent side, when it hands recalled relations to the reader.
- Node classification on citation graphs is not neocortex's task; the paper gives no multi-hop numbers.

[^html]: arXiv HTML full text (Sections 3-4, Tables 1-4)
