---
type: "Benchmark"
title: "SimpleQuestions"
description: "Large single-relation question-answering dataset of 100k questions over a knowledge base, used with Memory Networks."
resource: "https://arxiv.org/abs/1506.02075"
tags: ["benchmark", "simple-qa", "kbqa"]
year: 2015
venue: "arXiv 2015 (README row)"
llm_required: none
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/1506.02075"
    title: "arXiv 1506.02075 (abstract)"
  - id: code
    resource: "https://github.com/Jerryzhao-z/simple-question-answering-with-memory-networks"
    title: "Official repo / dataset page"
---
# Summary
- Introduces a dataset of 100k questions, used with existing benchmarks.[^paper]
- Shows that Memory Networks can be trained to perform well on it.[^paper]

# Method
- Memory Networks trained on the question-to-fact task.[^paper]

# Results
- Abstract says the trained model achieves excellent performance; no numbers read.[^paper]

# Relevance to neocortex
- Single-fact lookup. Neocortex's measured weak spots are multi-hop, so this is low value. Offline use would need the source knowledge base.
- Offline with neocortex: yes if the KB is supplied, but low value.

[^paper]: arXiv 1506.02075 (abstract)
