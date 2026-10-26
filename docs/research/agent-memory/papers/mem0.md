# Mem0 / Mem0g: production extract-and-update memory (Chhikara et al., 2025)
Source: https://arxiv.org/abs/2504.19413 (HTML read; Sec 2.1 extraction/update, 2.2 Mem0g, Table 1, Table 2).

**Problem.** Cheap, persistent, multi-session memory for chat agents.

**Data model.** Dense: a flat set of short natural-language fact memories in a vector store. Mem0g: directed labelled graph G=(V,E,L) in Neo4j, nodes = entities with type/embedding/timestamp, edges = relation triplets (Sec 2.2).

**Write path.** Extraction: LLM (gpt-4o-mini) reads the latest message pair + a rolling summary + last m=10 messages and emits candidate facts. Update: for each fact retrieve the top s=10 similar memories and one LLM tool call chooses ADD / UPDATE / DELETE / NOOP (Sec 2.1). Mem0g adds an LLM entity extractor and relation generator, then a conflict detector that marks superseded relations obsolete (not deleted) (Sec 2.2).

**Read path.** Dense: embed query, top-k similar memories. Mem0g: entity-centric (find query entities, expand incoming/outgoing edges) plus a "semantic triplet" match of the whole query to triplets (Sec 2.2). No lexical channel, no PPR.

**Time handling.** Timestamps on nodes/edges; obsolete flag on invalidated relations. No bi-temporal model.

**Dedup / identity.** Embedding similarity + LLM judgement per fact.

**Consolidation / forgetting.** UPDATE/DELETE are the only mechanisms; DELETE removes contradicted facts.

**Cost.** 2+ LLM calls per message pair on write. Read: search p50 0.148 s, p95 0.200 s; total p95 1.44 s; 1,764 tokens/question (Table 2).

**Reported results (LoCoMo, LLM-judge J).** Overall: Mem0 66.88, Mem0g 68.44, Zep 65.99, LangMem 58.10, OpenAI memory 52.90, A-Mem 48.38, full-context 72.90 (Table 2). Per category Mem0g wins temporal (J 58.13, F1 51.55), Zep wins open-domain (76.60) (Table 1). Full context beats every memory system on accuracy but costs 26k tokens and p95 17 s.

**What neocortex could steal.** The ADD/UPDATE/DELETE/NOOP decision is the LLM-free part neocortex already has in `resolve.rs` + `remember.rs` (exclusive predicates, supersession). Nothing to take except the benchmark protocol: report J, tokens/query and p95 latency together (touches `eval/` harness reporting).

**Weak / wrong.** Vendor paper; Zep's authors dispute the Zep numbers (see Zep blog, "Is Mem0 really SOTA", seen only as a search result; unverified). Memory systems still lose to full context on LoCoMo accuracy. MemTrace (2606.17328) finds Mem0 weakest of the tested systems on persistence and trajectory questions. DELETE loses history, which Memora's FAMA metric would penalise only if used wrongly, but it forbids "what was true before" queries.
