---
type: "Paper"
title: "CogMG: Collaborative Augmentation Between Large Language Model and Knowledge Graph"
description: "Agent demo in which an LLM decomposes a question into KG triples, fills the triples missing from the graph, and writes the completions back into the KG (ACL 2024 demo)."
resource: "https://aclanthology.org/2024.acl-demos.35/"
tags: ["kg-update", "agent", "kbqa", "kopl", "hallucination"]
year: 2024
venue: "ACL 2024 (system demonstrations, pp. 365-373)"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://aclanthology.org/2024.acl-demos.35/"
    title: "ACL Anthology abstract page (fetched)"
  - id: pdf
    resource: "https://aclanthology.org/2024.acl-demos.35.pdf"
    title: "ACL Anthology PDF (pdftotext -layout; Sections 1-3, Table 1)"
---
# Summary
- Targets two KG limits: incomplete coverage, and update misalignment when KGs are filled by extraction. The LLM identifies the required triples that are missing, and the completed triples are added back to the graph.[^paper][^pdf]
- Demonstrated as a ReAct-style agent (Thought-Action-Observation) with tools for knowledge decomposition, KG query, knowledge completion, and RAG verification, fine-tuned on about 100k behaviour SFT examples.[^pdf]
- The demo's KG is KQA Pro's knowledge base, queried through a fine-tuned 7B model that writes KoPL programs.[^pdf]

# Method
- Knowledge decomposition: the target triples of the formal query are written with unknown parts as question marks; a fine-tuned model infers the triple decomposition, with label-name constraints so that output matches the KoPL labels.[^pdf]
- When the KG cannot answer, a completion tool proposes missing triples; the completion is checked by RAG over Wikipedia (256-token chunks, BM25 index, top ten chunks, query built from the triples and the question).[^pdf]
- Two agent routes are used, depending on whether the KG contains the needed knowledge.[^pdf]

# Results
- Table 1 (manual correctness on 50 KQA Pro questions): direct LLM answer 40%; CogMG without knowledge (relevant facts deleted, answered by completion) 44%; CogMG with knowledge updated 86%.[^pdf]
- The paper does not report a benchmark-wide metric; the 50-question study is a demo-scale check.[^pdf]

# Relevance to neocortex
- The agent-side loop (ask for a missing triple, write the completion back as a memory) is the pattern neocortex's `remember` calls already support; the demo shows it on the calling-agent side, not inside the engine.[^pdf]
- The 40% vs 86% gap is the case for keeping a fact store up to date, but it comes from an LLM with a 7B SFT model and a 50-question manual check. Treat as a motivating example only.
- Needs LLM calls at write and query time, so it cannot run inside neocortex's engine.

[^paper]: ACL Anthology abstract page
[^pdf]: ACL Anthology PDF (pdftotext -layout; Sections 1-3, Table 1)
