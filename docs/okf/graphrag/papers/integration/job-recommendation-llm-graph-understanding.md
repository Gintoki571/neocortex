---
type: "Paper"
title: "Exploring Large Language Model for Graph Data Understanding in Online Job Recommendations"
description: "GLRec: a BELLE-LLaMA-7B recommender fed with behaviour-graph meta-path prompts and a path-attention augmentation, tuned with LoRA for job matching (AAAI 2024)."
resource: "https://dl.acm.org/doi/10.1609/aaai.v38i8.28769"
tags: ["job-recommendation", "graph-prompting", "meta-path", "lora", "behaviour-graph"]
year: 2024
venue: "AAAI 2024 (AAAI-24, pp. 9181 onward)"
llm_required: indexing
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://ojs.aaai.org/index.php/AAAI/article/download/28769/29476"
    title: "AAAI open-access PDF (pdftotext -layout; Sections 3-4, Tables 1-2); the DOI page is an ACM bot check, not used"
  - id: openalex
    resource: "https://api.openalex.org/works?search=Exploring%20Large%20Language%20Model%20for%20Graph%20Data%20Understanding"
    title: "OpenAlex record (abstract rebuilt from the inverted index)"
---
# Summary
- Turns a job-behaviour graph (candidates, jobs, matches, interactions) into LLM prompts by serialising meta-paths, so an LLM can read higher-order interactions.[^paper][^openalex]
- Three prompt strategies: path augmentation with phrase-level attention, a shuffle mechanism, and a hybrid of both with a path soft selector.[^paper]
- Evaluated on two job datasets, RecrX and RecrY, with point-wise and pair-wise AUC, random and out-of-distribution splits.[^paper]

# Method
- Backbone: BELLE-LLaMA-7B, tuned with LoRA on instruction data. Answer parsing takes the softmax probability of the yes/no token.[^paper]
- Optimizer: Adam with learning rate 1e-4 for the baselines; the update controller lambda is in (0, 0.5] for the word-embedding update.[^paper]
- Datasets: RecrX has 12,440 candidates, 19,318 jobs, 23,879 matches and 54,147 interactions; RecrY has 18,260 candidates, 26,576 jobs, 47,725 matches and 119,529 interactions; train:test ratio 5:1.[^paper]

# Results
- Table 2 (AUC): GLRec 0.891 / 0.876 / 0.810 / 0.843 / 0.814 / 0.852 / 0.905 / 0.883 across the eight columns, against TALLRec as the best baseline (0.842, 0.829, 0.770, 0.788, 0.766, 0.798, 0.849, 0.825 in the same order).[^paper]
- The table's average-improvement row reports gains of 13.2% to 29.8% over the baselines per column.[^paper]

# Relevance to neocortex
- Skip. It is a job-matching recommender whose core is a 7B LLM fine-tuned on job behaviour, which neocortex's memory engine does not need.
- The idea of serialising meta-paths for an LLM reader is the same as neocortex's agent-side path output, but neocortex already returns structured paths, so there is nothing to copy.

[^paper]: AAAI open-access PDF (pdftotext -layout; Sections 3-4, Tables 1-2)
[^openalex]: OpenAlex record (abstract)
