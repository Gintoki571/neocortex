# ColBERT / ColBERTv2: late interaction with residual compression (Khattab & Zaharia 2020; Santhanam et al. 2021)
Sources: https://arxiv.org/abs/2004.12832 (ColBERT) and https://arxiv.org/abs/2112.01488 (ColBERTv2) (HTML read; v1 Sec 3, v2 Sec 3.2-3.3, Table 1).

**Problem.** Single-vector dense retrieval loses token-level matching; cross-encoders are too slow.

**Data model.** Each passage = a bag of per-token embeddings (128-d). **Score = MaxSim**: `S(q,d) = sum_{i in q tokens} max_{j in d tokens} E_qi . E_dj` (v1 Sec 3.2). Query tokens padded with [MASK] to 32 for "query augmentation".

**Write path.** Encode every document token. v2 stores each token as the id of the nearest centroid (4 bytes) plus a **quantised residual** at 1 or 2 bits per dimension (16 or 32 bytes) (v2 Sec 3.3): index for MS MARCO 154 GiB (v1) -> 16 GiB (1 bit) or 25 GiB (2 bit).

**Read path.** Query tokens ANN-probe the centroids to generate candidates, decompress residuals and compute MaxSim exactly.

**Training.** v2: distillation from a MiniLM cross-encoder with 64-way tuples and KL loss, hard-negative mining (v2 Sec 3.2).

**Reported results.** MS MARCO dev MRR@10: ColBERT 36.0 -> ColBERTv2 39.7 (highest standalone retriever at publication), SPLADEv2 36.8 (v2 Table 1); strong zero-shot on BEIR/LoTTE.

**What neocortex could steal.** Late interaction is the principled version of what neocortex's `residual_after` + query-term-coverage do by hand: score a fact by how well each *query token* is explained. A tiny **MaxSim reranker over only the top-50 candidate facts/memories** (not an index over everything) needs per-token vectors for just those candidates, computed at query time by the 150M embedder or a small ColBERT (e.g. answerai-colbert-small, 33M, (inference): not read here). It plays the same role as nc-rerank's MiniLM cross-encoder but as a 2-5 ms matrix op once token vectors are cached; compare the two in the nc-rerank lane rather than adding both.

**Weak / wrong.** Storage is 100x a single-vector index even compressed (16 GiB for 8.8M passages: ~1.9 KB/passage), which at 100k memories is ~190 MB, acceptable but not free; needs a ColBERT-trained model, UltraBERT's token states are not trained for MaxSim; the headline gains are on web-search queries.
