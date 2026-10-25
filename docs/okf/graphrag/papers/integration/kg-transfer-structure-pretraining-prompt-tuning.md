---
type: "Paper"
title: "Structure Pretraining and Prompt Tuning for Knowledge Graph Transfer"
description: "KGTransformer: a transformer pretrained on sampled KG subgraphs with three self-supervised tasks, reused through triple prompt tuning for triple classification, zero-shot image classification and CommonsenseQA (WWW 2023)."
resource: "https://arxiv.org/abs/2303.03922"
tags: ["knowledge-graph", "pretraining", "prompt-tuning", "subgraph", "transformer"]
year: 2023
venue: "WWW 2023 (ACM Web Conference 2023; DOI 10.1145/3543507.3583301)"
llm_required: none
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2303.03922"
    title: "arXiv 2303.03922 (same paper as the WWW 2023 DOI; the ACM DOI page is a bot check and was not used)"
  - id: html
    resource: "https://arxiv.org/html/2303.03922"
    title: "arXiv HTML full text (Sections 3-4, Tables 3-6)"
  - id: openalex
    resource: "https://api.openalex.org/works?search=Structure%20Pretraining%20and%20Prompt%20Tuning%20for%20Knowledge%20Graph%20Transfer"
    title: "OpenAlex record (abstract rebuilt from the inverted index)"
  - id: code
    resource: "https://github.com/zjukg/KGTransformer"
    title: "Repository README (raw, main branch); no LICENSE file (404)"
---
# Summary
- KGTransformer is a transformer that encodes sampled KG subgraphs as its input and is pretrained with three self-supervised tasks: Masked Entity Modeling (MEM), Masked Relation Modeling (MRM) and Entity Pair Modeling (EPM).[^html][^openalex]
- Reused across tasks through a triple prompt: the task's triples are fed as prompt, so the same pretrained module serves as a knowledge representation and fusion module.[^openalex][^html]
- Evaluated on triple classification (WN18RR), zero-shot image classification (AwA-KG) and question answering (CommonsenseQA with ConceptNet subgraphs).[^html]

# Method
- Four KGTransformer layers, 768 hidden units, 12 attention heads; subgraphs of k = 126 triples; 15% of triples masked in MEM and MRM.[^html]
- Triple classification: 10 negative triples per positive by corrupting head or tail, batch size 16, Adam with learning rate 1e-4.[^html]
- QA: the model scores each question-choice pair against a ConceptNet subgraph extracted for it.[^html]

# Results
- Table 3 (WN18RR triple classification, accuracy / F1): KGTransformer 89.21 / 89.73 vs TransE 88.35 / 87.62, RotatE 88.26 / 87.57, ComplEx 85.07 / 82.94; KGT-scratch (no pretraining) 67.02 / 66.19.[^html]
- Table 6 (CommonsenseQA, in-house dev / test accuracy, RoBERTa-Large encoder): KGTransformer 77.64 / 74.13 vs QA-GNN 76.54 / 73.41 and MHGRN 74.45 / 71.11; the same-setup RoBERTa-Large (ours) reaches 72.24 / 68.49.[^html]

# Relevance to neocortex
- Skip for the engine: it is a neural model pretrained on KG subgraphs, so it needs GPU-scale training and a model at read time. neocortex's constraints (CPU, LLM-free engine) rule out learned subgraph encoders for now.
- The useful part is the evaluation idea: triple classification on WN18RR with negatives made by corrupting head or tail is an offline check for neocortex's relation facts that needs no LLM.
- The pretraining-vs-scratch gap (89.21 vs 67.02) shows that subgraph pretraining carries most of the gain, which argues for a learned component, not for changing neocortex's rules.

[^html]: arXiv HTML full text (Sections 3-4, Tables 3-6)
[^openalex]: OpenAlex record (abstract)
