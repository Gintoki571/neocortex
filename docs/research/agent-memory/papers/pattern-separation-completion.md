# Pattern separation and pattern completion in the hippocampus (Marr 1971; Yassa & Stark, Trends Neurosci. 2011; Teyler & DiScenna 1986)
Sources read: Yassa & Stark 2011 (PMC3183227, fetched via https://pmc.ncbi.nlm.nih.gov/articles/PMC3183227/); HippoRAG Sec 2 for the indexing-theory framing (`hipporag.md`). Hopfield side: `modern-hopfield.md`.

**Problem it solves.** Store many similar experiences without confusing them (separation) yet recall a full memory from a partial or noisy cue (completion).

**Mechanisms (read).** Separation: dentate gyrus granule cells (sparse firing) turn overlapping cortical inputs from entorhinal cortex into distinct CA3 patterns via the mossy-fibre pathway, which "forces" new orthogonalised representations onto CA3; completion: CA3 recurrent collaterals (Marr's autoassociator) reinstate a stored pattern from a degraded cue; the direct perforant-path input to CA3 is the weaker cue pathway. Human fMRI "lure" tasks show DG/CA3 treating similar lures as *new* (separation) while CA1 shows completion-like or mixed signals. Separation vs completion is a threshold on input similarity (rate and global remapping).

**Equation-level view (from the Hopfield side).** Retrieval from a cue xi by attention over stored patterns X: `xi_new = X softmax(beta X^T xi)`; large beta -> winner-take-all (separation / exact pattern retrieval), small beta -> averaging of similar patterns (completion, "metastable" mixtures). See `modern-hopfield.md`.

**What an SQLite + embedding system implements (LLM-free).**
1. **Separation at write = dedup/identity with a similarity band.** Two thresholds on cosine to the nearest existing memory/entity: `>= theta_same` (0.95): same item -> merge/reinforce; `theta_sep <= cos < theta_same` (0.75-0.95): a *lure*: store separately and **write an explicit `distinct_from` marker plus the discriminating tokens** (the numeric/capital tokens that differ, "Room 101" vs "Room 102") so retrieval cannot collapse them; `< theta_sep`: independent. neocortex's nc-f4 JW + blockers is this; the new element is *storing the discriminator* and using it at query time to prefer the candidate whose discriminating tokens appear in the query.
2. **Completion at read = cue-to-trace expansion.** A partial query (a name fragment, "the boat she takes out on the river") is completed by (a) KNN, (b) entity-neighbourhood expansion (PPR) — this is HippoRAG's pattern completion — and (c) the temperature of the softmax over candidates: use a sharp softmax when the top-1 margin is large (return one memory), a flat one when scores cluster (return the set and flag ambiguity). The margin (top-1 minus top-2 score) is a better abstention/ambiguity signal than raw cosine (the problem in `multihop.md` 2.6).
3. **Sparse expansion codes**: DG-like sparse expansion corresponds to SPLADE-style sparse term vectors (`splade-v2.md`): document-side expansion into a high-dimensional sparse code reduces collisions between near-duplicates.

**Evidence.** Rodent electrophysiology and human high-resolution fMRI lure studies (Yassa & Stark); the review notes inconsistency between animal studies (CA3 completion) and human fMRI (rapid separation). HippoRAG applies the indexing theory to retrieval.

**Weak / wrong.** The anatomy-to-algorithm mapping is a hypothesis; thresholds (0.75/0.95) are (inference) and embedder-specific (UltraBERT calibration needed, cf. overlapping cosine distributions in `multihop.md` 2.6).

**What neocortex could steal, concretely.** Two-band similarity thresholds with stored discriminator tokens in `resolve.rs` (extends nc-f4), margin-based ambiguity in `recall.rs`. No LLM.
