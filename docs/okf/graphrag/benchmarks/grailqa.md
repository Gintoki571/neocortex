---
type: "Benchmark"
title: "GrailQA"
description: "Large-scale Freebase KBQA dataset with 64,331 questions split to test three levels of generalization (i.i.d., compositional, zero-shot)."
resource: "https://arxiv.org/abs/2011.07743"
tags: ["benchmark", "kbqa", "generalization", "freebase", "s-expression"]
year: 2020
venue: "WWW 2021 (arXiv 2020; HTML lists the Web Conference 2021 proceedings)"
llm_required: none
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2011.07743"
    title: "arXiv 2011.07743 (abstract)"
  - id: html
    resource: "https://arxiv.org/html/2011.07743"
    title: "arXiv HTML full text (Sections 3-5, Table 3, Appendix D)"
  - id: code
    resource: "https://github.com/dki-lab/GrailQA"
    title: "Official repo (not read; linked from the paper)"
---
# Summary
- 64,331 question and logical-form pairs over Freebase, from 4,969 canonical logical forms and 29,457 paraphrases (including canonical questions).[^html]
- Released split is about 70/10/20 into 44,337 train, 6,763 validation and 13,231 test questions; half of validation and test is zero-shot (held-out domains), a quarter compositional (unseen logical forms) and a quarter i.i.d.[^html]
- Defines the three generalization levels that the abstract names: i.i.d., compositional and zero-shot.[^paper][^html]

# Method
- Logical forms are S-expressions, and exact match is graph isomorphism after converting back to graph queries, so equivalent queries score as correct.[^html]
- Two models: Transduction (seq2seq that generates the logical form over ontology-aware vocabulary, with vocabulary pruning) and Ranking (prunes the search space with each identified entity's neighbouring facts, then ranks candidates). Both use BERT for question-to-schema alignment; GloVe-300d variants replace BERT.[^html]
- Training: BERT models use batch size 1 with gradient accumulation 16, Adam with learning rate 1e-3 for the new parameters and 2e-5 for BERT; GloVe models use batch size 32 and learning rate 1e-3; early stopping with patience 3 on validation EM.[^html]

# Results
- Overall EM / F1: Ranking 50.6 / 58.0; Transduction 33.3 / 36.8; QGG (the adapted WebQ model) F1 36.7. Ranking without BERT falls to 39.5 / 45.1.[^html]
- Zero-shot EM / F1: Ranking 48.6 / 55.7; Transduction 25.7 / 29.3. Compositional EM / F1: Ranking 59.9 / 67.0; Transduction 31.0 / 36.0.[^html]
- Replacing BERT by GloVe drops Transduction F1 by 17.5% on compositional and 26.2% on zero-shot questions; the paper credits BERT contextual encoding for most of the generalization gap.[^html]

# Relevance to neocortex
- Not directly usable: the dataset is Freebase-based and the baselines are semantic parsers that need an LLM-scale encoder at training time. Neocortex's retrieval has no logical-form output, so the score is not comparable.
- Useful as a generalization split design: a zero-shot split (held-out relation domains) tests whether a multi-hop path search still works when the relation vocabulary is unseen. The EM with graph isomorphism is also a fair way to score structured answers.
- Watch only; no CPU-side baseline is reported in the paper.

[^paper]: arXiv 2011.07743 (abstract)
[^html]: arXiv HTML full text (Sections 3-5, Table 3, Appendix D)
