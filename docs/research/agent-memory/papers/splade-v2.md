# SPLADE v2: sparse lexical and expansion model (Formal et al., 2021)
Source: https://arxiv.org/abs/2109.10086 (HTML read; Sec 3, Table 1, Table 2; v1 is 2107.05720).

**Problem.** Learned sparse retrieval that expands terms (vocabulary mismatch) yet stays inverted-index friendly.

**Data model.** A BERT MLM head maps text to a |V|=30k vocabulary vector. Term weight (v2, max pooling): `w_j = max_{i in t} log(1 + ReLU(w_ij))` (Eq 6; v1 used a sum). Sparsity enforced by the FLOPS regulariser `l_FLOPS = sum_j (mean_d w_j^(d))^2` (Eq 4) with separate lambda for queries and documents. Score = dot product of query and doc sparse vectors, stored in an ordinary inverted index.

**Variants.** SPLADE-max (query+doc encoders), **SPLADE-doc** (no query encoder at all: query is a bag of its own tokens, expansion happens only on documents), DistilSPLADE-max (distilled from a cross-encoder).

**Write path.** One BERT forward pass per document. **Read path.** Query encoder pass (or none for SPLADE-doc) + inverted-index lookup.

**Time / dedup / forgetting.** Not applicable.

**Reported results.** MS MARCO dev MRR@10: BM25 0.184, SPLADE v1 0.322, SPLADE-max 0.340, **SPLADE-doc 0.322**, DistilSPLADE-max 0.368; TREC DL19 NDCG@10 0.684 / 0.667 / 0.729 (Table 1). BEIR NDCG@10 zero-shot beats BM25 on average (Table 2). In PLAID's table SPLADEv2 retrieval on one CPU thread is ~220 ms at k=1000 on 8.8M passages vs 8.3 ms for BM25 (copied from Mackenzie 2021).

**What neocortex could steal.** This is the most direct fix I found for the paraphrase gap ("partner" vs "married to") that stays LLM-free and fits SQLite. **Document-side expansion** (SPLADE-doc): at `remember` time, run the MLM expansion for the memory text and for each relation fact string ("Seun married to Naomi"), keep the top ~30-60 expansion terms with weights, and index them in a second FTS5 column / weighted term table; at recall the query stays a plain bag of tokens (no query model, so zero read-path model cost), so "partner" hits memories whose expansion contains "partner/spouse/wife". Touches `remember.rs::write_prepared` (add expansion terms to `relations_fts`/`memories_fts`), `recall.rs::relation_seeds_with_config`. Cost: one extra ~66M-param MLM pass per write on CPU (tens of ms), a model file (needs approval under the "no new model" rule), and a candle MLM head (candle-transformers has BERT; the MLM head is a linear layer; inference path (inference) needs checking). Expected benefit mirrors `multihop.md` 7: the failing hops are lexical paraphrases of relation words. Alternative with no new model: an offline predicate synonym table derived from argument-pair overlap (see `literature-map.md`, idea 1).

**Weak / wrong.** Trained on MS MARCO passages; expansions on short fact strings and names may be noisy (names expand to random tokens); needs evaluation on neocortex's multihop-holdout2 before adoption. Index grows 2-3x in terms.
