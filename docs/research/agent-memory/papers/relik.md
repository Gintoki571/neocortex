# ReLiK: retriever-reader entity linking and relation extraction (Orlando et al., ACL 2024)
Source: https://arxiv.org/abs/2408.00103 (HTML read; Sec 3, Table 2).

**Problem.** Entity linking + relation extraction in one fast pass without autoregressive generation.

**Mechanism.** *Retriever*: bi-encoder (E5-small) retrieves candidate entities/relations (by their textual descriptions) for the text window, trained with multi-label NCE. *Reader*: a DeBERTa-v3 encoder reads `text [SEP] candidate_1 ... candidate_K` in one forward pass, detects mention spans with start/end heads, links each mention to a candidate, and scores relation triplets between detected mentions over the candidate relation set (Sec 3.2). The candidate set is the non-parametric memory (the KB); the reader stays small.

**Reported results.** Relation extraction micro-F1 NYT 94.4-95.0, CoNLL04 94.4-94.9, REBEL 71.7-75.0 (10,936 Wikidata relations) with 33M retriever + 141M-434M reader; end-to-end EL+RE on REBEL 83.7-85.1 EL / 73.8-75.6 RE (Table 2). About 10 s for the entire NYT test set. Fastest of compared EL systems.

**What neocortex could steal.** The retriever-reader split maps onto neocortex's existing pieces: the **predicate table is the relation KB and `predicates_vec` is the retriever**; what is missing is a *reader* that decides, given the sentence and top-K candidate predicates, which pairs are related. A trained 141M reader is beyond a tiny-model budget, but the interface (K=24 candidates retrieved by embedding, fed with the text) shows a way to cap label-set size, which is also GLiREL's weak spot with many labels. Evidence that a closed, growing relation vocabulary (10,936 types) is workable.

**Weak / wrong.** Trained supervised on NYT/CoNLL04/REBEL, i.e. noisy distant supervision on Wikipedia sentences; fixed relation vocabularies; sentence windows only; no negation, time or coreference.
