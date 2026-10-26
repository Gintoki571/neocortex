# PLAID: engine for fast late-interaction retrieval (Santhanam et al., 2022)
Source: https://arxiv.org/abs/2205.09707 (HTML read; Sec 3-5, Table 3).

**Problem.** ColBERTv2 retrieval is dominated by index lookup and residual decompression (hundreds of ms on GPU, seconds on CPU).

**Mechanism.** *Centroid interaction*: score each candidate passage using only the centroid id of each token (a compact integer), producing an approximate bag-of-centroids MaxSim; *centroid pruning* drops centroids with low query-centroid scores before candidate generation; only the surviving top passages get residual decompression and exact MaxSim (Sec 4). Motivated by Fig 3 (centroids alone recover virtually all of the vanilla top-k) and Fig 4 (few centroids matter per query).

**Reported results (MS MARCO v1, 8.8M passages, Table 3).** Vanilla ColBERTv2: MRR@10 39.7, **3,485 ms** on 1 CPU thread, 922 ms on 8 threads, 53 ms GPU. PLAID k=10: MRR@10 39.4, **185.5 ms 1 CPU thread**, 31.5 ms 8 threads, 11.5 ms GPU; k=100: 39.8, 222 / 53 / 20 ms; k=1000: 39.8, 352 / 101 / 38 ms. BM25 in PISA 8.3 ms at MRR 18.7; SPLADEv2 PISA 220 ms.

**What neocortex could steal.** Evidence that late-interaction scales to CPU: sub-200 ms single thread on **8.8M** passages implies sub-ms on 100k memories. The reusable trick is **two-stage scoring with a cheap integer-signature first stage** (centroid ids) before the exact vector stage: for neocortex's vec0 KNN, a coarse IVF/centroid prefilter is what sqlite-vec lacks; not needed at <=100k vectors (brute-force ~ms). So: no action until the vector table exceeds ~500k rows. Record as the scale plan.

**Weak / wrong.** Heavy C++ kernels; requires ColBERTv2 indexes; quality numbers only on MS MARCO/LoTTE.
