# Agent Zero Memory: provenance-aware long-term memory (Zero Labs, 2026)
Source: https://arxiv.org/abs/2608.29606 (HTML read; Sec 1-3, 5.4). Industry preprint, self-reported SOTA.

**Problem.** Single-structure memories (fact store, vector index, KG) inherit blind spots; enforce provenance and abstention.

**Data model.** Three memories: Memory Events timeline (episodic), Entity-Event knowledge graph (associative), Hierarchical Documentary Memory (curated, source-locked facts). Every item is a pair `(x, pi(x))` with provenance `pi = <origin o(x) in {scan, infer, doc, dialogue, manual}, time t(x), evidence ev(x)>` (Definition 1).

**Write path.** Four-stage background pipeline: preprocess/classify sources, index (embedding + lexical) in parallel with LLM distillation into events and documentary entries, then an agentic "connect" step builds the entity-event graph.

**Read path.** An *intent gate* (small fast classifier) decides whether memory is needed; a source router restricts retrieval; three concurrent agentic searches over hybrid embedding+lexical retrieval with agent-controlled filters; **citation-locked answers**: every atomic claim must be backed by an item the reader opened, else return abstention (Definition 2).

**Cost.** LLM-heavy write (distillation) and read (agentic loops); a compact open model reaches 92.2% on LongMemEval at ~1/20 the per-query cost of the best backbone.

**Reported results.** LongMemEval 95.60% (next 94.87%, Hindsight 91.40%, Zep 71.20%); LoCoMo 93.60% (Mem0 92.50%). Retrieval-channel ablation (Table 7): embedding-only and lexical-only restricted agents each lose to the hybrid (channels complementary). No component-level ablation of the three memories or the intent gate (stated limitation).

**What neocortex could steal.** (1) The "citation-locked" rule is a tool-contract idea, no LLM: `recall` returns memory ids with every fact and the tool description tells the agent to cite them; neocortex already returns evidence memory ids. Add an explicit `abstain: true` with reason when no channel stands (already implemented as the cosine gate; this paper reinforces making it explicit and calibrated, see `multihop.md` 2.6). (2) Intent gate: classify "does this turn need memory" with rules; neocortex's agents call recall anyway, so low value.

**Weak / wrong.** Self-reported, backbone-dependent, no ablation of its main claims; the 95.6/93.6 numbers use an LLM judge and a frontier backbone; saturation (differences of 0.7-1.1 points) means these benchmarks no longer discriminate.
