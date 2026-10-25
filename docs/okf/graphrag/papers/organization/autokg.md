---
type: Paper
title: 'AutoKG: Efficient Automated Knowledge Graph Generation for Language Models'
description: Automated KG generation for language models (BigData 2023).
resource: https://ieeexplore.ieee.org/abstract/document/10386454
tags:
- kg-construction
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2023
venue: BigData 2023
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://ieeexplore.ieee.org/abstract/document/10386454
  title: IEEE Xplore page (plain fetch; abstract text readable, full text paywalled)
- id: arxiv
  resource: https://arxiv.org/abs/2311.14740
  title: arXiv preprint 2311.14740 with the same title (abstract read via arXiv API)
---

# Summary
- AutoKG builds a knowledge graph automatically over a text-block knowledge base, so that LLMs can retrieve through it instead of only through semantic similarity search.[^paper]
- An LLM extracts keywords from each text block; graph Laplace learning then weights the relation between each pair of keywords.[^paper]
- Retrieval is a hybrid of vector similarity and graph-based associations; the abstract reports only preliminary experiments with no numbers.[^arxiv]
- The IEEE full text is paywalled. The abstract text came from the IEEE page (plain fetch), and the arXiv preprint with the same title was read for its abstract; the two versions were not compared line by line.

# Method
- Keyword extraction with an LLM from each text block.[^arxiv]
- Pairwise keyword relation weights from graph Laplace learning over the keyword graph.[^arxiv]
- Hybrid search: vector similarity plus graph associations, used to enrich the LLM's context.[^arxiv]

# Results
- No quantitative results are given in the abstract; the paper is described as "preliminary experiments" with a qualitative claim of a more interconnected retrieval than similarity search.[^arxiv]

# Relevance to neocortex
- The LLM is used only to extract keywords at write time, which the calling agent could do. The weighting step (graph Laplace learning) and the hybrid vector-plus-graph search are LLM-free, CPU-cheap, and match neocortex's existing fusion of vectors and the entity graph.
- Watch: no numbers, so it is not evidence for any change. Check the arXiv full text before relying on the Laplace weighting.

[^paper]: IEEE Xplore, https://ieeexplore.ieee.org/abstract/document/10386454 (plain fetch; abstract text)
[^arxiv]: arXiv 2311.14740, https://arxiv.org/abs/2311.14740 (abstract)
