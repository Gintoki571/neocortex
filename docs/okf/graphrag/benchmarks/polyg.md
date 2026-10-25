---
type: "Benchmark"
title: "PolyBench (PolyG benchmark)"
description: "GraphRAG benchmark of 1,200 knowledge-graph questions built from a four-class question taxonomy, plus the PolyG adaptive method."
resource: "https://arxiv.org/abs/2504.02112"
tags: ["benchmark", "graphrag", "kg-qa", "question-taxonomy"]
year: 2025
venue: "arXiv 2025"
llm_required: retrieval
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2504.02112"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2504.02112"
    title: "arXiv HTML full text (Tables 1-2, Section 3)"
  - id: code
    resource: "https://github.com/Liu-rj/PolyG"
    title: "Repository (linked from the paper; not read)"
---
# Summary
- Taxonomy of basic patterns: ⟨s,*,*⟩, ⟨s,p,*⟩, ⟨s,*,o⟩, ⟨s,p,o⟩ (subject, predicate chain, object), plus nested questions built from them.[^html]
- PolyBench construction: question templates per KG, each paired with a manually written Cypher query; relation paths of 1 to 5 hops; 400 initial questions per knowledge graph, 1,200 in total across three KGs. Most questions have no gold answer.[^html]
- Table 1 compares coverage with SimpleQ (2015), WebQSP (2016), GraphQ, CWQ (2018), GrailQA (2021), RGBench, CypherBench and WikiQA; PolyBench is the only one listed with all four basic patterns.[^html]
- Judgment: LLM-based pairwise win rates, since gold answers are missing for most questions.[^html]

# Method
- PolyG first classifies a question as basic or nested, builds a query plan, and prompts an LLM to write a graph-database query per decomposed question on a unified execution engine.[^html]
- Fixed traversal strategies and free LLM exploration are the two baseline styles the paper argues against.[^html]

# Results
- PolyG's overall win rate is above 60% on the patterns other than the first, against the compared GraphRAG methods; the paper reports it best in 18 of 21 cases on the reported metrics.[^html]

# Relevance to neocortex
- The Cypher templates give exact gold answer sets for each question, so the offline score is exact: load each KG as relation facts in neocortex, run the question's basic pattern as a relation lookup or path search, and compare entity sets. This tests the four basic patterns separately.[^html]
- The KG names were not transcribed; check the paper before loading. The benchmark is small (1,200), so a run is cheap.[^html]
- Query generation by an LLM is the agent-side part; neocortex only needs to execute the pattern.

[^html]: arXiv HTML full text (Tables 1-2, Section 3)
