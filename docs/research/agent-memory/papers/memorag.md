# MemoRAG: global-memory-enhanced retrieval (Qian et al., 2024)
Source: https://arxiv.org/abs/2409.05591 (HTML read; Sec 3 method, Table 1).

**Problem.** Standard RAG fails when the query is implicit or the evidence is scattered ("what are the main themes of the book"); build a "global memory" of the whole context and use it to guess clues.

**Data model.** A light long-range LLM compresses the context window by window into a small set of memory tokens in KV space (compact global memory, Sec 3.2: memory tokens are appended per window and the raw KV cache is discarded). Raw text is kept for retrieval.

**Write path.** One pass of the memory model over the entire context (prefill with memory-token compression, compression ratio configurable). No graph, no entities.

**Read path.** Memory model reads the query against the global memory and writes a *draft answer / clues*; the clues are used as retrieval queries over the raw chunks; a heavy generator answers from retrieved evidence (Algorithm 1).

**Time / dedup / forgetting.** None; documents are static.

**Cost.** Requires a trained memory LLM (and GPU) plus a generator; latency seconds.

**Reported results.** Beats RAG variants and long-context LLMs on LongBench and InfiniteBench and 18 UltraDomain datasets (Table 1, Fig 3); no agent-conversation benchmark.

**What neocortex could steal.** The idea (query -> hypothetical draft -> retrieve) is HyDE-style query expansion. The LLM-free version: the *calling agent* writes the draft ("the person she married lives in ...") before calling `recall`; the tool schema can invite an optional `hypothesis` argument embedded as a second query vector and fused by RRF (touches `mcp.rs` schema, `recall.rs` query channels). No server-side LLM.

**Weak / wrong.** Needs a trained memory model; static documents; no update semantics, so unusable as agent memory as published.
