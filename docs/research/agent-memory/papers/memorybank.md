# MemoryBank: Ebbinghaus-inspired forgetting (Zhong et al., AAAI 2024)
Source: https://arxiv.org/abs/2305.10250 (HTML read; Sec 2.1-2.3, Table 2).

**Problem.** Companion chatbot memory that forgets and reinforces like a person and tracks user personality.

**Data model.** Memory storage = raw turns + hierarchical LLM daily/global event summaries + LLM user-portrait/personality summaries. Retrieval = DPR-style dual-tower dense retrieval over turn and summary embeddings (Sec 2.2).

**Write path.** LLM summarisation per day and for global summary; personality insights. Embedding with a bi-encoder, FAISS.

**Read path.** Dense top-k; no lexical, no graph.

**Time handling.** Forgetting curve only.

**Consolidation / forgetting (exact).** Retention `R = e^(-t/S)`; S is a discrete memory strength initialised to 1 at first mention; each time the memory is recalled in conversation, **S += 1 and t is reset to 0** (Sec 2.3). Items are forgotten (probabilistically) as R falls. The paper states this is a simplified model with no overlearning or meaningful-material terms.

**Cost.** LLM per summary; negligible per retrieval.

**Reported results.** Qualitative plus a 194-probe simulated test on SiliconFriend: retrieval accuracy 0.763-0.814 (en), response correctness varying by backbone (Table 2). No ablation isolates the forgetting mechanism, so it is not shown to help retrieval.

**What neocortex could steal.** The reset-on-recall rule is the cheapest implementable testing-effect/spacing mechanic: keep `strength S` and `last_access` per memory (neocortex has `access_count,last_access`, `recall.rs:1021`), rank term `exp(-(now-last_access)/S_days)`, increment S on use. See `testing-effect-forgetting.md` for why a power law (ACT-R) beats this exponential and gives the same cost. No LLM.

**Weak / wrong.** Exponential decay conflicts with the power-law forgetting data (Wixted & Carpenter, Anderson & Schooler); unvalidated; forgetting probabilistically deletes facts, which would break "what did I say in March".
