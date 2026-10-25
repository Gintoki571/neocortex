# Generative Agents: memory stream, importance, recency, reflection (Park et al., 2023)
Source: https://arxiv.org/abs/2304.03442 (HTML read; Sec 4.1 Memory Stream, Sec 4.2 Reflection).

**Problem.** Believable long-horizon agent behaviour needs retrieval that mixes relevance with recency and significance, and higher-level abstractions of raw observations.

**Data model.** Memory stream: list of natural-language records with creation timestamp, last-access timestamp, importance integer (Sec 4.1). Reflections are stored in the same stream and are retrievable alongside observations; they form trees (leaf = observation, parent = abstraction) (Sec 4.2).

**Write path.** Each observation is appended. Importance = one LLM call per memory: "On a scale of 1 to 10, where 1 is purely mundane ... and 10 is extremely poignant, rate the likely poignancy" (Sec 4.1). Reflection is triggered when the sum of importance of recent events exceeds 150 (about 2-3 times per day of sim time); the LLM is asked for 3 salient questions about the last 100 records, then retrieves for each and writes insights citing evidence ids (Sec 4.2).

**Read path (exact).** `score = a_rec*recency + a_imp*importance + a_rel*relevance`, every term min-max normalised to [0,1], all alphas = 1. recency = 0.995^(hours since LAST ACCESS); relevance = cosine(query, memory embedding); importance = the stored 1-10 integer. Top-ranked records that fit the prompt are used (Sec 4.1). Retrieval refreshes last-access, so frequently recalled memories stay recent.

**Time handling.** Only decay-by-last-access; no event-time vs record-time distinction.

**Dedup / identity.** None.

**Consolidation / forgetting.** No deletion. Reflection is the only consolidation. Decay only affects ranking.

**Cost.** 1 LLM call per observation (importance) + reflection calls; per-agent-day thousands of calls.

**Reported results.** Ablation of memory components (observation, planning, reflection) on interview "believability" TrueSkill; the full architecture beats every ablation, with reflection necessary for questions like "who would you pick to help with X" (Sec 6). No retrieval-recall numbers.

**What neocortex could steal.**
1. The *access-refresh* decay `0.995^(hours since last access)` is the cheapest form of the testing effect (see `testing-effect-forgetting.md`): neocortex `recall.rs:907` uses `0.9 + 0.1*2^(-age/half_life)` on age only, and `recall.rs:1021` already bumps `access_count,last_access`. Lane nc-e45 is doing access reinforcement; this paper is the origin of the recency-from-last-access form, but it is a rank term, not a store mutation.
2. Importance without an LLM: replace the 1-10 poignancy call by a deterministic salience score (is there a retraction, a number, a named entity count, an explicit "remember/decision" cue, surprise vs nearest neighbour, see `em-llm.md`). The calling agent can also pass `importance` through the tool interface (touches `remember.rs::prepare`).
3. Reflection trigger = running sum of importance > threshold. LLM-free variant: when the accumulated surprise/salience since the last consolidation crosses a bound, run the consolidation job (community refresh, `overview` cache invalidate, predicate merge). Touches engine background pass; no LLM.

**Weak / wrong.** Unit weights of 1 with min-max normalisation make scores incomparable across queries; no tuning evidence. Importance by LLM is expensive and noisy. Reflection quality unmeasured as retrieval.
