---
type: "Paper"
title: "Think-on-Graph 2.0: Deep and Faithful Large Language Model Reasoning with Knowledge-guided Retrieval Augmented Generation"
description: "Training-free hybrid RAG that iterates between text passages and a KG, linking documents through entities, in a tight-coupling loop."
resource: "https://arxiv.org/abs/2407.10805"
tags: ["hybrid-rag", "knowledge-graph", "document-linking", "multi-hop", "training-free"]
year: 2024
venue: "arXiv 2024"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2407.10805"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2407.10805"
    title: "arXiv HTML full text (Sections 1, 4, Table 1)"
---
# Summary
- ToG-2 retrieves iteratively from unstructured and structured knowledge in a tight-coupling manner: context retrieval is deepened through the KG, and graph retrieval is grounded in retrieved contexts.[^html]
- The paper says ToG-2 is training-free and plug-and-play, and it compares against text-based RAG, KG-based RAG (ToG) and a loosely coupled hybrid (CoK).[^html]

# Method
- Iterative collaboration: passages are linked to KG entities; the KG expands the search to entity neighbours; the retrieved context feeds the next graph step, until the LLM answers.[^html]
- Sources: full Wikipedia (unstructured) and Wikidata (structured).[^html]
- Datasets: WebQSP, AdvHotpotQA, QALD-10-en, FEVER, Creak and Zero-Shot RE; metrics exact match (FEVER and Creak use accuracy).[^html]

# Results
- Table 1 (GPT-3.5-turbo), WebQSP / AdvHotpotQA / QALD-10-en / Creak / Zero-Shot RE: ToG-2 81.1 / 42.9 / 54.1 / 93.5 / 91.0 versus ToG 76.2 / 26.3 / 50.2 / 93.8 / 88.0 and CoK 77.6 / 35.4 / 47.1 / 90.4 / 75.5 (the paper's own numbers, as extracted).[^html]
- The paper runs an ablation of entity-pruning tools and width and depth settings (Sections 4.5.2-4.5.3); numbers for those were not transcribed.[^html]

# Relevance to neocortex
- The document-entity link is the part neocortex already has in a different form: text-only memories and entity/relation facts in one store. The engine could expose entity-linked passage retrieval; the iteration loop needs an LLM and stays agent side.[^html]
- ToG-2 uses Wikipedia and Wikidata, not a personal memory. The transferable idea is the alternation: fetch passages for an entity, then fetch its neighbours, then fetch passages for the neighbours.

[^html]: arXiv HTML full text (Sections 1, 4, Table 1)
