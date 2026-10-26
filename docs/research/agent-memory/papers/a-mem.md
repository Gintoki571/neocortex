# A-MEM: agentic memory with Zettelkasten linking (Xu et al., 2025)
Source: https://arxiv.org/abs/2502.12110 (HTML read; Sec 3.1-3.3, Table 1, Table 3).

**Problem.** Fixed memory structures; propose notes that link and rewrite each other.

**Data model.** Note `m_i = {c_i content, t_i timestamp, K_i LLM keywords, G_i LLM tags, X_i LLM context description, e_i embedding, L_i links}` (Sec 3.1). Embedding is over the concatenation of content+keywords+tags+context.

**Write path.** (1) one LLM call builds K,G,X; (2) cosine to all stored notes, top-k nearest candidates (Sec 3.2); (3) a second LLM call decides which candidates to link; (4) memory evolution: an LLM rewrites context/keywords/tags of the neighbours given the new note (Sec 3.3). So every write = at least 3 LLM calls and mutates old notes in place.

**Read path.** Embed query, cosine top-k (k=10 default; per-category k in Appendix A.5), then the linked neighbours of hits are appended. No graph algorithm, no fusion with BM25. Embedder all-MiniLM-L6-v2.

**Time handling.** Timestamp stored; no validity logic.

**Dedup / identity.** None beyond the LLM's link decision.

**Consolidation / forgetting.** Evolution rewrites neighbours; no deletion.

**Cost.** Paper: ~1,200-2,500 tokens per retrieval vs ~16,900 for LoCoMo/MemGPT baselines (Table 1); 5.4 s per memory op with gpt-4o-mini, 1.1 s with local Llama 3.2 1B (Sec 4.3).

**Reported results.** LoCoMo F1, gpt-4o-mini, Multi-Hop category: A-Mem 45.85 vs LoCoMo-baseline 18.41 vs MemGPT 25.52 (Table 1; I did not verify the category-label order the LoCoMo release uses, and the Mem0 paper re-runs A-Mem and gets overall J 48.38 vs Mem0 66.88, arXiv 2504.19413 Table 2). Temporal F1 is flat (12.14 vs 12.04 baseline). Ablation: removing link generation + evolution collapses multi-hop; w/o evolution is intermediate (Table 3). Memora (2604.20006 Table 3) finds A-Mem strongest at weekly "Remembering" (FAMA 71.82) but falling to 40.78 at quarterly scale.

**What neocortex could steal.** Link generation as an LLM-free step: at insert time take the k nearest memories by vector (neocortex already does `remember.rs::knn_memory` and `write_similar`/`write_temporal_links`) and persist them as typed `semantic` edges used by graph expansion at recall. This is what RippleMem does without an LLM (see `ripplemem.md`). The "evolve neighbours" step needs an LLM; skip, or let the calling agent do it via `remember` updates.

**Weak / wrong.** Mutating old notes with an LLM destroys provenance and cannot be audited; there is no temporal contradiction handling; gains in the paper are on weak baselines and F1 metrics, and independent re-runs (Mem0 paper) show much lower numbers.
