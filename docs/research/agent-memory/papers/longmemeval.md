# LongMemEval: benchmarking chat-assistant long-term memory (Wu et al., ICLR 2025)
Source: https://arxiv.org/abs/2410.10813 (HTML read; Sec 3-5, Fig 3, Tables 2-4).

**Problem.** Existing memory benchmarks are short and shallow; test five abilities with scalable histories.

**Data model.** 500 questions; each has a timestamped haystack of user-assistant sessions. LongMemEval-S ~115k tokens (~50 sessions), LongMemEval-M ~1.5M tokens. Seven question types: single-session-user, single-session-assistant, single-session-preference, multi-session, knowledge-update, temporal-reasoning, abstention. Evidence sessions are human-edited; sessions carry evidence-turn labels so Recall@k / NDCG@k can be computed (Sec 3.3).

**Read path of the paper's recommended design (Sec 5).** Value = round (turn pair), not session; key = value + extracted user facts (document expansion); **time-aware query expansion**: index values by event date and let an LLM infer a time range from the query to filter candidates; reading with Chain-of-Note and JSON format. Retriever Stella V5 1.5B dense.

**Findings (exact).** Commercial memory assistants lose accuracy vs offline reading of the same history: ChatGPT -37%, Coze -64% with GPT-4o (Fig 3a). Long-context LLMs drop sharply from Oracle to S (Fig 3b). Round granularity beats session; session summaries/facts alone do not improve recall, but **K = V + fact** improves recall@k by 9.4% and QA by 5.4% (Table 3). Time-aware indexing + query expansion improves temporal-question recall by 6.8% (session) to 11.3% (round), and only when a strong LLM infers the time range (Table 4). Even with perfect retrieval, reading strategy changes accuracy by up to 10 points (Fig 6).
Other reports: Zep 71.2% (gpt-4o, `zep-graphiti.md`); Engram 83.6% (`engram-bitemporal.md`); Agent Zero Memory 95.6% (self-reported, `agent-zero-memory.md`). Memora notes 85% of LongMemEval questions need at most two sessions (2604.20006 Introduction).

**What neocortex could steal.** (1) Evaluate on LongMemEval-S with neocortex's retrieval-only metrics (Recall@k on evidence turns) since labels exist: no LLM judge needed, and it exercises temporal and knowledge-update classes neocortex claims. (2) Rule-based time-range extraction from the query ("last weekend", "in March", "two weeks ago") is already in `temporal.rs`; use it to filter/boost by `valid_from` the way Table 4 does, but without the LLM (the paper shows a weak 8B LLM gives no gain, so a rule parser is plausible). (3) Document expansion: index each memory's text *plus* extracted fact strings in FTS5 and in the vector (K = V + fact).

**Weak / wrong.** LLM-generated user side of the haystack; judged by LLM; S-set is small enough that full context nearly works for frontier models, so it under-tests scale. Single-session-assistant questions penalise fact-only stores.
