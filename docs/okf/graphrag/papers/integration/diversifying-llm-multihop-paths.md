---
type: "Paper"
title: "Different Paths to the Same Destination: Diversifying LLMs Generation for Multi-hop Open-domain Question Answering"
description: "DP-CoT: diverse passage- and sentence-level evidence generation, pruned by a BERT-style evidence classifier, for multi-hop open-domain QA (KBS 2025)."
resource: "https://www.sciencedirect.com/science/article/abs/pii/S0950705124014230"
tags: ["multi-hop", "chain-of-thought", "open-domain-qa", "evidence-pruning", "retrieval"]
year: 2025
venue: "Knowledge-Based Systems vol. 309 (2025), article 112789"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://www.sciencedirect.com/science/article/abs/pii/S0950705124014230"
    title: "ScienceDirect article page, fetched with obscura --stealth (abstract, highlights and introduction; the method and results sections are behind Elsevier organizational access, not bypassed)"
  - id: crossref
    resource: "https://doi.org/10.1016/j.knosys.2024.112789"
    title: "Crossref / OpenAlex metadata (title, DOI, authors; no abstract)"
  - id: code
    resource: "https://github.com/XD-BDIV-NLP/DP-CoT"
    title: "Repository LICENSE (Apache License text fetched); the README is an IRCoT quote and not a method description"
---
# Summary
- DP-CoT generates diverse evidence at two granularities, passage-level sampling and sentence-level proposal generation, so a multi-hop chain is not tied to a single reasoning path.[^paper]
- A BERT-style evidence classifier prunes the reasoning paths before the reasoning module.[^paper]
- The abstract reports recall gains of 4.8% (HotpotQA) and 1.1% (2WikiMultihopQA) over IRCoT with GPT-3 as the backbone.[^paper]

# Method
- Two-level diverse CoT generation (passage sampling, sentence proposals) followed by a trained BERT-style classifier that scores evidence and prunes the paths.[^paper]
- Evaluated on HotpotQA and 2WikiMultihopQA. Hyperparameters and the classifier training details are in the method section, which was not accessible.

# Results
- Recall improvement over IRCoT (GPT-3 backbone): +4.8% on HotpotQA, +1.1% on 2WikiMultihopQA (from the abstract).[^paper]

# Relevance to neocortex
- The problem (one reasoning path gives no chance to correct errors) matches neocortex's multi-hop weak spot, and the pruning classifier is the same kind of step as neocortex's hop scoring, but it is an LLM-driven generator plus a BERT-sized classifier.
- The generation half needs an LLM on the agent side. The classifier is BERT-style, so CPU use is plausible, but neocortex would need its own labelled evidence per hop; the training data was not verified.
- Watch. Read the full method once access allows before adopting any part.[^paper]

[^paper]: ScienceDirect article page (stealth fetch; abstract, highlights, introduction)
