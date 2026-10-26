# MemGPT / Letta: LLMs as operating systems (Packer et al., 2023)
Source: https://arxiv.org/abs/2310.08560 (HTML full text read; Sec 2 architecture, Sec 3 Table 2 DMR).

**Problem.** Fixed context windows; give the illusion of unbounded context by paging between a small "main context" and external stores.

**Data model.** Main context = system instructions (read-only) + working context (writable scratchpad of key facts) + FIFO message queue (Sec 2.1). External context = recall storage (full message log, searchable) and archival storage (arbitrary text, embedding search) (Sec 2).

**Write path.** The LLM itself calls functions (`core_memory_append/replace`, `archival_memory_insert`, ...) to move facts from the queue into working context or archival. Every memory edit is an LLM function call (Sec 2.3). Queue manager appends each message to the FIFO and to recall storage.

**Read path.** The LLM issues search functions (recall by text/date, archival by embedding) with pagination; results re-enter the queue. Function chaining lets it page through results (Sec 2.4). No scoring formula is specified in the paper; ranking is whatever the vector store returns.

**Time handling.** Recall storage is date-searchable; no validity intervals, no supersession logic.

**Dedup / identity.** None. The agent is trusted to edit working context.

**Consolidation / forgetting.** Memory pressure: a system "warning" message is injected at a token threshold (the paper's example is 70% of context, Sec 2.2); the queue evicts a fraction of messages and replaces them with a recursive summary. Evicted messages stay in recall storage.

**Cost.** Several LLM calls per user turn (heartbeat/function chaining); latency is LLM-bound (seconds).

**Reported results.** Deep Memory Retrieval (MSC-derived): GPT-4 Turbo 35.3% -> 93.4% with MemGPT; GPT-4 32.1% -> 92.5%; GPT-3.5 38.7% -> 66.9% (Table 2). Zep (arXiv 2501.13956 Sec 4.1) later showed that simply stuffing the whole DMR conversation into gpt-4-turbo gets 94.4%, so DMR does not discriminate memory systems.

**What neocortex could steal.** Nothing for the engine: neocortex is already the "archival + recall" tier and the calling agent is the MemGPT controller. The one portable idea is the explicit **working-context block** (a small, always-injected, agent-editable set of facts). neocortex could expose it as a pinned-memory kind surfaced by `overview` (touches `recall.rs::overview`, `engine.rs`); no LLM needed server-side. Low priority.

**Weak / wrong.** DMR is saturated by full context (94.4%), so the 93.4% headline is not evidence of better memory. Every operation costs an LLM call and failure of the controller silently loses facts. No temporal validity.
