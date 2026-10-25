---
type: Paper
title: 'SayPlan: Grounding Large Language Models using 3D Scene Graphs for Scalable Robot Task Planning'
description: Robot task planning where an LLM does semantic search over a collapsed 3D scene graph, then plans with a classical path planner.
resource: https://arxiv.org/abs/2307.06135
tags:
- robotics
- scene-graph
- semantic-search
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2023
venue: CoRL 2023
llm_required: both
neocortex_relevance: skip
sources:
- id: paper
  resource: https://arxiv.org/abs/2307.06135
  title: arXiv abstract
---

# Summary
- Uses the hierarchy of 3D scene graphs so an LLM can search for task-relevant subgraphs, and a classical planner for paths (abstract).[^paper]

# Relevance to neocortex
- Robotics; skip. The collapse-then-search over a hierarchy is a general idea.

[^paper]: arXiv abstract
