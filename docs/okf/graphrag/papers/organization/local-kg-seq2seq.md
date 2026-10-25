---
type: Paper
title: Using Local Knowledge Graph Construction to Scale Seq2Seq Models to Multi-Document Inputs
description: Local KG construction to feed multi-document inputs to seq2seq models (ACL 2019).
resource: https://aclanthology.org/D19-1428.pdf
tags:
- kg-construction
- seq2seq
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2019
venue: ACL 2019
llm_required: none
neocortex_relevance: watch
sources:
- id: paper
  resource: https://aclanthology.org/D19-1428.pdf
  title: ACL Anthology PDF (D19-1428, downloaded and read with pdftotext -layout)
- id: arxiv
  resource: https://arxiv.org/abs/1910.08435
  title: arXiv preprint 1910.08435 (abstract read via arXiv API)
---

# Summary
- For a query, a small local knowledge graph is built from the web search results: coreference and information extraction turn the text into nodes and edges, and duplicate nodes and edges are merged to remove redundancy.[^paper]
- The graph is linearised into a sequence so that a standard Seq2Seq model (Transformer) can encode it, instead of feeding retrieved text paragraphs.[^paper]
- Tasks: long-form question answering (ELI5) and multi-document summarisation (WikiSum).[^arxiv]
- The PDF was read in full with pdftotext -layout; the table captions are partly garbled in the extraction.

# Method
- Graph construction: coreference resolution and information extraction over each search result, then merging of identical nodes and edges to build a per-query graph.[^paper]
- Linearisation: the graph is serialised into a token sequence; graph nodes and edges receive embeddings that encode weight and query relevance; the paper also uses a gated graph weight embedding.[^paper]
- Model: a Transformer encoder for the linearised graph, with a hierarchical attention over the text and graph encodings, and a multi-task variant; the paper reports an E-MCA (memory-compressed attention) variant at 11K input tokens.[^paper]
- Input budget: the graph variants use 850 tokens for the default, and up to 11K tokens for E-MCA.[^paper]

# Results
- ELI5 ROUGE-1 / ROUGE-2 / ROUGE-L (table block as extracted): Q + D to A with TF-IDF 28.3 / 5.1 / 22.8; Multi-task 28.9 / 5.4 / 23.1; Multi-task Graph 29.5 / 5.6 / 23.6; Multi-task Graph with Top-100 Attention 29.7 / 5.7 / 23.8; Multi-task Graph with E-MCA (11K input) 30.0 / 5.8 / 24.0.[^paper]
- The text reports about a 0.6 ROUGE-1 gain from the graph input over the Multi-task baseline on ELI5; the table caption naming the dataset was not recovered, so the WikiSum block is not quoted.[^paper]

# Relevance to neocortex
- Building a graph from text needs an information extractor. This paper's extractor is not an LLM, but it is a trained coreference and IE pipeline, so it does not fit the rules-only extraction that neocortex measured at 0.07 F1.
- The linearised graph is a generation-side input. neocortex returns facts and does not generate, so the relevance is to the agent's prompt format, not to the engine.
- Watch: the gains are ROUGE on summarisation tasks, not on recall or multi-hop QA.

[^paper]: ACL Anthology D19-1428 PDF, https://aclanthology.org/D19-1428.pdf (read with pdftotext -layout)
[^arxiv]: arXiv abstract, https://arxiv.org/abs/1910.08435
