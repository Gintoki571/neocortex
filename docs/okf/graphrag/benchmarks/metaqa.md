---
type: "Benchmark"
title: "MetaQA"
description: "Multi-hop KB question answering benchmark (1-, 2- and 3-hop) built on WikiMovies, with NTM-paraphrased and audio variants; over 400K questions in total."
resource: "https://arxiv.org/abs/1709.04071"
tags: ["benchmark", "multi-hop", "kbqa", "knowledge-graph", "wikimovies"]
year: 2018
venue: "AAAI 2018"
llm_required: none
neocortex_relevance: adopt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/1709.04071"
    title: "arXiv abstract"
  - id: pdf
    resource: "https://arxiv.org/pdf/1709.04071v3"
    title: "arXiv PDF v3 (text extracted with pdftotext; Tables 1, 3)"
  - id: code
    resource: "https://github.com/yuyuz/MetaQA"
    title: "Repository README (dataset layout, licence, paper citation)"
---
# Summary
- MetaQA (MoviE Text Audio QA) is a KG QA benchmark; the 1-hop split is derived from WikiMovies, and 2-hop and 3-hop questions are generated from the same knowledge base.[^pdf][^code]
- Question counts (Table 3, train / validation / test): 1-hop 96,106 / 9,992 / 9,947; 2-hop 118,980 / 14,872 / 14,872; 3-hop 114,196 / 14,274 / 14,274. The paper states more than 400K questions in total.[^pdf]
- Question types: 21 types for 2-hop and 15 for 3-hop, each with 10 text templates; the NTM variant paraphrases each question via French and back, keeping the entities.[^code]
- Metric: hits@1 (%).[^pdf]

# Method
- Vanilla split: questions are template-generated; the paper compares a setting where entity labels are given in all questions with one where only 5% of entity labels are given.[^pdf]
- Models are trained on question-answer pairs only; the paper's model is a variational reasoning approach that handles noisy entity mentions and learns multi-hop reasoning end to end.[^paper][^pdf]
- Split protocol: the same train/validation/test split is used for all datasets in the benchmark.[^pdf]

# Results
- Table 1 reports hits@1 for the paper's method and baselines on Vanilla and Vanilla-EU, and Table 2 on NTM-EU and Audio-EU. The text extraction of the tables did not allow an unambiguous row-to-method mapping, so no per-method numbers are recorded here; read the paper's Table 1 before quoting.[^pdf]

# Relevance to neocortex
- Strong fit for neocortex's weakest measured area (three-hop chains). The KB is structured (triples), so the whole task runs with no LLM: load the KB as relation facts and call neocortex's multi-hop path search from the topic entity.[^code]
- Offline scoring recipe: for each test question, resolve the topic entity (typos in the NTM variant test entity resolution), run the k-hop path search with depth equal to the hop count, and score hits@1 against the answer entity. Report 1-, 2- and 3-hop separately.[^pdf]
- The KB triples come with the dataset download (Google Drive link in the README). The file list was not inspected, so the triple format and the KB size are not recorded here.

[^paper]: arXiv abstract
[^pdf]: arXiv PDF v3 (text extracted with pdftotext; Tables 1, 3)
[^code]: Repository README (dataset layout, licence, paper citation)
