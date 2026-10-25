---
type: "Paper"
title: "Mitigating Large Language Model Hallucinations via Autonomous Knowledge Graph-based Retrofitting"
description: "KGR checks an LLM draft's claims against KG facts, selects and verifies them, and retrofits the draft during reasoning."
resource: "https://arxiv.org/abs/2311.13314"
tags: ["hallucination", "kg-refinement", "retrofitting", "draft-correction", "claim-verification"]
year: 2023
venue: "AAAI 2024"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2311.13314"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2311.13314"
    title: "arXiv HTML full text (Sections 1-4, Table 1)"
---
# Summary
- Existing KG methods query the KG only with the user input, so they miss hallucinations the LLM produces during reasoning. KGR also checks the LLM's own draft.[^html]
- KGR uses LLMs to extract, select, validate and retrofit factual statements in the draft, with no manual effort.[^html]

# Method
- Five components: (1) claim extraction, (2) entity detection and KG retrieval, (3) fact selection, (4) claim verification, (5) response retrofitting.[^html]
- Datasets: SimpleQuestions (100k questions from Freebase), Mintaka and HotpotQA; models ChatGPT and text-davinci-003; metrics exact match and F1.[^html]
- Baselines include chain-of-thought (CoT), CRITIC and QKR, which shares KGR's entity detection and fact selection.[^html]

# Results
- KGR's F1 improves on QKR by at least 6.2 points on Mintaka and 1.1 points on HotpotQA (paper text, Table 1).[^html]
- The paper reports that KGR performs well compared with CoT and CRITIC on HotpotQA.[^html]

# Relevance to neocortex
- Steps 2 and 4 can run in neocortex: entity detection against the entity table, retrieval of matching facts (with bi-temporal validity), and a claim-to-fact match that returns supported, contradicted or missing. Steps 1, 3 and 5 need an LLM and stay agent side.[^html]
- A draft-check call would return, for each claim, the current supporting fact and any superseded fact. Link: [KG-grounded draft correction](/techniques/kg-grounded-draft-correction.md).

[^html]: arXiv HTML full text (Sections 1-4, Table 1)
