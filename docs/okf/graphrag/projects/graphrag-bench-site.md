---
type: "Open-source Project"
title: "GraphRAG-Bench (repository)"
description: "Code repository and dataset host for the GraphRAG-Bench benchmark; the benchmark's method and data are in its benchmark entry."
resource: "https://github.com/GraphRAG-Bench/GraphRAG-Benchmark"
tags: ["open-source", "benchmark", "graphrag", "evaluation"]
license: "MIT"
language: "Python"
llm_required: both
neocortex_relevance: adopt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://github.com/GraphRAG-Bench/GraphRAG-Benchmark"
    title: "Repository README and Evaluation/ scripts (MIT)"
  - id: dataset
    resource: "https://huggingface.co/datasets/GraphRAG-Bench/GraphRAG-Bench"
    title: "Hugging Face dataset files (Datasets/Questions and Datasets/Corpus)"
---
# Summary
- Repository contents: Evaluation/ (generation_eval.py, retrieval_eval.py, indexing_eval.py and metrics/ for Answer Accuracy, Context Recall and Relevance, Coverage, Faithfulness, ROUGE), Examples/ (run scripts for Fast-GraphRAG, HippoRAG2 and LightRAG), and the Hugging Face dataset.[^code][^dataset]
- Setup: pip install -r requirements.txt, then pip install -e . (per the README).[^code]

# Method
- Scoring scripts are in the repo; the dataset is on Hugging Face with novel and medical question and corpus files.[^dataset]

# Results
- No benchmark numbers were read for the repository itself; results are in the benchmark entry.[^code]

# Relevance to neocortex
- Use retrieval_eval.py as the reference for Evidence Recall and Context Relevance, so neocortex's numbers match the benchmark's definitions. The example runner for LightRAG is a template for an adapter that calls neocortex.[^code]
- See [GraphRAG-Bench](/benchmarks/graphrag-bench.md) for the data and the scoring details.

[^code]: Repository README and Evaluation/ scripts (MIT)
[^dataset]: Hugging Face dataset files (Datasets/Questions and Datasets/Corpus)
