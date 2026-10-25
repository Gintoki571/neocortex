---
type: "Paper"
title: "An Explanation of In-context Learning as Implicit Bayesian Inference"
description: "Theory that in-context learning emerges when pretraining documents are mixtures of HMM concepts, so the LM infers a latent concept from the prompt; checked on the synthetic GINC dataset (ICLR 2022)."
resource: "https://arxiv.org/abs/2111.02080"
tags: ["in-context-learning", "theory", "bayesian-inference", "hmm", "synthetic-data"]
year: 2021
venue: "ICLR 2022"
llm_required: none
neocortex_relevance: skip
generated: { by: claude-haiku/okf-cataloger, at: 2026-10-25T00:00:00Z }
status: draft
sources:
  - id: paper
    resource: "https://arxiv.org/abs/2111.02080"
    title: "arXiv abstract page"
  - id: html
    resource: "https://arxiv.org/html/2111.02080"
    title: "arXiv HTML full text (Sections 2-4; theorems and GINC experiments)"
---
# Summary
- Pretraining documents are modelled as a mixture of HMMs: a latent concept theta sets the token-transition matrix, so a long document has one coherent concept.[^html]
- In-context learning is the model inferring the shared latent concept from the prompt's examples, despite a distribution mismatch between prompts and pretraining documents.[^paper][^html]
- The paper gives formal results (Theorems 1-3) for distinguishable concepts, the non-distinguishable case and varying-length test examples.[^html]

# Method
- Five assumptions (delimiter hidden states, bounded delimiter transitions, distribution shift from the prompt start, well-specification, regularity) support the theorems.[^html]
- Synthetic GINC dataset: a uniform mixture of HMMs over 5 concepts, 1,000 pretraining documents (about 10 million tokens); GPT-2-style Transformers and LSTMs are trained on it with vocabulary sizes 50, 100 and 150, averaged over 5 pretraining runs.[^html]

# Results
- In-context accuracy rises with the number of examples n and the example length k for both Transformers and LSTMs (Figure 3).[^html]
- Ablation: pretraining on a single concept makes in-context learning fail (flat curves, Figure 4 left); larger Transformers (4, 12, 16 layers) increase in-context accuracy (Figure 6).[^html]

# Relevance to neocortex
- Background only. The result is that a prompt's examples are used to pick a latent concept; neocortex does not learn from prompts, so nothing here changes its engine or its recall design.
- The theory is about pretraining mixtures, not retrieval, so it gives no numbers neocortex can use. Skip.

[^paper]: arXiv abstract page
[^html]: arXiv HTML full text (Sections 2-4)
