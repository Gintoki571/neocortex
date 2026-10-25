---
type: Paper
title: Intelligent question answering for water conservancy project inspection driven by knowledge graph and large language model collaboration
description: Hydroengineering inspection QA where the LLM clarifies a query along a path guided by a knowledge graph (LHB 2024; abstract only).
resource: https://www.tandfonline.com/doi/full/10.1080/27678490.2024.2397337
tags:
- domain-qa
- kg-llm
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: LHB 2024
llm_required: both
neocortex_relevance: skip
sources:
- id: answerthis
  resource: https://doi.org/10.1080/27678490.2024.2397337
  title: AnswerThis exact-DOI search record (abstract, authors, volume 110 issue 1, article 2397337)
- id: paper
  resource: https://www.tandfonline.com/doi/full/10.1080/27678490.2024.2397337
  title: Taylor & Francis page (blocked)
- id: listing
  resource: https://github.com/DEEP-PolyU/Awesome-GraphRAG
  title: Awesome-GraphRAG README entry
---

# Summary
- Abstract only: the Taylor & Francis page is blocked and no open-access PDF was listed.[^answerthis][^paper]
- Hydroengineering inspection QA. Detectors' unclear queries are handled by an LLM that clarifies the query in five units, guided by a knowledge graph.[^answerthis]
- The method addresses vocabulary limits and fixed templates in KGQA, and LLM noise in plain LLM QA.[^answerthis]
- Authors: Yangrui Yang, Sisi Chen, Yaping Zhu, Xuemei Liu, Shifeng Pan, Xin Wang.[^answerthis]

# Method
- Five-unit clarification pipeline: the LLM progressively clarifies the query; the KG supplies an optimal clarification path that is passed to the LLM.[^answerthis]

# Results
- Abstract reports MRR, MAP, precision and recall all above 0.73 on benchmark experiments. The benchmark set and baselines were not read.[^answerthis]

# Relevance to neocortex
- Domain QA with an LLM in the loop (clarification step). Nothing here is an engine feature for neocortex.
- Relevance: skip.

[^answerthis]: AnswerThis search record, https://doi.org/10.1080/27678490.2024.2397337 (abstract only)
[^paper]: Taylor & Francis page, https://www.tandfonline.com/doi/full/10.1080/27678490.2024.2397337 (blocked)
[^listing]: Awesome-GraphRAG README, https://github.com/DEEP-PolyU/Awesome-GraphRAG
