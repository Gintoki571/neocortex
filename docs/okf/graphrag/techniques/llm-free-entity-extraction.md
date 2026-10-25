---
type: Technique
title: LLM-free entity extraction
description: Get entities from text with NER or statistics instead of an LLM, so indexing costs no tokens.
tags:
- ner
- spacy
- indexing
- no-llm
generated:
  by: claude-haiku/okf-cataloger
  at: '2026-10-25T00:00:00Z'
status: draft
llm_required: none
neocortex_relevance: adapt
---

# Summary
- Sentence split by punctuation, then spaCy NER for entities; entity-passage and entity-sentence matrices replace relation triples.

# Method
- Sentence split by punctuation, then spaCy NER for entities; entity-passage and entity-sentence matrices replace relation triples.
- Relation extraction is skipped; entities alone serve as the bridges between passages.

# Papers using it
- [LinearRAG](/papers/organization/linearrag.md): spaCy NER, no relation extraction, zero tokens.
- [E2GraphRAG](/papers/organization/e2graphrag.md): SpaCy entities for the graph; LLM only for the summary tree.
- [AGRAG](/papers/organization/agrag.md): statistics-based graph construction instead of LLM entity extraction.

# Relevance to neocortex
- Directly answers neocortex's weak spot (2): relation extraction without an LLM scored 0.07 F1 with rules and 0.20 with GLiNER. Entity-only graphs avoid that failure. Quality of entity-only linking vs. relation facts is not measured in these papers' text read.
