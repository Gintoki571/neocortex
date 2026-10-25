---
type: "Benchmark"
title: "Mintaka"
description: "End-to-end multilingual complex QA dataset with 20,000 English question-answer pairs, annotated with Wikidata entities."
resource: "https://arxiv.org/abs/2210.01613"
tags: ["benchmark", "multilingual", "kbqa", "multi-hop"]
year: 2022
venue: "COLING 2022"
llm_required: none
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2210.01613"
    title: "arXiv 2210.01613 (abstract)"
---
# Summary
- Contains 20,000 English question-answer pairs annotated with Wikidata entities, translated into 8 further languages (180,000 samples in total).[^paper]
- Includes 8 question types, among them superlative, intersection and multi-hop questions.[^paper]

# Method
- Crowd-elicited questions with Wikidata annotations (abstract level).[^paper]

# Results
- The best baseline reaches 38% hits@1 in English and 31% multilingually (abstract).[^paper]

# Relevance to neocortex
- Wikidata-based multi-hop questions with a published baseline, so neocortex can be compared offline if the Wikidata subset is loaded. Low priority for the English-only engine.
- Offline with neocortex: yes if a Wikidata subset is loaded; no LLM needed for entity matching.

[^paper]: arXiv 2210.01613 (abstract)
