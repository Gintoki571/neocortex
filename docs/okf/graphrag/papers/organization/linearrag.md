---
type: Paper
title: 'LinearRAG: Linear Graph Retrieval Augmented Generation on Large-scale Corpora'
description: Relation-free Tri-graph of entities, sentences and passages, built with spaCy NER, plus two-stage entity activation and PPR passage ranking; zero LLM tokens for indexing and retrieval.
resource: https://arxiv.org/abs/2510.10114
tags:
- graphrag
- tri-graph
- spacy-ner
- ppr
- no-relation-extraction
- multi-hop
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2026
venue: ICLR 2026
llm_required: none
neocortex_relevance: adopt
sources:
- id: paper
  resource: https://arxiv.org/abs/2510.10114
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2510.10114
  title: arXiv HTML full text
---

# Summary
- Drops explicit relation extraction; keeps entity-sentence-passage links from lightweight NER, arguing aligned entities are the useful anchors.[^html]
- Reports indexing time reduced by over 77% versus relation-based GraphRAG, and zero token usage for the full pipeline.[^html]
- Uses GPT-4o-mini only for answer generation and GPT-based evaluation in the experiments; retrieval itself makes no LLM call.[^html]

# Method
- Tri-graph: passages are split into sentences by punctuation; spaCy (BERT-based models) extracts entities; contain matrix (passage-entity) and mention matrix (sentence-entity) are the only edges.[^html]
- Stage 1 (entity activation): query entities are matched to the nearest graph entity; activation spreads over the sentence-entity subgraph for n hops (n is generally <= 4) using matrix products, with sentence similarity to the query as weights.[^html]
- Stage 2 (global importance): PPR with damping d (typically 0.85) on the passage-entity bipartite graph; passage initial scores combine lambda times dense similarity with entity activation terms, scaled by a passage weight W_p; top-k passages are returned.[^html]
- Experiments use all-mpnet-base-v2 for embeddings and k = 5 for all methods.[^html]

# Results
- Evaluated on HotpotQA, 2Wiki, MuSiQue and a medical GraphRAG-Bench set; outperforms GraphRAG baselines in the paper's Table 1 (exact numbers not extracted here).[^html]
- Efficiency table: LightRAG indexing takes 4,933 s on 2Wiki in the paper's measurements; LinearRAG is reported as the fastest end-to-end pipeline with zero tokens.[^html]
- Ablations on entity activation and passage scoring confirm both stages contribute (Figure 4).[^html]

# Relevance to neocortex
- Strongest match in this section for neocortex: NER with spaCy is LLM-free and CPU-cheap, the graph is a sparse matrix, and retrieval is matrix products plus PPR. This directly addresses weak spot (2) (relation extraction without an LLM: rules 0.07 F1, GLiNER 0.20), because it avoids relation extraction altogether.
- Multi-hop comes from entity activation over sentences; weak spot (1), paraphrased middle hops, is only partly covered since matching is still entity-string based plus similarity.
- Fit to neocortex: entity and passage nodes map onto neocortex's entity graph and memories; the sentence layer would be new. Retrieval cost is bounded by n <= 4 matrix iterations plus PPR, which should fit a 100 ms budget, but this is not measured in the paper's text read.
- Related: [LLM-free entity extraction](/techniques/llm-free-entity-extraction.md), [personalized PageRank seeding](/techniques/ppr-seeding.md).

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
