---
type: Paper
title: 'Graph of Records: Boosting Retrieval Augmented Generation for Long-context Summarization with Graphs'
description: Links retrieved text chunks to the LLM responses that used them, then trains a GNN with a BERTScore-based objective to retrieve for global summarisation.
resource: https://arxiv.org/abs/2410.11001
tags:
- graph-of-responses
- long-context-summarization
- gnn
- self-supervised
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://arxiv.org/html/2410.11001
  title: arXiv HTML full text (read in this pass)
- id: abs
  resource: https://arxiv.org/abs/2410.11001
  title: arXiv abstract page
---

# Summary
- Builds a graph whose nodes are document chunks and the LLM-generated responses to queries simulated over those chunks, with an edge from each retrieved chunk to the response that used it.[^paper]
- A graph neural network produces node embeddings, trained self-supervised with a contrastive loss and a BERTScore-based ranking loss, so reference-summary signal reaches the node embeddings.[^paper]
- Evaluated against 12 baselines on four long-context summarisation sets (QMSum, AcademicEval, WCEP, BookSum) by the authors.[^paper]

# Method
- Chunking: LangChain TokenTextSplitter with chunk size 256 and overlap 32.[^paper]
- Default retriever: a sentence-embedding model retrieves the 6 most similar chunks, fed to LLaMA-2-7b-chat.[^paper]
- Graph construction: queries are simulated from random chunks, and each (retrieved chunks, response) pair adds edges; the simulated pairs also serve as training data.[^paper]
- Objective: L = L_CL + alpha * L_RANK, where L_CL is an InfoNCE-style contrastive loss over query-node similarity with temperature tau, and L_RANK orders nodes by BERTScore against the reference.[^paper]
- Retrieval: the trained GNN node embeddings are used to rank nodes against the query; the parsed text did not give the exact top-k rule used for the summary context.[^paper]

# Results
- Rouge-L (Table 1): GoR 19.8 on QMSum (tied with Gemma-8K at 19.8), 24.7 on AcademicEval (best baseline 23.6), 18.1 on WCEP (best baseline 16.7, Mistral-8K), 14.9 on BookSum (best baseline 14.4).[^paper]
- The paper also reports ROUGE-1 and ROUGE-2; GoR is highest there on all four sets.[^paper]

# Relevance to neocortex
- Storing an agent's past answers as nodes linked to the memories they used is already close to what neocortex's `remember` does with explicit entities and relations. The GoR graph is a link of "answer used chunk" edges, which neocortex could add as a plain relation type.
- The retrieval side depends on a GNN trained on simulated queries, and the writer depends on an LLM, so the engine cannot run either. The LLM-free part is storing and traversing the edges.
- Watch only: the gains are on summarisation, not on the fact and multi-hop questions neocortex is measured on.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2410.11001 (read in this pass)
