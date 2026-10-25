---
type: Paper
title: 'Simple is Effective: The Roles of Graphs and Large Language Models in Knowledge-Graph-Based Retrieval-Augmented Generation'
description: Analysis of the roles of graphs and LLMs in KG-based RAG (ICLR 2025).
resource: https://openreview.net/forum?id=JvkuZZ04O7
tags:
- analysis
- kg-rag
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: ICLR 2025
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/html/2410.20724v4
  title: arXiv HTML full text for 2410.20724v4 (title "Simple is Effective"; read in part)
- id: openreview
  resource: https://openreview.net/forum?id=JvkuZZ04O7
  title: 'OpenReview page (blocked: verification challenge, not completed)'
---

# Summary
- The arXiv preprint with this title introduces SubgraphRAG: a lightweight MLP that scores every triple in the KG against the query, and the top-K triples form the subgraph the LLM reasons over.[^paper]
- Structure enters through directional distance encoding (DDE) of each triple relative to the query's topic entities; the MLP itself is cheap, so retrieval needs no LLM call.[^paper]
- Smaller LLMs (Llama 3.1-8B-Instruct) give competitive results on the retrieved subgraphs, and GPT-4o reaches the reported state of the art, without fine-tuning the LLM.[^paper]
- The OpenReview page was not readable (verification challenge); the identification of the preprint with the OpenReview entry rests on the title match.

# Method
- Triple scoring: an MLP over the query and triple text embeddings plus DDE, scoring all triples in parallel rather than by path search.[^paper]
- Training: weak supervision from shortest-path triples between topic and answer entities; a GPT-4o pass labels up to 20 relevant triples per test question for evaluation.[^paper]
- Subgraph size: top-K triples, default 100; the paper also reports 200 and 500.[^paper]
- Baselines in the comparison include RoG (path search), G-Retriever, cosine similarity and GNN-RAG.[^paper]

# Results
- Retrieval recall (Table 1): SubgraphRAG is best on shortest-path, GPT-4o-labelled and answer-entity recall. With GPT-4o-labelled triples RoG drops 45.6% on WebQSP and 52.2% on CWQ, while SubgraphRAG changes by -2.0% and +3.6%.[^paper]
- QA (Table 3): the paper reports up to 12% Macro-F1 and 9% Hit improvement over baselines with Llama 3.1-70B and GPT-4o, and up to 4% over prior SOTA with the 8B model. Table cells are not cleanly extractable here, so the per-row values are not quoted.[^paper]
- Efficiency: slightly slower than cosine similarity on WebQSP, and one to two orders of magnitude faster than GNN-RAG and other learned baselines, on an RTX 6000 Ada.[^paper]

# Relevance to neocortex
- The retriever is the part neocortex could use: an LLM-free, small MLP that scores candidate relation facts with a structural feature, which is the trade-off neocortex needs (fits CPU, under the latency budget, no LLM at read time).
- The top-K triple budget is the same control neocortex already has as a result limit. Training needs labelled answer paths; neocortex could collect these from its own confirmed recall outcomes, but that is new work.
- The reasoner stays with the calling agent. Adapt: test the DDE feature as a re-ranker over neocortex's relation facts before building a trainer.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2410.20724v4 (read in part)
