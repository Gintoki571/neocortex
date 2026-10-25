---
type: "Paper"
title: "Visual In-Context Learning for Large Vision-Language Models"
description: "Visual in-context learning for LVLMs: retrieve demonstration images, summarize them by intent, and compose language demonstrations."
resource: "https://arxiv.org/abs/2402.11574"
tags: ["in-context-learning", "vision", "demonstration-retrieval"]
year: 2024
venue: "ACL Findings 2024"
llm_required: both
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2402.11574"
    title: "arXiv 2402.11574"
---
# Summary
- VICL retrieves demonstration images with a retrieve-and-rerank paradigm.[^paper]
- It summarizes images by task intent and composes language demonstrations to cut token count.[^paper]

# Method
- Visual demonstration retrieval, intent-oriented image summarization, and intent-oriented demonstration composition (abstract level).[^paper]

# Results
- Evaluated on five visual reasoning datasets per the abstract; numbers not read.[^paper]

# Relevance to neocortex
- Vision domain; no overlap with neocortex's text and graph memory. Skip.

[^paper]: arXiv 2402.11574
