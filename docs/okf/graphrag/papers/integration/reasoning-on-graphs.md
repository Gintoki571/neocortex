---
type: "Paper"
title: "Reasoning on Graphs: Faithful and Interpretable Large Language Model Reasoning"
description: "RoG plans relation paths with an LLM, then retrieves faithful reasoning paths from the KG and answers from them."
resource: "https://arxiv.org/abs/2310.01061"
tags: ["knowledge-graph", "relation-path", "planning", "multi-hop", "faithful-reasoning"]
year: 2023
venue: "ICLR 2024"
llm_required: retrieval
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2310.01061"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2310.01061"
    title: "arXiv HTML full text (Sections 3-5, Tables 1-2)"
---
# Summary
- RoG pairs a planning module with a retrieval-reasoning module, so the reasoning paths are grounded in KG facts and the answer is traceable.[^html]
- Planning optimization distills KG knowledge into the LLM so that it generates faithful relation paths as plans; retrieval-reasoning optimization teaches it to reason over retrieved paths.[^html]

# Method
- Planning: the LLM outputs relation paths conditioned on the question (the plan).[^html]
- Retrieval: valid reasoning paths are retrieved from the KG by matching the plans; the paper uses beam search and keeps K = 3 relation paths.[^html]
- Reasoning: the same LLM answers from the retrieved paths; planning and reasoning share one backbone, LLaMA2-Chat-7B, trained jointly on two instruction-tuning tasks.[^html]
- Metrics: Hits@1 and F1.[^html]

# Results
- Table 1 (Hits@1 / F1): RoG on WebQSP 85.7 / 70.8 and on CWQ 62.6 / 56.2. The 22.3% (Hits@1) and about 14.5% (F1) gains on CWQ match the relative gain over UniKGQA (51.2 / 49.1), not over the best baseline in the table: KD-CoT scores 55.7 Hits@1 on CWQ.[^html]
- Other baselines in the table include ChatGPT+CoT (CWQ 48.9 Hits@1) and DECAF (WebQSP 82.1 Hits@1).[^html]

# Relevance to neocortex
- The retrieval half is deterministic path search over relation facts, which neocortex already runs for its relation beam; it can run in the engine. Planning is an LLM call and stays on the agent side (the agent sends the plan as a predicate sequence).[^html]
- The plan-then-retrieve split suits neocortex's API: recall takes an ordered list of predicate hints and returns the matching fact chains with their valid_from and valid_to fields.[^html]
- Link: [KG path beam search](/techniques/kg-path-beam-search.md).

[^html]: arXiv HTML full text (Sections 3-5, Tables 1-2)
