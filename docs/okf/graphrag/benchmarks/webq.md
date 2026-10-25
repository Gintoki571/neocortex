---
type: "Benchmark"
title: "WebQuestions"
description: "5,810 Google Suggest questions answered over Freebase, the original weakly supervised KBQA benchmark (EMNLP 2013)."
resource: "https://nlp.stanford.edu/pubs/semparseEMNLP13.pdf"
tags: ["benchmark", "kbqa", "freebase", "simple-questions"]
year: 2013
venue: "EMNLP 2013"
llm_required: none
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://aclanthology.org/D13-1160.pdf"
    title: "ACL Anthology copy of Berant et al. 2013 (pdftotext -layout; Sections 1-4, Table 3)"
  - id: stanford
    resource: "https://nlp.stanford.edu/pubs/semparseEMNLP13.pdf"
    title: "Stanford PDF: network error from this sandbox (sanity-checked, not read)"
  - id: codalab
    resource: "https://worksheets.codalab.org/worksheets/0xba659fe363cb46e7a505c5b6a774dc8a"
    title: "CodaLab worksheet linked from the README (not fetched)"
---
# Summary
- 5,810 questions with answers from Freebase; questions come from the Google Suggest API and answers were collected from Freebase by crowd workers.[^paper]
- Table 3 lists WebQuestions with 5,810 examples and 4,525 word types, versus 917 for Free917 and 5,418 for ATIS.[^paper]
- The paper's point is that question-answer pairs alone, without logical forms, can train a semantic parser.[^paper]

# Method
- Semantic parser over simple lambda-DCS logical forms, learned from question-answer pairs only; binary predicates come from an alignment lexicon and a bridging operation.[^paper]
- Accuracies on the development set: alignment only 38.0 on Free917 and 30.6 on WebQ; bridging only 66.9 and 21.2; the full system 71.3 and 32.9.[^paper]

# Results
- The full system reaches 32.9 on WebQuestions (Table 4, development set, accuracy).[^paper]

# Relevance to neocortex
- Skipped: the questions are short single-relation lookups over Freebase, which neocortex's multi-hop and temporal work does not target. Its successor WebQSP (see [WebQSP](/benchmarks/webqsp.md)) is the version used in practice.
- The sandbox could not reach the Stanford PDF; the ACL Anthology copy was read instead, and the README's CodaLab worksheet was not fetched.[^stanford]

[^paper]: ACL Anthology copy of Berant et al. 2013 (Sections 1-4, Table 3-4)
[^stanford]: nlp.stanford.edu PDF returned a network error from the sandbox; not retried
