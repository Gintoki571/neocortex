---
type: "Benchmark"
title: "CRUD-RAG"
description: "Chinese RAG benchmark with four application types (Create, Read, Update, Delete) that scores every RAG component, not only the LLM."
resource: "https://arxiv.org/abs/2401.17043"
tags: ["benchmark", "rag", "chinese", "crud", "chunking", "retriever"]
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
  - id: html
    resource: "https://arxiv.org/html/2401.17043"
    title: "arXiv HTML full text (Sections 2-4, Tables 2, 5, 9)"
---
# Summary
- Six sub-datasets map to four scenarios: Create (text continuation, 10,728), Read (QA with 1, 2 and 3 documents, 3,199 / 3,192 / 3,189), Update (hallucination modification, 5,130) and Delete (multi-document summarization, 10,728); the retrieval database has 86,834 documents.[^html]
- The authors argue that most RAG benchmarks score only the LLM or only the retriever in knowledge-intensive settings, and not the knowledge-base construction or the retrieval stage end to end.[^paper]
- The default RAG configuration is chunk size 128, overlap 0%, bge-base embeddings, dense retriever, top-k 8 and GPT-3.5.[^html]

# Method
- Ablations change one component at a time: chunk size (64, 128, 256, 512), chunk overlap (0-70%), embedding model (m3e-base, bge-base, stella-base, gte-base), retriever (dense, BM25, hybrid, hybrid+rerank), top-k (2-10) and LLM (GPT-3.5, GPT-4, ChatGLM2-6B, Baichuan2-13B, Qwen-7B, Qwen-14B).[^html]
- Metrics: BLEU, ROUGE-L, BERTScore, and RAGQuestEval, a question-generation and question-answering metric. Its recall is the share of reference-derived questions the generated text can answer, and its precision is the token F1 on the answerable questions.[^html]
- Due to cost, the component experiments use one fifth of the dataset.[^html]

# Results
- Table 5 (retrievers): in the 1-document QA rows, Hybrid+Rerank has the top value on most columns, while the paper states that the hybrid retriever alone gives only limited gains and that reranking improves almost all tasks.[^html]
- Recommendations (Table 9): BM25 for Update (error correction), hybrid+rerank for Read, dense retrieval for Create, and a larger top-k for multi-document QA.[^html]
- The paper reports that Qwen-14B is close to GPT-4 on text continuation and summarization, and that Baichuan2-13B suits single- and multi-document QA.[^html]

# Relevance to neocortex
- The retrieval-side ablations (chunk size, overlap, top-k, BM25 vs dense vs hybrid vs rerank) are the parts neocortex can test offline: recall plus its own BM25 and dense fusion, with no LLM at read time. The generation metrics need an LLM judge on the agent side.[^html]
- The Chinese corpus and LLM-centric scoring make it a weak fit as a neocortex gate. Use it only as a second check for the hybrid-retrieval design (see [hybrid lexical-dense retrieval](/techniques/hybrid-lexical-dense-retrieval.md)).

[^paper]: arXiv 2401.17043 (abstract)
[^html]: arXiv HTML full text (Sections 2-4, Tables 2, 5, 9)
