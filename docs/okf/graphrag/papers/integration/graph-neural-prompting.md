---
type: "Paper"
title: "Graph Neural Prompting with Large Language Models"
description: "GNP: a plug-and-play GNN that encodes a retrieved ConceptNet or biomedical KG subgraph into soft prompts for a frozen or LoRA-tuned FLAN-T5 (AAAI 2024)."
resource: "https://arxiv.org/abs/2309.15427"
tags: ["graph-prompting", "subgraph-retrieval", "gnn", "conceptnet", "flan-t5"]
year: 2023
venue: "AAAI 2024 (per README)"
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2309.15427"
    title: "arXiv abstract page (fetched)"
  - id: html
    resource: "https://arxiv.org/html/2309.15427"
    title: "arXiv HTML full text (Sections 3-5, Tables 1-3, implementation details)"
---
# Summary
- Plug-and-play graph neural prompting: subgraphs of the KG are retrieved from the question and options, encoded by a GNN, and passed to the LLM as soft prompts, so the LLM needs no joint KG training.[^html]
- Components: GNN encoder, cross-modality pooling, domain projector, and a self-supervised link-prediction objective.[^paper]
- Knowledge graphs: ConceptNet (commonsense) and UMLS (biomedical). Datasets: OBQA, ARC, PIQA and RiddleSense (commonsense); PubMedQA and BioASQ (biomedical).[^html]

# Method
- Subgraph retrieval from entities in the question and options, then a GNN with 2 to 5 layers, 1 to 3 cross-modality pooling layers, hidden dimension 1024 (layer counts chosen per dataset).[^html]
- Training: learning rate 1e-4, batch size 8, 50 epochs; LLM settings are LLM Frozen (prompt only) and LLM Tuned (full fine-tuning or LoRA).[^html]
- Models: FLAN-T5 xlarge (3B) and xxlarge (11B), run on four V100 GPUs.[^html]

# Results
- Table 1, FLAN-T5 3B, LLM Frozen: LLM-only total 64.49; prompt tuning 65.95; GNP 74.36 (the table's +12.76% relative gain over prompt tuning). RiddleSense goes from 53.73 (LLM-only) to 66.86, and BioASQ from 65.85 to 89.43.[^html]
- Table 1, FLAN-T5 3B with LoRA: LoRA total 76.23; LoRA + GNP 77.43 (the Total column; the gain is largest on RiddleSense, 72.94 to 75.49).[^html]
- Table 1, FLAN-T5 11B (frozen) LLM-only average 66.88; the 11B GNP row is reported in the same table.[^html]

# Relevance to neocortex
- Skip for the engine: it needs a 3B to 11B seq2seq LLM at training and inference, and the gains are on multiple-choice QA, not on memory recall.
- The subgraph-to-text idea is the reusable part: retrieve a small neighbourhood around the query entities and hand it to the reader. neocortex already does this with path search, so the GNN encoder is not needed.
- The KG-flattening baselines (REL and BFS ordering, Table 1) show that flat subgraph text can hurt the frozen LLM, which matters for how neocortex formats recalled facts.[^html]

[^paper]: arXiv abstract page
[^html]: arXiv HTML full text (Sections 3-5, Table 1, implementation details)
