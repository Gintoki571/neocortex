# Beam Retrieval: end-to-end multi-hop passage retrieval (Zhang et al., NAACL 2024)
Source: https://arxiv.org/abs/2308.08973 (HTML read; Sec 3, Table 2, Table 3).

**Problem.** Retrieve the *chain* of passages for multi-hop questions with arbitrary hop count without per-dataset customised two-step retrievers.

**Mechanism.** Treat retrieval as decoding: a passage is a "token", the chain is the "sequence". An encoder reads `[Q, p_1..p_{t-1}, candidate]`; classifier-1 scores the next passage to extend each hypothesis (softmax over candidates with prior chain), classifier-2 judges whether a hypothesis chain is complete / relevant. Beam size B kept at every hop; all hops trained jointly with cross-entropy plus a "stop" objective using negative chains (Sec 3.1-3.3). Inference = k encoder calls for a k-hop question.

**Time / dedup / forgetting.** Not applicable.

**Cost.** Same order as previous retrievers: one encoder call per hop (Table 2); needs a supervised model (the paper uses DeBERTa-class encoders) and hop labels.

**Reported results.** MuSiQue-Ans retrieval EM 53.50 -> **77.37** with beam size 1 (nearly +50% relative) and higher with larger beams; HotpotQA new SOTA; **99.9% precision on 2WikiMultiHopQA** (Table 3, abstract).

**What neocortex could steal.** Evidence that *conditioning the next hop on the chain so far* is what separates 3-4 hop from 2-hop retrieval (MuSiQue 53 -> 77): neocortex's semantic beam does this with a residual-vector heuristic (`paths.rs::residual_after`) and the three-hop miss (0.667) is a middle-hop paraphrase. A tiny supervised **hop scorer on `[question vec, chain-so-far vec, candidate fact vec]`** (an MLP on frozen UltraBERT embeddings, ~100k parameters, trained on `gen_world` data) is the CPU analogue and overlaps nc-hop's SubgraphRAG scorer; the new element is feeding the chain history into the scorer (SubgraphRAG scores triples independently given seeds). Also keep beam >= 2 even for cheap scorers: the paper shows early-hop errors are recoverable only with a beam.

**Weak / wrong.** Passage-level Wikipedia QA with gold chains for training; encoder cost grows with chain length because the concatenation re-encodes; no updates/time.
