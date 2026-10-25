---
type: "Technique"
title: "KG-grounded draft correction"
description: "Check an LLM draft's claims against KG facts, then retrofit or re-plan only the unsupported parts."
tags: ["technique", "hallucination", "verification", "knowledge-graph", "self-correction"]
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: kgr
    resource: "https://arxiv.org/html/2311.13314"
    title: "KGR, arXiv HTML full text"
  - id: pog
    resource: "https://arxiv.org/html/2410.23875"
    title: "Plan-on-Graph, arXiv HTML full text"
  - id: cok
    resource: "https://arxiv.org/abs/2305.13269"
    title: "Chain-of-Knowledge, arXiv abstract"
---
# Summary
- KGR extracts claims from a draft, detects entities and retrieves KG facts, selects and verifies facts, then retrofits the response.[^kgr]
- Plan-on-Graph keeps a memory of what was found and reflects on whether to self-correct erroneous paths.[^pog]
- Chain-of-Knowledge corrects rationales step by step when sampled answers lack a majority; the abstract is the only part read.[^cok]

# Method
- Claim extraction, entity detection, fact selection, verification and retrofit are the five KGR stages; the LLM is used for extraction, selection, validation and retrofit.[^kgr]
- Verification is a fact match against retrieved KG triples; the retrofit rewrites only unsupported statements.[^kgr]
- PoG's memory update and reflection steps are LLM-driven loops around KG exploration.[^pog]

# Results
- KGR gains F1 over QKR by at least 6.2 on Mintaka and 1.1 on HotpotQA (paper text).[^kgr]
- PoG's memory ablation: removing memory drops CWQ Hits@1 from 63.2 to 58.9 and GrailQA from 76.5 to 69.3.[^pog]

# Relevance to neocortex
- The deterministic half is a good fit for neocortex: the calling agent sends a draft's claims, and the engine returns the matching current facts, superseded facts (with valid_from/valid_to) and conflicts. The rewrite stays with the agent. Claim matching is a lexical and entity check, so it needs no LLM.[^kgr]
- Memory-of-findings (PoG) is the same idea as storing intermediate facts; neocortex can store them with an episode tag so they are auditable.[^pog]
- Papers: [KGR](/papers/integration/kg-retrofitting-hallucination.md), [Plan-on-Graph](/papers/integration/plan-on-graph.md), [Chain-of-Knowledge](/papers/integration/chain-of-knowledge.md).

[^kgr]: KGR, arXiv HTML full text
[^pog]: Plan-on-Graph, arXiv HTML full text
[^cok]: Chain-of-Knowledge, arXiv abstract
