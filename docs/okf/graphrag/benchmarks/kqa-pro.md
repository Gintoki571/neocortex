---
type: "Benchmark"
title: "KQA Pro"
description: "About 118K natural-language complex KBQA questions with KoPL programs and SPARQL queries, for diagnosing compositional reasoning skills."
resource: "https://arxiv.org/abs/2007.03875"
tags: ["benchmark", "kbqa", "compositional", "program", "sparql"]
year: 2020
venue: "ACL 2022"
llm_required: none
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2007.03875"
    title: "arXiv abstract"
  - id: pdf
    resource: "https://arxiv.org/pdf/2007.03875"
    title: "arXiv PDF (text extracted with pdftotext; Sections 3-5, Tables 3-4)"
  - id: code
    resource: "https://github.com/shijx12/KQAPro_Baselines"
    title: "Repository linked from the paper (not read)"
---
# Summary
- 117,970 diverse natural-language questions (the abstract rounds to about 120K), each with a KoPL program and a SPARQL query.[^pdf]
- Reasoning skills covered: multi-hop reasoning, value comparisons, set operations (intersect, union), verification and counting. Knowledge types: relations (363 relation types), attributes and qualifiers.[^pdf]
- Questions are generated from templates by random sampling and recursive composition, then paraphrased; paraphrases are filtered by edit distance to the canonical utterance.[^pdf]

# Method
- Locating stage: 7 strategies to pick the entity or concept the question is about; asking stage: templates with four quantitative operators (=, !=, <, >).[^pdf]
- The KoPL program is the explicit reasoning trace; the paper argues SPARQL alone cannot describe intermediate steps.[^pdf]

# Results
- The paper reports that state-of-the-art KBQA models score far lower on KQA Pro than on MetaQA and WebQSP; SRN reaches 12.33% on the relational-only subset (Table 4).[^pdf]

# Relevance to neocortex
- Gold relation chains come from the KoPL program, so neocortex can be scored per reasoning skill: multi-hop questions against the path search, and comparisons or counts against the relation facts. Set operations are not supported by neocortex today and will score zero, which is useful as a gap list.[^pdf]
- A KB slice must be loaded offline. The paper samples entities from its knowledge base and cites Virtuoso for SPARQL execution; the exact KB build was not verified, and the sampling code is in the baselines repo (not read).

[^pdf]: arXiv PDF (text extracted with pdftotext; Sections 3-5, Tables 3-4)
