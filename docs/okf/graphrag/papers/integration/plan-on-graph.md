---
type: "Paper"
title: "Plan-on-Graph: Self-Correcting Adaptive Planning of Large Language Model on Knowledge Graphs"
description: "PoG decomposes a question into sub-objectives, explores KG paths adaptively, updates memory, and self-corrects by reflection."
resource: "https://arxiv.org/abs/2410.23875"
tags: ["knowledge-graph", "adaptive-planning", "self-correction", "memory", "hits-at-1"]
year: 2024
venue: "NeurIPS 2024"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2410.23875"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2410.23875"
    title: "arXiv HTML full text (Sections 3-5, Tables 1-2, ablation)"
---
# Summary
- PoG first decomposes the question into sub-objectives, then repeats three steps until it reaches an answer: adaptive exploration of reasoning paths, memory update, and reflection on whether to self-correct erroneous paths.[^html]
- The paper's ablation names the mechanisms Guidance, Memory, Reflection and Adaptive Breadth.[^html]

# Method
- Topic entity, relation and sub-objective prompts are assembled per step (the appendix prompt takes the question, sub-objectives, topic entities and relations).[^html]
- Adaptive breadth: the breadth of paths explored is chosen during exploration, rather than predefined as in ToG or StructGPT, per the paper's stated limitation of predefined breadth.[^html]
- Metric: exact match Hits@1 on CWQ, WebQSP and GrailQA.[^html]

# Results
- Table 1, GPT-3.5 setting (CWQ / WebQSP): PoG 63.2 / 82.0, ToG 57.1 / 76.2. GrailQA PoG 76.5.[^html]
- Table 1, GPT-4 setting: PoG 75.0 (CWQ) and 87.3 (WebQSP), against ToG 67.6 and 82.6.[^html]
- Ablation (CWQ / WebQSP / GrailQA): full PoG 63.2 / 82.0 / 76.5; without Memory 58.9 / 77.5 / 69.3; without Reflection 59.4 / 78.1 / 70.5; without Guidance 60.1 / 80.3 / 72.4; without Adaptive Breadth 61.3 / 80.2 / 73.8.[^html]
- Input tokens on CWQ are about 4.6% lower than ToG, per the paper.[^html]

# Relevance to neocortex
- The memory update is the largest single ablation effect (4.3 points on CWQ, 4.5 on WebQSP, 7.2 on GrailQA). For neocortex, this suggests the intermediate facts found during a multi-hop search are worth storing as memories so the agent can reuse them.[^html]
- The explore-update-reflect loop is agent-side (LLM). The engine parts are the path search and the fact store. Link: [KG-grounded draft correction](/techniques/kg-grounded-draft-correction.md).

[^html]: arXiv HTML full text (Sections 3-5, Tables 1-2, ablation)
