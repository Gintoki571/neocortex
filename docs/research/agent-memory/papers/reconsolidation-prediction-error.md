# Reconsolidation on recall: prediction error and nonmonotonic plasticity (Sinclair & Barense, Trends Neurosci. 2019; Nader et al. 2000)
Source read: Sinclair & Barense 2019 review, author PDF (https://barense.psych.utoronto.ca/wp-content/uploads/2019/11/2019_Sinclair_Barense_TINs.pdf; Glossary, "Reconsolidation of associative memories", "Nonmonotonic plasticity"). Nader 2000 is cited through it. REALM (`realm-reconsolidation.md`) is the LLM-agent instantiation.

**Problem it solves.** Retrieved memories are not read-only: after reactivation a trace becomes labile for hours and is re-stored, which lets it be updated, strengthened or distorted. Not every reminder does this.

**Conditions (read).** (1) A **prediction error** (surprise, expectancy violation) at reactivation is a prerequisite for destabilisation in many paradigms; **incomplete reminders** (cues that only partly replicate the encoding event) elicit it. (2) **Nonmonotonic plasticity hypothesis (NMPH)**: weak reactivation -> no change (trace does not become labile); **moderate reactivation -> synaptic weakening, i.e. plasticity/updating**; strong reactivation -> strengthening (Glossary; Fig 4B). (3) Misinformation and updating studies show new content can be integrated into a reactivated memory.

**Pseudo-equation (author's inference, not in the paper).** Let a = degree of reactivation of memory m at recall (match between cue and trace, e.g. cosine), PE = mismatch between what the memory predicts and the new input. Change in trace strength `dw = +lr_s` if a > a_hi (strengthen), `dw = -lr_w` and editable if a_lo < a < a_hi and PE > theta (destabilise and update), `dw = 0` if a < a_lo.

**What an SQLite + embedding system implements (LLM-free).**
1. **Labile window.** On `recall`, mark returned memories/facts `labile_until = now + W` (e.g. W = 6 h; the biology says hours). Writes during the window that overlap the recalled memory (same entity and predicate, or cosine > 0.8 to the recalled vector) are routed through a **merge/reconcile** step instead of creating an independent duplicate: update `valid_to` of the old fact, attach the new evidence to the same relation id, or create a revision link (`supersedes`). This is a principled home for TM-10 retractions and updated-chain cases: the retraction text arrives right after the agent recalled the fact it contradicts.
2. **Prediction-error gate.** Compute PE at write time as the mismatch between the new statement and the live facts for (subject, predicate): same slot and different object = high PE (supersession candidate); same object = low PE (reinforce); novel slot = no PE (add). Only high-PE writes may end a fact; low-PE writes only add evidence. (Engram does the same slot logic: `engram-bitemporal.md`.)
3. **Moderate-reactivation rule for edge weights**: the REALM update `w <- clip(w + eta*c)` applies with eta from the three-way NMPH regime above, i.e. strengthen when a high-similarity memory was used, weaken when it surfaced with moderate similarity and was ignored.

**Evidence.** Rodent and human reconsolidation studies (review). For LLM agents: REALM reports +2.0 points overall, +4.97 multi-hop from LLM-driven graph reconsolidation; no LLM-free test.

**Weak / wrong.** Human results on fear/episodic memory with small effects and replication debates (NMPH has mixed support, per the review); the labile-window numbers are biological, not engineering.

**What neocortex could steal, concretely.** `labile_until` on recalled rows plus slot-mismatch prediction-error gating of supersession in `remember.rs::write_prepared` / `resolve.rs`; edge-weight update by the NMPH regime. No LLM.
