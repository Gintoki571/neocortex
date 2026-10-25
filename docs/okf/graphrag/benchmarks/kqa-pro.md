---
type: "Benchmark"
title: "KQA Pro"
description: "117,970 natural-language complex KBQA questions with KoPL programs and SPARQL queries, used to diagnose compositional reasoning skills (ACL 2022)."
resource: "https://arxiv.org/abs/2007.03875"
tags: ["benchmark", "kbqa", "compositional", "program", "sparql"]
year: 2020
venue: "ACL 2022"
llm_required: none
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2007.03875"
    title: "arXiv abstract and submission record"
  - id: html
    resource: "https://arxiv.org/html/2007.03875"
    title: "arXiv HTML v4 (fetched; Sections 3-5)"
  - id: pdf
    resource: "https://arxiv.org/pdf/2007.03875"
    title: "arXiv PDF (pdftotext -layout; Tables 1, 4, 5 and Section 5.1-5.4)"
  - id: code
    resource: "https://github.com/shijx12/KQAPro_Baselines"
    title: "Baselines repository README and LICENSE (MIT); dataset licence from the dataset homepage text in the README"
---
# Summary
- 117,970 questions (the abstract rounds to about 120K), each with a KoPL program and a SPARQL query; the KB has 363 relation types, with attributes and qualifiers.[^pdf][^html]
- Reasoning skills covered: multi-hop, qualifier, comparison (quantitative and temporal), logical union or intersection, counting, verification and zero-shot (answers unseen in training).[^pdf]
- Questions are generated from templates by sampling and recursive composition of 27 KoPL functions, then paraphrased by crowd workers; paraphrases are filtered by edit distance to the canonical question.[^pdf]
- Split randomly 8:1:1 into 94,376 train, 11,797 validation and 11,797 test; about 30% of test answers are unseen in training.[^pdf]

# Method
- Locating stage: 7 strategies pick the entity or concept a question is about; asking stage: templates with four quantitative operators (=, !=, <, >).[^pdf]
- KoPL is a tree-structured program; the authors serialise it to a token sequence with special structure tokens for seq2seq parsing.[^pdf]
- Baseline parser: BART-base (HuggingFace) for SPARQL and KoPL, Adam; learning rate 3e-5 for BART and 1e-3 for other parameters, weight decay 1e-5, chosen on validation.[^pdf]
- Compositional generalization split: train on short programs and test on long ones (5,899 valid and 5,899 test, 106,182 train).[^pdf]

# Results
- Table 4 (accuracy, MetaQA 1/2/3-hop, WebQSP, KQA Pro): KVMemNet 96.2 / 82.7 / 48.9 / 46.7 / 16.61; SRN 97.0 / 95.1 / 75.2 / - / 12.33 (relational-only subset); EmbedKGQA 97.5 / 98.8 / 94.8 / 66.6 / 28.36; RGCN - / - / - / 37.2 / 35.07; BART 99.9 (3-hop) / 67.5 (WebQSP) / 90.55 (KQA Pro).[^pdf]
- Table 5 (KQA Pro test, overall / multi-hop / qualifier / comparison / logical / count / verify / zero-shot): BART KoPL 90.55 / 89.46 / 84.76 / 95.51 / 89.30 / 86.68 / 93.30 / 89.59; BART SPARQL 89.68 / 88.49 / 83.09 / 96.12 / 88.67 / 85.78 / 92.33 / 87.88; RNN KoPL 43.85 / 37.71 / 22.19 / 65.90 / 47.45 / 50.04 / 42.13 / 34.96; human 97.50 / 97.24 / 95.65 / 100.00 / 98.18 / 83.33 / 95.24 / 100.00.[^pdf]
- Compositional split: BART KoPL drops from 90.55% to 77.86%.[^pdf]

# Relevance to neocortex
- Gold relation chains come from the KoPL program, so neocortex can be scored per reasoning skill: multi-hop questions against path search, comparisons and counts against stored relation facts. Set operations (logical union and intersection) are not supported by neocortex today and will score zero, which makes a useful gap list.[^pdf]
- The benchmark is LLM-free to score (answers are KB entities), but neocortex needs the KB loaded as relation facts first. The KB is distributed in the dataset download (`kb.json` in the baselines repo layout), not in the paper text.[^code]
- Use the zero-shot and multi-hop rows of Table 5 as targets; neocortex has no baseline on this benchmark yet.

[^pdf]: arXiv PDF (pdftotext -layout; Sections 3-5, Tables 1, 4, 5)
[^html]: arXiv HTML v4 (Sections 3-4)
[^code]: github.com/shijx12/KQAPro_Baselines README (raw HEAD) and LICENSE (MIT); the README states the dataset is licensed under CC BY-SA 4.0 via the homepage
