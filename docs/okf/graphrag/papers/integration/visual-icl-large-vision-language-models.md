---
type: "Paper"
title: "Visual In-Context Learning for Large Vision-Language Models"
description: "VICL: retrieve demonstration images (retrieve and rerank), summarise them by task intent and compose text demonstrations to cut tokens for LVLMs (ACL Findings 2024)."
resource: "https://arxiv.org/abs/2402.11574"
tags: ["in-context-learning", "lvlm", "visual-demonstrations", "retrieval", "rerank"]
year: 2024
venue: "ACL Findings 2024"
llm_required: both
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2402.11574"
    title: "arXiv abstract page"
  - id: html
    resource: "https://arxiv.org/html/2402.11574"
    title: "arXiv HTML full text (Sections 3-4; Table 1 text read, numeric cells not extracted)"
---
# Summary
- Visual demonstration retrieval uses a retrieve-and-rerank pipeline over images.[^paper][^html]
- Intent-oriented image summarisation turns each demonstration image into task-specific text (task intent plus visual parsing), and intent-oriented composition builds language demonstrations that reduce token count.[^paper][^html]
- Five image reasoning datasets: EmoSet, Emotion6, UnBiasedEmo, CIFAR10 and MNIST; four LVLMs: LLaVA-7B, MiniGPT-4, Qwen-VL and one more LVLM.[^html]

# Method
- Retrieve, rerank, summarise, compose: the four stages from the abstract, plus an information-flow analysis of where the demonstrations act.[^html]
- Baselines are zero-shot prompting, plain ICL with image demonstrations, and VICL.[^html]

# Results
- The paper states that VICL consistently outperforms ICL and zero-shot across all models and datasets (Table 1, Section 4.2).[^html]
- The numeric cells of Table 1 were not recoverable from the HTML text extraction, so no per-dataset numbers are recorded here.

# Relevance to neocortex
- Vision domain, with no overlap with neocortex's text and graph memory. Skip.
- The one transferable pattern is the rerank-after-retrieve step, which is already how neocortex combines BM25 and dense results. Nothing else applies.

[^paper]: arXiv abstract page
[^html]: arXiv HTML full text (Sections 3-4, Table 1 caption and text)
