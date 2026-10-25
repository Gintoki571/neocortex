---
type: Paper
title: Knowledge Graph Prompting for Multi-Document Question Answering
description: Builds a passage and structure graph over several documents, then uses an LLM-guided traversal agent to select the prompt context.
resource: https://arxiv.org/abs/2308.11730
tags:
- passage-graph
- multi-document-qa
- graph-traversal
- prompting
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2023
llm_required: both
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/html/2308.11730
  title: arXiv HTML full text (read in this pass)
- id: abs
  resource: https://arxiv.org/abs/2308.11730
  title: arXiv abstract page
---

# Summary
- Knowledge Graph Prompting (KGP) builds a graph over multiple documents whose nodes are passages and document structures (pages, tables), with edges for semantic or lexical similarity and structural relations.[^paper]
- A traversal agent walks the graph and gathers supporting passages; the authors use it to formulate the context for an LLM reader.[^paper]
- The traversal agent can be an off-the-shelf LLM (ChatGPT with few-shot demonstrations), or a fine-tuned T5-large, LLaMA-7B (LoRA) or MDR, and the authors report a fine-tuned agent (KGP-T5) as best on the reported benchmarks.[^paper]

# Method
- Graph construction: passage nodes linked by KNN over sentence-transformer embeddings (KNN-ST) or by TAGME entity links; the paper reports that denser graphs improve F1 and raise traversal latency.[^paper]
- Traversal: at each step the agent scores the candidate neighbours of the current nodes, keeps the top-k passage nodes by rank, and updates a candidate-neighbour queue and a reasoning-path queue (Algorithm 1 and 2).[^paper]
- Fine-tuning: LLaMA-7B and T5 are instruction-tuned on HotpotQA rewritten as "What evidence do we need to answer the question given the current evidence".[^paper]
- Comparison setting: 30 retrieved passages across baselines, with ChatGPT as the downstream reader for the IRCoT comparison.[^paper]

# Results
- Table 1 (accuracy / EM / F1 for HotpotQA, IIRC, 2WikiMQA, MuSiQue): KGP-T5 gets 76.53 / 46.51 / 66.77 on HotpotQA, 63.50 / 39.80 / 53.50 on 2WikiMQA and 50.92 / 27.90 / 41.19 on MuSiQue, against MDR 75.30 / 45.55 / 65.16, 63.00 / 36.00 / 52.44 and 48.39 / 23.49 / 37.03 on the same columns.[^paper]
- Table 1 also reports PDFTriage Struct-EM: KGP-T5 67.00.[^paper]
- Table 2: the T5 traversal agent gives the best F1 on MuSiQue (41.19) among the traversal agents, with ChatGPT at 38.66 and LLaMA at 40.01.[^paper]

# Relevance to neocortex
- A passage-node graph with similarity edges and a scored traversal is what HippoRAG-style systems do with personalised PageRank. The paper's traversal is an agent decision, so an engine-side version would replace the agent with a score function.
- The LLM-guided traversal and the fine-tuned T5 or LLaMA both need a model at query time, so neocortex can use the idea only with a caller-side agent.
- Cheapest borrowable idea: keep passages linked by similarity edges inside the store, then expand the hit set through those edges under a path budget. This needs no LLM.
- Reference for the algorithm: [HippoRAG 2](/papers/retrieval/hipporag-2.md).

[^paper]: arXiv HTML full text, https://arxiv.org/html/2308.11730 (read in this pass)
