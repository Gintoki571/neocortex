---
type: Paper
title: Biomedical knowledge graph-enhanced prompt generation for large language models
description: KG-RAG retrieves a compact, embedding-pruned slice of the SPOKE biomedical knowledge graph and places it in the prompt, cutting token use by about half versus a Cypher-schema baseline (Bioinformatics 2024).
resource: https://academic.oup.com/bioinformatics/article/40/9/btae560/7759620
tags:
- biomedical
- prompt-generation
- context-pruning
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: Bioinformatics 2024
llm_required: both
neocortex_relevance: watch
sources:
- id: answerthis
  resource: https://doi.org/10.1093/bioinformatics/btae560
  title: AnswerThis exact-DOI-matched search record (abstract, authors, quartile 1)
- id: pdf
  resource: https://arxiv.org/pdf/2311.17330
  title: arXiv 2311.17330 v2 full text (preprint with the title "Biomedical knowledge graph-optimized prompt generation for large language models"; read via pdftotext)
- id: paper
  resource: https://academic.oup.com/bioinformatics/article/40/9/btae560/7759620
  title: OUP Bioinformatics page (blocked)
- id: listing
  resource: https://github.com/DEEP-PolyU/Awesome-GraphRAG
  title: Awesome-GraphRAG README entry
---

# Summary
- Full text read from the arXiv preprint 2311.17330 (v2, May 2024). Its title is "Biomedical knowledge graph-optimized prompt generation..."; the DOI 10.1093/bioinformatics/btae560 and the authors (Soman et al.) match the catalogued Bioinformatics paper. The OUP page itself stayed blocked.[^answerthis][^pdf][^paper]
- KG-RAG grounds LLM prompts in the SPOKE biomedical knowledge graph, using a minimal graph schema for context extraction and embedding-based context pruning.[^pdf]
- Tested with Llama-2-13b, GPT-3.5-Turbo and GPT-4 on True/False and multiple-choice (MCQ) biomedical questions written for the study.[^pdf]
- Reports an average of 53.9% fewer tokens than a Cypher-schema RAG baseline (3693 vs 8006 tokens).[^pdf]
- Lifts Llama-2 MCQ accuracy from 0.31 to 0.53.[^pdf]

# Method
- Disease entities in the prompt are recognised and matched to SPOKE disease nodes using embeddings (node retrieval accuracy 99.7% reported).[^pdf]
- Graph context is limited to a "context volume" (number of KG associations), set to 150 for most tasks and 100 for True/False questions.[^pdf]
- Associations are ranked by cosine similarity to the prompt vector, using PubMedBERT as the context embedding model because it beat MiniLM by about 8-10% on Jaccard similarity.[^pdf]
- The selected context is converted to natural language and prepended to the prompt. The LLM is not fine-tuned.[^pdf]

# Results
- True/False accuracy, prompt-only vs KG-RAG: Llama-2-13b 0.89 vs 0.94; GPT-3.5-Turbo 0.87 vs 0.95; GPT-4 0.90 vs 0.95.[^pdf]
- MCQ accuracy, prompt-only vs KG-RAG: Llama-2-13b 0.31 vs 0.53; GPT-3.5-Turbo 0.63 vs 0.79; GPT-4 0.68 vs 0.74. The paper notes GPT-4 scored lower than GPT-3.5-Turbo on MCQ with KG-RAG.[^pdf]
- Token usage: average 3693 tokens for KG-RAG vs 8006 for Cypher-RAG, a 53.9% reduction.[^pdf]

# Relevance to neocortex
- The reusable idea is the budget control: keep a fixed number of KG associations, ranked by embedding similarity to the query. neocortex already returns a top-K list, so the change would be ranking with its UltraBERT vectors. This is cheap and LLM-free.
- The rest needs an LLM at answer time (agent side only). The domain is biomedical and the benchmarks are not memory recall.
- Relevance: watch. The number that matters for neocortex is the token saving, not the accuracy.

[^answerthis]: AnswerThis search record, https://doi.org/10.1093/bioinformatics/btae560 (abstract)
[^pdf]: arXiv PDF full text, https://arxiv.org/pdf/2311.17330 (read in this pass)
[^paper]: OUP Bioinformatics page, https://academic.oup.com/bioinformatics/article/40/9/btae560/7759620 (blocked)
[^listing]: Awesome-GraphRAG README, https://github.com/DEEP-PolyU/Awesome-GraphRAG
