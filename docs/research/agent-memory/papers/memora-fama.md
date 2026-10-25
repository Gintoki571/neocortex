# Memora: weeks-to-months memory benchmark with Forgetting-Aware Memory Accuracy (Uddin et al., 2026)
Source: https://arxiv.org/abs/2604.20006 (HTML read; Sec 1, 3-4, Table 1, Table 3). Benchmark preprint.

**Problem.** Benchmarks measure inclusion of facts, not *misuse of obsolete facts*, and need few-session evidence.

**Data model (benchmark).** 10 professional personas; simulated sessions at weekly / monthly / quarterly horizons (avg 155 / 615 / 1,991 sessions per persona); memory types preference, activity, goal; a simulator mutates memory (drift, recurrence, invalidation); tasks: Remembering, Reasoning, Recommending.

**Metric (exact).** `FAMA = max(0, MPA - lambda*(1 - FAA))`, `lambda = N_forget / (N_presence + N_forget)`; MPA = fraction of memory-presence criteria met, FAA = fraction of forgetting-absence criteria met (obsolete info must NOT appear) (Sec 4).

**Findings.** Intro: 94% of LoCoMo and 85% of LongMemEval questions need at most two earlier sessions; average consolidation across prior benchmarks is ~1 session. Frontier LLMs reading the history (GPT-5.2, Claude Sonnet 4.5, Gemini 3 Pro, Qwen3-32B) score FAMA 21-27 on Remembering; memory agents (A-Mem, LangMem, Nemori, MemoryOS, Mem0, MemoBase) range 40-72 at weekly scale and **degrade toward the quarterly setting** (A-Mem 71.82 -> 40.78, Table 3, assuming the first three columns are Remembering weekly/monthly/quarterly); Reasoning stays near 0-30 for every system. Abstract: "Memory agents offer marginal improvements"; frequent reuse of invalid memories.

**What neocortex could steal.** (1) Adopt FAMA-style grading in neocortex's eval: add `expect_absent` penalties (already present in holdout: "expect_absent touring-kayak") as a first-class lambda-weighted term and report it per horizon. (2) Build a months-horizon synthetic world with mutation (the gen_world generator + update chains) and report recall as a function of number of intervening sessions; this is the literal weak spot "temporal reasoning over months". (3) Honest baseline: A-Mem/Nemori-style LLM memories already beat full-context LLMs at weekly scale but not at quarterly; a no-LLM bi-temporal store that never *reuses* an ended fact is the structural fix for FAA.

**Weak / wrong.** Simulation-generated; LLM judging of criteria; memory-agent configs run with default prompts and a single backbone.
