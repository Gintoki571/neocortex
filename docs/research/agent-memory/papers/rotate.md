# RotatE: KG embedding as rotation in complex space (Sun et al., ICLR 2019)
Source: https://arxiv.org/abs/1902.10197 (HTML read; Sec 2-3; link-prediction numbers from NBFNet's Table 3).

**Problem.** Model symmetry, antisymmetry, inversion and composition of relations in one embedding.

**Mechanism.** Entities and relations in C^k, with `|r_i| = 1`; relation = element-wise rotation: `t = h o r`; distance `d_r(h,t) = ||h o r - t||`; loss with self-adversarial negative sampling `p(h'_j,r,t'_j) = softmax(alpha * f(h'_j,t'_j))` (Sec 3.1-3.2). Symmetric iff r_i = +-1; inverse = conjugate; **composition r_3 = r_1 o r_2** (so "manager" then "lives in" composes to a derived relation).

**Reported results.** FB15k-237 MRR 0.338 (H@10 0.553), WN18RR MRR 0.476 (H@10 0.571) (Sun et al. as quoted in NBFNet Table 3); beaten by path/GNN methods (NBFNet 0.415 / 0.551).

**What neocortex could steal.** Be sceptical. KGE needs many triples per entity and relation (FB15k-237 has about 272k training triples over 14.5k entities and 237 relations, ~19 per entity (dataset statistic from general knowledge, not from this paper)); a personal memory has most entities with 1-5 relations and ~1k predicates, so embeddings underfit and need retraining as the graph grows. Two narrow uses: (1) **relation embeddings as a predicate-paraphrase signal**: two predicates connecting overlapping (head,tail) sets obtain nearly equal rotations; the cheaper equivalent is a direct argument-pair Jaccard (literature-map idea 1). (2) **composition rules** `r1 o r2 ~ r3` mined from the graph (AMIE-style, not read here) to *materialise* derived edges at write time (manager + lives_in => "boss lives in"): one-hop retrieval for two-hop questions, with provenance flag `inferred`. Both avoid training a neural model.

**Weak / wrong.** Transductive; static; poor on sparse graphs; entity-only features.
