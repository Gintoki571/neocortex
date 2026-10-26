# EM-LLM: human-like episodic memory for infinite context (Fountas et al., 2024)
Source: https://arxiv.org/abs/2407.09450 (HTML read; Sec 3.2 surprise, 3.3 refinement, 3.4 retrieval, Table 1, Fig 3-4).

**Problem.** Million-token context inside a Transformer; organise the KV cache into episodic events and retrieve them like humans do.

**Data model.** The KV cache is partitioned into *events* (contiguous token spans). Each event is represented by a few representative key vectors. Event boundaries come from surprise, then refined by graph clustering.

**Write path (exact).** Token t is a boundary when its surprisal `-log P(x_t | x_<t)` exceeds `T = mu_{t-tau:t} + gamma * sigma_{t-tau:t}` (running mean + gamma standard deviations over a window of past surprisals, Eq 1; gamma = 1e-3 in Table 2 experiments). Boundaries are then moved to maximise modularity (or minimise conductance) of the similarity graph of keys inside each event (Sec 3.3, Algorithm 1).

**Read path (exact).** Two buffers: a *similarity buffer* of k_s events by k-NN of the current query against event representatives, and a *contiguity buffer* of size k_c that enqueues the temporal neighbours (+-n events) of every retrieved event, giving the human free-recall contiguity-and-asymmetry effect (Sec 3.4; Fig 3A cites Howard & Kahana 2002). Recency is a third local window.

**Time handling.** Order only; contiguity is positional.

**Cost.** Inside the LLM forward pass; needs the model internals (surprise from its logits, KV keys). Not applicable to a CPU memory service as published.

**Reported results.** Outperforms InfLLM on 80% of LongBench task groups across 5 base models and beats NV-Embed-v2 RAG and full context with LLaMA-3.1-8B (Table 1, Fig 1); human-perceived event boundaries are matched better by surprise segmentation than by fixed/random segmentation (Fig 4).

**What neocortex could steal.** Two portable mechanisms, both LLM-free:
1. **Surprise-gated segmentation/encoding.** Replace token NLL by *embedding novelty*: `surprise(m_t) = 1 - max_j cos(e_t, e_j)` over the entity's or session's recent memories (or distance to a running context vector); mark an event boundary / a "salient" memory when `surprise > mu_window + gamma*sigma_window`. Use it (a) to assign `importance` at write, (b) to group successive memories into events for `overview`, (c) to decide which memories earn a replay/consolidation slot. Touches `remember.rs::prepare` (compute from `knn_memory` result already fetched), `recall.rs`.
2. **Contiguity buffer** = neocortex's `temporal_link_ids` (`recall.rs:104`) is already this (neighbours by valid_from). EM-LLM's addition is *asymmetry*: bias forward neighbours (next events) over backward. Test a forward weight 1.0 vs backward 0.5 in the temporal link expansion; cheap A/B on `eval/` multihop and temporal sets.

**Weak / wrong.** Needs model internals; evaluation is long-document QA, not agent facts or updates; boundary quality validated against human annotations on one audio dataset.
