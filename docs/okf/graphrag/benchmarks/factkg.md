---
type: "Benchmark"
title: "FACTKG"
description: "108,674 fact-checking claims over DBpedia triples, labelled SUPPORTED or REFUTED, with five reasoning types and written and colloquial styles (ACL 2023)."
resource: "https://aclanthology.org/2023.acl-long.895.pdf"
tags: ["benchmark", "fact-checking", "kg", "dbpedia", "multi-hop"]
year: 2023
venue: "ACL 2023"
llm_required: none
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://aclanthology.org/2023.acl-long.895.pdf"
    title: "ACL Anthology PDF (pdftotext -layout; Sections 3-4, Tables 2-4)"
  - id: acl_page
    resource: "https://aclanthology.org/2023.acl-long.895.pdf"
    title: "ACL Anthology landing page (abstract)"
  - id: code
    resource: "https://github.com/jiho283/FactKG"
    title: "Repository README (fetched; no LICENSE file, HTTP 404 on raw LICENSE)"
---
# Summary
- 108,674 claims labelled SUPPORTED or REFUTED against DBpedia (2015 version), with five reasoning types: One-hop, Conjunction, Existence, Multi-hop and Negation.[^paper][^acl_page]
- Two writing styles: 34,462 written-style and 64,788 colloquial-style claims; a further 9,424 presupposition claims are used for the presupposition types.[^paper]
- Split 8:1:1 into train, dev and test, with the triples of each split disjoint from the others.[^paper]

# Method
- Claims are generated from DBpedia triples with templates, then negated, substituted (entity or relation) for REFUTED labels, and rewritten into colloquial style by annotators.[^paper]
- Baselines: Claim-only BERT, BlueBERT and Flan-T5, and GEAR with graph evidence. The modified GEAR retrieves 2-hop paths from the claim entities and converts them to text with a separator token.[^paper]

# Results
- Table 3 (accuracy, total): BERT claim-only 65.20, BlueBERT 59.93, Flan-T5 62.70, GEAR with evidence 77.65.[^paper]
- Per type, GEAR scores 83.23 on One-hop and 68.84 on Multi-hop; claim-only BERT scores 70.06 on Multi-hop, so the graph-evidence model is lower than BERT on that type.[^paper]
- Table 4 (cross-style, GEAR): W to W 81.00, W to C 75.43, C to C 80.81, C to W 78.80.[^paper]

# Relevance to neocortex
- Graph evidence is a set of DBpedia triples, which is the same shape as neocortex's relation facts. A neocortex run would retrieve the triples for the claim entities and check whether a support path exists; the one-hop, conjunction and negation types need no learned model beyond the label rule.[^paper]
- Labels need a verifier (GEAR or an NLI model), which is not LLM-free in neocortex's constraints. Only the evidence-retrieval half (recall of gold triples) can be scored offline.
- The multi-hop result shows that graph retrieval alone does not beat a claim-only model, which matches neocortex's known three-hop weak spot. Watch.
- Licence: the repository has no LICENSE file and its README states no licence; the dataset licence is unknown.[^code]

[^paper]: ACL Anthology PDF (pdftotext -layout; Sections 3-4, Tables 2-4)
[^acl_page]: ACL Anthology landing page (abstract)
[^code]: Repository README and HTTP 404 for LICENSE at raw.githubusercontent.com/jiho283/FactKG (HEAD)
