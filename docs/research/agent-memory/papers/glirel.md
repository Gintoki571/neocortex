# GLiREL: zero-shot relation classification (Boylan et al., 2025)
Source: https://arxiv.org/abs/2501.03172 (HTML read; Sec 3, Tables 1, 3, 6).

**Problem.** Fast zero-shot relation extraction given already-identified entity spans.

**Mechanism.** DeBERTa-v3-large encodes `[REL] label_1 [REL] label_2 ... [SEP] text`; the **entity spans are supplied** as inputs; every entity pair's representation (concatenation + refinement layers, N^2 pairs) is scored against each relation-label embedding by a similarity scorer (Sec 3.3). All pairs and all labels in one forward pass. Trained on synthetic data labelled with an LLM.

**Reported results.** Wiki-ZSL / FewRel unseen-relation macro F1 up to 83.67 / 87.60 for m=10 unseen labels (Table 3), several points over RelationPrompt, at 47.6 sentences/s on a Tesla T4 and **4.6-4.9 sentences/s on CPU** (batch 32; 467M parameters; Table 3, Sec 5.1), i.e. ~200 ms per sentence on CPU, vs RelationPrompt 2.0 and TMC-BERT 0.12-1.9 sentences/s on CPU. **Document-level (Re-DocRED, Table 6): F1 53.24 with gold coreference clusters but 25.97 with predicted clusters.**

**Local measurement.** neocortex `extraction.md` held-out: GLiNER2.5-small relation F1 0.200 on free text (0.701 on the team's own gold with label lists chosen from the gold). GLiREL's own document-level drop (53 -> 26 when coreference is predicted) is the same phenomenon.

**What neocortex could steal.** (1) Use it as a **relation *typer* for pairs the agent or rules tier already found**, not as an extractor: when two entities co-occur in a sentence and no relation was passed, ask GLiREL to classify among the predicates already in the `predicates` table (zero-shot, label list = canonical predicate names). This addresses predicate resolution ("partner" -> `married_to`) by giving the classifier the *sentence context*, not just the verb phrase, which is what `resolve_predicate` embeds today (`resolve.rs:979`). Background pass only. (2) Coreference is the limiter: gate on same-sentence pairs.

**Weak / wrong.** Needs entity spans; 467M parameters (DeBERTa-v3-large) and ~200 ms/sentence on CPU, heavier than the 150M UltraBERT embedder, so background-only; synthetic labels; sentence-level benchmarks.
