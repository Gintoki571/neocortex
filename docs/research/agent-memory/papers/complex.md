# ComplEx: complex embeddings for simple link prediction (Trouillon et al., ICML 2016)
Source: https://arxiv.org/abs/1606.06357 (HTML read; Sec 3-4). Numeric results not extracted here.

**Problem.** Dot-product factorisation (DistMult) cannot model antisymmetric relations; use complex vectors.

**Mechanism.** Score `phi(r,s,o) = Re(<w_r, e_s, conj(e_o)>) = Re(sum_k w_rk e_sk conj(e_ok))`; symmetric relations have imaginary part of w_r zero, antisymmetric have real part zero; time and space complexity O(K) per triple (Table 1); trained with logistic loss and negative sampling on FB15K/WN18.

**What neocortex could steal.** Same caution as `rotate.md`. The relevant fact is the *cost model*: O(K) per triple and a single matrix of entity vectors, so completing a candidate (head, predicate, ?) over N entities is one batched dot product (N*K flops, K=64: 100k entities = 6.4M flops, <1 ms) and could rank the "missing tail" candidates for a `follow` that hits a gap. The real obstacle is data volume, not compute. Not recommended for the first implementation.

**Weak / wrong.** Transductive; needs retraining; benchmark datasets have heavy test leakage through inverse relations (known issue from later work; not in this paper).
