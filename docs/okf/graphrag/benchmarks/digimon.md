---
type: "Benchmark"
title: "DIGIMON (graph RAG unified-framework benchmark)"
description: "Study that places graph-based RAG methods in one framework and compares them on 11 QA datasets, from specific to abstract questions."
resource: "https://arxiv.org/abs/2503.04338"
tags: ["benchmark", "graphrag", "unified-framework", "comparison"]
year: 2025
venue: "arXiv 2025"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2503.04338"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2503.04338"
    title: "arXiv HTML full text (Table 5 and dataset table)"
  - id: code
    resource: "https://github.com/JayLZhou/GraphRAG"
    title: "Repository README (DIGIMON; no licence set on GitHub)"
---
# Summary
- 11 datasets: six specific-QA (Quality, PopQA, MusiqueQA, HotpotQA, ALCE, MultihopQA) and five abstract-QA (Mix, MultihopSum, Agriculture, CS, Legal).[^html]
- Sizes (tokens / questions / chunks): MultihopQA 1,434,889 / 2,556 / 609; Quality 1,522,566 / 4,609 / 265; PopQA 2,630,554 / 1,172 / 33,595; MusiqueQA 3,280,174 / 3,000 / 29,898; HotpotQA 8,495,056 / 3,702 / 66,581; ALCE 13,490,670 / 948 / 89,562; Mix 611,602 / 125 / 61; MultihopSum 1,434,889 / 125 / 609; Agriculture 1,949,584 / 125 / 12; CS 2,047,923 / 125 / 10; Legal 4,774,255 / 125 / 94.[^html]
- MultihopQA has the same 2,556 questions and 609 chunks as MultiHop-RAG, so MultiHop-RAG results are reusable for it.[^html]
- Metrics: accuracy and recall for MultihopQA, PopQA and MusiqueQA; string recall (STRREC), exact match (STREM) and string hit (STRHIT) for ALCE; abstract QA is judged pairwise by GPT-4o on comprehensiveness, diversity, empowerment and overall.[^html]

# Method
- The unified framework decomposes graph RAG into stages (indexing, retrieval, generation) and reuses components across methods; over 100 variants of the retrieval stage are evaluated.[^html]
- Methods in the results table include LGraphRAG (local GraphRAG), RAPTOR and VGraphRAG (a method named in the table); chunk size is varied per dataset.[^html]
- Pairwise abstract-QA judging with GPT-4o: each pair of answers is judged per metric, not scored.[^html]

# Results
- Table 5 values: MultihopQA accuracy, LGraphRAG 55.36, RAPTOR 56.06, VGraphRAG 59.66; MusiqueQA accuracy 12.47, 24.13, 26.93; ALCE STRREC 28.45, 35.26, 41.02 (same method order).[^html]
- The paper finds HippoRAG and RAPTOR best on simple QA and RAPTOR and LGraphRAG best on complex QA (MultihopQA, ALCE). Smaller chunks help simple questions; complex questions are stable across chunk sizes.[^html]

# Relevance to neocortex
- Use the MultihopQA slice as a second source of multi-hop retrieval numbers that are directly comparable with MultiHop-RAG, and reuse the unified harness to compare neocortex against those methods.[^html]
- Offline: indexing and answering need an LLM in the harness (OpenAI or Ollama, see the project entry), so only the retrieval slice can run without one, and only after the harness is adapted to call neocortex.[^code]

[^html]: arXiv HTML full text (Table 5 and dataset table)
[^code]: Repository README (DIGIMON; no licence set on GitHub)
