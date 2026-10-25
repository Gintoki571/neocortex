---
type: "Paper"
title: "Graph Chain-of-Thought: Augmenting Large Language Models by Reasoning on Graphs"
description: "Graph-CoT lets an LLM traverse a text-attributed graph step by step (reason, interact, execute) and ships GRBench, 1,740 questions over 10 domain graphs."
resource: "https://arxiv.org/abs/2404.07103"
tags: ["graph-cot", "text-attributed-graph", "benchmark", "grbench", "agent"]
year: 2024
venue: "arXiv 2024 (README row)"
llm_required: both
neocortex_relevance: watch
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2404.07103"
    title: "arXiv abstract page (fetched)"
  - id: html
    resource: "https://arxiv.org/html/2404.07103"
    title: "arXiv HTML full text (Sections 3-5, Tables 1-4)"
---
# Summary
- Graph-CoT iterates three sub-steps per round: LLM reasoning, LLM-graph interaction, and graph execution (graph functions run by the system and returned to the LLM).[^html]
- Introduces GRBench: 1,740 questions over 10 domain graphs (5 domains shown in Table 2: Academic, E-commerce, Literature, Healthcare, Legal), built from manually designed templates plus GPT-4 question expression and graph-function answer generation.[^paper][^html]
- Table 1 lists Academic graphs such as CS (about 8M nodes, 52M edges, 150 questions) and Biology, Chemistry, Material Science, Medicine and Physics (140 questions each).[^html]

# Method
- Each iteration: the LLM picks the next graph operation from the question and the observations so far, the graph executes it and returns the results as text.[^html]
- Retriever for the baselines and the method: all-mpnet-base-v2 with FAISS indexing. Backbone: GPT-3.5-turbo-16k (main results), temperature 0 for consistency.[^html]
- Compared against standard LLMs, text RAG and graph RAG (1-node, 1-hop and 2-hop ego-graphs, linearised to text).[^html]

# Results
- Table 2 (ROUGE-L, GRBench): Graph-CoT 31.89 (Academic), 42.40 (E-commerce), 41.59 (Literature), 22.33 (Healthcare), 30.52 (Legal); the highest ROUGE-L in every domain.[^html]
- The best graph-RAG row per domain (Mixtral-8x7b in four domains, GPT-3.5 in Healthcare) is 27.77, 32.87, 20.08, 8.69 and 23.48 ROUGE-L, so Graph-CoT leads by 4.1, 9.5, 21.5, 13.6 and 7.0 points respectively.[^html]

# Relevance to neocortex
- The three-step loop is an agent-side pattern: the calling agent asks neocortex for neighbours or paths in several calls, which matches how neocortex's recall is already used. The engine itself stays LLM-free.[^html]
- GRBench questions are long-form answers scored by an LLM, so they are not a clean offline benchmark for neocortex. Use the templates for path questions only.
- Watch. The cost is one LLM call per traversal step, so latency and token use would be high on the agent side.

[^paper]: arXiv abstract page
[^html]: arXiv HTML full text (Sections 3-5, Tables 1-4)
