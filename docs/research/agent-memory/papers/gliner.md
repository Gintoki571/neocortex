# GLiNER: generalist NER with a bidirectional transformer (Zaratiana et al., 2023)
Source: https://arxiv.org/abs/2311.08526 (HTML read; Sec 2, Table 1).

**Problem.** Open-type NER without a 7B autoregressive LLM.

**Mechanism.** Input = entity-type prompts `[ENT] t1 [ENT] t2 ... [SEP] text`; a BiLM (DeBERTa-v3) encodes everything; span embeddings (spans up to **K = 12** tokens) are `FFN(h_start || h_end)` and entity-type embeddings come from the [ENT] tokens; score `phi(i,j,t) = sigmoid(span_ij . type_t)`; trained with binary cross-entropy on Pile-NER (synthetic, ChatGPT-labelled passages with free-form entity types) (Sec 2.2, 3.1). Decoding: greedy non-overlapping spans above threshold.

**Write path / read path.** One forward pass per text for all types at once; parallel over spans; CPU-friendly.

**Reported results (zero-shot out-of-domain F1, 7 datasets, Table 1).** GLiNER-S 50M: 52.7 avg; GLiNER-M 90M: 55.4; **GLiNER-L 0.3B: 60.9** vs ChatGPT 47.5, UniNER-7B 53.7, UniNER-13B 55.6, GoLLIE-7B 58.0, InstructUIE-11B 47.2.

**Local measurement for comparison.** neocortex `extraction.md`: GLiNER2.5-small entity F1 0.858 on the team's 128-memory gold set and **0.707 on the independent 150-memory holdout** (rules tier 0.682, heuristic 0.698), 186 ms/memory in Python+torch. So zero-shot NER is ~55-61 F1 in the paper's out-of-domain benchmarks and ~0.71 in neocortex's own holdout: it is not a large win over a good heuristic on agent-style short sentences.

**What neocortex could steal.** Nothing new beyond the existing "optional background enrichment" plan; this card is the evidence base for it. If a candle port is attempted, GLiNER (v1 architecture, 50M-90M) is a much smaller port than GLiNER2.5's boundary head (the 4-6 engineer-week estimate in `extraction.md`); a v1-style span-matching head is ~200 lines on top of a DeBERTa encoder. Entities-only; no relations, dates or negation.

**Weak / wrong.** Synthetic training labels from ChatGPT; no calibration across type lists; no coreference; entity typing depends on the label list given at inference.
