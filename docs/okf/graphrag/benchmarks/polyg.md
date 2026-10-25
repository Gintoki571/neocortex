---
type: "Benchmark"
title: "PolyBench (PolyG benchmark)"
description: "GraphRAG benchmark of 1,200 questions over three knowledge graphs (DBLP physics academia, Goodreads literature, Amazon e-commerce) covering a four-class question taxonomy, introduced with the PolyG method."
resource: "https://arxiv.org/abs/2504.02112"
tags: ["benchmark", "graphrag", "kg-qa", "question-taxonomy", "cypher"]
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
    title: "arXiv HTML full text (Sections 3-6, Tables 1-4, 10-11, Appendix F)"
  - id: code
    resource: "https://github.com/Liu-rj/PolyG"
    title: "Repository README and LICENSE (Apache-2.0 text fetched; code not read)"
---
# Summary
- Four basic patterns, subject-predicate-object with wildcards: ⟨s,\*,\*⟩, ⟨s,p,\*⟩, ⟨s,\*,o⟩ and ⟨s,p,o⟩, plus four nested patterns built from them (Table 2).[^html]
- Three knowledge graphs, each with 400 questions (1,200 in total): academia (the Physics subset of DBLP; 2.7M entities, 67M relations), literature (Goodreads; 3.7M entities, 28M relations) and e-commerce (Amazon product graph; 8.1M entities, 193M relations). Templates per KG: 24, 28 and 21.[^html]
- Relation paths in the templates range from 1 to 5 hops.[^html]
- Only the ⟨s,p,\*⟩ and nested ⟨s,p,\*⟩ templates have gold answers, from manually written Cypher queries; the other questions are judged by LLM pairwise win rates.[^html]
- Questions are paraphrased four ways by Claude-3.5-Sonnet, and one paraphrase is picked at random.[^html]

# Method
- PolyG pipeline: question categorisation (basic or nested), decomposition into basic steps, LLM Cypher generation per step, asynchronous execution, and context formation.[^html]
- Traversal per pattern (Table 3): BFS neighbour expansion for ⟨s,\*,\*⟩; meta-path guided walk for ⟨s,p,\*⟩; top-k shortest paths for ⟨s,\*,o⟩; top-k constrained shortest paths for ⟨s,p,o⟩.[^html]
- Compared with MS_GraphRAG, RoG, Fast-GraphRAG, Graph-CoT and a direct Cypher baseline. Models: Claude-3.5-Sonnet and DeepSeek-R1 via AWS Bedrock; the experiments ran on one g5.16xlarge (A10 GPU).[^html]

# Results
- Table 4, Claude-3.5-Sonnet block: PolyG overall-winner 54.75% (next best Graph-CoT 13.83%, RoG 19.25%); F1 0.7084 and Hit 0.8900 on the gold-answer questions (RoG 0.6934 / 0.8867); latency 25.38 s vs 16.75 s (RoG) and 53.81 s (Graph-CoT); tokens 6,950 vs 1,417 (RoG) and 104,594 (Graph-CoT).[^html]
- Table 4, DeepSeek-R1 block: PolyG overall-winner 42.92%, F1 0.6153 vs 0.6048 for RoG, latency 22.40 s, tokens 5,796.[^html]
- Table 5: PolyG has the highest win rate on ⟨s,p,o⟩ (70.42%) and nested questions (65.42%); RoG is strongest on ⟨s,p,\*⟩ among baselines (47.08%) but PolyG is close at 45.00%.[^html]

# Relevance to neocortex
- The three graphs are large (up to 193M relations), so neocortex at 10k memories cannot host them; only the question taxonomy and the gold-answer subset are transferable.[^html]
- The traversal table is a concrete design for neocortex: BFS expansion for entity lookups, meta-path walks for predicate-constrained questions, and shortest-path search for the ⟨s,\*,o⟩ and ⟨s,p,o⟩ checks. This is close to the existing path beam and [PPR seeding](/techniques/ppr-seeding.md) work.[^html]
- PolyG needs an LLM for categorisation and Cypher generation, so it can only run on the agent side. The gold-answer subset (⟨s,p,\*⟩ types) can be scored offline if the graph is ingested.
- Licence: the repository LICENSE is Apache-2.0 (text fetched).[^code]

[^html]: arXiv HTML full text (Sections 3-6, Tables 1-4, 10-11, Appendix F)
[^code]: github.com/Liu-rj/PolyG LICENSE (raw file, HTTP 200, Apache License text)
