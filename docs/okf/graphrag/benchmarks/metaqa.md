---
type: "Benchmark"
title: "MetaQA"
description: "Multi-hop KB question answering benchmark (1-, 2- and 3-hop) over the WikiMovies knowledge base, with Vanilla, NTM-paraphrased and audio variants; about 400K questions in total."
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
    title: "arXiv PDF v3 (pdftotext -layout; Tables 1-3, Appendix A)"
  - id: readme
    resource: "https://github.com/yuyuz/MetaQA"
    title: "Repository README (dataset layout, KB format, licence statement; HTTP 200 for README, 404 for LICENSE)"
---
# Summary
- MetaQA (MoviE Text Audio QA) has 1-, 2- and 3-hop question sets over the WikiMovies knowledge base; Vanilla, NTM (neural-translation paraphrases) and audio variants.[^pdf][^readme]
- Question counts (Table 3, train / validation / test): 1-hop 96,106 / 9,992 / 9,947; 2-hop 118,980 / 14,872 / 14,872; 3-hop 114,196 / 14,274 / 14,274.[^pdf]
- The knowledge base is one file, kb.txt, with one `subject|relation|object` triple per line, for example `Kismet|directed_by|William Dieterle`.[^readme]
- Vanilla 2-hop uses 21 question types and Vanilla 3-hop uses 15, each with 10 templates; 1-hop is derived from the WikiMovies wiki_entities branch with ambiguous questions removed.[^pdf]
- Metric: hits@1 (%).[^pdf]

# Method
- Variational reasoning (VRN): a latent reasoning graph over entities within T hops of the topic entity; the answer is scored from a joint model of the answer and the path, trained with a policy-gradient style estimator with a learned baseline and variance reduction.[^pdf]
- Topic-entity recognition is trained jointly with reasoning, so questions with unlabelled entities (only 5% labelled in the Vanilla-EU setting) can still be answered.[^pdf]
- Inference uses beam search over candidate entities; the paper reports that k = 1 (greedy) already performs well.[^pdf]
- Hyperparameter values were not found in the extracted PDF text.

# Results
- Table 1 (hits@1, Vanilla 1/2/3-hop | Vanilla-EU 1/2/3-hop): VRN 97.5 / 89.9 / 62.5 | 82.0 / 75.6 / 38.3; Bordes et al. QA system 95.7 / 81.8 / 28.4 | 39.5 / 38.3 / 26.9; KV-MemNN 95.8 / 25.1 / 10.1 | 35.8 / 10.3 / 10.5; supervised embedding 54.4 / 29.1 / 28.9 | 18.1 / 23.2 / 25.3.[^pdf]
- Table 2 (hits@1, NTM-EU 1/2/3-hop | Audio-EU 1/2/3-hop): VRN 81.3 / 69.7 / 38.0 | 37.0 / 24.6 / 21.1; Bordes et al. 32.5 / 32.3 / 25.3 | 18.5 / 19.3 / 15.3; KV-MemNN 33.9 / 8.7 / 10.2 | 4.3 / 7.0 / 15.3; supervised embedding 16.1 / 22.8 / 24.2 | 4.1 / 6.1 / 12.1.[^pdf]
- The paper states that 2- and 3-hop questions are harder, and in Vanilla-EU the supervised embedding is the only method whose score rises with hop count (18.1, 23.2, 25.3).[^pdf]

# Relevance to neocortex
- Strong fit for neocortex's weakest measured area (three-hop chains). The KB is a triple file, so the whole task runs with no LLM: load the triples as relation facts and run multi-hop path search from the topic entity.[^readme][^pdf]
- Offline recipe: for each test question, resolve the topic entity (the Vanilla-EU and NTM-EU variants test entity resolution), run path search with depth equal to the hop count, and score hits@1 against the answer. Report 1-, 2- and 3-hop separately, and per question type via the qtype files.[^pdf][^readme]
- Baseline to beat: VRN's Vanilla-EU 3-hop score of 38.3 and NTM-EU 3-hop score of 38.0 (Tables 1-2) are the numbers to compare with.[^pdf]
- Licence: the README says the data is released under a Creative Commons public licence, with a copy included in the data bundle; no LICENSE file is in the repository (404), so the exact CC variant is unknown.[^readme]

[^pdf]: arXiv PDF v3 (pdftotext -layout; Tables 1-3, Appendix A)
[^readme]: github.com/yuyuz/MetaQA README (raw.githubusercontent.com HEAD; LICENSE returned 404)
