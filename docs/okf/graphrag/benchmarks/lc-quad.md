---
type: "Benchmark"
title: "LC-QuAD"
description: "5,000-question gold KBQA dataset (4,000 train, 1,000 test) pairing questions with SPARQL queries over DBpedia 2016-04 (ISWC 2017)."
resource: "https://dl.acm.org/doi/10.1007/978-3-319-68204-4_22"
tags: ["benchmark", "kbqa", "sparql", "dbpedia", "complex-questions"]
year: 2017
venue: "ISWC 2017"
llm_required: none
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: answerthis
    resource: "https://doi.org/10.1007/978-3-319-68204-4_22"
    title: "AnswerThis exact-title search record (abstract, authors, DOI)"
  - id: paper
    resource: "https://dl.acm.org/doi/10.1007/978-3-319-68204-4_22"
    title: "ACM DL page (blocked by a bot-verification interstitial; not bypassed)"
  - id: repo
    resource: "https://github.com/AskNowQA/LC-QuAD"
    title: "Repository README, LICENSE.txt (GPL-3.0) and data branch (fetched)"
---
# Summary
- Paper abstract read from the AnswerThis record (Trivedi, Maheshwari, Dubey, Lehmann; ISWC 2017, pp. 210-218; DOI matches). The full text was not read: the ACM page stayed blocked.[^answerthis][^paper]
- Provides 5,000 questions with their SPARQL queries over DBpedia, built to test the robustness and accuracy of complex KGQA systems that learn SPARQL.[^answerthis]
- Released as train (4,000) and test (1,000) JSON files, with corrected questions and the SPARQL template id.[^repo]
- Questions were produced by generating SPARQL from 2-hop seed entities and predicate whitelists, creating intermediate questions and hand-correcting them.[^repo]
- The repository licence is GPL-3.0.[^repo]

# Method
- SPARQL queries are generated from templates over a seed entity and predicate whitelist, then given natural-language templates and manually corrected.[^repo]
- The abstract says the creation process was designed for a high variety of questions. The creation steps in detail were not read in the paper body.[^answerthis]

# Results
- The paper is a dataset paper. Its abstract gives no baseline numbers, and none were read.[^answerthis]

# Relevance to neocortex
- The 1,000-question test split is small enough for an offline SPARQL-free check: map each gold SPARQL to its entity and predicate chain, then score whether neocortex's path search reaches the answer entity. This needs a DBpedia 2016-04 slice, which the README recommends hosting locally.[^repo]
- Superseded by LC-QuAD 2.0 in the README; keep as low priority.
- GPL-3.0 licence: fine for evaluation, but check obligations before redistributing derived data.[^repo]

[^answerthis]: AnswerThis exact-title search record, https://doi.org/10.1007/978-3-319-68204-4_22 (abstract)
[^paper]: ACM Digital Library page, https://dl.acm.org/doi/10.1007/978-3-319-68204-4_22 (blocked by a bot-verification interstitial; no bypass attempted)
[^repo]: github.com/AskNowQA/LC-QuAD README, LICENSE.txt and train-data.json / test-data.json (4000 and 1000 records)
