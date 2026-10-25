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
    title: "Repository README and Evaluation/ scripts; LICENSE is MIT (raw LICENSE fetched)"
  - id: hf_card
    resource: "https://huggingface.co/datasets/GraphRAG-Bench/GraphRAG-Bench"
    title: "Hugging Face README (raw, fetched) and API metadata: no license field"
---
# Summary
- Two corpora: Novel (pre-20th-century novels from Project Gutenberg; 20 records in the released novel.json) and Medical (a single 1,052,159-character record in medical.json, built from clinical guideline texts such as NCCN guidelines).[^html][^dataset]
- Released question files: novel 2,010 and medical 2,062 (4,072 in total); the paper states 4,076. Per corpus, question types are Fact Retrieval (novel 971, medical 1,098), Complex Reasoning (610, 509), Contextual Summarize (362, 289) and Creative Generation (67, 166).[^dataset]
- Each question has a gold answer and gold evidence sentences; novel questions also carry evidence_triple (subject, relation, object) and medical questions carry evidence_relations. Average evidence per question is about 2.3 for novel.[^dataset]
- Corpus-size scaling (Table 12, novel corpus): small 56k, medium 603k and large 1132k; HippoRAG2 ACC on Fact Retrieval is 60.14, 59.99 and 59.19 respectively, and RAG (with rerank) is 64.73, 58.43 and 58.04.[^html]
- Licence: the repository LICENSE is MIT (Copyright 2025 XMU-DeepLIT). The Hugging Face card has no `license:` YAML field; its README badge links the MIT licence of the GitHub repo, so the dataset's own licence is not stated explicitly.[^code][^hf_card]

# Method
- Evaluates seven GraphRAG frameworks plus basic RAG against tailored stage metrics: structure-based graph quality, Context Relevance and Evidence Recall for retrieval, and answer accuracy (ACC), ROUGE-L, coverage and faithfulness for generation.[^html]
- Generation results (Table 2) use GPT-4o-mini: Fact Retrieval and Complex Reasoning with ACC and ROUGE-L; Contextual Summarize with ACC and coverage; Creative Generation with ACC, FS and coverage.[^html]
- Table 7 repeats the generation evaluation with Qwen2.5-14B for a subset of methods.[^html]

# Results
- Novel, generation (Table 2): Fact Retrieval ACC, RAG without rerank 58.76, RAG with rerank 60.92, HippoRAG2 60.14, LightRAG 58.62, Fast-GraphRAG 56.95, MS-GraphRAG 49.29. Complex Reasoning ACC: RAG 41.35, HippoRAG2 53.38, MS-GraphRAG 50.93.[^html]
- Medical, generation (Table 2): Fact Retrieval ACC, RAG without rerank 63.72, with rerank 64.73, HippoRAG 56.14; MS-GraphRAG 38.63.[^html]
- Obs. 4 (Table 3): RAG reaches 83.2% Evidence Recall on Fact Retrieval over the novel corpus; the paper attributes graph redundancy to the weaker GraphRAG results on simple questions. Obs. 5: on Level 2-3 novel questions HippoRAG reaches 87.9-90.9% Evidence Recall, and HippoRAG2 leads in Context Relevance at 85.8-87.8%.[^html]

# Relevance to neocortex
- Offline retrieval scoring is possible without an LLM: insert each novel or medical document as text, call recall for every question, and compute Evidence Recall against the gold evidence sentences (the repo's retrieval_eval.py does this). Use Fact Retrieval and Complex Reasoning for the retrieval metrics; the other two levels need an LLM judge.[^dataset][^code]
- The novel evidence_triple field maps directly onto neocortex relation facts (subject, relation, object). Scoring entity and relation recall against those triples tests the bi-temporal relation store without any LLM.[^dataset]
- Answer-level scores (ACC, ROUGE-L) need an LLM on the agent side and are not offline. Creative Generation is LLM-judged.[^html]

[^html]: arXiv HTML full text (Sections 3-5, Tables 1-2, Table 12)
[^dataset]: Hugging Face dataset card and files (Datasets/Questions/*.json, Datasets/Corpus/*.json)
[^code]: Repository README and Evaluation/ scripts; LICENSE (MIT, Copyright 2025 XMU-DeepLIT)
[^hf_card]: Hugging Face dataset README and API metadata (no license field)
