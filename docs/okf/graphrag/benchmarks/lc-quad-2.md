---
type: "Benchmark"
title: "LC-QuAD 2.0"
description: "30,000 complex natural-language questions with paraphrases and SPARQL queries over Wikidata and DBpedia 2018 (ISWC 2019)."
resource: "https://link.springer.com/chapter/10.1007/978-3-030-30796-7_5"
tags: ["benchmark", "kgqa", "sparql", "wikidata", "dbpedia"]
year: 2019
venue: "ISWC 2019"
llm_required: none
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://link.springer.com/chapter/10.1007/978-3-030-30796-7_5"
    title: "Springer chapter page (abstract and reference list only; body behind subscription)"
  - id: figshare
    resource: "https://figshare.com/projects/LCQuAD_2_0/62270"
    title: "figshare project page (published 2019-04-10; no licence text shown in the fetched page)"
  - id: site
    resource: "http://lc-quad.sda.tech/"
    title: "Project website linked from the Springer page (not fetched)"
---
# Summary
- 30,000 questions, their paraphrases and SPARQL queries, compatible with Wikidata and DBpedia 2018.[^paper]
- The predecessor LC-QuAD has 5,000 questions; LC-QuAD 2.0 is the larger successor.[^paper]
- Resource type is a dataset, distributed through figshare.[^paper][^figshare]

# Method
- Built by generating questions from SPARQL templates and then verbalising them; full construction steps are in the paper body, which was not reachable (Springer subscription page).[^paper]

# Results
- No benchmark results were read; the paper body was not accessible.

# Relevance to neocortex
- Useful as a source of multi-hop SPARQL-to-question pairs over Wikidata. neocortex would need a local Wikidata slice and a SPARQL-to-relation mapping to score it, so it is a larger build than the other benchmarks.
- Its questions are template-generated and paraphrased, the same family as KQA Pro and MetaQA. Watch, since it does not test anything those already cover.
- Licence: not stated in the fetched figshare page; check the dataset files before reuse.

[^paper]: Springer chapter page, abstract only (the body is behind subscription; not bypassed)
[^figshare]: figshare project page (published 2019-04-10)
