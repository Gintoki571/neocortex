---
type: "Benchmark"
title: "MetaQA-3 (3-hop split)"
description: "The 3-hop split of MetaQA: 142,744 template questions over the WikiMovies KB, requiring three-hop logical reasoning (AAAI 2018)."
resource: "https://arxiv.org/abs/1709.04071"
tags: ["benchmark", "multi-hop", "three-hop", "kbqa"]
year: 2018
venue: "AAAI 2018"
llm_required: none
neocortex_relevance: adopt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: pdf
    resource: "https://arxiv.org/pdf/1709.04071v3"
    title: "arXiv PDF v3 (pdftotext -layout; Table 1-2 3-hop columns, Table 3, Appendix A)"
  - id: readme
    resource: "https://github.com/yuyuz/MetaQA"
    title: "Repository README (3-hop folder layout, qtype files, KB format)"
---
# Summary
- The 3-hop split has 114,196 train, 14,274 validation and 14,274 test questions (142,744 in total; the test split is the same size as validation).[^pdf][^readme]
- 15 question types with 10 templates each (Vanilla); the 3-hop folder also has NTM and audio variants with the same split.[^pdf][^readme]
- Question types exclude the counter-intuitive Actor/Writer/Director to Movie to Actor/Writer/Director to Movie chains, which the paper dropped as confusing.[^pdf]
- Metric: hits@1 (%).[^pdf]

# Method
- Same protocol as MetaQA (see [MetaQA](/benchmarks/metaqa.md)): questions are sampled from templates over the WikiMovies KB and answered from the triple file.[^pdf][^readme]
- Question-type identifier files (qa_test_qtype.txt) give one type per question line, so per-type accuracy can be reported.[^readme]

# Results
- Vanilla 3-hop hits@1: VRN 62.5; Bordes et al. QA system 28.4; KV-MemNN 10.1; supervised embedding 28.9 (Table 1).[^pdf]
- Vanilla-EU 3-hop hits@1 (5% entity labels): VRN 38.3; Bordes 26.9; KV-MemNN 10.5; supervised embedding 25.3 (Table 1).[^pdf]
- NTM-EU 3-hop hits@1: VRN 38.0; Bordes 25.3; KV-MemNN 10.2; supervised embedding 24.2 (Table 2).[^pdf]
- Audio-EU 3-hop hits@1: VRN 21.1; Bordes 15.3; KV-MemNN 15.3; supervised embedding 12.1 (Table 2).[^pdf]

# Relevance to neocortex
- Matches the neocortex blind-set weakness (three-hop chains, 0.667 acc@10 on the project's blind set, as quoted in the bundle brief; that number is not from this benchmark). Run the 14,274-question test split offline for a like-for-like check. At about 100 ms per recall, the full test run is roughly 24 minutes.[^pdf]
- Use the question-type identifier files to report accuracy per question type, which exposes which three-hop patterns fail.[^readme]
- The NTM-EU 3-hop column (VRN 38.0) tests paraphrase robustness of the path search, the same weak spot as paraphrased middle hops.[^pdf]

[^pdf]: arXiv PDF v3 (pdftotext -layout; Tables 1-3, Appendix A)
[^readme]: github.com/yuyuz/MetaQA README
