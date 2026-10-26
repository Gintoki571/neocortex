# OpenIE6: iterative grid labeling and coordination analysis (Kolluru et al., EMNLP 2020)
Source: https://arxiv.org/abs/2010.03147 (HTML read; Sec 1, Sec 3, Table 2).

**Problem.** Neural OpenIE was either accurate-and-slow (generation, IMoJIE: 2.6 sentences/s) or fast-and-weak (sequence labelling, RnnOIE: 149 sents/s).

**Mechanism.** *Iterative Grid Labeling*: OpenIE as a 2-D grid (extraction round x word) with labels S/R/O/N produced by BERT, each iteration conditioned on previous labels via max-pool; soft constraints (coverage, POS, head-verb) enforced by constrained training; a second IGL model for **coordination analysis** (splits "A and B" conjunctions) feeds the extractor (Sec 3).

**Reported results (Table 2).** CaRB F1 52.7 (OpenIE6) / 54.0 (CIGL-OIE w/o coordination) vs IMoJIE 53.5, ClausIE 45.0, OpenIE4 51.6; Wire57 F1 40.0 vs 36.0; speed 31.7 sents/s (OpenIE6), 142 sents/s (CIGL-OIE), IMoJIE 2.6, ClausIE 4.0, OpenIE5 3.1 (paper setup: V100 GPU + 4 Xeon cores); claim 10x faster than prior SOTA.

**Local relevance.** HippoRAG replaced GPT-3.5 OpenIE with REBEL and lost 12-18 R@5 (`hipporag.md`), and GPT-3.5 produced ~2x the triples. OpenIE6-class extraction is the strongest pre-LLM open triple extractor; it outputs (argument, relation phrase, argument) *strings* with no canonical predicate or typed entities.

**What neocortex could steal.** Not the model (BERT-base labelling grid + coordination model, GPU-time numbers). The **coordination splitting rule** is portable to the rules tier: "Ravi and Seun live in Pune" -> two relations; "X works at A and B" -> two; `extract_model.rs` appears to have 16 verb patterns, and coordination/appositive expansion is an inexpensive precision/recall win (rule-based, mirrors OpenIE6's coordination analyzer output). Treat as rules work, no model.

**Weak / wrong.** Sentence-level, English, no coref, no negation; CaRB metric rewards matching gold phrase spans, not canonical facts.
