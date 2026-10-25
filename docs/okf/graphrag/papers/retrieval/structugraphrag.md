---
type: Paper
title: 'StructuGraphRAG: Structured Document-Informed Knowledge Graphs for Retrieval-Augmented Generation'
description: Builds structured-document-informed KGs for RAG (AAAI 2024 Spring Symposium).
resource: https://ojs.aaai.org/index.php/AAAI-SS/article/view/31798/33965
tags:
- kg-construction
- structured-documents
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
year: 2024
venue: AAAI 2024 Symposium
llm_required: both
neocortex_relevance: watch
sources:
- id: paper
  resource: https://ojs.aaai.org/index.php/AAAI-SS/article/view/31798/33965
  title: AAAI FSS-24 paper, downloaded PDF read with pdftotext -layout
---

# Summary
- StructuGraphRAG builds knowledge graphs from document structure for social-science RAG, using the NSDUH codebook as the test source.[^paper]
- The codebook PDF is parsed page by page into tables and figures; each variable description becomes its own chunk, then entities and relations are extracted with multi-agent LLM prompting.[^paper]
- The paper compares against a GraphRAG baseline and reports better accuracy, comprehensiveness and contextual relevance; no numeric results were recovered from the PDF text, and the evaluation is described as expert review.[^paper]

# Method
- Parsing: rectangle detection for tables and figures on each page, converted to structured text; one variable per chunk to avoid the fragmentation of fixed-length chunking.[^paper]
- Extraction: multi-agent LLM prompting (GPT-4o named in the workflow) for entity and relation extraction, then embedding of the extracted elements.[^paper]
- Retrieval: similarity search over the chunks plus graph traversal, answering with the LLM.[^paper]

# Results
- No table of numbers was found in the extracted PDF text; the claims are qualitative.[^paper]

# Relevance to neocortex
- The LLM is needed for extraction at write time, so it stays on the agent side.
- The reusable idea is structure-aware chunking: keep one record per codebook variable, so that each memory is a self-contained fact. neocortex's `remember` already takes one fact per call, so this is a guideline, not new code.
- Watch: a short symposium paper with no quantitative results.

[^paper]: AAAI Fall Symposium FSS-24 paper, https://ojs.aaai.org/index.php/AAAI-SS/article/view/31798/33965 (PDF read with pdftotext -layout)
