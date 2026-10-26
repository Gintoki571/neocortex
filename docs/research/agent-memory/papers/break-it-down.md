# Break It Down: question understanding via decomposition, QDMR (Wolfson et al., TACL 2020)
Source: https://arxiv.org/abs/2001.11770 (HTML read; Sec 2, 5, 7).

**Problem.** A source-agnostic formal representation of a question's reasoning steps.

**Mechanism.** QDMR = ordered list of natural-language steps with references (#1, #2) and 13 operation types (select, project, filter, aggregate, group, superlative, comparative, union, intersection, discard, sort, boolean, arithmetic). Dataset **Break**: 83,978 questions over 10 datasets/3 modalities, crowd-annotated. A seq2seq-with-copy parser predicts QDMR (Sec 7).

**Reported results.** Parser: manual analysis ~54% exact-structure accuracy (Sec 7); on HotpotQA open-domain, adding QDMR structures to an RC model raised F1 from 43.3 to 52.4 (Sec 5).

**What neocortex could steal.** The *operation vocabulary* is the useful part: neocortex already has intent detectors for negation, causal, history, aggregation, ordering/duration (TM-12). QDMR's list shows the full set of operators a question can need (project = follow a relation, filter, aggregate, superlative, comparative, boolean); the agent (an LLM) can produce QDMR itself and call `follow`/`aggregate` per step. A rule decomposition of two-hop possessive chains ("the city where X's manager lives") into `follow(X, manager) -> follow(that, lives_in)` is what `paths.rs` does with terms. No model.

**Weak / wrong.** 54% parser accuracy on in-domain questions; needs a trained parser; coverage of agent-memory questions untested.
