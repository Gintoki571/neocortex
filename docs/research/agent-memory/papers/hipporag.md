# HippoRAG: hippocampal-indexing KG + Personalized PageRank (Gutierrez et al., NeurIPS 2024)
Source: https://arxiv.org/abs/2405.14831 (HTML read; Sec 2-3, Tables 2, 3, 5).

**Problem.** Single-step multi-hop retrieval and knowledge integration across passages, inspired by hippocampal indexing theory (neocortex = LLM extractor, parahippocampal = synonymy detector, hippocampus = KG + PPR).

**Data model.** Schemaless KG of noun-phrase nodes N and OpenIE relation edges E, plus synonymy edges E' between phrase nodes whose retriever-embedding cosine >= tau = 0.8 (Sec 3, Sec 4 hyper-params). Phrase->passage map P_i = set of passages containing node i.

**Write path.** LLM (GPT-3.5, 1-shot) NER then OpenIE per passage; embed phrases; add synonym edges. Costly: one or two LLM calls per passage.

**Read path (exact).** LLM extracts query named entities; each is linked to the closest KG node by embedding (R_q); PPR is run with reset distribution on R_q, damping factor 0.5 (the restart probability, tuned on 100 MuSiQue examples). Each seed's probability is multiplied by **node specificity** s_i = |P_i|^-1 (a local IDF); passages are ranked by the summed PPR mass of their nodes (Sec 3.2-3.3).

**Time / dedup / forgetting.** None. Dedup only through synonym edges (soft, not merged).

**Cost.** Online retrieval 10-30x cheaper and 6-13x faster than IRCoT (abstract); offline indexing needs an LLM per passage.

**Reported results (R@2 / R@5).** MuSiQue 40.9/51.9, 2Wiki 70.7/89.1, HotpotQA 60.5/77.7 (ColBERTv2 retriever, Table 2). Multi-step HippoRAG+IRCoT adds +4 / +18 / +1 R@5 (Table 3).
Ablations, R@5 (Table 5): replacing the LLM OpenIE with **REBEL** drops MuSiQue 51.9 -> 39.6, 2Wiki 89.1 -> 76.5, HotpotQA 77.7 -> 59.2; Llama-3.1-8B OpenIE: 51.9 / 77.5 / 75.1; PPR replaced by seed-node scores only: 41.0 / 61.4 / 66.2 (so propagation is worth +28 R@5 on 2Wiki); no node specificity: 50.2 / 88.8 / 73.7; no synonymy edges: 50.2 / 85.6 / 75.7.

**What neocortex could steal.** (1) neocortex already has PPR (`recall.rs::overview_ppr`, graph channel). Check whether seeds are weighted by node specificity |P_i|^-1, i.e. the number of memories mentioning the entity (a one-line `COUNT(*)` on mentions): hub entities such as a city or a language otherwise drown the chain (this is exactly the hub failure found in `multihop.md` 3.3). (2) HippoRAG confirms that a weak extractor (REBEL-class) costs 12-18 R@5 points: extraction quality is the ceiling for graph retrieval, so an LLM-free extractor must be judged by downstream multi-hop, not by NER F1. (3) Synonym edges at cosine 0.8 are the soft-merge neocortex already has as `similar` hints (`remember.rs::write_similar`); HippoRAG shows they should be *traversable* by PPR, not only displayed. Verify they are in the PPR adjacency.

**Weak / wrong.** LLM at index time; seeds depend on an LLM NER at query time (HippoRAG 2 removes this, see `hipporag2.md`); recall of passages only, 2Wiki-style named-entity questions favour it. Hyper-parameters tuned on 100 examples.
