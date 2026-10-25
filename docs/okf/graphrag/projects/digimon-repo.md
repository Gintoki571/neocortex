---
type: "Open-source Project"
title: "DIGIMON (repository)"
description: "Code repository of the DIGIMON unified graph-RAG framework (arXiv 2503.04338) with runnable configs for nine GraphRAG methods; no licence is published."
resource: "https://github.com/JayLZhou/GraphRAG"
tags: ["open-source", "graphrag", "framework", "harness", "python"]
language: "Python"
llm_required: both
neocortex_relevance: adapt
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: code
    resource: "https://raw.githubusercontent.com/JayLZhou/GraphRAG/HEAD/README.md"
    title: "Repository README (raw, HEAD; fetched)"
  - id: license
    resource: "https://raw.githubusercontent.com/JayLZhou/GraphRAG/HEAD/LICENSE"
    title: "Repository LICENSE: HTTP 404 (no licence file at the root)"
  - id: api
    resource: "https://api.github.com/repos/JayLZhou/GraphRAG"
    title: "GitHub REST metadata: license null, 1,541 stars, default branch master, last push 2025-07-01 (fetched)"
  - id: paper
    resource: "https://arxiv.org/abs/2503.04338"
    title: "arXiv 2503.04338 title: In-depth Analysis of Graph-based RAG in a Unified Framework (title from the arXiv page)"
---
# Summary
- Modular, decoupled implementation of graph RAG methods for comparison; the README points to the arXiv paper for the study, which is the benchmark entry in this bundle.[^code][^paper]
- Runs each method from a YAML config: `python main.py -opt Option/Method/<METHOD>.yaml -dataset_name your_dataset`. Methods listed: Dalk, GR, LGraphRAG (local GraphRAG search), GGraphRAG (global search), HippoRAG, KGP, LightRAG, RAPTOR and ToG.[^code]
- LLM backends: OpenAI models (gpt-4, gpt-3.5-turbo) by default; local models via Ollama or LlamaFactory when `api_type` is set to `open_llm`.[^code]
- Environment: conda (`experiment.yml`) per the README.[^code]

# Method
- Each method is a pipeline of LLM-driven graph construction (indexing) and LLM-driven retrieval and answering; the shared parts are the dataset loader and the evaluation scripts.[^code]

# Results
- No results measured in this pass; the study numbers are in the benchmark entry [DIGIMON](/benchmarks/digimon.md).

# Relevance to neocortex
- Useful as a comparison harness: its methods (HippoRAG, LightRAG, RAPTOR, ToG) are the reference points for neocortex's retrieval design, and the dataset loader can feed neocortex's recall step.[^code]
- Licence: GitHub reports no licence, and the repository has no LICENSE file. Do not copy code into neocortex until the authors grant a licence; re-implement from the method descriptions instead.[^license][^api]
- The configs call an LLM (OpenAI or local); the README does not say which stages are LLM-free, so check each method's code before an offline comparison.[^code]

[^code]: Repository README (raw.githubusercontent.com HEAD)
[^license]: Repository LICENSE, HTTP 404 (raw.githubusercontent.com HEAD)
[^api]: GitHub REST metadata (license null)
[^paper]: arXiv 2503.04338 abstract page (title)
