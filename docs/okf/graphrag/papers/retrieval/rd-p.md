---
type: Paper
title: 'RD-P: A Trustworthy Retrieval-Augmented Prompter with Knowledge Graphs for LLMs'
description: Trains one pre-trained-LM prompter whose retriever finds KG reasoning paths and whose discriminator scores their credibility, then builds a prompt for the LLM (CIKM 2024; abstract only).
resource: https://dl.acm.org/doi/10.1145/3627673.3679659
tags:
- prompter
- trust
- kg-retrieval
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: CIKM 2024
llm_required: both
neocortex_relevance: skip
sources:
- id: answerthis
  resource: https://doi.org/10.1145/3627673.3679659
  title: AnswerThis exact-title search record (abstract, authors, DOI; no open full text)
- id: paper
  resource: https://dl.acm.org/doi/10.1145/3627673.3679659
  title: ACM DL page (blocked)
- id: listing
  resource: https://github.com/DEEP-PolyU/Awesome-GraphRAG
  title: Awesome-GraphRAG README entry
---

# Summary
- Abstract only: the ACM page is blocked and no open-access PDF was listed.[^answerthis][^paper]
- Targets hallucination and the misuse of irrelevant retrieved knowledge in RAG, by making retrieval and credibility checking one model.[^answerthis]
- Retrieve-and-Discriminate Prompter (RD-P): one pre-trained language model with shared parameters holds a retriever (finds KG reasoning paths) and a discriminator (scores path credibility with "logical coverage calculation").[^answerthis]
- The discriminator feeds back into retrieval. The final prompt guides the LLM to use both retrieved and implicit knowledge.[^answerthis]
- The abstract reports better answer coverage with a smaller retrieval scale on KGQA. No numbers were read.[^answerthis]
- Authors: Yubo Huang, Guosun Zeng. Proceedings pp. 942-952.[^answerthis]

# Method
- Shared-parameter pre-trained LM trained as retriever plus discriminator; the discriminator's logical-coverage score guides which KG paths are retrieved. Training details were not read.[^answerthis]

# Results
- Abstract only: no numbers. Qualitative claims: higher answer coverage with fewer retrieved items, and better complex KGQA than RAG baselines.[^answerthis]

# Relevance to neocortex
- The idea that matters is the credibility check on retrieved reasoning paths, which neocortex's hop scoring could borrow. The implementation needs a trained LM and an LLM for the answer, so it cannot run inside the engine as described.
- Relevance: skip (the method is a trained prompter, not a memory feature). Re-rate if the full text shows a light scorer that could run on CPU.

[^answerthis]: AnswerThis search record, https://doi.org/10.1145/3627673.3679659 (abstract only)
[^paper]: ACM DL page, https://dl.acm.org/doi/10.1145/3627673.3679659 (blocked)
[^listing]: Awesome-GraphRAG README, https://github.com/DEEP-PolyU/Awesome-GraphRAG
