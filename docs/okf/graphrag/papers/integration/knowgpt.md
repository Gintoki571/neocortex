---
type: "Paper"
title: "KnowledGPT: Enhancing Large Language Models with Retrieval and Storage Access on Knowledge Bases"
description: "Program-based retrieval over public KBs plus a personalized KB that the LLM fills from documents; not the same work as KnowGPT (arXiv 2312.06185)."
resource: "https://arxiv.org/abs/2308.11761"
tags: ["knowledge-base", "retrieval", "storage", "program-of-thought", "agent"]
year: 2023
venue: "EMNLP 2023"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2308.11761"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2308.11761"
    title: "arXiv HTML full text (Sections 3-4, Tables 1, 4)"
---
# Summary
- KnowledGPT bridges LLMs with several knowledge bases for retrieval and for storage. Retrieval generates code that calls KB operations; storage adds a personalized KB (PKB) built from user documents.[^html]
- This is EMNLP 2023 work. The README entry KnowGPT (NeurIPS 2024, arXiv 2312.06185, Knowledge Graph based PrompTing) is a different paper and is catalogued under the retrieval folder by another worker.

# Method
- Retrieval as program-of-thought: the LLM writes search code over pre-defined KB functions: entity linking, get entity information, find entity or value, and find relationship.[^html]
- Storage: the LLM extracts knowledge from user documents into the PKB in three forms: entity descriptions, relational triples and entity-aspect information.[^html]
- Experiments: NLPCC-100 (100 questions from the NLPCC 2016 KBQA test set) and NLPCC-MH-59 (59 questions selected from NLPCC-MH), plus HotpotQA with a PKB built from 10 documents per question.[^html]

# Results
- Table 4, HotpotQA with PKB (25 questions): comparison 5/5, bridge 15/20, all 20/25 answered correctly.[^html]
- Table 3 (averaged F1 on NLPCC-100 and NLPCC-MH-59) reports KnowledGPT above BM25 and embedding retrieval for single-hop queries; the numbers were not transcribed from the extraction.[^html]

# Relevance to neocortex
- The storage path maps onto neocortex's remember call: the agent extracts entity descriptions, triples and aspects from documents and writes them with entities and relations. The engine stores them; the extraction stays agent side.[^html]
- The program-of-thought retrieval is a useful interface pattern: the agent passes a small set of typed operations (entity lookup, neighbours, relation search) to neocortex rather than free text. The operations are deterministic and fit in the engine.

[^html]: arXiv HTML full text (Sections 3-4, Tables 1, 4)
