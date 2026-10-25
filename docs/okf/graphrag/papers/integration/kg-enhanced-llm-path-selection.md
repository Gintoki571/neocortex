---
type: "Paper"
title: "Knowledge Graph-Enhanced Large Language Models via Path Selection"
description: "KELP scores KG paths against the input text by latent semantic matching, then feeds the selected paths to the LLM (ACL Findings 2024)."
resource: "https://aclanthology.org/2024.findings-acl.376/"
tags: ["path-selection", "knowledge-graph", "reasoning", "latent-semantic-matching", "few-shot"]
year: 2024
venue: "ACL Findings 2024"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://aclanthology.org/2024.findings-acl.376/"
    title: "ACL Anthology page (metadata)"
  - id: pdf
    resource: "https://aclanthology.org/2024.findings-acl.376.pdf"
    title: "ACL Findings 2024 PDF (text extracted with pdftotext)"
---
# Summary
- KELP has three stages; the core idea is to score knowledge paths with the input text by latent semantic matching, so the LLM does not have to judge each path.[^pdf]
- Path candidates are first filtered by relations (RelationOnly Ranking), which cuts the number of paths that need encoding on large KGs.[^pdf]
- The paper notes that LLM-based selection is inflexible (binary judgment) and tends to pick only paths with direct semantic overlap with the question.[^pdf]

# Method
- Scoring: a trained encoder scores the text against each path sentence; the top-ranked paths are the contexts for the LLM.[^pdf]
- Tasks: weakly semantic knowledge (FACTKG, claims checked against DBpedia) and strongly semantic knowledge (MetaQA multi-hop questions).[^pdf]
- Settings: few-shot, with accuracy and standard deviation reported over the few-shot settings.[^pdf]

# Results
- Table 1 reports few-shot accuracy for KELP and baselines on both tasks, including a baseline that uses LLM-based evidence (LLME); the per-cell numbers were not transcribed.[^pdf]

# Relevance to neocortex
- Relevant to neocortex's path scoring: the step that matters is replacing the LLM's binary path judgment with a learned score over the text, which is a small encoder problem that could run on CPU in the engine. Read the full table before adopting.[^pdf]
- Link: [KG path beam search](/techniques/kg-path-beam-search.md), [Think-on-Graph](/papers/integration/think-on-graph.md).

[^pdf]: ACL Findings 2024 PDF (text extracted with pdftotext)
