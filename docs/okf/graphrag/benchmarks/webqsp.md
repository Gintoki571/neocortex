---
type: "Benchmark"
title: "WebQSP"
description: "WebQuestionsSP: SPARQL semantic-parse labels for 4,737 WebQuestions over Freebase, showing that parse labels beat answer-only training (ACL 2016)."
resource: "https://aclanthology.org/P16-2033.pdf"
tags: ["benchmark", "kbqa", "freebase", "semantic-parsing", "sparql"]
year: 2016
venue: "ACL 2016"
llm_required: none
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://aclanthology.org/P16-2033.pdf"
    title: "ACL Anthology PDF, Yih et al. 2016 'The Value of Semantic Parse Labeling' (pdftotext -layout; Sections 1-4, Table 1-2)"
  - id: msr
    resource: "http://aka.ms/WebQSP"
    title: "Microsoft short link from the README (redirect not followed)"
---
# Summary
- WebQuestionsSP adds SPARQL semantic parses to 4,737 WebQuestions questions answerable over Freebase; the remaining 18.5% are labelled not answerable.[^paper]
- The paper shows that learning from semantic parses gives about a 5-point gain over learning from answers only, and that parse labelling can be done as cheaply as answer labelling with the authors' interface.[^paper]

# Method
- Labelling UI guides the annotator to pick topic entities and constraints and then builds the query graph and SPARQL, with the Freebase identifiers used directly.[^paper]
- Training split follows WebQuestions; 3,098 questions were used for training and 1,639 for evaluation, scored with per-question F1 and exact-answer accuracy.[^paper]

# Results
- Table 1 (test, answers-only vs semantic parses): Avg. F1 66.8 vs 71.7; accuracy 58.8 vs 63.9.[^paper]
- Table 2 (50 sampled questions): labelling time 82 s per question for experts writing answers and 21 s with the parse UI; correctness 92% (expert answers) and 94% (expert parses) vs 66% for MTurk answers.[^paper]

# Relevance to neocortex
- Skipped: single-relation and two-hop Freebase questions over a KB that neocortex does not hold, and the scoring requires a semantic parser. The paper's lesson (explicit intermediate labels help) matches the KQA Pro and MetaQA discussion but adds nothing new for neocortex's relation store.

[^paper]: ACL Anthology PDF, Yih et al. 2016 (Sections 1-4, Tables 1-2)
