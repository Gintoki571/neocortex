---
type: Paper
title: 'DocPolicyKG: A Lightweight LLM-Based Framework for Knowledge Graph Construction from Chinese Policy Documents'
description: Builds a knowledge graph from Chinese investment-promotion policy documents with a fine-tuned 7B-class LLM plus ontology and prompting, then adds Graph RAG for policy QA (CIKM 2025; abstract only).
resource: https://dl.acm.org/doi/abs/10.1145/3746252.3760904
tags:
- kg-construction
- policy
- graph-rag
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: CIKM 2025
llm_required: indexing
neocortex_relevance: skip
sources:
- id: answerthis
  resource: https://doi.org/10.1145/3746252.3760904
  title: AnswerThis exact-title search record (abstract, authors, DOI; no open full text)
- id: paper
  resource: https://dl.acm.org/doi/abs/10.1145/3746252.3760904
  title: ACM DL page (blocked)
- id: listing
  resource: https://github.com/DEEP-PolyU/Awesome-GraphRAG
  title: Awesome-GraphRAG README entry
---

# Summary
- Abstract only: the ACM page is blocked and no open-access PDF was listed.[^answerthis][^paper]
- Builds a knowledge graph from Chinese investment-promotion policy documents using a lightweight LLM that is fine-tuned, with a domain ontology and prompt engineering.[^answerthis]
- The base model is DeepSeek-R1-7B. The abstract says the method beats it and is competitive with GPT-4o on named entity recognition (NER) and relation triplet extraction (RTE).[^answerthis]
- Adds Graph RAG on top of the constructed graph for policy question answering. The graph is said to be public at github.com/hanshenmesen/DocPolicyKG (not read).[^answerthis]
- Authors: Chen Han, Yuanyuan Li, Xijin Tang. Four-page proceedings paper (pp. 4753-4757).[^answerthis]

# Method
- Ontology-guided extraction with a fine-tuned small LLM, plus prompt engineering for NER and RTE. Training data and hyperparameters were not read.[^answerthis]

# Results
- Abstract only: no metric values were read. Qualitative claims: beats DeepSeek-R1-7B and is competitive with GPT-4o on NER and RTE.[^answerthis]

# Relevance to neocortex
- Indexing needs a fine-tuned LLM for extraction. That is the step neocortex's engine avoids (its entity and relation extraction is supplied by the calling agent or by rules and GLiNER).
- The domain is Chinese policy text, and the work is extraction rather than retrieval.
- Relevance: skip.

[^answerthis]: AnswerThis search record, https://doi.org/10.1145/3746252.3760904 (abstract only)
[^paper]: ACM DL page, https://dl.acm.org/doi/abs/10.1145/3746252.3760904 (blocked)
[^listing]: Awesome-GraphRAG README, https://github.com/DEEP-PolyU/Awesome-GraphRAG
