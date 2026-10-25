---
type: Paper
title: 'GraphReader: Building Graph-based Agent to Enhance Long-Context Abilities of Large Language Models'
description: An LLM agent that structures a long text into a graph and explores it with read-node and neighbour functions.
resource: https://aclanthology.org/2024.findings-emnlp.746/
tags:
- long-context
- graph-agent
- exploration
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: EMNLP 2024 Findings
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://aclanthology.org/2024.findings-emnlp.746/
  title: ACL Anthology abstract page
---

# Summary
- Structures a long text into a graph and explores it coarse-to-fine with predefined read-node and neighbour functions, recording insights as it goes (abstract).[^paper]
- Reports GraphReader with a 4k window outperforming GPT-4-128k across 16k to 256k contexts on LV-Eval (abstract).[^paper]

# Relevance to neocortex
- The agent explores a graph with explicit read and neighbour functions, the shape of neocortex's recall-and-expand tools. Agent-side only; watch.

[^paper]: ACL Anthology abstract page
