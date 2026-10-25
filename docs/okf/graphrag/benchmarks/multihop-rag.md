---
type: "Benchmark"
title: "MultiHop-RAG"
description: "RAG benchmark of 2,556 multi-hop news queries with gold evidence sentences, ground-truth answers and null (unanswerable) queries."
resource: "https://arxiv.org/abs/2401.15391"
tags: ["benchmark", "multi-hop", "rag", "news", "evidence-retrieval"]
year: 2024
venue: "COLING 2024"
llm_required: both
neocortex_relevance: adopt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2401.15391"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2401.15391"
    title: "arXiv HTML full text (sections 3-4, Tables 3-6)"
  - id: code
    resource: "https://github.com/yixuantt/MultiHop-RAG"
    title: "Repository README and evaluation scripts (retrieval_evaluate.py, qa_evaluate.py, simple_retrieval.py)"
---
# Summary
- 2,556 queries over a knowledge base of 609 English news articles in six categories (average 2,046 tokens); the articles were published 26 Sep to 26 Dec 2023 and only articles of at least 1,024 tokens were kept.[^html]
- Query types are inference, comparison, temporal and null; 301 queries (11.78%) are null queries whose answers cannot be derived from the knowledge base.[^html]
- Evidence needed per query: 2 pieces for 1,078 queries (42.18%), 3 for 779 (30.48%), 4 for 398 (15.56%).[^html]
- The dataset is distributed through GitHub and Hugging Face (yixuantt/MultiHopRAG); the repo README states ODC-BY, while the arXiv HTML page shows CC BY-SA 4.0 for the paper.[^code][^html]

# Method
- Chunking: the corpus is split into 256-token chunks, embedded into a vector database, and the top-K chunks by cosine similarity are retrieved (LlamaIndex pipeline).[^html]
- Retrieval metrics: MAP@K, MRR@K and Hit@K; null queries are excluded from retrieval evaluation because they have no evidence.[^html]
- Reranking: retrieve 20 chunks, then keep the top K with bge-reranker-large.[^html]
- Generation: top-6 chunks capped at 2,048 tokens; a second setting feeds the ground-truth evidence as a ceiling. Models: GPT-4, GPT-3.5, Claude-2.1, Google PaLM, Mixtral-8x7B-Instruct and Llama-2-70b-chat.[^html]
- The repo's retrieval script (retrieval_evaluate.py) scores each non-null query: a hit at K if any gold fact string (whitespace removed) is contained in one of the top-K retrieved texts; MRR and MAP are computed per query and averaged. It reports Hits@10 and Hits@4.[^code]

# Results
- Retrieval without reranker, voyage-02: MRR@10 0.3934, MAP@10 0.3143, Hits@10 0.6506, Hits@4 0.4619. The paper's Table 5 is the source; other embedding models in that table score lower.[^html]
- Retrieval with bge-reranker-large on voyage-02: MRR@10 0.586, MAP@10 0.4795, Hits@10 0.7467, Hits@4 0.6625. The authors call the gap to a usable retriever 'still significant'.[^html]
- Answer accuracy (Table 6), retrieved chunks vs ground-truth evidence: GPT-4 0.56 vs 0.89; Claude-2.1 0.52 vs 0.56; Google PaLM 0.47 vs 0.74; ChatGPT 0.44 vs 0.57; Mixtral-8x7B 0.32 vs 0.36; Llama-2-70b-chat 0.28 vs 0.32.[^html]

# Relevance to neocortex
- Offline retrieval scoring with neocortex needs no LLM. Load the 609 articles as text memories (chunk to roughly 256 tokens, as the paper does, or keep whole articles). Keep title, source and published_at as metadata. Then, for each non-null query in MultiHopRAG.json, call recall with the query text and check Hit@K and MRR against the gold evidence list (field evidence_list[].fact). This reuses the repo's own protocol, so numbers are comparable with the paper's Table 5 rows.[^code][^html]
- Stratify by evidence count (2, 3, 4) to see whether neocortex's multi-hop path logic helps beyond single-chunk lexical recall. Null queries (301) give a no-hallucination check: recall should return nothing confident; answer quality needs an LLM on the agent side.[^html]
- Answer accuracy (field answer, with the four question types) needs an LLM to read the recalled text, so it stays agent side; the paper's ceiling setting (ground-truth evidence fed to the LLM) shows how much is lost to retrieval.[^html]
- Neocortex does not require the entity/relation extraction step for this benchmark because the gold units are sentences; the extraction question (plain-text relations without an LLM) appears only if neocortex is asked to build facts from the articles.

[^html]: arXiv HTML full text (sections 3-4, Tables 3-6)
[^code]: Repository README and evaluation scripts (retrieval_evaluate.py, qa_evaluate.py, simple_retrieval.py)
