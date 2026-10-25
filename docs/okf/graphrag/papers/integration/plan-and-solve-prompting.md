---
type: "Paper"
title: "Plan-and-Solve Prompting: Improving Zero-Shot Chain-of-Thought Reasoning by Large Language Models"
description: "PS and PS+ zero-shot prompts: first plan the subtasks, then solve with explicit variable and calculation instructions; they beat Zero-shot-CoT on arithmetic and commonsense reasoning (ACL 2023)."
resource: "https://arxiv.org/abs/2305.04091"
tags: ["prompting", "zero-shot-cot", "planning", "arithmetic", "commonsense"]
year: 2023
venue: "ACL 2023"
llm_required: both
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2305.04091"
    title: "arXiv abstract page"
  - id: html
    resource: "https://arxiv.org/html/2305.04091"
    title: "arXiv HTML full text (Sections 2-4, Tables 2-6)"
---
# Summary
- Two steps: a zero-shot prompt asks the LLM to devise a plan and carry it out step by step ("Let's first understand the problem and devise a plan..."), then a second prompt extracts the answer.[^html]
- PS+ adds explicit instructions to extract variables and calculate intermediate results, which targets calculation and missing-step errors.[^html]
- Evaluated on six arithmetic datasets (MultiArith, GSM8K, AddSub, AQuA, SingleEq, SVAMP), commonsense (CSQA, StrategyQA) and symbolic reasoning.[^html]

# Method
- Backbone: text-davinci-003 in the main tables; the paper says it follows Auto-CoT's GPT-3 setup and uses the public API.[^html]
- Zero-shot prompts only; no demonstrations are needed for PS and PS+.[^html]

# Results
- Table 2 (text-davinci-003, average over six math datasets): Zero-Shot-CoT 70.4; PS+ 76.7; PS 72.9; Manual-CoT (few-shot) 77.6.[^html]
- Table 2, GSM8K: Zero-Shot-CoT 56.4, PS+ 59.3, Manual-CoT 58.4. Table 2, SVAMP: Zero-Shot-CoT 69.9, PS+ 75.7, Manual-CoT 80.3.[^html]
- Table 3 (commonsense, CSQA / StrategyQA): Zero-shot-CoT 65.2 / 63.8; Zero-shot-PS+ 71.9 / 65.4; Few-shot Manual-CoT 78.3 / 71.2.[^html]

# Relevance to neocortex
- Skip: a prompting method for the calling LLM, with no engine component. It needs several LLM calls per question and is not retrieval.
- The one idea that matches neocortex's use is the plan-then-extract-variables split: the calling agent can write its entities and relations first and only then ask neocortex for facts. That is an agent-side habit, not a feature.

[^html]: arXiv HTML full text (Sections 2-4, Tables 2-6)
