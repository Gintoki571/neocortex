---
type: "Benchmark"
title: "MetaQA-3 (3-hop split)"
description: "The 3-hop split of MetaQA: 142,744 template questions over the WikiMovies KB, requiring three-hop logical reasoning."
resource: "https://arxiv.org/abs/1709.04071"
tags: ["benchmark", "multi-hop", "three-hop", "kbqa"]
year: 2018
venue: "AAAI 2018"
llm_required: none
neocortex_relevance: adopt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/1709.04071"
    title: "arXiv abstract"
  - id: pdf
    resource: "https://arxiv.org/pdf/1709.04071v3"
    title: "arXiv PDF v3 (Table 3 statistics)"
  - id: code
    resource: "https://github.com/yuyuz/MetaQA"
    title: "Repository README (3-hop: 15 question types, 10 templates each)"
---
# Summary
- The 3-hop split has 114,196 train, 14,274 validation and 14,274 test questions (total 142,744).[^pdf]
- 15 question types with 10 text templates each, and an NTM paraphrased copy and an audio copy exist for the same split.[^code]
- Metric: hits@1 (%); the split is the same train/validation/test partition used across MetaQA.[^pdf]

# Method
- Same protocol as MetaQA (see that entry). The split's test questions are the natural offline set for the three-hop weak spot.[^pdf]

# Results
- The paper's per-method hits@1 for 3-hop could not be mapped unambiguously from the PDF text extraction; quote from Table 1 directly.[^pdf]

# Relevance to neocortex
- Matches the neocortex blind-set weakness (three-hop chains at 0.667 acc@10 in the project's own blind set; that number is the project's, not this benchmark's). Run the 14,274-question test split offline for a like-for-like check. At about 100 ms per recall, the full test run is roughly 24 minutes of recall time.[^pdf]
- Use the question-type identifier files in the dataset to report accuracy per question type, which exposes which three-hop patterns fail.

[^pdf]: arXiv PDF v3 (Table 3 statistics)
[^code]: Repository README (3-hop: 15 question types, 10 templates each)
