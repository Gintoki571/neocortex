# Unsupervised Question Decomposition for QA, ONUS (Perez et al., EMNLP 2020)
Source: https://arxiv.org/abs/2002.09758 (HTML read; Sec 2.1, Table 1, Table 2).

**Problem.** Decompose multi-hop questions into single-hop sub-questions without labelled decompositions.

**Mechanism.** *Pseudo-decomposition* by retrieval, with no neural training: for each hard question q find a set of simple questions `d'` from a corpus S that maximise `sum_i f(q, s_i) - sum_{i != j} f(s_i, s_j)` (similar to q yet mutually diverse; f = TF-IDF or FastText cosine; Eq 1). Then train a one-to-N unsupervised seq2seq (ONUS) to map hard questions to these pseudo-decompositions; answer sub-questions with a single-hop QA model; a recomposition model combines sub-answers (Sec 2).

**Reported results.** HotpotQA test F1 79.34 vs RoBERTa baseline 77.0 (dev); out-of-domain dev: the **pseudo-decomposition alone (retrieval, no learning) gives 72.0 vs 67.1 baseline F1** (random pseudo-decomposition 70.7); sub-answers are essential (without them 76.9 vs 77.0 baseline) (Sec 4, Table 2).

**What neocortex could steal.** The retrieval-with-diversity objective (similar to the query, dissimilar to each other) is a drop-in for choosing **which stored facts to render as the evidence chain**: the semantic beam in `paths.rs::search_semantic` scores each hop against the *residual* query, which is the same idea (decorrelate successive hops). The paper supports using the FastText-style cosine retrieval of sub-queries as a decomposition proxy; the equation can replace the greedy residual rule as a global set objective (MMR-style) over the candidate fact set. LLM-free, tiny. Verify on multihop-holdout2 three-hop where the residual beam currently gets 0.667.

**Weak / wrong.** Decomposition only helped when sub-answers were available; training the ONUS seq2seq needs GPUs and 2M questions; evaluation is span-extraction QA.
