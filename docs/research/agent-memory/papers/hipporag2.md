# HippoRAG 2: "From RAG to Memory" (Gutierrez et al., ICML 2025)
Source: https://arxiv.org/abs/2502.14802 (HTML read; Sec 3.2-3.5, Tables 3, 4, 5, Fig 3).

**Problem.** HippoRAG 1 is concept-centric and loses on simple factual QA and context-heavy queries; make the PPR memory robust across factual, associative and discourse tasks.

**Data model.** Open KG as in HippoRAG, plus **passage nodes** connected to every phrase they contain by "contains" edges, plus synonym edges (Sec 3.2). Triples are embedded too (for query-to-triple matching).

**Write path.** LLM OpenIE (Llama-3.3-70B in the paper) + embeddings; unchanged offline cost.

**Read path (exact).** (1) *Query to triple*: embed the whole query, retrieve top-k triples by embedding similarity (replaces NER-to-node linking). (2) *Recognition memory*: an LLM filter keeps relevant triples. (3) Seeds = up to k phrase nodes, ranked by their average triple rank score, reset probability from the scores; **all passage nodes are also seeds** with reset probability proportional to query-passage embedding similarity times a **weight factor** (best 0.05 on MuSiQue, 0.05-0.1 on NQ; 0.5 is 2 points worse, Table 5). (4) PPR; passages ranked by PageRank score. If no triple survives, fall back to dense passage ranking (Sec 3.5).

**Time / dedup / forgetting.** None.

**Cost.** Offline LLM; online one LLM call for the triple filter.

**Reported results.** Passage recall@5 average over NQ, PopQA, MuSiQue, 2Wiki, HotpotQA: HippoRAG 2 78.2 vs NV-Embed-v2 73.4 vs HippoRAG 1 63.8 vs RAPTOR 65.6 (Table 3). MuSiQue 74.7 (NV-Embed-v2 69.7), 2Wiki 90.4, HotpotQA 96.3.
**Ablations (Table 4, R@5 MuSiQue / 2Wiki / HotpotQA / avg):** full 74.7 / 90.4 / 96.3 / 87.1; NER-to-node 53.8 / 91.2 / 78.8 / 74.6; query-to-node 44.9 / 65.5 / 68.3 / 59.6; no passage node 63.7 / 90.3 / 88.9 / 81.0; **no LLM triple filter 73.0 / 90.7 / 95.4 / 86.4** (-0.7 average).

**What neocortex could steal, concretely.**
1. The LLM triple filter is worth only 0.7 R@5. Query-to-triple + passage-node PPR *without any LLM* keeps ~99% of HippoRAG 2. neocortex's semantic beam already does query-to-fact matching (KNN on `relations_vec`, `paths.rs::search_semantic`); the missing piece is feeding those matched facts' endpoints as **weighted PPR seeds** (score-proportional reset) in the `graph` channel instead of only as beam starts (touches `recall.rs` graph channel seeds, `paths.rs`).
2. Passage nodes = memory nodes inside the PPR graph, with reset mass proportional to the memory's query cosine times a small factor (0.05). neocortex's prose-hop (`PROSE_HOP 0.8`) is a heuristic version of this; unify as one PPR with memory nodes (+6.1 R@5 average in the paper when passage nodes are present). Needs no LLM.
3. NER-to-node vs query-to-triple is the largest swing (MuSiQue 53.8 vs 74.7): never seed only from extracted entity mentions.

**Weak / wrong.** Still needs an LLM extractor at write time (the paper uses a 70B model); no temporal semantics; results are passage recall on Wikipedia QA, not agent-conversation memory. MemTrace (2606.17328) shows HippoRAG-v2 is the strongest retriever on current/historical state questions but collapses (~20-24%) on trajectory questions.
