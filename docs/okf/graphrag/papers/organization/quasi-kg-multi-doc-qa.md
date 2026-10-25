---
type: Paper
title: Answering complex questions by joining multi-document evidence with quasi knowledge graphs
description: Joins evidence across documents through a quasi-KG (SIGIR 2019).
resource: https://dl.acm.org/doi/10.1145/3331184.3331252
tags:
- quasi-kg
- multi-document
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2019
venue: SIGIR 2019
llm_required: none
neocortex_relevance: watch
sources:
- id: paper
  resource: https://dl.acm.org/doi/10.1145/3331184.3331252
  title: 'ACM DL page (blocked: Cloudflare challenge, see Summary)'
- id: arxiv
  resource: https://arxiv.org/abs/1908.00469
  title: arXiv preprint 1908.00469 with the same title (abstract read via arXiv API)
---

# Summary
- QUEST answers complex questions from text on the fly: it builds a noisy quasi knowledge graph from dynamically retrieved entity names and relational phrases, with node and edge weights.[^arxiv]
- The quasi graph is augmented with types and semantic alignments, and the answers come from a Group Steiner Tree computation over it.[^arxiv]
- The method is unsupervised, so it needs no training data, which the authors present as its main advantage for ad hoc topics.[^arxiv]
- The ACM page was blocked; the arXiv preprint with the same title was read (abstract and a few body passages). The numbers in the results tables were not extracted in this pass.

# Method
- Retrieval of entity names and relational phrases from several documents, weighted into a quasi KG.[^arxiv]
- A recall-oriented open information extraction step, designed by the authors because the precision-oriented Stanford OpenIE loses long-sentence facts.[^arxiv]
- Answer computation: Group Steiner Tree over the weighted quasi KG, which joins evidence across documents.[^arxiv]
- QA metrics: Precision@1 (correct answer at rank 1) and Hit@5.[^arxiv]

# Results
- Abstract: QUEST "substantially outperforms state-of-the-art baselines" on two complex-question benchmarks; the paper reports it beats DrQA and other graph methods at almost all settings. Exact numbers were not recorded in this pass.[^arxiv]

# Relevance to neocortex
- Unsupervised and LLM-free: the graph, the weights and the Steiner-style answer search need no model call. This is the cleanest match among the GraphRAG papers for neocortex's constraints.
- Relevant to the three-hop weak spot: joining partial evidence across documents is the same problem as the middle hop. The recall-over-precision lesson applies to neocortex's relation extraction too: losing facts hurts QA more than extra noisy edges.
- Adapt: a Steiner-tree search over the entity graph is a candidate for the multi-hop path step. Compare against the existing beam first; the Steiner search cost grows with the graph.

[^paper]: ACM DL, https://dl.acm.org/doi/10.1145/3331184.3331252 (blocked)
[^arxiv]: arXiv abstract and body text, https://arxiv.org/abs/1908.00469 (same title)
