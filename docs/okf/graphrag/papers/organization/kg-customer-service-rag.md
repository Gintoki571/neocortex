---
type: Paper
title: Retrieval-augmented generation with knowledge graphs for customer service question answering
description: KG-augmented RAG for customer service (SIGIR 2024).
resource: https://dl.acm.org/doi/abs/10.1145/3626772.3661370
tags:
- customer-service
- kg-rag
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: SIGIR 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://dl.acm.org/doi/abs/10.1145/3626772.3661370
  title: 'ACM DL page (blocked: Cloudflare challenge, see Summary)'
- id: arxiv
  resource: https://arxiv.org/html/2404.17723
  title: arXiv preprint 2404.17723 with the same title (full HTML read in part)
- id: arxiv-abs
  resource: https://arxiv.org/abs/2404.17723
  title: arXiv abstract page
---

# Summary
- Customer-service question answering over past issue tickets: the tickets are turned into a knowledge graph that keeps each ticket's internal structure and links related tickets, instead of treating the corpus as plain text.[^arxiv]
- At query time an LLM maps the query onto the graph and retrieves related sub-graphs to generate the answer.[^arxiv]
- Reported as deployed in LinkedIn's customer service team for about six months.[^arxiv-abs]
- The ACM page was blocked (Cloudflare challenge in plain, stealth and rendered fetches). The text read is the arXiv preprint with the same title; the published version may differ.

# Method
- Ticket graph: each issue has sections (for example issue summary, issue description) as nodes, and implicit connections between tickets come from embedding similarity of ticket titles with a threshold.[^arxiv]
- Text embeddings are E5; each entity value is matched to graph nodes in its section by cosine similarity.[^arxiv]
- Query processing: an LLM rewrites the query, maps it to the graph schema and produces a graph query (Cypher for Neo4j in the paper's example), then answers from the retrieved sub-graph. The paper used GPT-4 for both groups in its comparison.[^arxiv]
- Long tickets are split to fit the embedding model's context, which the paper names as a source of lost connections.[^arxiv]

# Results
- Retrieval: MRR improves 77.6% over the baseline on the authors' benchmark (abstract).[^arxiv-abs]
- Generation: BLEU improves by 0.32 (abstract).[^arxiv-abs]
- Operations: the median per-issue resolution time falls by 28.6% in the deployed group (Table 3 text).[^arxiv]

# Relevance to neocortex
- Structure-preserving records with explicit inter-record similarity edges are close to neocortex's memory graph. The graph build needs only embeddings and a threshold, which is LLM-free.
- The query-to-graph-query step needs an LLM, so on neocortex it belongs to the calling agent, which would issue structured recall calls.
- Adapt: the intra-issue section structure is a concrete pattern for typed memory fields. Benchmarks are private to the paper, so the numbers are not a transfer estimate.

[^paper]: ACM DL, https://dl.acm.org/doi/abs/10.1145/3626772.3661370 (blocked)
[^arxiv]: arXiv HTML full text, https://arxiv.org/html/2404.17723 (read in part; same title as the ACM paper)
[^arxiv-abs]: arXiv abstract, https://arxiv.org/abs/2404.17723
