---
type: "Benchmark"
title: "GrailQA"
description: "Large-scale KBQA dataset with 64,331 questions and three levels of generalization (i.i.d., compositional, zero-shot)."
resource: "https://arxiv.org/abs/2011.07743"
tags: ["benchmark", "kbqa", "generalization"]
year: 2020
venue: "arXiv 2020 (README row)"
llm_required: none
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2011.07743"
    title: "arXiv 2011.07743 (abstract)"
  - id: code
    resource: "https://github.com/dki-lab/GrailQA"
    title: "Official repo / dataset page"
---
# Summary
- Releases 64,331 questions and evaluation settings for i.i.d., compositional and zero-shot generalization.[^paper]
- Proposes a BERT-based KBQA model and shows the role of contextual embeddings in generalization.[^paper]

# Method
- BERT-based KBQA model evaluated across the three generalization levels (abstract level).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- The zero-shot generalization split tests what neocortex's entity resolution must handle for unseen entities. Needs a KB offline.
- Offline with neocortex: yes if the KB is loaded; no LLM needed for the BERT baseline.

[^paper]: arXiv 2011.07743 (abstract)
