---
type: "Paper"
title: "Chain-of-Knowledge: Grounding Large Language Models via Dynamic Knowledge Adapting over Heterogeneous Sources"
description: "Prepares rationales, then adapts them step by step with grounding from heterogeneous knowledge sources when answers disagree."
resource: "https://arxiv.org/abs/2305.13269"
tags: ["in-context-learning", "grounding", "heterogeneous-sources"]
year: 2023
venue: "ICLR 2024"
llm_required: both
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2305.13269"
    title: "arXiv 2305.13269"
---
# Summary
- CoK has three stages: reasoning preparation, dynamic knowledge adapting, and answer consolidation.[^paper]
- Rationales are corrected step by step only when the sampled answers lack majority consensus.[^paper]

# Method
- Multi-sample rationale generation with consensus check, then knowledge adaptation (abstract level).[^paper]

# Results
- Not read beyond the abstract.[^paper]

# Relevance to neocortex
- A prompting pipeline with several LLM calls per question. Not an engine feature; skip for neocortex, but the consensus check is a cheap idea to borrow on the agent side.

[^paper]: arXiv 2305.13269
