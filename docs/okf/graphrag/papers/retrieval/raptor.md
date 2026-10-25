---
type: Paper
title: 'RAPTOR: Recursive Abstractive Processing for Tree-Organized Retrieval'
description: Recursively embeds, clusters and summarises chunks into a tree, then retrieves across tree levels at inference.
resource: https://arxiv.org/abs/2401.18059
tags:
- tree-index
- recursive-summary
- clustering
- retrieval
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: ICLR 2024
llm_required: indexing
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/html/2401.18059
  title: arXiv HTML full text (read in this pass)
- id: abs
  resource: https://arxiv.org/abs/2401.18059
  title: arXiv abstract page
---

# Summary
- Splits a corpus into chunks of at most 100 tokens, embeds them, clusters them, summarises each cluster with an LLM, and repeats until a root is reached, giving a tree whose levels hold text at different abstraction levels.[^paper]
- At query time the collapsed-tree variant scores every node in every layer against the query and keeps the top nodes up to a 2000-token budget, which the authors report beat layer-by-layer tree traversal.[^paper]
- Headline claim: coupling RAPTOR retrieval with GPT-4 gives a 20% absolute accuracy gain on QuALITY.[^paper]
- Full text was read (arXiv HTML); the parsed text was truncated at 60k characters, so the appendix on scalability was not read in full.

# Method
- Chunking: sentences are kept whole, and a sentence that would exceed the 100-token limit moves to the next chunk.[^paper]
- Embedding: SBERT (multi-qa-mpnet-base-cos-v1) embeds chunks and every summary node.[^paper]
- Clustering: Gaussian Mixture Model soft clustering after UMAP dimensionality reduction; the number of clusters is chosen by BIC, and a cluster whose combined text exceeds the summariser's token threshold is clustered again recursively.[^paper]
- Summarisation: GPT-3.5-turbo summarises each cluster; the summary becomes a parent node.[^paper]
- Querying: collapsed tree, all nodes ranked by cosine similarity to the query, top nodes until 2000 tokens (about the top 20 nodes). It outperformed tree traversal with top-k=1 to 3 per layer on a QASPER subset, so it was used for all main results.[^paper]

# Results
- QASPER F1 (Table 3): RAPTOR 53.1 (GPT-3), 55.7 (GPT-4), 36.6 (UnifiedQA 3B); DPR 51.3 / 53.0 / 32.1; BM25 46.6 / 50.2 / 26.4.[^paper]
- QuALITY accuracy (Table 4, dev set): RAPTOR 62.4 with GPT-3 and 56.6 with UnifiedQA 3B; DPR 60.4 / 53.9; BM25 57.3 / 49.9.[^paper]
- QuALITY test and hard subset (Table 7): RAPTOR + GPT-4 82.6 / 76.2, against CoLISA (DeBERTaV3-large) 62.3 / 54.7.[^paper]
- NarrativeQA (Table 1 and 6, UnifiedQA 3B): RAPTOR raises ROUGE, BLEU and METEOR for SBERT, BM25 and DPR retrievers; with SBERT, ROUGE 30.87% with RAPTOR against 29.26% without.[^paper]

# Relevance to neocortex
- Write-time summarisation needs an LLM. neocortex cannot run it inside the engine; the calling agent would have to write the summary nodes through `remember`, which fits the existing contract.
- Retrieval is LLM-free: embed every node, rank the whole tree with cosine similarity under a token budget. This is a plain vector search over one table, which sqlite-vec already does. The engine would need only a parent-child link between memories.
- Clustering with UMAP and GMM is CPU-feasible at a few thousand memories, but it adds a periodic consolidation job. The collapsed-tree rule (rank all levels together, cap by tokens) is the cheap part to borrow now.
- Relates to community summaries in [GraphRAG-style community layers](/papers/organization/graphrag-local-to-global.md) and to neocortex's consolidation ideas.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2401.18059 (read in this pass)
