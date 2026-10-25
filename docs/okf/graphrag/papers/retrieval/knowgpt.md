---
type: Paper
title: 'KnowGPT: Knowledge Graph based PrompTing for Large Language Models'
description: Extracts the most informative KG knowledge with an RL-based path policy and a bandit over extraction and prompt templates, then turns it into a hard prompt for closed LLMs.
resource: https://arxiv.org/abs/2312.06185
tags:
- kg-prompting
- closed-source-llm
- bandit
- reinforcement-learning
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/html/2312.06185
  title: arXiv HTML full text (read in this pass)
- id: abs
  resource: https://arxiv.org/abs/2312.06185
  title: arXiv abstract page
---

# Summary
- Targets KG-enhanced prompting of closed-source LLMs, which only accept hard prompts, and names three problems: huge search space, high API cost and laborious prompt engineering.[^paper]
- Two modules: a knowledge extraction module that selects informative KG knowledge, and a context-aware prompt construction module that turns the selection into a prompt.[^paper]
- Built on GPT-3.5; the paper reports a 23.7% average test-accuracy gain over GPT-3.5 and 2.9% over GPT-4 on CommonsenseQA, OpenBookQA and MedQA.[^paper]
- Not the same work as KnowGPT in [papers/integration/knowgpt.md](/papers/integration/knowgpt.md); the identity of the two was not checked in this pass.

# Method
- Extraction: two strategies, a subgraph method and a reinforcement-learning path policy. The policy is trained with reward +1 for reaching the target within K actions and -1 otherwise, with a conciseness term on path length.[^paper]
- Selection: a multi-armed bandit learns, per question, which combination of extraction strategy and prompt format to use, updated from LLM feedback; it balances exploitation (c times alpha) against exploration.[^paper]
- Prompt formats: three prompt translation methods are compared in the ablation (Table 4).[^paper]
- Closed LLMs are called only through their API with the constructed prompt, so no fine-tuning is needed.[^paper]

# Results
- Table 1 test accuracy (KnowGPT, GPT-3.5 base): CommonsenseQA 0.818 (IH-test), OpenBookQA 0.924, MedQA 0.781; GPT-4 row 0.786 (IH-test), 0.910, 0.763.[^paper]
- OpenBookQA official leaderboard (Table 2): KnowGPT 92.6% accuracy, against the human performance of 91.7 and GreaseLM 84.8.[^paper]
- Ablation (Table 3) and prompt-format study (Table 4) support both modules.[^paper]

# Relevance to neocortex
- Only the bandit and the prompt construction need an LLM. The RL path policy is a trained model over the KG, which would need offline training and a small policy network, which neocortex cannot host without new code.
- What neocortex already returns is a compact list of facts, which a calling agent can place in its prompt; the KnowGPT prompt format is a reference for that step, not a feature to build in the engine.
- Low priority: benchmark numbers are for multiple-choice QA with GPT-3.5 and GPT-4, not for memory recall.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2312.06185 (read in this pass)
