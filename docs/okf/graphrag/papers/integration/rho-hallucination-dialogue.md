---
type: "Paper"
title: "RHO (ρ): Reducing Hallucination in Open-domain Dialogues with Knowledge Grounding"
description: "RHO grounds a dialogue response in linked KG entities and relation predicates (local and global knowledge grounding) and re-ranks candidates, tested on OpenDialKG (ACL Findings 2023)."
resource: "https://arxiv.org/abs/2212.01588"
tags: ["dialogue", "hallucination", "knowledge-grounding", "kg-embeddings", "opendialkg"]
year: 2022
venue: "ACL Findings 2023"
llm_required: both
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2212.01588"
    title: "arXiv abstract page"
  - id: html
    resource: "https://arxiv.org/html/2212.01588"
    title: "arXiv HTML full text (Sections 3-5, Table 1)"
---
# Summary
- Grounds a dialogue generator in the representations of linked KG entities and relation predicates, instead of flattened triples.[^html]
- Local knowledge grounding (LKG) mixes the textual context with entity and predicate embeddings of the linked KG nodes; global knowledge grounding (GKG) adds the KG neighbourhood; response re-ranking (RR) picks among candidates.[^html]
- Data: OpenDialKG, where each dialogue turn is aligned with a KG path, so dialogues can be read as KG path traversals.[^html]

# Method
- Input with knowledgeable task guidance, then the LKG and GKG modules inject KG representations into the BART-based generator (the paper also reports GPT-2 and BART variants with the NPH baseline).[^html]
- Metrics include BLEU-4, ROUGE-L, FeQA, QuestEval, entity coverage, and reference-dependent and reference-free hallucination metrics (precision, recall, F1).[^html]

# Results
- Table 1 (OpenDialKG test, BLEU-4 / ROUGE-L): BART 14.45 / 33.33; BART+NPH 15.53 / 34.99; KG-BART 13.72 / 33.31; RHO with local grounding only 19.89 / 39.95; RHO full implementation 19.11 / 38.45.[^html]
- The paper reports that RHO outperforms the baselines on overlap and hallucination metrics in automatic evaluation. The remaining Table 1 columns (FeQA, QuestEval, entity coverage, reference-dependent and reference-free hallucination scores) were not reliably mapped from the text extraction, so they are not quoted here.[^html]

# Relevance to neocortex
- Skip: a dialogue generator that needs a seq2seq model with KG-embedding inputs. Not a memory or retrieval component.
- The one transferable idea is grounding faithfulness: score a generated answer against the KG facts it cites. neocortex can supply the facts, but the scoring needs an LLM or NLI on the agent side.

[^html]: arXiv HTML full text (Sections 3-5, Table 1)
