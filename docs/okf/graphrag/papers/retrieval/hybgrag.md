---
type: Paper
title: 'HybGRAG: Hybrid Retrieval-Augmented Generation on Textual and Relational Knowledge Bases'
description: Combines a bank of text and graph retrievers with a router and a critic that feeds corrective feedback back into retrieval for hybrid questions.
resource: https://arxiv.org/abs/2412.16311
tags:
- hybrid-retrieval
- semi-structured-kb
- critic
- stark
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/html/2412.16311
  title: arXiv HTML full text (read in this pass)
- id: abs
  resource: https://arxiv.org/abs/2412.16311
  title: arXiv abstract page
---

# Summary
- Targets "hybrid" questions over a semi-structured knowledge base (text documents linked by relations), which need both text and relation evidence.[^paper]
- Retriever bank: a text retriever (vector similarity), graph retrieval (personalised PageRank), a hybrid module and a router that chooses per question.[^paper]
- A critic module reviews the retrieved output and feeds corrective feedback to refine entity and relation extraction; more self-reflection iterations improve results.[^paper]
- Evaluated on STaRK-MAG and STaRK-Prime retrieval and on CRAG end-to-end QA.[^paper]

# Method
- Hybrid retrieval module: the paper reports a relative Hit@1 gain of about 24% over AvaTaR on STaRK while using 2 API calls, where AvaTaR needs at least 500 API calls during training.[^paper]
- Critic: a validator and a commenter (5-shot) are compared; the paper also tests an oracle variant that uses ground truth at inference.[^paper]
- Self-reflection: performance rises as iterations go from 1 to 2 and beyond.[^paper]
- Base models: Claude 3 Sonnet as main LLM; Claude 3 Haiku is about 1.96x faster with lower scores (Table 6); Llama 3.1 70B is used for CRAG (Table 8).[^paper]
- Starred baselines (Think-on-Graph, VSS with LLM reranker) were run on only 10% of test questions because the full run took more than a week, per the Table 5 caption.[^paper]

# Results
- STaRK-MAG (Table 5), Hit@1 / MRR: HybGRAG 0.6540 / 0.6980, against AvaTaR 0.4436 / 0.5115 and VSS with LLM reranker 0.3654 / 0.4415.[^paper]
- STaRK-Prime Hit@1 / MRR: HybGRAG 0.2856 / 0.3449, against AvaTaR 0.1844 / 0.2673.[^paper]
- CRAG end-to-end accuracy (Table 8): with Claude 3 Sonnet, HybGRAG 0.6322 against Text & Graph RAG 0.5820; with Llama 3.1 70B, 0.5206 against Graph-Only RAG 0.4861.[^paper]
- Hallucination with Claude 3 Sonnet: 0.2959, against 0.3416 for Text & Graph RAG (lower is better).[^paper]

# Relevance to neocortex
- The retriever bank is the LLM-free part: BM25 or vector text retrieval plus relation-based PageRank, fused. neocortex already fuses BM25, vectors and graph with RRF, so the bank is close to existing parts.
- The router, the critic and the entity re-extraction all need an LLM, so on neocortex they stay on the calling agent's side, as the paper's own API cost argument suggests.
- Relevant to the weak spot of relation extraction from text: a critic loop is the agent-side remedy when rules-only extraction misses a relation.
- Related: [HippoRAG 2](/papers/retrieval/hipporag-2.md) for the PPR side.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2412.16311 (read in this pass)
