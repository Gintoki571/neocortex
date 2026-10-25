---
type: Paper
title: Answering Complex Open-Domain Questions with Multi-Hop Dense Retrieval
description: Recursive dense retrieval that finds the sequence of passages for multi-hop questions, with the next query formed from the question plus previously retrieved passages, using beam search.
resource: https://arxiv.org/abs/2009.12756
tags:
- multi-hop
- dense-retrieval
- beam-search
- hotpotqa
- no-graph
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2020
llm_required: none
neocortex_relevance: adopt
sources:
- id: paper
  resource: https://arxiv.org/abs/2009.12756
  title: arXiv abstract
- id: html
  resource: https://arxiv.org/html/2009.12756
  title: arXiv HTML full text
---

# Summary
- Multi-hop retrieval with a single dense encoder and no corpus hyperlinks or entity markers, applicable to any text corpus.[^paper]
- Reports state-of-the-art on HotpotQA and multi-evidence FEVER; matches the best published HotpotQA accuracy while being 10 times faster at inference.[^paper]

# Method
- Each hop encodes the question concatenated with previously retrieved passages; the retriever is trained as DPR with positive and negative passages.[^html]
- Inference: encode the whole corpus once into a vector index; beam search keeps top-k passage sequences, scored by the sum of inner products across hops.[^html]

# Results
- Results on HotpotQA and multi-evidence FEVER (numbers in the paper's tables, not extracted here).[^paper]

# Relevance to neocortex
- A no-LLM, no-graph answer to middle-hop retrieval: each hop re-queries the vector index with the question plus evidence so far. It targets weak spot (1) on neocortex's dense side and needs only the existing UltraBERT embeddings, with a bounded beam.
- Cost: one vector lookup per hop per beam; a 3-hop chain on 10k memories should be checked against the ~100 ms budget.
- Training: the retriever is trained DPR-style with positive and negative passages; neocortex would need to train a hop-conditioned retriever or use UltraBERT as-is, which the paper does not test.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text
