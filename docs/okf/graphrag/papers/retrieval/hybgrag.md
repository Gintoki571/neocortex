---
type: Paper
title: 'HybGRAG: Hybrid Retrieval-Augmented Generation on Textual and Relational Knowledge Bases'
description: Retriever bank of textual and relational retrievers, with an agentic critic that refines the retrieved output for hybrid questions.
resource: https://arxiv.org/abs/2412.16311
tags:
- hybrid-retrieval
- retriever-bank
- critic
- semi-structured-kb
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: ACL 2024 Findings
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/abs/2412.16311
  title: arXiv abstract
---

# Summary
- Combines a bank of retrievers over text and relations with a critic that feeds back on the output for questions needing both (abstract).[^paper]
- Full text not read in this pass.

# Relevance to neocortex
- A retriever bank with a critic maps onto neocortex's fused recall, but the critic is an LLM. The bank itself (BM25 plus relation retrieval) is LLM-free, so the bank is the part to borrow.

[^paper]: arXiv abstract
