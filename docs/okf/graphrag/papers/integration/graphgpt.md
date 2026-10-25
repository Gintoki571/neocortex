---
type: "Paper"
title: "GraphGPT: Graph Instruction Tuning for Large Language Models"
description: "Fine-tunes a Vicuna-7B LLM with graph instruction tuning (text-graph grounding plus CoT distillation) so one model handles node classification and link prediction across graph datasets."
resource: "https://arxiv.org/abs/2310.13023"
tags: ["fine-tuning", "instruction-tuning", "node-level", "graph-encoder", "zero-shot"]
year: 2023
venue: "SIGIR 2024 (per README; proceedings header confirms 47th SIGIR)"
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2310.13023"
    title: "arXiv abstract page (fetched)"
  - id: html
    resource: "https://arxiv.org/html/2310.13023"
    title: "arXiv HTML full text (Sections 3-4, Tables 1-6)"
---
# Summary
- Graph instruction tuning: a text-graph grounding step links graph structure to text, then a dual-stage instruction tuning (self-supervised, then task-specific) trains the LLM.[^html]
- A chain-of-thought distillation step adds reasoning traces to the instruction data for generalization.[^html]
- Base models are Vicuna-7B-v1.1 and v1.5; training runs 3 epochs with learning rate 2e-3, warmup ratio 3e-2 and maximum input length 2048.[^html]

# Method
- Structural encoding of graph neighbourhoods is turned into text-graph grounding pairs so the LLM sees node and neighbour context.[^html]
- Self-supervised instruction tuning, then task-specific instruction tuning for node classification and link prediction, with the instruction mixture studied in Tables 2-3.[^html]
- Evaluation: supervised and zero-shot node classification across citation datasets (Arxiv, PubMed, Cora) and link prediction on PubMed.[^html]

# Results
- Table 1, in-domain Arxiv-Arxiv accuracy: GraphGPT-7B-v1.5-stage2 0.7511 vs GLNN 0.6088 and GCN 0.5267.[^html]
- Table 1, zero-shot Arxiv-to-PubMed accuracy: GraphGPT-7B-v1.5-std 0.7011 vs vicuna-7B-v1.5 (no graph tuning) 0.6351 and GLNN 0.4298.[^html]
- Table 1 reports macro-F1 next to accuracy; the zero-shot Arxiv-to-Cora numbers are much lower (GraphGPT-7B-v1.5-std accuracy 0.1256), which the paper does not discuss in the abstract.[^html]

# Relevance to neocortex
- Requires fine-tuning a 7B LLM and an LLM at inference, so it cannot run inside neocortex. Skip.
- The reusable idea is the text-graph grounding: serialise a node's neighbourhood next to its text when building memory embeddings. That is cheap and LLM-free, but it is a different thing from GraphGPT's training.
- Its zero-shot Cora numbers are weak, so the claimed generalization is not general. Use with care.

[^paper]: arXiv abstract page
[^html]: arXiv HTML full text (Sections 3-4, Tables 1-6)
