# Deep generative replay (Shin et al., NeurIPS 2017) - AI instantiation of CLS replay
Source: https://arxiv.org/abs/1705.08690 (HTML read; Sec 1-3).

**Problem.** Sequentially learn tasks without keeping old data; avoid catastrophic forgetting.

**Mechanism.** A "scholar" = generative model G + solver S. When task t+1 arrives, G_t generates pseudo-samples of earlier tasks (with S_t labelling them), which are interleaved with real data of the new task to train both G_{t+1} and S_{t+1}. Inspired explicitly by hippocampal replay and CLS (Sec 1): the generator plays the hippocampus.

**Reported results.** On sequential MNIST/SVHN tasks the scholar retains earlier tasks close to joint training without storing data (Sec 4; detailed numbers not extracted).

**What neocortex could steal.** In a symbolic store, replay does not need a generator: the "experience" is already stored (raw memories). The portable insight is the interleaving ratio rule: when a consolidation pass updates shared structure (predicate groups, entity merges, community labels), process each new batch together with a replay sample of older memories (e.g. 1:1) so that the shared structure does not drift toward the recent burst (`cls-consolidation.md`). A synthetic-pseudo-rehearsal use: generate paraphrase queries for old facts from the predicate synonym table (and their `similar`/alias sets) to regression-test recall after each consolidation (a self-check, runnable offline with no LLM).

**Weak / wrong.** Vision/classification benchmarks, GAN generators; no relevance for exact-fact memories beyond the interleaving principle.
