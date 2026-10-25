---
type: Paper
title: "What Makes Good Examples for Visual In-Context Learning?"
description: Shows that visual in-context learning is sensitive to the choice of demonstrations and proposes prompt retrieval methods to select them.
resource: "https://arxiv.org/abs/2301.13670"
tags:
- visual-icl
- prompt-retrieval
- vision
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2023
llm_required: none
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/html/2301.13670
  title: arXiv HTML (abstract and introduction read in this pass)
- id: abs
  resource: https://arxiv.org/abs/2301.13670
  title: arXiv abstract page
---

# Summary
- Investigates in-context learning in large vision models, where demonstrations are placed in the prompt and the model parameters are not updated.[^paper]
- Finds that performance is highly sensitive to which in-context examples are chosen.[^paper]
- Proposes a prompt retrieval framework: an unsupervised nearest-example search with an off-the-shelf model, and a second retrieval component that the parsed abstract cuts off.[^paper]
- Only the abstract and introduction were read; the experimental tables were not inspected in this pass.

# Method
- Unsupervised prompt retrieval by nearest-example search in an off-the-shelf embedding space.[^paper]

# Results
- Not read beyond the abstract, so no numbers are recorded.[^paper]

# Relevance to neocortex
- Vision domain. Skip for neocortex.

[^paper]: arXiv HTML, https://arxiv.org/html/2301.13670 (abstract and introduction)
