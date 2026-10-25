---
type: Paper
title: 'AGRAG: Advanced Graph-based Retrieval-Augmented Generation for LLMs'
description: Builds a statistics-based graph and retrieves an explicit Minimum Cost Maximum Influence subgraph as reasoning paths for the LLM.
resource: https://arxiv.org/abs/2511.05549
tags:
- graph-rag
- subgraph-retrieval
- steiner-tree
- entity-extraction
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
llm_required: both
neocortex_relevance: adapt
sources:
- id: paper
  resource: https://arxiv.org/html/2511.05549
  title: arXiv HTML full text (read in part; truncated at 60k characters)
- id: abs
  resource: https://arxiv.org/abs/2511.05549
  title: arXiv abstract page
---

# Summary
- Replaces LLM entity extraction with a statistics-based score (equation 2 and threshold tau), which the authors say avoids hallucination in graph construction.[^paper]
- Retrieval is posed as a Minimum Cost Maximum Influence (MCMI) subgraph problem, proved NP-hard by reduction to Steiner tree, and solved with a greedy algorithm based on Mehlhorn's approximation; the authors state a 2-approximation.[^paper]
- The subgraph is passed to the LLM as explicit reasoning paths, so cycles are allowed, unlike tree-shaped paths.[^paper]
- The LLM is still used for relation extraction (equation 4) and for triplet filtering (equation 9), so only the entity step is LLM-free.[^paper]
- Only the first 60k characters of the HTML were returned, so the later experimental setup and remaining tables were not read.

# Method
- Entity scoring: ER(v,t) combines term count and inverse chunk frequency, and an entity enters a chunk when ER exceeds tau.[^paper]
- Relations: an LLM extracts triples per chunk (prompt p_re); synonym edges join entities whose embedding cosine exceeds phi.[^paper]
- Mapping: the top k_a triples by query-fact cosine form the raw set, which an LLM filters (prompt p_tf).[^paper]
- Influence: personalised PageRank over the graph, with the filtered facts as the personalisation vector and damping factor d.[^paper]
- Edge cost: c(e) = (1 - MS(q, f_e)) / 2, where MS is cosine similarity between the query and the fact text; pseudo-edges to a hub node cost 10 to penalise paths through it.[^paper]
- Greedy MCMI: start from the minimum spanning tree of the mapped triples, then repeatedly add the neighbour with the lowest cost-to-influence ratio c/s_v while it is below the current subgraph ratio r(G_MCMI).[^paper]
- Hybrid retrieval: k_r text chunks come from a mix of hybrid similarity (BM25 and embedding); the final prompt combines chunks with the subgraph string.[^paper]

# Results
- Evaluation uses GraphRAG-Bench (Novel and Medical) plus WOS and IFS-REL text classification, with Qwen2.5-14B-Instruct as backbone.[^paper]
- Efficiency (Table IV, Novel dataset): AGRAG indexing 65 min and query 142 min, against HippoRAG2 95 / 134, GraphRAG 347 / 205 and NaiveRAG 13 / 371; input tokens per query 6,487 against HippoRAG2 8,138 and NaiveRAG 9,437.[^paper]
- Effectiveness (Table III): in the parsed table the AGRAG row appears to hold the top value in most metric columns across the five tasks; the column layout is garbled in the extraction, so no single headline number is quoted here.[^paper]
- Top-k is 5 for all graph RAG systems, matching HippoRAG2 and Fast-GraphRAG.[^paper]

# Relevance to neocortex
- The LLM-free parts are the statistics-based entity score and the greedy subgraph search over the entity graph. Both are CPU-cheap and would fit neocortex's graphqlite layer, which already does personalised PageRank.
- The relation extraction and filtering steps need an LLM, so on neocortex they would stay on the agent side. Without them, the subgraph idea is only useful if relations come from the caller, as they do today.
- Candidate to test against the entity graph for the three-hop weak spot: the Steiner-style subgraph gives explicit chains rather than a single beam. Cost is greedy and bounded, but the cost constant and the hub penalty need tuning.
- Related: [personalised PageRank seeding](/papers/retrieval/hipporag-2.md), and [Steiner subgraph retrieval](/techniques/steiner-subgraph-retrieval.md) if that technique file exists in the bundle.

[^paper]: arXiv HTML full text, https://arxiv.org/html/2511.05549 (read in part)
