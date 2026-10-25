---
type: "Benchmark"
title: "UltraDomain"
description: "Domain-specific long-context QA dataset, introduced with MemoRAG (the dataset link in the README points to the MemoRAG repo)."
resource: "https://arxiv.org/abs/2409.05591"
tags: ["benchmark", "domain-qa", "long-context"]
year: 2024
venue: "arXiv 2024 (README row)"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2409.05591"
    title: "arXiv 2409.05591 (abstract)"
  - id: code
    resource: "https://github.com/qhjqhj00/MemoRAG#dataset"
    title: "Official repo / dataset page"
---
# Summary
- The MemoRAG abstract motivates long-context retrieval where queries are not explicit and knowledge is not well structured.[^paper]

# Method
- Not described in the abstract; the MemoRAG method is a global memory with KV compression (abstract).[^paper]

# Results
- Abstract claims gains across long-context evaluations; no numbers read.[^paper]

# Relevance to neocortex
- Useful as a domain QA check of neocortex's recall on long documents. The README places the dataset in the MemoRAG repo, so the source of the dataset is MemoRAG.
- Offline with neocortex: partial; answering needs an LLM.

[^paper]: arXiv 2409.05591 (abstract)
