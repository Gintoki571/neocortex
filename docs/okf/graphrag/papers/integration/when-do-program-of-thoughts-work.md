---
type: Paper
title: "When Do Program-of-Thoughts Work for Reasoning?"
description: Proposes the complexity-impacted reasoning score (CIRS) that ties the structure and logic of code data to whether program-of-thought prompting helps.
resource: "https://arxiv.org/abs/2308.15452"
tags:
- program-of-thought
- code-data
- reasoning
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2023
llm_required: none
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/html/2308.15452
  title: arXiv HTML (abstract read in this pass; body not inspected)
- id: abs
  resource: https://arxiv.org/abs/2308.15452
  title: arXiv abstract page
---

# Summary
- Proposes CIRS, which combines structural and logical attributes of code to measure how code data correlates with LLM reasoning ability.[^paper]
- Uses the abstract syntax tree for structure and a cyclomatic-complexity-style measure for logical complexity.[^paper]
- Finds that not all complex code helps: an optimal complexity level matters for program-aided prompting, and the paper proposes an automatic selection of such data.[^paper]
- Only the abstract was read in this pass; the method details and experiment tables were not inspected.

# Method
- Scoring by CIRS over AST structure and logical complexity, as described in the abstract.[^paper]

# Results
- Not read beyond the abstract, so no numbers are recorded.[^paper]

# Relevance to neocortex
- Code-reasoning domain with no direct fit to neocortex. Skip.

[^paper]: arXiv HTML, https://arxiv.org/html/2308.15452 (abstract)
