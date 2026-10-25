---
type: "Benchmark"
title: "CRUD-RAG"
description: "Chinese RAG benchmark with four application types (Create, Read, Update, Delete) that evaluates every RAG component."
resource: "https://arxiv.org/abs/2401.17043"
tags: ["benchmark", "rag", "chinese", "crud"]
year: 2024
venue: "arXiv 2024 (README row, spelled CURD)"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2401.17043"
    title: "arXiv 2401.17043 (abstract)"
---
# Summary
- Covers Create, Read, Update and Delete application scenarios of RAG.[^paper]
- Evaluates the retriever, knowledge base construction, context length and LLM, not only the LLM.[^paper]

# Method
- Four task families with datasets per family (abstract level).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- Its retriever and knowledge-base analysis is useful, but the content is Chinese and LLM-centred. Watch.
- Offline with neocortex: partial.

[^paper]: arXiv 2401.17043 (abstract)
