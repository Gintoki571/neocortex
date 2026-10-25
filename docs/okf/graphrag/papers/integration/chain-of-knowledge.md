---
type: "Paper"
title: "Chain-of-Knowledge: Grounding Large Language Models via Dynamic Knowledge Adapting over Heterogeneous Sources"
description: "Prepares sampled rationales, and only when the answers disagree corrects them step by step from heterogeneous sources (text, Wikidata, tables)."
resource: "https://arxiv.org/abs/2305.13269"
tags: ["llm-prompting", "knowledge-grounding", "hallucination", "sparql", "heterogeneous-sources"]
year: 2023
venue: "ICLR 2024"
llm_required: both
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2305.13269"
    title: "arXiv abstract"
  - id: html
    resource: "https://arxiv.org/html/2305.13269"
    title: "arXiv HTML full text (Sections 2, 4, Tables 2-7)"
---
# Summary
- Three stages: reasoning preparation (sampled rationales, domain selection), dynamic knowledge adapting (correction step by step using retrieved knowledge), and answer consolidation.[^html]
- Correction runs only when the sampled answers lack a majority consensus; otherwise the rationales are used as they are.[^paper]
- Adaptive query generator writes unstructured queries, SPARQL and SQL for Wikidata, tables and text sources.[^html]

# Method
- Five sampled rationales at temperature 0.7 (greedy decoding elsewhere), with ChatGPT (gpt-3.5-turbo-0613) as the black-box LLM.[^html]
- Few-shot demonstrations (3-shot or 6-shot) per domain; domains covered: factual (FEVER, HotpotQA), medical (MedMCQA), physics and biology (MMLU subsets) and table QA (FeTaQA).[^html]

# Results
- Table 2, 3-shot: CoK vs CoT-SC: FEVER 63.4% vs 59.9%; HotpotQA EM 34.1% vs 30.8%; MedMCQA 70.5% vs 60.3%; MMLU Physics 45.5% vs 42.7%; MMLU Biology 83.0% vs 81.1%; FeTaQA BLEU 25.0 vs VE 21.6.[^html]
- Table 2, 6-shot: CoK vs CoT-SC: FEVER 58.5% vs 56.2%; HotpotQA EM 35.4% vs 33.4%; MedMCQA 73.3% vs 65.8%.[^html]

# Relevance to neocortex
- Needs several LLM calls per question (sampling, domain selection, correction, consolidation), so it runs only on the agent side and cannot be part of neocortex's engine (no LLM inside the engine).[^html]
- The idea worth borrowing is agent-side: check the agreement of sampled answers and only spend retrieval on disagreeing questions. This is a cheap gate for the calling agent's recall calls.
- Not a retrieval method; neocortex's recall stays as it is.

[^paper]: arXiv abstract
[^html]: arXiv HTML full text (Sections 2 and 4, Tables 2-7)
