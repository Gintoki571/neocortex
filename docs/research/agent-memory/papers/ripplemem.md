# RippleMem: associative recollection over an event graph (Ji et al., 2026)
Source: https://arxiv.org/abs/2608.13334 (HTML read; Sec 3.2-3.4, Tables 3, 5, 8-12). Preprint.

**Problem.** Evidence for a question is scattered across memories; first-hop recall returns an incomplete set.

**Data model.** Memory unit `m_i = (r_i restated event, v_i embedding, P_i participants, L_i locations, T_i time interval)`. Sparse weighted graph with two typed edge families: semantic `s_sem = cos(v_i, v_j)` and structural `s_str = sum_x beta_x * 1_x * u^x / sum_x beta_x * 1_x` where `u^P, u^L` are Jaccard overlaps of canonical participant / location sets and `u^T = 1` if intervals overlap else `exp(-gap/tau)` (Eq 2-4).

**Write path.** A schema-guided LLM turns overlapping dialogue windows into memory units (the only LLM step on write). Edges are LLM-free: candidate pool from vector KNN + inverted indexes over participants/locations/time, keep edges with **s_sem >= 0.85** (up to K_sem) and **s_str >= 0.60** (up to K_str) (Table 5).

**Read path.** Hybrid first-hop recall (semantic + lexical + structured cue match) then **R = 1 extra round** of anchor-local expansion along both edge types; an LLM "recollection controller" picks anchors and a "missing-support target" (2,880 tokens per question) (Eq 5-6; Table 4, 12).

**Cost.** Construction ~30x faster and 48.7-69.3x fewer tokens than Mem0g/Zep (Table 3); answer context 1,472 tokens/question. Read path has an LLM controller (can be dropped, see ablation).

**Reported results.** LoCoMo LLM-judge 87.14 (+3.95% over best baseline); LongMemEval-S up to +11.87% relative. **Ablations (Fig 3 / Table 8, LoCoMo J):** full 87.14; w/o graph expansion 83.12 (-4.0); w/o structural cues 83.83 (-3.3); w/o planned recollection (LLM controller) 84.35 (-2.8). **LLM-Edge replacement (Tables 9-10):** replacing the deterministic edge scoring by GPT-4.1-mini decisions gives 86.62 overall (worse) with 27.7x tokens and 33x calls, though multi-hop alone rises 77.67 -> 79.79.

**What neocortex could steal.** The graph here is *memories* linked by shared (who, where, when) cues, scored by Jaccard + temporal-overlap, thresholded, degree-capped, built incrementally at insert from KNN + inverted index. neocortex already has entity-mention edges and temporal links; the specific additions: (1) a **structural edge weight** between memories = weighted Jaccard of their entity sets (participants/locations) plus temporal-interval compatibility, stored with the existing `write_temporal_links` machinery and used for one bounded expansion round after first-hop recall (touches `remember.rs` link writers, `recall.rs` expansion; no LLM). (2) The measured ordering of gains tells where to spend effort: graph expansion (+4.0) > structural cues (+3.3) > LLM planning (+2.8). The LLM planner is the agent's job in neocortex (it can call `follow`).

**Weak / wrong.** Write path still needs an LLM extractor for units; thresholds fixed by hand; only English chat; memory units drop verbatim text (see `selective-forgetting.md` for what that costs).
