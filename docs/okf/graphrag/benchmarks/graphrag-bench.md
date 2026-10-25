---
type: "Benchmark"
title: "GraphRAG-Bench"
description: "Benchmark of about 4,000 questions over two corpora (Gutenberg novels and medical guidelines) asking when graph structure helps RAG across four task levels."
resource: "https://arxiv.org/abs/2506.05690"
tags: ["benchmark", "graphrag", "evaluation", "novel", "medical"]
year: 2025
venue: "arXiv 2025"
llm_required: both
neocortex_relevance: adopt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2506.05690"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2506.05690"
    title: "arXiv HTML full text (Sections 3-5, Tables 1-2, Table 12)"
  - id: dataset
    resource: "https://huggingface.co/datasets/GraphRAG-Bench/GraphRAG-Bench"
    title: "Hugging Face dataset card and files (Datasets/Questions/*.json, Datasets/Corpus/*.json)"
  - id: code
    resource: "https://github.com/GraphRAG-Bench/GraphRAG-Benchmark"
    title: "Repository README and Evaluation/ scripts (MIT licence)"
---
# Summary
- Two corpora: Novel (pre-20th-century novels from Project Gutenberg; 20 records in the released novel.json) and Medical (a single 1,052,159-character record in medical.json, built from clinical guideline texts such as NCCN guidelines).[^html][^dataset]
- Released question files: novel 2,010 and medical 2,062 (4,072 in total); the paper states 4,076. Per corpus, question types are Fact Retrieval (novel 971, medical 1,098), Complex Reasoning (610, 509), Contextual Summarize (362, 289) and Creative Generation (67, 166).[^dataset]
- Each question has a gold answer and gold evidence sentences; novel questions also carry evidence_triple (subject, relation, object) and medical questions carry evidence_relations. Average evidence per question is about 2.3 for novel.[^dataset]
- Corpus sizes for token-scaling tests: small about 56k tokens, medium about 603k, large about 1,132k (the novel corpus).[^html]
- Licence: the repository code is MIT; the dataset licence was not verified.

# Method
- Evaluates seven GraphRAG frameworks plus basic RAG against tailored stage metrics: structure-based graph quality, Context Relevance and Evidence Recall for retrieval, and answer accuracy (ACC), ROUGE-L, coverage and faithfulness for generation.[^html]
- Generation results (Table 2) use GPT-4o-mini: Fact Retrieval and Complex Reasoning with ACC and ROUGE-L; Contextual Summarize with ACC and coverage; Creative Generation with ACC, FS and coverage.[^html]
- A representative subset of models was also run on Qwen2.5-14B because of time and resource limits.[^html]

# Results
- Novel, generation (Table 2): Fact Retrieval ACC, RAG without rerank 58.76, RAG with rerank 60.92, HippoRAG2 60.14, LightRAG 58.62, Fast-GraphRAG 56.95, MS-GraphRAG 49.29. Complex Reasoning ACC: RAG 41.35, HippoRAG2 53.38, MS-GraphRAG 50.93.[^html]
- Medical, generation (Table 2): Fact Retrieval ACC, RAG without rerank 63.72, with rerank 64.73, HippoRAG 56.14; MS-GraphRAG 38.63.[^html]
- The paper reports RAG at 83.2% Evidence Recall on Fact Retrieval in the novel corpus, and HippoRAG at 87.9% or higher Evidence Recall on Level 2-3 novel questions (the extraction cut the upper bound). The paper's conclusion is that graphs help multi-hop and summarization but not simple fact lookup.[^html]

# Relevance to neocortex
- Offline retrieval scoring is possible without an LLM: insert each novel or medical document as text, call recall for every question, and compute Evidence Recall against the gold evidence sentences (the repo's retrieval_eval.py does this). Use Fact Retrieval and Complex Reasoning for the retrieval metrics; the other two levels need an LLM judge.[^dataset][^code]
- The novel evidence_triple field maps directly onto neocortex relation facts (subject, relation, object). Scoring entity and relation recall against those triples tests the bi-temporal relation store without any LLM.[^dataset]
- Answer-level scores (ACC, ROUGE-L) need an LLM on the agent side and are not offline. Creative Generation is LLM-judged.[^html]

[^html]: arXiv HTML full text (Sections 3-5, Tables 1-2, Table 12)
[^dataset]: Hugging Face dataset card and files (Datasets/Questions/*.json, Datasets/Corpus/*.json)
[^code]: Repository README and Evaluation/ scripts (MIT licence)
