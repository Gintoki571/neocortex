---
type: "Benchmark"
title: "TutorQA"
description: "Expert-verified NLP education QA benchmark of 1,200 questions in six concept-graph tasks, introduced in the Graphusion paper (arXiv 2407.10794)."
resource: "https://arxiv.org/abs/2407.10794"
tags: ["benchmark", "concept-graph", "education", "nlp", "path-search"]
year: 2024
venue: "arXiv 2024 (README row)"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2407.10794"
    title: "arXiv 2407.10794 (abstract; the paper is titled Graphusion and introduces TutorQA)"
  - id: html
    resource: "https://arxiv.org/html/2407.10794"
    title: "arXiv HTML full text (Sections 4-6, Tables 1-5)"
  - id: code
    resource: "https://github.com/IreneZihuiLi/CGPrompt"
    title: "Code and data link as printed in the paper (repo not fetched)"
---
# Summary
- TutorQA is introduced in the Graphusion paper, whose arXiv title is "Graphusion: Leveraging Large Language Models for Scientific Knowledge Graph Fusion and Construction in NLP Education". The README's arXiv link is correct for TutorQA.[^html]
- Six expert-verified tasks, 1,200 QA pairs in total, built on an NLP concept graph constructed by the Graphusion pipeline.[^html]
- Tasks: relation judgment (T1), prerequisite prediction (T2), path searching (T3), subgraph completion (T4), similar concepts (T5) and open-ended idea generation (T6, judged by experts).[^html]

# Method
- Graphusion builds the NLP KG from ACL, EMNLP and NAACL papers (2021-2023) with an LLM pipeline; TutorQA questions are answered by a command-query step followed by answer generation over the graph.[^html]
- Metrics: accuracy for T1 and T4 (binary), a similarity score for T2 and T3 (concept lists), hit rate for T5, and expert ratings (relevancy, coverage, convincity, factuality) for T6.[^html]

# Results
- Table 4, GPT-4o, zero-shot vs graph-augmented: T1 accuracy 69.20 vs 92.00; T2 similarity 64.42 vs 80.29; T3 similarity 66.61 vs 77.85; T4 accuracy 44.00 vs 50.00; T5 hit rate 11.45 vs 15.65.[^html]
- Table 5, expert scores on T6: factuality 4.625 vs 4.770 and coverage 4.840 vs 4.905 (zero-shot vs graph-augmented).[^html]

# Relevance to neocortex
- Task 3 (path searching over a concept graph, scored by a similarity to the gold concept list) matches neocortex's multi-hop path search over entity relations. The graph is small and domain-specific, so it is a quick offline check of path recall, provided the KG is ingested as memories.[^html]
- The gains come from an LLM reading the graph, so the scores are only meaningful on the agent side. Relation judgment (T1) needs no generation and could be scored with neocortex's fact store plus an LLM-free comparison.
- Watch: the benchmark is small and the graph was LLM-built, so it measures graph quality as much as retrieval.

[^paper]: arXiv 2407.10794 (abstract)
[^html]: arXiv HTML full text (Sections 4-6, Tables 1-5)
