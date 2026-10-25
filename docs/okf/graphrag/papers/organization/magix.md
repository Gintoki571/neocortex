---
type: Paper
title: 'MaGiX: A Multi-Granular Adaptive Graph Intelligence Framework for Enhancing Cross-Lingual RAG'
description: Multi-granular graph RAG for cross-lingual retrieval (EMNLP 2025 Findings).
resource: https://aclanthology.org/anthology-files/anthology-files/pdf/findings/2025.findings-emnlp.279.pdf
tags:
- cross-lingual
- multi-granular
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2025
venue: EMNLP 2025 Findings
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://aclanthology.org/anthology-files/anthology-files/pdf/findings/2025.findings-emnlp.279.pdf
  title: ACL Anthology PDF (findings-emnlp.279, read with pdftotext -layout)
---

# Summary
- MaGiX is a graph-based RAG framework for English-Vietnamese cross-lingual question answering.[^paper]
- It builds a multi-granular cross-lingual KG from fine-grained attribute descriptions per entity and cross-synonym edges, and uses a multilingual embedding model trained with contrastive learning for semantic alignment.[^paper]
- Retrieval combines graph-based reasoning with a semantic-aware reranking step; the authors evaluate on five benchmarks.[^paper]

# Method
- Multi-granular graph: entities keep attribute-level descriptions, so retrieval can work at the attribute granularity as well as the entity level.[^paper]
- Cross-lingual alignment: a contrastive (SimCSE-style, in-batch with hard negatives) embedding model maps Vietnamese and English descriptions together.[^paper]
- Ablation (Table 2, Recall@10 Vi-to-En as extracted): LightRAG baseline 23.11; + granular retrieval 78.90; + cross-synonym 80.73; + composite score 88.58; + fine-tuned embedding (full MaGiX) 87.79.[^paper]
- The graph is built with an LLM (Gemini 2.0 Flash is named as the generator in the experiment setup).[^paper]

# Results
- Passage retrieval (Table 1, Vi-En and En-Vi): MaGiX has the top value in most of the parsed columns, against HippoRAG 2 and BGE-M3 (for example 81.32 against 79.27 and 76.29 on one column).[^paper]
- QA (Table 3): win rates against four baselines across accuracy, comprehensiveness, diversity, logicality and coherence; the paper reports high win rates on most dimensions.[^paper]

# Relevance to neocortex
- The graph-building step is LLM-driven, so it is not usable inside the engine. The attribute-level node description and the contrastive cross-lingual embedding are the reusable ideas.
- Watch only: the setting is Vietnamese-English cross-lingual QA. neocortex's current weak spots are not cross-lingual.

[^paper]: ACL Anthology findings-emnlp.279 PDF, https://aclanthology.org/anthology-files/anthology-files/pdf/findings/2025.findings-emnlp.279.pdf (read with pdftotext -layout)
