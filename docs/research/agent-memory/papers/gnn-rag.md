# GNN-RAG: graph neural retrieval for LLM reasoning (Mavromatis & Karypis, 2024)
Source: https://arxiv.org/abs/2405.20139 (HTML read; Sec 4.1-4.4, Fig 1, Table 1).

**Problem.** LLM-based KG retrievers handle simple questions but miss multi-hop paths; GNNs can reason on dense subgraphs.

**Mechanism.** (1) Entity linking + **PageRank-based subgraph extraction** around question entities (He et al. 2021 recipe) (Sec 5). (2) A GNN (ReaRev, 3 layers) scores every node in the subgraph as answer/non-answer, trained on QA pairs with node classification. (3) Reasoning paths = **shortest paths from question entities to the GNN's top answer candidates**, verbalised for the LLM. Optional retrieval augmentation (RA) unions GNN paths with an LLM-based retriever's paths (Sec 4.4).

**Time / dedup / forgetting.** None; static Freebase.

**Cost.** GNN inference once per question, no extra LLM calls (abstract); the paper reports GNN-RAG with a 7B tuned LLM matching GPT-4 on WebQSP.

**Reported results.** WebQSP Hit 85.7 / F1 71.3; CWQ Hit 66.8 / F1 59.4 (SubgraphRAG Table 3 copy); +8.9-15.5 points over other KG-RAG methods on multi-hop/multi-entity questions (Fig 1). **Why GNNs: a deep GNN (L=3) beats a shallow one (L=1) and the LLM retriever on multi-hop answer coverage, but the LLM retriever is better on 1-hop questions** because single-hop needs accurate question-relation matching not graph search (Sec 4.3, with a theorem in the appendix).

**What neocortex could steal.** The structure of the pipeline is exactly neocortex's: PPR to get a candidate subgraph, then score nodes, then render shortest paths as facts. The extra mechanism is **answer-node scoring over the PPR subgraph with a learned model, then path extraction to the top answers**; neocortex ranks memories/chains, not answer entities. The 1-hop caveat supports keeping the lexical/relation-matching channel for single-hop and using graph machinery only when the question names a chain (neocortex's current gating). A CPU GNN (3 layers, hidden 64-128) on a 200-node ego graph is milliseconds, but needs labeled QA training data, which is the same blocker as SubgraphRAG (lane nc-hop is on that); do not duplicate.

**Weak / wrong.** Freebase single-answer-type QA, needs supervised training per KG; shortest-path verbalisation drops side evidence; results copy in other papers vary by reasoning LLM.
